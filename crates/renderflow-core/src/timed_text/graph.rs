//! Artifact-native subtitle edges for the shared planner and executor.
//!
//! Conversion passes through canonical timed-transcript JSON. Export metadata
//! retains the canonical artifact reference and all disclosed losses. Native
//! extraction reads the immutable content-addressed object directly and never
//! reopens a mutable original or stages a media copy for a filename extension.

use std::fs;
use std::io::{self, Read, Write};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::artifact::{
    Artifact, ArtifactDescriptor, ArtifactDigest, ArtifactId, ArtifactStorageClass,
    ArtifactStore, ArtifactTransform,
};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::process::ProcessCancellationToken;

use super::provider::{extract_stored, SubtitleProviderError, MAX_SOURCE_BYTES};
use super::{
    export_losses, parse, render, validate, OverlapPolicy, TimedTextError, TimedTextFormat,
    TimedTextLoss, TimedTranscript, TranscriptSource, MAX_INPUT_BYTES,
};

pub const SUBTITLE_ARTIFACT_METADATA_KEY: &str = "renderflow.subtitle";
pub const SUBTITLE_ARTIFACT_SCHEMA: &str = "renderflow.subtitle-artifact/v1";

/// Immutable canonical payload/record needed to inspect metadata omitted by an
/// SRT or WebVTT export. A newly produced canonical artifact references itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalTranscriptReference {
    pub artifact_id: ArtifactId,
    pub sha256: String,
}

/// Namespaced metadata carried by the native artifact manifest. These fields
/// disclose provenance and loss; they are not added to Flow's minimal envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubtitleArtifactEvidence {
    pub schema_version: String,
    pub operation: String,
    pub input_artifact_id: ArtifactId,
    pub input_sha256: String,
    pub source: TranscriptSource,
    pub language: Option<String>,
    pub overlap_policy: OverlapPolicy,
    pub canonical_transcript: CanonicalTranscriptReference,
    pub prior_losses: Vec<TimedTextLoss>,
    pub export_losses: Vec<TimedTextLoss>,
}

/// Pure SRT/WebVTT parsing or canonical JSON export. Only the four declared
/// edges are accepted; generic JSON and richer subtitle formats are excluded.
pub struct TimedTextArtifactTransform {
    input_format: Format,
    output_format: Format,
    cacheable: bool,
}

impl TimedTextArtifactTransform {
    pub fn new(input: Format, output: Format) -> Result<Self> {
        if !matches!((input, output),
            (Format::Srt | Format::WebVtt, Format::TimedTranscriptJson)
            | (Format::TimedTranscriptJson, Format::Srt | Format::WebVtt))
        {
            bail!("unsupported timed-text graph edge '{input}' -> '{output}'");
        }
        Ok(Self { input_format: input, output_format: output, cacheable: true })
    }

    /// Speech descendants carry execution-specific evidence outside their text
    /// bytes. Until cache identity includes it, exports must never reuse records.
    pub fn without_cache(mut self) -> Self {
        self.cacheable = false;
        self
    }
}

impl ArtifactTransform for TimedTextArtifactTransform {
    fn name(&self) -> &str { "renderflow.subtitle.convert" }

    fn version(&self) -> &str { "1" }

    fn cache_identity(&self) -> String {
        format!("{}:v1:{}:{}:plain-utf8-ms", self.name(), self.input_format, self.output_format)
    }

    fn cacheable(&self) -> bool { self.cacheable }

    fn fidelity(&self) -> Option<FidelityDeclaration> {
        Some(if self.output_format == Format::TimedTranscriptJson {
            FidelityDeclaration::Lossless
        } else {
            FidelityDeclaration::Partial
        })
    }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        require_formats(input, self.input_format, output_format, self.output_format)?;
        if input.metadata().contains_key(crate::speech::graph::SPEECH_ARTIFACT_METADATA_KEY) {
            crate::speech::graph::retained_artifacts(input)?;
            if self.input_format != Format::TimedTranscriptJson {
                bail!("speech exports cannot be reparsed while claiming unchanged recognition evidence");
            }
        }
        let bytes = read_bounded_artifact(input, store)?;
        let transcript = parse(&bytes, timed_format(self.input_format)?)
            .context("subtitle graph input did not satisfy the plain timed-text profile")?;
        let output = render_bounded(&transcript, timed_format(output_format)?)?;
        let operation = if output_format == Format::TimedTranscriptJson { "parse" } else { "export" };
        store_transcript_output(input, output_format, &transcript, &output, operation, store)
    }
}

/// Selected-stream native extraction. Its provider versions are observed at
/// execution time, so this transform always bypasses persistent result reuse.
pub struct SubtitleExtractionTransform {
    input_format: Format,
    stream_index: u32,
    cancellation: ProcessCancellationToken,
}

