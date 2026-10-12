//! Local recognition bundles and separate operator-mapped transcript exports.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::{SpeechAlignArguments, SpeechInspectArguments, SpeechTimelineArguments, SpeechTranscribeArguments};
use crate::process::ProcessCancellationToken;
use crate::speech::audio::{self, AudioInspectRequest, AudioPrepareRequest, AudioTool, PreparedAudio};
use crate::speech::provider::{self, SpeechRequest};
use crate::speech::{attach_audio_preparation, SpeechOptions, SpeechTranscription};
use crate::speech::{alignment, recovery, timeline};
use crate::timed_text::{self, TimedTextFormat, MAX_INPUT_BYTES};

pub fn run_inspect(arguments: SpeechInspectArguments) -> Result<()> {
    let cancellation = command_cancellation()?;
    let request = AudioInspectRequest {
        input: PathBuf::from(arguments.input),
        ffprobe: AudioTool {
            path: PathBuf::from(arguments.ffprobe),
            sha256: arguments.ffprobe_sha256,
        },
    };
    eprintln!("Inspecting local audio streams; no stream will be selected automatically.");
    let inspection = audio::inspect(&request, cancellation)?;
    std::io::stdout().write_all(&bounded_json(&inspection.inventory)?)?;
    Ok(())
}

/// Read only the pinned receipt and its retained sibling files. No source media,
/// executable, model, artifact store, or output directory is opened or created.
pub fn run_timeline(arguments: SpeechTimelineArguments) -> Result<()> {
    let cancellation = command_cancellation()?;
    let saved = recovery::inspect_saved_bundle(
        Path::new(&arguments.receipt), &arguments.receipt_sha256, &cancellation,
    )?;
    let assessment = timeline::assess(&saved);
    std::io::stdout().write_all(&bounded_json(&assessment)?)?;
    if arguments.require_original_media {
        timeline::require_original_media_timeline(&saved)?;
    }
    Ok(())
}

/// Publish independent derivatives from an operator-asserted mapping. The
/// original saved bundle remains required evidence and is never modified.
pub fn run_align(arguments: SpeechAlignArguments) -> Result<()> {
    let output = PathBuf::from(&arguments.output_dir);
    require_new_directory(&output)?;
    let cancellation = command_cancellation()?;
    let saved = recovery::inspect_saved_bundle(
        Path::new(&arguments.receipt), &arguments.receipt_sha256, &cancellation,
    )?;
    let aligned = alignment::align_saved_result(
        &saved, Path::new(&arguments.mapping), &arguments.mapping_sha256, &cancellation,
    )?;
    let (files, manifest) = aligned_bundle_files(&aligned, &cancellation)?;
    publish_bundle(&output, &files, &cancellation)?;
    eprintln!("Published transcript with operator-asserted original-media timing: {}", output.display());
    eprintln!("Speech recognition and synchronization remain unreviewed; retain the original saved-result bundle as required evidence.");
    std::io::stdout().write_all(&manifest)?;
    Ok(())
}

