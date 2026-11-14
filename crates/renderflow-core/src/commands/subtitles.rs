//! Standalone subtitle CLI. Graph execution and release qualification remain separate.

use std::fs::{self, Metadata, OpenOptions};
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

pub fn run_convert(
    input: &str,
    input_format: &str,
    output: &str,
    output_format: &str,
) -> Result<()> {
    let bytes = capture_input(Path::new(input), || {})?;
    let transcript = timed_text::parse(&bytes, parse_format(input_format)?)?;
    let input_sha256 = format!("{:x}", Sha256::digest(&bytes));
    write_transcript(&transcript, &input_sha256, output, output_format)
}

fn same_source(left: &Metadata, right: &Metadata) -> bool {
    if !left.is_file()
        || !right.is_file()
        || right.file_type().is_symlink()
        || left.len() != right.len()
        || left.modified().ok().is_none()
        || left.modified().ok() != right.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.nlink() == right.nlink()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(not(unix))]
    true
}

/// The private hook makes a metadata/open race deterministic in the owning
/// tests; normal delivery always supplies the no-op closure above.
fn capture_input(path: &Path, before_open: impl FnOnce()) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path).context("cannot observe subtitle input")?;
    if !before.is_file() || before.file_type().is_symlink() {
        bail!("subtitle input must be a regular file, not a symlink or special file");
    }
    if before.len() > MAXIMUM_INPUT_BYTES {
        bail!("subtitle input exceeds the 16 MiB limit");
    }
    before_open();
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A FIFO swapped in after metadata must not block open before the
        // regular-file check; a final-component link must never be followed.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    let mut file = options
        .open(path)
        .context("cannot open selected subtitle input")?;
    if !same_source(&before, &file.metadata()?) {
        bail!("subtitle input changed before capture");
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAXIMUM_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != before.len()
        || !same_source(&before, &file.metadata()?)
        || !same_source(&before, &fs::symlink_metadata(path)?)
    {
        bail!("subtitle input changed during capture");
    }
    Ok(bytes)
}

pub fn run_extract(
    input: &str,
    stream_index: u32,
    output: &str,
    output_format: &str,
) -> Result<()> {
    let transcript =
        provider::extract_with_cancellation(Path::new(input), stream_index, cancellation()?)?;
    write_transcript(
        &transcript,
        &transcript.source.sha256,
        output,
        output_format,
    )
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

fn write_transcript(
    transcript: &TimedTranscript,
    input_sha256: &str,
    output: &str,
    output_format: &str,
) -> Result<()> {
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
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .context("output parent must already exist and be writable")?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(output)
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

    #[test]
    fn selected_regular_bytes_are_captured_and_oversize_refuses_before_open() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("manual 歌詞.srt");
        let original = "1\n00:00:00,125 --> 00:00:01,875\nMoon 🌙\n".as_bytes();
        fs::write(&path, original).unwrap();
        assert_eq!(capture_input(&path, || {}).unwrap(), original);
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAXIMUM_INPUT_BYTES + 1)
            .unwrap();
        assert!(capture_input(&path, || panic!("oversize source reached open")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn exchanged_link_or_same_byte_inode_is_refused_before_conversion() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let selected = directory.path().join("selected.srt");
        let unrelated = directory.path().join("unrelated.srt");
        let original = b"1\n00:00:00,000 --> 00:00:01,000\nOriginal\n";
        fs::write(&selected, original).unwrap();
        fs::write(&unrelated, b"private canary").unwrap();
        assert!(capture_input(&selected, || {
            fs::remove_file(&selected).unwrap();
            symlink(&unrelated, &selected).unwrap();
        })
        .is_err());
        assert_eq!(fs::read(&unrelated).unwrap(), b"private canary");
        fs::remove_file(&selected).unwrap();
        fs::write(&selected, original).unwrap();
        let modified = fs::metadata(&selected).unwrap().modified().unwrap();
        assert!(capture_input(&selected, || {
            fs::rename(&selected, directory.path().join("retained-original.srt")).unwrap();
            fs::write(&selected, original).unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(&selected)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(modified))
                .unwrap();
        })
        .is_err());
        assert_eq!(
            fs::read(directory.path().join("retained-original.srt")).unwrap(),
            original
        );
    }

    #[cfg(unix)]
    #[test]
    fn exchanged_fifo_is_refused_without_waiting_for_a_writer() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::OpenOptionsExt;
        use std::sync::mpsc;
        use std::time::Duration;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("selected.srt");
        fs::write(&path, b"original").unwrap();
        let fifo_path = path.clone();
        let (ready_send, ready_receive) = mpsc::channel();
        let (send, receive) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = capture_input(&fifo_path, || {
                fs::rename(&fifo_path, fifo_path.with_extension("retained")).unwrap();
                let name = CString::new(fifo_path.as_os_str().as_bytes()).unwrap();
                // SAFETY: name is a valid terminated path and mode is private.
                assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
                ready_send.send(()).unwrap();
            });
            send.send(result.is_err()).unwrap();
        });
        ready_receive.recv_timeout(Duration::from_secs(5)).unwrap();
        let result = receive.recv_timeout(Duration::from_secs(1));
        let unblock = if result.is_err() {
            // Unblock a regressed blocking open before failing the assertion;
            // read+write/nonblocking also avoids waiting if the worker finished
            // immediately after the timeout. Hold it until the worker joins.
            Some(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                    .open(&path)
                    .unwrap(),
            )
        } else {
            None
        };
        worker.join().unwrap();
        drop(unblock);
        assert!(result.unwrap());
        assert_eq!(
            fs::read(path.with_extension("retained")).unwrap(),
            b"original"
        );
    }
}
