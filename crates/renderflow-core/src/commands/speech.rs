//! One local recognition run produces one canonical transcript and its exports.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::{SpeechInspectArguments, SpeechTimelineArguments, SpeechTranscribeArguments};
use crate::process::ProcessCancellationToken;
use crate::speech::audio::{self, AudioInspectRequest, AudioPrepareRequest, AudioTool, PreparedAudio};
use crate::speech::provider::{self, SpeechRequest};
use crate::speech::{attach_audio_preparation, SpeechOptions, SpeechTranscription};
use crate::speech::{recovery, timeline};
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
fn publish_bundle(output: &Path, files: &[(String, Vec<u8>)], cancellation: &ProcessCancellationToken) -> Result<()> {
    if cancellation.is_cancelled() { bail!("speech publication cancelled"); }
    fs::create_dir(output).context("cannot reserve a new speech output directory")?;
    for (name, bytes) in files {
        if cancellation.is_cancelled() { bail!("speech publication cancelled; partial directory retained"); }
        let mut temporary = tempfile::NamedTempFile::new_in(output)?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
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