pub fn run_transcribe(arguments: SpeechTranscribeArguments) -> Result<()> {
    let output = PathBuf::from(&arguments.output_dir);
    require_new_directory(&output)?;
    let cancellation = command_cancellation()?;
    let options = SpeechOptions {
        language: arguments.language.clone(),
        threads: arguments.threads,
        timeout_seconds: arguments.timeout_seconds,
    };
    options.validate()?;
    let preparation = preparation_request(&arguments)?;
    let prepared = if let Some(request) = preparation {
        eprintln!("Preparing the explicitly selected audio stream; timestamps will start at zero.");
        Some(audio::prepare(&request, cancellation.clone())?)
    } else { None };
    let request = SpeechRequest {
        input: prepared.as_ref().map(|audio| audio.path.clone())
            .unwrap_or_else(|| PathBuf::from(arguments.input)),
        provider_path: PathBuf::from(arguments.provider),
        provider_sha256: arguments.provider_sha256,
        model_path: PathBuf::from(arguments.model),
        model_id: arguments.model_id,
        model_sha256: arguments.model_sha256,
        options,
    };
    eprintln!("Checking local inputs and transcribing audio; Ctrl-C requests cancellation.");
    let run = provider::transcribe(&request, cancellation.clone())?;
    let mut transcription = run.transcription;
    if let Some(audio) = &prepared {
        audio.recheck(&cancellation)?;
        attach_audio_preparation(&mut transcription, &audio.evidence)?;
    }
    let mut files = bundle_files(&transcription, run.provider_output)?;
    if let Some(audio) = &prepared {
        append_preparation_files(&mut files, audio, &cancellation)?;
    }
    let artifacts = files.iter().map(|(name, bytes)| serde_json::json!({
        "path": name,
        "sha256": format!("{:x}", Sha256::digest(bytes)),
        "size_bytes": bytes.len(),
    })).collect::<Vec<_>>();
    let receipt = serde_json::json!({
        "schema_version": "renderflow.speech-bundle/v1",
        "speech_evidence": "transcription.json",
        "human_reviewed": false,
        "artifacts": artifacts,
        "export_losses": {
            "transcript.srt": timed_text::export_losses(&transcription.transcript, TimedTextFormat::Srt),
            "transcript.vtt": timed_text::export_losses(&transcription.transcript, TimedTextFormat::WebVtt),
            "transcript.txt": timed_text::export_losses(&transcription.transcript, TimedTextFormat::Text),
        },
    });
    let receipt_bytes = bounded_json(&receipt)?;
    files.push(("bundle.json".to_string(), receipt_bytes.clone()));
    publish_bundle(&output, &files, &cancellation)?;
    eprintln!("Published unreviewed speech transcript bundle: {}", output.display());
    std::io::stdout().write_all(&receipt_bytes)?;
    Ok(())
}

fn command_cancellation() -> Result<ProcessCancellationToken> {
    let cancellation = ProcessCancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    Ok(cancellation)
}

fn preparation_request(arguments: &SpeechTranscribeArguments) -> Result<Option<AudioPrepareRequest>> {
    let Some(stream_index) = arguments.audio_stream else {
        if arguments.input_sha256.is_some() || arguments.ffprobe.is_some()
            || arguments.ffprobe_sha256.is_some() || arguments.ffmpeg.is_some()
            || arguments.ffmpeg_sha256.is_some() || arguments.audio_timeout_seconds.is_some()
        {
            bail!("audio preparation flags require an explicit --audio-stream");
        }
        return Ok(None);
    };
    let required = |value: &Option<String>, name: &str| -> Result<String> {
        value.clone().with_context(|| format!("--audio-stream requires --{name}"))
    };
    Ok(Some(AudioPrepareRequest {
        inspection: AudioInspectRequest {
            input: PathBuf::from(&arguments.input),
            ffprobe: AudioTool {
                path: PathBuf::from(required(&arguments.ffprobe, "ffprobe")?),
                sha256: required(&arguments.ffprobe_sha256, "ffprobe-sha256")?,
            },
        },
        ffmpeg: AudioTool {
            path: PathBuf::from(required(&arguments.ffmpeg, "ffmpeg")?),
            sha256: required(&arguments.ffmpeg_sha256, "ffmpeg-sha256")?,
        },
        expected_source_sha256: required(&arguments.input_sha256, "input-sha256")?,
        stream_index,
        timeout_seconds: arguments.audio_timeout_seconds.unwrap_or(300),
    }))
}

fn append_preparation_files(
    files: &mut Vec<(String, Vec<u8>)>,
    audio: &PreparedAudio,
    cancellation: &ProcessCancellationToken,
) -> Result<()> {
    if format!("{:x}", Sha256::digest(&audio.probe_output)) != audio.evidence.probe_output_sha256 {
        bail!("audio probe output does not match preparation evidence");
    }
    let wav = audio.read_wav(cancellation)?;
    files.push(("audio-probe.json".to_owned(), audio.probe_output.clone()));
    files.push(("prepared.wav".to_owned(), wav));
    Ok(())
}

