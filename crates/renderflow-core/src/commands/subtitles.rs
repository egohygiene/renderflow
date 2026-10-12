//! Standalone subtitle CLI. Graph execution and release qualification remain separate.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::process::ProcessCancellationToken;
use crate::timed_text::{self, provider, TimedTextFormat, TimedTranscript};

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

pub fn run_inspect(input: &str) -> Result<()> {
    let report = provider::inspect_with_cancellation(Path::new(input), cancellation()?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub fn run_convert(input: &str, input_format: &str, output: &str, output_format: &str) -> Result<()> {
    if !std::fs::symlink_metadata(input)?.file_type().is_file() {
        bail!("subtitle input must be a regular file, not a symlink or special file");
    }
    let file = File::open(input).context("cannot open subtitle input")?;
    if !file.metadata()?.is_file() {
        bail!("subtitle input must be a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAXIMUM_INPUT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAXIMUM_INPUT_BYTES {
        bail!("subtitle input exceeds the 16 MiB limit");
    }
    let transcript = timed_text::parse(&bytes, parse_format(input_format)?)?;
    let input_sha256 = format!("{:x}", Sha256::digest(&bytes));
    write_transcript(&transcript, &input_sha256, output, output_format)
}

pub fn run_extract(input: &str, stream_index: u32, output: &str, output_format: &str) -> Result<()> {
    let transcript = provider::extract_with_cancellation(Path::new(input), stream_index, cancellation()?)?;
    write_transcript(&transcript, &transcript.source.sha256, output, output_format)
}

fn cancellation() -> Result<ProcessCancellationToken> {
    let token = ProcessCancellationToken::new();
    let signal = token.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    Ok(token)
}

fn parse_format(value: &str) -> Result<TimedTextFormat> {
    match value {
        "srt" => Ok(TimedTextFormat::Srt),
        "vtt" => Ok(TimedTextFormat::WebVtt),
        "json" => Ok(TimedTextFormat::Json),
        "text" => Ok(TimedTextFormat::Text),
        _ => bail!("unsupported subtitle format: {value}"),
    }
}

fn write_transcript(transcript: &TimedTranscript, input_sha256: &str, output: &str, output_format: &str) -> Result<()> {
    let format = parse_format(output_format)?;
    let bytes = timed_text::render(transcript, format)?;
    let losses = timed_text::export_losses(transcript, format);
    // Serialize before publication so a serialization failure cannot leave an output.
    let report = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": "renderflow.subtitle-export/v1",
        "input_sha256": input_sha256,
        "output": output,
        "output_format": output_format,
        "output_sha256": format!("{:x}", Sha256::digest(&bytes)),
        "output_size_bytes": bytes.len(),
        "export_losses": losses,
        "transcript": transcript,
    }))?;
    write_new(Path::new(output), &bytes)?;
    println!("{report}");
    Ok(())
}

fn write_new(output: &Path, bytes: &[u8]) -> Result<()> {
    let parent = output.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .context("output parent must already exist and be writable")?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(output)
        .map_err(|error| error.error)
        .context("cannot publish subtitle output; destination must not already exist")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_existing_destination_without_changing_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("captions.vtt");
        std::fs::write(&output, "original").unwrap();
        assert!(write_new(&output, b"replacement").is_err());
        assert_eq!(std::fs::read_to_string(output).unwrap(), "original");
    }

    #[test]
    fn publishes_exact_bytes_to_new_destination() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("captions.vtt");
        write_new(&output, b"WEBVTT\n\n").unwrap();
        assert_eq!(std::fs::read(output).unwrap(), b"WEBVTT\n\n");
    }
}