impl SubtitleExtractionTransform {
    pub fn new(input: Format, stream_index: u32, cancellation: Option<Arc<AtomicBool>>) -> Result<Self> {
        if !supports_media_format(input) {
            bail!("unsupported subtitle media container '{input}'");
        }
        Ok(Self {
            input_format: input,
            stream_index,
            cancellation: cancellation.map(ProcessCancellationToken::from_shared).unwrap_or_default(),
        })
    }
}

/// Closed container set supported without staging/reopening a mutable source.
pub fn supports_media_format(format: Format) -> bool {
    matches!(format, Format::Mkv | Format::WebM | Format::Mp4 | Format::Mov)
}

impl ArtifactTransform for SubtitleExtractionTransform {
    fn name(&self) -> &str { "renderflow.subtitle.extract" }

    fn version(&self) -> &str { "1" }

    fn cache_identity(&self) -> String {
        format!("{}:v1:{}:stream={}", self.name(), self.input_format, self.stream_index)
    }

    fn cacheable(&self) -> bool { false }

    fn fidelity(&self) -> Option<FidelityDeclaration> { Some(FidelityDeclaration::Partial) }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        require_formats(input, self.input_format, output_format, Format::TimedTranscriptJson)?;
        if self.cancellation.is_cancelled() { return Err(SubtitleProviderError::Cancelled.into()); }
        if input.size_bytes() > MAX_SOURCE_BYTES { return Err(SubtitleProviderError::SourceTooLarge.into()); }
        let path = store.payload_path(input)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != input.size_bytes() {
            return Err(SubtitleProviderError::SourceChanged.into());
        }
        let transcript = extract_stored(&path, self.input_format, input.digest().value(),
            self.stream_index, self.cancellation.clone())
            .context("subtitle extraction from the immutable artifact failed")?;
        if self.cancellation.is_cancelled() { return Err(SubtitleProviderError::Cancelled.into()); }
        let output = render_bounded(&transcript, TimedTextFormat::Json)?;
        if self.cancellation.is_cancelled() { return Err(SubtitleProviderError::Cancelled.into()); }
        store_transcript_output(input, output_format, &transcript, &output, "extract", store)
    }
}

fn require_formats(input: &Artifact, expected_input: Format, output: Format, expected_output: Format) -> Result<()> {
    if input.format().as_str() != expected_input.to_string() || output != expected_output {
        bail!("subtitle transform expected '{expected_input}' -> '{expected_output}', received '{}' -> '{output}'", input.format());
    }
    Ok(())
}

fn timed_format(format: Format) -> Result<TimedTextFormat> {
    match format {
        Format::Srt => Ok(TimedTextFormat::Srt),
        Format::WebVtt => Ok(TimedTextFormat::WebVtt),
        Format::TimedTranscriptJson => Ok(TimedTextFormat::Json),
        _ => bail!("format '{format}' is not a supported timed-text graph payload"),
    }
}

fn read_bounded_artifact(input: &Artifact, store: &ArtifactStore) -> Result<Vec<u8>> {
    if input.size_bytes() > MAX_INPUT_BYTES as u64 { return Err(TimedTextError::InputTooLarge.into()); }
    let path = store.payload_path(input)?;
    let before = fs::symlink_metadata(&path)?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(SubtitleProviderError::SourceChanged.into());
    }
    if before.len() > MAX_INPUT_BYTES as u64 { return Err(TimedTextError::InputTooLarge.into()); }
    let file = store.open(input)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != input.size_bytes() {
        return Err(SubtitleProviderError::SourceChanged.into());
    }
    // Bounds apply to the actual read even if the object grows after metadata
    // inspection. A replaced/truncated payload cannot satisfy its record hash.
    let mut bytes = Vec::with_capacity(input.size_bytes() as usize);
    file.take(MAX_INPUT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT_BYTES { return Err(TimedTextError::InputTooLarge.into()); }
    if bytes.len() as u64 != input.size_bytes()
        || format!("{:x}", Sha256::digest(&bytes)) != input.digest().value()
    { return Err(SubtitleProviderError::SourceChanged.into()); }
    Ok(bytes)
}

/// Preflight output size before the shared renderer allocates output bytes.
fn render_bounded(transcript: &TimedTranscript, format: TimedTextFormat) -> Result<Vec<u8>> {
    validate(transcript)?;
    if format == TimedTextFormat::Json {
        let mut count = OutputByteCounter { bytes: 1, exceeded: false }; // renderer's final newline
        if let Err(error) = serde_json::to_writer_pretty(&mut count, transcript) {
            if count.exceeded { return Err(TimedTextError::OutputTooLarge.into()); }
            return Err(error.into());
        }
    } else {
        let mut count = if format == TimedTextFormat::WebVtt { 8_usize } else { 0 };
        for (index, cue) in transcript.cues.iter().enumerate() {
            let id_bytes = if format == TimedTextFormat::Srt { (index + 1).to_string().len() } else { cue.id.len() };
            // identifier/newline + two 12-byte timestamps + arrow (5),
            // timing newline (1), cue text and final blank separator (2).
            count = count.checked_add(id_bytes + 33 + cue.text.len())
                .ok_or(TimedTextError::OutputTooLarge)?;
            if count > MAX_INPUT_BYTES { return Err(TimedTextError::OutputTooLarge.into()); }
        }
    }
    Ok(render(transcript, format)?)
}