fn bundle_files(transcription: &SpeechTranscription, provider_output: Vec<u8>) -> Result<Vec<(String, Vec<u8>)>> {
    if format!("{:x}", Sha256::digest(&provider_output)) != transcription.evidence.provider_output_sha256 {
        bail!("speech provider output does not match the recorded digest");
    }
    let mut files = vec![
        ("provider-output.json".to_string(), provider_output),
        ("transcription.json".to_string(), bounded_json(transcription)?),
    ];
    for (name, format) in [
        ("transcript.timed-transcript.json", TimedTextFormat::Json),
        ("transcript.srt", TimedTextFormat::Srt),
        ("transcript.vtt", TimedTextFormat::WebVtt),
        ("transcript.txt", TimedTextFormat::Text),
    ] {
        files.push((name.to_string(), timed_text::render(&transcription.transcript, format)?));
    }
    Ok(files)
}

fn aligned_bundle_files(
    aligned: &alignment::AlignedSpeechResult,
    cancellation: &ProcessCancellationToken,
) -> Result<(Vec<(String, Vec<u8>)>, Vec<u8>)> {
    require_publication_active(cancellation)?;
    let mut files = vec![
        ("mapping.json".to_owned(), aligned.mapping_bytes().to_vec()),
        ("alignment.json".to_owned(), bounded_json(aligned)?),
    ];
    for (name, format) in [
        ("aligned.timed-transcript.json", TimedTextFormat::Json),
        ("aligned.srt", TimedTextFormat::Srt),
        ("aligned.vtt", TimedTextFormat::WebVtt),
        ("aligned.txt", TimedTextFormat::Text),
    ] {
        require_publication_active(cancellation)?;
        files.push((name.to_owned(), timed_text::render(aligned.transcript(), format)?));
    }
    let evidence = aligned.evidence();
    if file_digest(&files[0].1) != evidence.mapping_sha256
        || file_digest(&files[2].1) != evidence.aligned_transcript_sha256
    {
        bail!("aligned speech bundle differs from its mapping or canonical evidence");
    }
    let manifest = aligned_bundle_manifest(evidence, aligned.transcript(), &files)?;
    files.push(("bundle.json".to_owned(), manifest.clone()));
    require_publication_active(cancellation)?;
    Ok((files, manifest))
}

fn aligned_bundle_manifest(
    evidence: &alignment::SpeechAlignmentEvidence,
    transcript: &timed_text::TimedTranscript,
    files: &[(String, Vec<u8>)],
) -> Result<Vec<u8>> {
    let artifacts = files.iter().map(|(name, bytes)| serde_json::json!({
        "path": name,
        "sha256": file_digest(bytes),
        "size_bytes": bytes.len(),
    })).collect::<Vec<_>>();
    bounded_json(&serde_json::json!({
        "schema_version": "renderflow.speech-aligned-bundle/v1",
        "alignment_evidence": "alignment.json",
        "mapping": "mapping.json",
        "timestamp_origin": "operator_asserted_original_media",
        "basis": "operator_supplied",
        "human_reviewed": false,
        "original_source_reobserved": false,
        "derivatives_require_alignment_evidence": true,
        "source_receipt": {
            "schema_version": recovery::SAVED_RESULT_SCHEMA,
            "sha256": evidence.receipt_sha256,
            "canonical_transcript_sha256": evidence.canonical_transcript_sha256,
            "original_source_sha256": evidence.original_source_sha256,
            "external_bundle_required": true,
        },
        "artifacts": artifacts,
        "export_losses": {
            "aligned.srt": timed_text::export_losses(transcript, TimedTextFormat::Srt),
            "aligned.vtt": timed_text::export_losses(transcript, TimedTextFormat::WebVtt),
            "aligned.txt": timed_text::export_losses(transcript, TimedTextFormat::Text),
        },
    }))
}

