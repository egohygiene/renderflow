//! Offline, operator-supplied timing translations for verified prepared speech.
//!
//! A pinned map is an assertion supplied by the operator, not recovered native
//! timestamp evidence or a synchronization certificate. Every entire cue must
//! fit one declared interval. No cue is split, dropped, clipped, or rescaled.
//! Original recognition artifacts and their replay identities stay unchanged.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::process::ProcessCancellationToken;
use crate::timed_text::{self, StreamIdentity, TimedTextFormat, TimedTranscript, TranscriptSource, MAX_INPUT_BYTES, MAX_TIMESTAMP_MS};

use super::provider::{ensure_not_cancelled, expected_digest, read_bounded, sha256};
use super::recovery::{parse_exact, SavedSpeechResult};

pub const SPEECH_TIME_MAP_SCHEMA_V1: &str = "renderflow.speech-time-map/v1";
pub const SPEECH_ALIGNMENT_SCHEMA_V1: &str = "renderflow.speech-alignment/v1";
pub const SPEECH_TIME_MAP_PROFILE: &str = "operator-piecewise-translation-ms/v1";
pub const MAX_MAPPING_BYTES: u64 = 256 * 1024;
pub const MAX_MAPPING_SEGMENTS: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeechTimeMap {
    schema_version: String,
    profile: String,
    basis: String,
    receipt_sha256: String,
    original_source_sha256: String,
    transcript_source_sha256: String,
    canonical_transcript_sha256: String,
    note: String,
    segments: Vec<TimeMapSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeMapSegment {
    transcript_start_ms: u64,
    transcript_end_ms: u64,
    original_start_ms: u64,
}

/// Evidence binds a derived transcript to selected retained bytes and an
/// operator assertion. None of these fields attest recognition accuracy,
/// synchronization accuracy, or current original-source bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechAlignmentEvidence {
    pub profile: String,
    pub basis: String,
    pub timestamp_origin: String,
    pub mapping_sha256: String,
    pub receipt_sha256: String,
    pub canonical_transcript_sha256: String,
    /// SHA-256 of `timed_text::render(transcript, TimedTextFormat::Json)`,
    /// including its trailing newline; this is not the alignment envelope hash.
    pub aligned_transcript_sha256: String,
    pub original_source_sha256: String,
    pub transcript_source_sha256: String,
    pub selected_stream_index: u32,
    pub note: String,
    pub human_reviewed: bool,
    pub original_source_reobserved: bool,
    pub limitations: Vec<String>,
}

/// An immutable derived result. Only the verifier-backed alignment entry point
/// can construct one publicly. Keep its envelope and exact mapping alongside
/// exports: the timed-transcript interchange model alone cannot carry the map.
#[derive(Debug, Clone, Serialize)]
pub struct AlignedSpeechResult {
    schema_version: String,
    transcript: TimedTranscript,
    evidence: SpeechAlignmentEvidence,
    #[serde(skip)]
    mapping_bytes: Vec<u8>,
}

impl AlignedSpeechResult {
    pub fn transcript(&self) -> &TimedTranscript { &self.transcript }
    pub fn mapping_bytes(&self) -> &[u8] { &self.mapping_bytes }
    pub fn evidence(&self) -> &SpeechAlignmentEvidence { &self.evidence }
}

/// Apply a pinned operator map to a verified prepared-media result, offline.
///
/// The map is read once as a bounded regular-file snapshot and its exact bytes
/// are retained. No media, executable, model, artifact store, or native process
/// is opened here. Saved-source identity is recorded evidence, not a new read.
/// Strict input WAV already has its own sample clock and is refused by this
/// profile; mapping that WAV onto a separate media file needs a new contract.
pub fn align_saved_result(
    saved: &SavedSpeechResult,
    mapping_path: &Path,
    mapping_sha256: &str,
    cancellation: &ProcessCancellationToken,
) -> Result<AlignedSpeechResult> {
    ensure_not_cancelled(cancellation)?;
    let transcription = saved.transcription();
    let preparation = transcription.evidence.audio_preparation.as_ref()
        .context("speech alignment requires prepared-media evidence; strict input WAV already has its own sample clock")?;
    let input = AlignmentInput {
        transcript: &transcription.transcript,
        original_source: TranscriptSource {
            sha256: saved.source().digest().value().to_owned(),
            format: saved.source().format().to_string(),
            stream: Some(StreamIdentity {
                index: preparation.selected_stream.index,
                codec: preparation.selected_stream.codec_name.clone(),
                language: preparation.selected_stream.language.clone(),
            }),
            // These are recognized words, not an extracted subtitle stream.
            extraction: None,
        },
        receipt_sha256: saved.receipt_sha256(),
        canonical_transcript_sha256: saved.canonical().digest().value(),
        prepared_duration_ms: preparation.prepared_wav_duration_ms,
    };
    let bytes = read_mapping(mapping_path, mapping_sha256, cancellation)?;
    align_bytes(&input, bytes, mapping_sha256, cancellation)
}