struct OutputByteCounter { bytes: usize, exceeded: bool }

impl Write for OutputByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(total) = self.bytes.checked_add(bytes.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("timed-text output size overflow"));
        };
        if total > MAX_INPUT_BYTES {
            self.exceeded = true;
            return Err(io::Error::other("timed-text output exceeds its byte limit"));
        }
        self.bytes = total;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

fn store_transcript_output(
    input: &Artifact,
    output_format: Format,
    transcript: &TimedTranscript,
    output: &[u8],
    operation: &str,
    store: &ArtifactStore,
) -> Result<Artifact> {
    let canonical_transcript = if output_format == Format::TimedTranscriptJson {
        let sha256 = format!("{:x}", Sha256::digest(output));
        let artifact_id = ArtifactId::from_record(&ArtifactDigest::sha256(sha256.clone()),
            &output_format.into(), &[input.id().clone()]);
        CanonicalTranscriptReference { artifact_id, sha256 }
    } else {
        CanonicalTranscriptReference {
            artifact_id: input.id().clone(), sha256: input.digest().value().to_string(),
        }
    };
    let evidence = SubtitleArtifactEvidence {
        schema_version: SUBTITLE_ARTIFACT_SCHEMA.to_string(),
        operation: operation.to_string(),
        input_artifact_id: input.id().clone(),
        input_sha256: input.digest().value().to_string(),
        source: transcript.source.clone(),
        language: transcript.language.clone(),
        overlap_policy: transcript.overlap_policy,
        canonical_transcript,
        prior_losses: transcript.losses.clone(),
        export_losses: export_losses(transcript, timed_format(output_format)?),
    };
    let mut descriptor = ArtifactDescriptor::for_format(output_format, ArtifactStorageClass::Intermediate)
        .with_source(input.id().clone())
        .with_metadata(SUBTITLE_ARTIFACT_METADATA_KEY, serde_json::to_value(evidence)?);
    if let Some(speech) = input.metadata().get(crate::speech::graph::SPEECH_ARTIFACT_METADATA_KEY) {
        descriptor = descriptor.with_metadata(crate::speech::graph::SPEECH_ARTIFACT_METADATA_KEY, speech.clone());
    }
    let output = store.put_bytes(output, descriptor)?;
    crate::speech::graph::retained_artifacts(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &[u8] = b"1\n00:00:01,000 --> 00:00:02,000\nSynthetic cue\n";

    fn source(store: &ArtifactStore) -> Artifact {
        store.put_bytes(SRT, ArtifactDescriptor::for_format(Format::Srt, ArtifactStorageClass::Source)).unwrap()
    }

    #[test]
    fn canonical_chain_retains_immediate_lineage_original_source_and_export_losses() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let original = source(&store);
        let canonical = TimedTextArtifactTransform::new(Format::Srt, Format::TimedTranscriptJson).unwrap()
            .apply(&original, Format::TimedTranscriptJson, &store).unwrap();
        let exported = TimedTextArtifactTransform::new(Format::TimedTranscriptJson, Format::WebVtt).unwrap()
            .apply(&canonical, Format::WebVtt, &store).unwrap();
        assert_eq!(canonical.sources(), &[original.id().clone()]);
        assert_eq!(exported.sources(), &[canonical.id().clone()]);
        let canonical_evidence: SubtitleArtifactEvidence = serde_json::from_value(
            canonical.metadata()[SUBTITLE_ARTIFACT_METADATA_KEY].clone()).unwrap();
        let export_evidence: SubtitleArtifactEvidence = serde_json::from_value(
            exported.metadata()[SUBTITLE_ARTIFACT_METADATA_KEY].clone()).unwrap();
        assert_eq!(canonical_evidence.canonical_transcript.artifact_id, *canonical.id());
        assert_eq!(canonical_evidence.canonical_transcript.sha256, canonical.digest().value());
        assert_eq!(export_evidence.input_sha256, canonical.digest().value());
        assert_eq!(export_evidence.source.sha256, original.digest().value());
        assert_eq!(export_evidence.canonical_transcript.artifact_id, *canonical.id());
        assert!(export_evidence.export_losses.contains(&TimedTextLoss::SourceProvenance));
        assert_eq!(store.read_bytes(&original).unwrap(), SRT);
    }

    #[test]
    fn generic_json_rich_subtitle_and_direct_shortcuts_are_not_graph_edges() {
        for (input, output) in [(Format::Json, Format::Srt), (Format::Ass, Format::TimedTranscriptJson),
            (Format::Srt, Format::WebVtt), (Format::TimedTranscriptJson, Format::TimedTranscriptJson)]
        { assert!(TimedTextArtifactTransform::new(input, output).is_err()); }
        assert!(SubtitleExtractionTransform::new(Format::Avi, 0, None).is_err());
    }

    #[test]
    fn mutated_immutable_input_fails_before_conversion() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let original = source(&store);
        fs::write(store.payload_path(&original).unwrap(), b"changed").unwrap();
        let error = TimedTextArtifactTransform::new(Format::Srt, Format::TimedTranscriptJson).unwrap()
            .apply(&original, Format::TimedTranscriptJson, &store).unwrap_err();
        assert!(matches!(error.downcast_ref::<SubtitleProviderError>(), Some(SubtitleProviderError::SourceChanged)));
    }

    #[test]
    fn native_transform_is_never_cacheable_and_cancelled_before_provider_launch() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let input = store.put_bytes(b"synthetic container", ArtifactDescriptor::for_format(
            Format::Mkv, ArtifactStorageClass::Source)).unwrap();
        let cancellation = Arc::new(AtomicBool::new(true));
        let transform = SubtitleExtractionTransform::new(Format::Mkv, 3, Some(cancellation)).unwrap();
        assert!(!transform.cacheable());
        let error = transform.apply(&input, Format::TimedTranscriptJson, &store).unwrap_err();
        assert!(matches!(error.downcast_ref::<SubtitleProviderError>(), Some(SubtitleProviderError::Cancelled)));
    }

    #[test]
    fn escaped_json_expansion_is_refused_before_output_allocation() {
        let mut transcript = parse(SRT, TimedTextFormat::Srt).unwrap();
        let mut cue = transcript.cues[0].clone();
        cue.text = "\"".repeat(64 * 1024);
        transcript.cues = (0..200).map(|index| {
            let mut cue = cue.clone();
            cue.id = (index + 1).to_string();
            cue
        }).collect();
        validate(&transcript).unwrap();
        let error = render_bounded(&transcript, TimedTextFormat::Json).unwrap_err();
        assert!(matches!(error.downcast_ref::<TimedTextError>(), Some(TimedTextError::OutputTooLarge)));
    }

    #[test]
    fn canonical_export_retains_media_provenance_language_and_accumulated_losses() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let media = store.put_bytes(b"synthetic media", ArtifactDescriptor::for_format(
            Format::Mkv, ArtifactStorageClass::Source)).unwrap();
        let mut transcript = parse(SRT, TimedTextFormat::Srt).unwrap();
        let subtitle_sha256 = transcript.source.sha256.clone();
        transcript.source = TranscriptSource {
            sha256: media.digest().value().to_string(),
            format: "mkv".to_string(),
            stream: Some(super::super::StreamIdentity {
                index: 3, codec: "subrip".to_string(), language: Some("eng".to_string()),
            }),
            extraction: Some(super::super::ExtractionProvenance {
                provider: "adapter.subtitles.ffmpeg".to_string(),
                ffprobe_version: "ffprobe version 7.1 synthetic".to_string(),
                ffmpeg_version: "ffmpeg version 7.1 synthetic".to_string(),
                subtitle_sha256,
            }),
        };
        transcript.language = Some("eng".to_string());
        transcript.losses = vec![TimedTextLoss::CueIds];
        let canonical = store.put_bytes(&render(&transcript, TimedTextFormat::Json).unwrap(),
            ArtifactDescriptor::for_format(Format::TimedTranscriptJson, ArtifactStorageClass::Source)
                .with_source(media.id().clone())).unwrap();
        let exported = TimedTextArtifactTransform::new(Format::TimedTranscriptJson, Format::Srt).unwrap()
            .apply(&canonical, Format::Srt, &store).unwrap();
        let evidence: SubtitleArtifactEvidence = serde_json::from_value(
            exported.metadata()[SUBTITLE_ARTIFACT_METADATA_KEY].clone()).unwrap();
        assert_eq!(evidence.input_sha256, canonical.digest().value());
        assert_eq!(evidence.source, transcript.source);
        assert_eq!(evidence.language, transcript.language);
        assert_eq!(evidence.prior_losses, vec![TimedTextLoss::CueIds]);
        assert!(evidence.export_losses.contains(&TimedTextLoss::PriorLossEvidence));
        assert!(evidence.export_losses.contains(&TimedTextLoss::Language));
        assert_eq!(evidence.canonical_transcript.artifact_id, *canonical.id());
        assert_eq!(exported.sources(), &[canonical.id().clone()]);
    }
}