fn file_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn require_publication_active(cancellation: &ProcessCancellationToken) -> Result<()> {
    if cancellation.is_cancelled() { bail!("speech publication cancelled"); }
    Ok(())
}

fn require_new_directory(output: &Path) -> Result<()> {
    match fs::symlink_metadata(output) {
        Ok(_) => bail!("speech output directory must not already exist"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = output.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if !parent.is_dir() { bail!("speech output parent must already be a directory"); }
    Ok(())
}

/// Reserve a new directory with create_dir; never replace an existing path.
/// Write the completion receipt last. I/O failure deliberately retains the
/// partial directory for inspection; it must not be mistaken for completion.
/// Cancellation after the final no-clobber persist may leave a complete bundle.
fn publish_bundle(output: &Path, files: &[(String, Vec<u8>)], cancellation: &ProcessCancellationToken) -> Result<()> {
    if cancellation.is_cancelled() { bail!("speech publication cancelled"); }
    fs::create_dir(output).context("cannot reserve a new speech output directory")?;
    for (name, bytes) in files {
        if cancellation.is_cancelled() { bail!("speech publication cancelled; partial directory retained"); }
        let mut temporary = tempfile::NamedTempFile::new_in(output)?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        require_publication_active(cancellation)?;
        temporary.persist_noclobber(output.join(name)).map_err(|error| error.error)?;
    }
    Ok(())
}

fn bounded_json(value: &impl Serialize) -> Result<Vec<u8>> {
    struct BoundedBytes(Vec<u8>);
    impl Write for BoundedBytes {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) >= MAX_INPUT_BYTES {
                return Err(std::io::Error::other("speech report exceeds its 16 MiB limit"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    let mut output = BoundedBytes(Vec::new());
    serde_json::to_writer_pretty(&mut output, value)?;
    output.0.push(b'\n');
    Ok(output.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use crate::cli::{Cli, Commands, SpeechCommands};

    #[test]
    fn alignment_cli_requires_both_receipt_and_mapping_pins() {
        let arguments = [
            "renderflow", "speech", "align", "--receipt", "saved-result.json",
            "--receipt-sha256", "receipt-pin", "--mapping", "time-map.json",
            "--mapping-sha256", "mapping-pin", "--output-dir", "aligned-output",
        ];
        let parsed = Cli::try_parse_from(arguments).unwrap();
        let Some(Commands::Speech { subcommand: SpeechCommands::Align(alignment) }) = parsed.command else {
            panic!("expected the standalone speech align command");
        };
        assert_eq!(alignment.receipt, "saved-result.json");
        assert_eq!(alignment.receipt_sha256, "receipt-pin");
        assert_eq!(alignment.mapping, "time-map.json");
        assert_eq!(alignment.mapping_sha256, "mapping-pin");
        assert_eq!(alignment.output_dir, "aligned-output");
        for missing in ["--receipt-sha256", "--mapping-sha256"] {
            let position = arguments.iter().position(|value| *value == missing).unwrap();
            let incomplete = arguments.iter().enumerate()
                .filter_map(|(index, value)| (index != position && index != position + 1).then_some(*value));
            assert!(Cli::try_parse_from(incomplete).is_err());
        }
    }

    #[test]
    fn alignment_cli_has_no_native_provider_or_inferred_offset_mode() {
        let arguments = [
            "renderflow", "speech", "align", "--receipt", "saved-result.json",
            "--receipt-sha256", "receipt-pin", "--mapping", "time-map.json",
            "--mapping-sha256", "mapping-pin", "--output-dir", "aligned-output",
        ];
        for unsupported in ["--provider", "--offset-ms"] {
            let mut requested = arguments.to_vec();
            requested.extend([unsupported, "1"]);
            assert!(Cli::try_parse_from(requested).is_err());
        }
    }

    #[test]
    fn failed_payload_publication_never_writes_the_completion_manifest() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("incomplete-alignment");
        let files = vec![
            ("mapping.json".to_owned(), b"{}\n".to_vec()),
            ("missing-parent/alignment.json".to_owned(), b"{}\n".to_vec()),
            ("bundle.json".to_owned(), b"{}\n".to_vec()),
        ];
        assert!(publish_bundle(&output, &files, &ProcessCancellationToken::new()).is_err());
        assert_eq!(fs::read(output.join("mapping.json")).unwrap(), b"{}\n");
        assert!(!output.join("bundle.json").exists());
        assert!(require_new_directory(&output).is_err());
    }

    #[test]
    fn aligned_manifest_requires_external_and_alignment_evidence_for_all_exports() {
        let transcript = timed_text::TimedTranscript::new(
            timed_text::TranscriptSource {
                sha256: "1".repeat(64), format: "wav".to_owned(), stream: None, extraction: None,
            },
            vec![timed_text::TimedCue {
                id: "cue-1".to_owned(), start_ms: 5_000, end_ms: 6_000,
                text: "Synthetic speech".to_owned(), speaker: Some("speaker-1".to_owned()),
            }],
        );
        let evidence = alignment::SpeechAlignmentEvidence {
            mapping_sha256: "2".repeat(64), receipt_sha256: "3".repeat(64),
            canonical_transcript_sha256: "4".repeat(64), aligned_transcript_sha256: "5".repeat(64),
            original_source_sha256: "6".repeat(64), transcript_source_sha256: "1".repeat(64),
            selected_stream_index: 2, timestamp_origin: "operator_asserted_original_media".to_owned(),
            basis: "operator_supplied".to_owned(), profile: "synthetic".to_owned(),
            note: "Synthetic assertion for a unit fixture".to_owned(),
            human_reviewed: false, original_source_reobserved: false, limitations: vec![],
        };
        let files = vec![("mapping.json".to_owned(), b"{\n}\n".to_vec())];
        let manifest: serde_json::Value = serde_json::from_slice(
            &aligned_bundle_manifest(&evidence, &transcript, &files).unwrap(),
        ).unwrap();
        assert_eq!(manifest["human_reviewed"], false);
        assert_eq!(manifest["original_source_reobserved"], false);
        assert_eq!(manifest["derivatives_require_alignment_evidence"], true);
        assert_eq!(manifest["source_receipt"]["external_bundle_required"], true);
        assert_eq!(manifest["source_receipt"]["sha256"], evidence.receipt_sha256);
        assert_eq!(manifest["source_receipt"]["canonical_transcript_sha256"], evidence.canonical_transcript_sha256);
        assert_eq!(manifest["artifacts"][0]["sha256"], file_digest(&files[0].1));
        assert_eq!(manifest["artifacts"][0]["size_bytes"], files[0].1.len());
        let text_losses = manifest["export_losses"]["aligned.txt"].as_array().unwrap();
        assert!(text_losses.contains(&serde_json::json!("timing")));
        assert!(text_losses.contains(&serde_json::json!("speakers")));
        assert!(manifest.get("speech_evidence").is_none());
    }

    #[test]
    fn publication_refuses_existing_directories_and_preserves_their_contents() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("original.txt"), b"keep").unwrap();
        let files = vec![("bundle.json".to_string(), b"{}\n".to_vec())];
        assert!(publish_bundle(root.path(), &files, &ProcessCancellationToken::new()).is_err());
        assert_eq!(fs::read(root.path().join("original.txt")).unwrap(), b"keep");
        assert!(!root.path().join("bundle.json").exists());
    }

    #[test]
    fn pre_cancelled_publication_creates_no_output_directory() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("new-bundle");
        let cancellation = ProcessCancellationToken::new();
        cancellation.cancel();
        assert!(publish_bundle(&output, &[], &cancellation).is_err());
        assert!(!output.exists());
    }
}