struct AlignmentInput<'a> {
    transcript: &'a TimedTranscript,
    original_source: TranscriptSource,
    receipt_sha256: &'a str,
    canonical_transcript_sha256: &'a str,
    prepared_duration_ms: u64,
}

fn read_mapping(
    path: &Path, pin: &str, cancellation: &ProcessCancellationToken,
) -> Result<Vec<u8>> {
    ensure_not_cancelled(cancellation)?;
    expected_digest(pin, "speech time-map digest")?;
    let bytes = read_bounded(path, MAX_MAPPING_BYTES, cancellation)
        .context("cannot read bounded speech time map")?;
    if sha256(&bytes) != pin { bail!("speech time-map bytes do not match the selected digest"); }
    Ok(bytes)
}

fn align_bytes(
    input: &AlignmentInput<'_>, bytes: Vec<u8>, pin: &str,
    cancellation: &ProcessCancellationToken,
) -> Result<AlignedSpeechResult> {
    ensure_not_cancelled(cancellation)?;
    expected_digest(pin, "speech time-map digest")?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_MAPPING_BYTES {
        bail!("speech time map must contain between 1 and {MAX_MAPPING_BYTES} bytes");
    }
    if sha256(&bytes) != pin { bail!("speech time-map bytes do not match the selected digest"); }
    let mapping: SpeechTimeMap = parse_exact(&bytes, "speech time map")?;
    validate_mapping(&mapping, input, cancellation)?;
    let mut transcript = input.transcript.clone();
    transcript.source = input.original_source.clone();
    let mut segment_index = 0;
    for cue in &mut transcript.cues {
        ensure_not_cancelled(cancellation)?;
        // Intervals meet at endpoints: a cue ending at a boundary fits the
        // preceding interval; a cue starting there fits the following one.
        while segment_index < mapping.segments.len()
            && cue.start_ms >= mapping.segments[segment_index].transcript_end_ms
        {
            segment_index += 1;
        }
        let segment = mapping.segments.get(segment_index)
            .with_context(|| format!("speech cue {} has no mapped interval", cue.id))?;
        if cue.start_ms < segment.transcript_start_ms || cue.end_ms > segment.transcript_end_ms {
            bail!("speech cue {} is uncovered or crosses a mapping boundary; splitting and clipping are unsupported", cue.id);
        }
        cue.start_ms = translate(cue.start_ms, segment)?;
        cue.end_ms = translate(cue.end_ms, segment)?;
    }
    timed_text::validate(&transcript)?;
    let canonical = timed_text::render(&transcript, TimedTextFormat::Json)?;
    let selected_stream_index = input.original_source.stream.as_ref()
        .context("speech alignment requires a recorded selected audio stream")?.index;
    let result = AlignedSpeechResult {
        schema_version: SPEECH_ALIGNMENT_SCHEMA_V1.to_owned(),
        transcript,
        evidence: SpeechAlignmentEvidence {
            profile: SPEECH_TIME_MAP_PROFILE.to_owned(),
            basis: "operator_supplied".to_owned(),
            timestamp_origin: "operator_asserted_original_media".to_owned(),
            mapping_sha256: pin.to_owned(),
            receipt_sha256: mapping.receipt_sha256,
            canonical_transcript_sha256: mapping.canonical_transcript_sha256,
            aligned_transcript_sha256: sha256(&canonical),
            original_source_sha256: mapping.original_source_sha256,
            transcript_source_sha256: mapping.transcript_source_sha256,
            selected_stream_index,
            note: mapping.note,
            human_reviewed: false,
            original_source_reobserved: false,
            limitations: vec![
                "The supplied map asserts original-media timing; retained-byte verification does not prove synchronization accuracy or operator review.".to_owned(),
                "Original-source identity comes from the verified saved receipt; the original media was not read again and its duration was not established here.".to_owned(),
                "Recognition words and segment timing remain an unreviewed draft; cue text, IDs, speakers, and language are unchanged.".to_owned(),
                "Only whole-cue, equal-rate millisecond translations are supported; no splitting, clipping, dropping, stretching, native offset inference, or muxing occurs.".to_owned(),
                "The mapping may cover only selected audio intervals, but every transcript cue must be covered; it does not establish a mapping for the entire source.".to_owned(),
                "Keep the alignment envelope and exact mapping with exports; standalone timed-transcript JSON and subtitle formats do not retain this alignment evidence.".to_owned(),
            ],
        },
        mapping_bytes: bytes,
    };
    if serde_json::to_vec_pretty(&result)?.len().saturating_add(1) > MAX_INPUT_BYTES {
        bail!("speech alignment envelope exceeds the 16 MiB output limit");
    }
    ensure_not_cancelled(cancellation)?;
    Ok(result)
}

fn translate(timestamp: u64, segment: &TimeMapSegment) -> Result<u64> {
    timestamp.checked_sub(segment.transcript_start_ms)
        .and_then(|relative| segment.original_start_ms.checked_add(relative))
        .filter(|value| *value <= MAX_TIMESTAMP_MS)
        .context("speech mapping timestamp is outside the supported millisecond range")
}

fn validate_mapping(
    mapping: &SpeechTimeMap, input: &AlignmentInput<'_>,
    cancellation: &ProcessCancellationToken,
) -> Result<()> {
    if mapping.schema_version != SPEECH_TIME_MAP_SCHEMA_V1
        || mapping.profile != SPEECH_TIME_MAP_PROFILE || mapping.basis != "operator_supplied"
    {
        bail!("unsupported speech time-map schema, profile, or basis");
    }
    for (name, selected, observed) in [
        ("receipt_sha256", mapping.receipt_sha256.as_str(), input.receipt_sha256),
        ("original_source_sha256", mapping.original_source_sha256.as_str(), input.original_source.sha256.as_str()),
        ("transcript_source_sha256", mapping.transcript_source_sha256.as_str(), input.transcript.source.sha256.as_str()),
        ("canonical_transcript_sha256", mapping.canonical_transcript_sha256.as_str(), input.canonical_transcript_sha256),
    ] {
        expected_digest(selected, name)?;
        if selected != observed { bail!("speech time-map {name} does not match the verified saved result"); }
    }
    if mapping.note.trim().is_empty() || mapping.note.chars().count() > 2048
        || mapping.note.chars().any(char::is_control)
    {
        bail!("speech time-map note must be nonempty, at most 2048 characters, and contain no controls");
    }
    if mapping.segments.is_empty() || mapping.segments.len() > MAX_MAPPING_SEGMENTS {
        bail!("speech time map requires between 1 and {MAX_MAPPING_SEGMENTS} intervals");
    }
    let mut previous_source_end = 0;
    let mut previous_original_end = 0;
    for (index, segment) in mapping.segments.iter().enumerate() {
        ensure_not_cancelled(cancellation)?;
        if segment.transcript_start_ms >= segment.transcript_end_ms
            || segment.transcript_end_ms > input.prepared_duration_ms
        {
            bail!("speech time-map interval {index} must have positive duration within the verified prepared audio");
        }
        let original_end = translate(segment.transcript_end_ms, segment)?;
        if index > 0 && (segment.transcript_start_ms < previous_source_end
            || segment.original_start_ms < previous_original_end)
        {
            bail!("speech time-map intervals must be sorted and nonoverlapping in both clocks");
        }
        previous_source_end = segment.transcript_end_ms;
        previous_original_end = original_end;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::SpeechError;
    use crate::timed_text::{OverlapPolicy, TimedCue};

    fn transcript() -> TimedTranscript {
        let mut value = TimedTranscript::new(TranscriptSource {
            sha256: "3".repeat(64), format: "wav".to_owned(), stream: None, extraction: None,
        }, vec![
            TimedCue { id: "first".to_owned(), start_ms: 100, end_ms: 1000, text: "First words".to_owned(), speaker: Some("speaker".to_owned()) },
            TimedCue { id: "second".to_owned(), start_ms: 1000, end_ms: 2000, text: "Second words".to_owned(), speaker: None },
        ]);
        value.language = Some("en".to_owned());
        value.overlap_policy = OverlapPolicy::Reject;
        value
    }

    fn input(transcript: &TimedTranscript) -> AlignmentInput<'_> {
        AlignmentInput {
            transcript,
            original_source: TranscriptSource {
                sha256: "2".repeat(64), format: "mkv".to_owned(),
                stream: Some(StreamIdentity { index: 2, codec: "opus".to_owned(), language: Some("en".to_owned()) }),
                extraction: None,
            },
            receipt_sha256: "1111111111111111111111111111111111111111111111111111111111111111",
            canonical_transcript_sha256: "4444444444444444444444444444444444444444444444444444444444444444",
            prepared_duration_ms: 3000,
        }
    }

    fn mapping() -> SpeechTimeMap {
        SpeechTimeMap {
            schema_version: SPEECH_TIME_MAP_SCHEMA_V1.to_owned(), profile: SPEECH_TIME_MAP_PROFILE.to_owned(),
            basis: "operator_supplied".to_owned(), receipt_sha256: "1".repeat(64), original_source_sha256: "2".repeat(64),
            transcript_source_sha256: "3".repeat(64), canonical_transcript_sha256: "4".repeat(64),
            note: "Synthetic operator assertion; no recording was checked.".to_owned(),
            segments: vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 3000, original_start_ms: 9000 }],
        }
    }

    fn apply(transcript: &TimedTranscript, map: &SpeechTimeMap) -> Result<AlignedSpeechResult> {
        let bytes = serde_json::to_vec_pretty(map).unwrap();
        let pin = sha256(&bytes);
        align_bytes(&input(transcript), bytes, &pin, &ProcessCancellationToken::default())
    }

    #[test]
    fn translation_retains_words_and_binds_distinct_derived_bytes() {
        let original = transcript();
        let before = original.clone();
        let map = mapping();
        let result = apply(&original, &map).unwrap();
        assert_eq!(original, before);
        assert_eq!((result.transcript().cues[0].start_ms, result.transcript().cues[0].end_ms), (9100, 10000));
        assert_eq!(result.transcript().cues[0].id, original.cues[0].id);
        assert_eq!(result.transcript().cues[0].text, original.cues[0].text);
        assert_eq!(result.transcript().cues[0].speaker, original.cues[0].speaker);
        assert_eq!(result.transcript().language, original.language);
        assert_eq!(result.transcript().overlap_policy, original.overlap_policy);
        assert_eq!(result.transcript().source, input(&original).original_source);
        assert!(result.transcript().source.extraction.is_none());
        assert_eq!(result.evidence().aligned_transcript_sha256, sha256(&timed_text::render(result.transcript(), TimedTextFormat::Json).unwrap()));
        assert_eq!(result.evidence().mapping_sha256, sha256(result.mapping_bytes()));
        assert!(!result.evidence().human_reviewed);
        assert!(!result.evidence().original_source_reobserved);
        let wire = serde_json::to_value(&result).unwrap();
        assert!(wire.get("mapping_bytes").is_none());
        assert_eq!(wire["evidence"]["timestamp_origin"], "operator_asserted_original_media");
    }

    #[test]
    fn adjacent_source_boundaries_can_insert_a_declared_original_gap() {
        let mut map = mapping();
        map.segments = vec![
            TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 1000, original_start_ms: 0 },
            TimeMapSegment { transcript_start_ms: 1000, transcript_end_ms: 3000, original_start_ms: 9000 },
        ];
        let result = apply(&transcript(), &map).unwrap();
        assert_eq!((result.transcript().cues[0].start_ms, result.transcript().cues[0].end_ms), (100, 1000));
        assert_eq!((result.transcript().cues[1].start_ms, result.transcript().cues[1].end_ms), (9000, 10000));
    }

    #[test]
    fn translation_to_an_earlier_clock_does_not_require_negative_timestamps() {
        let mut map = mapping();
        map.segments = vec![TimeMapSegment { transcript_start_ms: 100, transcript_end_ms: 3000, original_start_ms: 0 }];
        let result = apply(&transcript(), &map).unwrap();
        assert_eq!((result.transcript().cues[0].start_ms, result.transcript().cues[0].end_ms), (0, 900));
    }

    #[test]
    fn uncovered_or_boundary_crossing_cues_are_not_silently_changed() {
        let mut map = mapping();
        map.segments = vec![
            TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 500, original_start_ms: 0 },
            TimeMapSegment { transcript_start_ms: 500, transcript_end_ms: 3000, original_start_ms: 500 },
        ];
        assert!(apply(&transcript(), &map).unwrap_err().to_string().contains("crosses a mapping boundary"));
        map.segments = vec![TimeMapSegment { transcript_start_ms: 101, transcript_end_ms: 3000, original_start_ms: 0 }];
        assert!(apply(&transcript(), &map).is_err());
        map.segments = vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 1000, original_start_ms: 0 }];
        assert!(apply(&transcript(), &map).is_err());
    }

    #[test]
    fn malformed_interval_order_duration_and_destination_bounds_are_refused() {
        let invalid_segments = [
            vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 0, original_start_ms: 0 }],
            vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 3001, original_start_ms: 0 }],
            vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 3000, original_start_ms: u64::MAX }],
            vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 3000, original_start_ms: MAX_TIMESTAMP_MS - 2999 }],
            vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 1000, original_start_ms: 0 }, TimeMapSegment { transcript_start_ms: 999, transcript_end_ms: 3000, original_start_ms: 1000 }],
            vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 1000, original_start_ms: 9000 }, TimeMapSegment { transcript_start_ms: 1000, transcript_end_ms: 3000, original_start_ms: 9999 }],
            vec![TimeMapSegment { transcript_start_ms: 1000, transcript_end_ms: 3000, original_start_ms: 0 }, TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 1000, original_start_ms: 3000 }],
        ];
        for segments in invalid_segments {
            let mut map = mapping();
            map.segments = segments;
            assert!(apply(&transcript(), &map).is_err());
        }
    }

    #[test]
    fn every_recorded_identity_must_match_the_selected_saved_result() {
        for field in ["receipt_sha256", "original_source_sha256", "transcript_source_sha256", "canonical_transcript_sha256"] {
            let mut wire = serde_json::to_value(mapping()).unwrap();
            wire[field] = serde_json::json!("5".repeat(64));
            let map: SpeechTimeMap = serde_json::from_value(wire).unwrap();
            assert!(apply(&transcript(), &map).unwrap_err().to_string().contains(field));
        }
        let mut map = mapping();
        map.receipt_sha256 = "A".repeat(64);
        assert!(apply(&transcript(), &map).is_err());
    }

    #[test]
    fn mapping_profile_note_and_segment_count_are_bounded() {
        for note in [String::new(), " ".to_owned(), "review\nassertion".to_owned(), "x".repeat(2049)] {
            let mut map = mapping();
            map.note = note;
            assert!(apply(&transcript(), &map).is_err());
        }
        let mut map = mapping();
        map.profile = "automatic-native-mapping/v1".to_owned();
        assert!(apply(&transcript(), &map).is_err());
        map = mapping();
        map.segments.clear();
        assert!(apply(&transcript(), &map).is_err());
        map.segments = vec![TimeMapSegment { transcript_start_ms: 0, transcript_end_ms: 1, original_start_ms: 0 }; MAX_MAPPING_SEGMENTS + 1];
        assert!(apply(&transcript(), &map).is_err());
    }

    #[test]
    fn exact_parser_refuses_duplicate_unknown_positional_and_fractional_fields() {
        let wire = serde_json::to_string(&mapping()).unwrap();
        let bad = [
            wire.replacen("{", "{\"note\":\"duplicate\",", 1),
            wire.replacen("{", "{\"extra\":true,", 1),
            wire.replace("\"transcript_start_ms\":0", "\"transcript_start_ms\":0.5"),
            "[]".to_owned(),
        ];
        for value in bad {
            let bytes = value.into_bytes();
            let pin = sha256(&bytes);
            assert!(align_bytes(&input(&transcript()), bytes, &pin, &ProcessCancellationToken::default()).is_err());
        }
    }

    #[test]
    fn mapping_reader_pins_exact_bytes_and_refuses_oversized_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mapping.json");
        let bytes = serde_json::to_vec_pretty(&mapping()).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let cancellation = ProcessCancellationToken::default();
        assert_eq!(read_mapping(&path, &sha256(&bytes), &cancellation).unwrap(), bytes);
        assert!(read_mapping(&path, &"0".repeat(64), &cancellation).is_err());
        std::fs::write(&path, vec![b' '; MAX_MAPPING_BYTES as usize + 1]).unwrap();
        assert!(read_mapping(&path, &"0".repeat(64), &cancellation).is_err());
    }

    #[test]
    fn cancellation_precedes_mapping_io_and_transform() {
        let cancellation = ProcessCancellationToken::default();
        cancellation.cancel();
        let error = read_mapping(Path::new("absent-mapping.json"), &"0".repeat(64), &cancellation).unwrap_err();
        assert!(matches!(error.downcast_ref::<SpeechError>(), Some(SpeechError::Cancelled)));
        let bytes = serde_json::to_vec(&mapping()).unwrap();
        let pin = sha256(&bytes);
        let error = align_bytes(&input(&transcript()), bytes, &pin, &cancellation).unwrap_err();
        assert!(matches!(error.downcast_ref::<SpeechError>(), Some(SpeechError::Cancelled)));
    }

    #[cfg(unix)]
    #[test]
    fn mapping_reader_refuses_a_symlink() {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("original.json");
        let link = directory.path().join("mapping.json");
        let bytes = serde_json::to_vec(&mapping()).unwrap();
        std::fs::write(&original, &bytes).unwrap();
        std::os::unix::fs::symlink(&original, &link).unwrap();
        assert!(read_mapping(&link, &sha256(&bytes), &ProcessCancellationToken::default()).is_err());
    }
}
