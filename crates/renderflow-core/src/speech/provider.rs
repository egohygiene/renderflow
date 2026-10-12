//! Opt-in, local whisper.cpp CLI adapter for a bounded PCM WAV profile.
//!
//! The caller supplies an executable and a model with expected SHA-256 digests.
//! Nothing is discovered on PATH, installed, downloaded, or sent to a service.
//! File identity checks detect persistent changes; they do not prevent hostile
//! path replacement, freeze linked libraries or model sidecars, or provide an
//! operating-system network, memory, or disk sandbox. Ancestor symlinks are
//! resolved; the supplied final path component must be a regular non-symlink.

use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::process::{
    ProcessCancellationToken, ProcessEnvironment, ProcessError, ProcessExecutor,
    ProcessExpectedOutput, ProcessNetworkPolicy, ProcessOutputMode, ProcessRequest,
    ProcessResult, ProcessTermination,
};

use super::{
    normalize_whisper_json, SpeechError, SpeechIdentity, SpeechOptions,
    SpeechTranscription, WHISPER_FIXED_ARGUMENTS, WHISPER_PROVIDER_ID,
};

pub const MAX_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_DURATION_MS: u64 = 10 * 60 * 1000;
pub const MAX_MODEL_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_PROVIDER_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_PROVIDER_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
const HASH_TIMEOUT: Duration = Duration::from_secs(120);
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct SpeechRequest {
    pub input: PathBuf,
    pub provider_path: PathBuf,
    pub provider_sha256: String,
    pub model_path: PathBuf,
    pub model_id: String,
    pub model_sha256: String,
    pub options: SpeechOptions,
}

/// Parsed recognition together with the exact bounded bytes named by the
/// envelope's provider_output_sha256. Callers should retain both for review.
#[derive(Debug)]
pub struct SpeechRun {
    pub transcription: SpeechTranscription,
    pub provider_output: Vec<u8>,
}

/// Recognize speech using one explicitly selected local executable and model.
///
/// The accepted source is a RIFF/WAVE file containing mono, 16 kHz, signed
/// 16-bit little-endian PCM. Video/container decoding and audio normalization
/// are separate explicit operations in the `speech::audio` module. Empty recognition output is
/// a refusal, not a successful transcript. GPU use is disabled by CLI option;
/// compiled accelerator integrations and native-library behavior are not
/// attested by this adapter.
pub fn transcribe(
    request: &SpeechRequest,
    cancellation: ProcessCancellationToken,
) -> Result<SpeechRun, SpeechError> {
    transcribe_inner(request, None, cancellation)
}

/// Recognize the immutable artifact selected by a graph policy. Verify its
/// expected digest against the provider's own source snapshot before launching
/// any executable, including the provider version probe.
pub fn transcribe_stored(
    request: &SpeechRequest,
    expected_source_sha256: &str,
    cancellation: ProcessCancellationToken,
) -> Result<SpeechRun, SpeechError> {
    ensure_not_cancelled(&cancellation)?;
    expected_digest(expected_source_sha256, "source_sha256")?;
    transcribe_inner(request, Some(expected_source_sha256), cancellation)
}

fn transcribe_inner(
    request: &SpeechRequest,
    expected_source_sha256: Option<&str>,
    cancellation: ProcessCancellationToken,
) -> Result<SpeechRun, SpeechError> {
    ensure_not_cancelled(&cancellation)?;
    request.options.validate()?;
    expected_digest(&request.provider_sha256, "provider_sha256")?;
    expected_digest(&request.model_sha256, "model_sha256")?;
    if request.model_id.trim().is_empty() || request.model_id.chars().count() > 256
        || request.model_id.chars().any(char::is_control)
    {
        return Err(SpeechError::InvalidRequest(
            "model_id must be nonempty, at most 256 characters, and contain no controls".to_owned(),
        ));
    }
    let source = FileSnapshot::read(&request.input, "source", MAX_SOURCE_BYTES, &cancellation)?;
    if let Some(expected) = expected_source_sha256 { require_digest(&source, expected)?; }
    let source_bytes = read_bounded(&source.path, MAX_SOURCE_BYTES, &cancellation)?;
    if sha256(&source_bytes) != source.sha256 {
        return Err(SpeechError::SourceChanged("source changed before WAV inspection".to_owned()));
    }
    let duration_ms = inspect_pcm_wav(&source_bytes)?;
    drop(source_bytes);
    let provider = FileSnapshot::read(
        &request.provider_path, "provider", MAX_PROVIDER_BYTES, &cancellation,
    )?;
    require_digest(&provider, &request.provider_sha256)?;
    let model = FileSnapshot::read(&request.model_path, "model", MAX_MODEL_BYTES, &cancellation)?;
    require_digest(&model, &request.model_sha256)?;

    let temporary = tempfile::Builder::new().prefix("renderflow-speech-").tempdir()?;
    let work_directory = fs::canonicalize(temporary.path())?;
    let output_base = work_directory.join("transcription");
    let output_path = work_directory.join("transcription.json");
    let executor = ProcessExecutor::new();
    let version_result = run_checked(&executor, bounded_request(
        &provider.path, &work_directory, VERSION_TIMEOUT, &cancellation,
    )?.arg("--version"))?;
    let identity = SpeechIdentity {
        provider_id: WHISPER_PROVIDER_ID.to_owned(),
        provider_version: parse_version(version_result.stdout().bytes())?,
        provider_sha256: provider.sha256.clone(),
        model_id: request.model_id.clone(),
        model_sha256: model.sha256.clone(),
        source_sha256: source.sha256.clone(),
    };
    identity.validate()?;
    for snapshot in [&source, &provider, &model] {
        snapshot.recheck(&cancellation)?;
    }
    let process = bounded_request(
        &provider.path, &work_directory,
        Duration::from_secs(request.options.timeout_seconds), &cancellation,
    )?
        .args(transcription_arguments(&source.path, &model.path, &output_base, &request.options)?)
        .expect_output(ProcessExpectedOutput::file(&output_path)
            .require_non_empty().max_bytes(MAX_PROVIDER_OUTPUT_BYTES));
    // Upstream emits ordinary diagnostics on stderr even with --no-prints.
    // Bounded stderr alone is therefore not evidence of a recognition failure.
    run_checked(&executor, process)?;
    for snapshot in [&source, &provider, &model] {
        snapshot.recheck(&cancellation)?;
    }
    let output = read_bounded(&output_path, MAX_PROVIDER_OUTPUT_BYTES, &cancellation)?;
    let transcription = normalize_whisper_json(&output, &identity, &request.options)?;
    if transcription.evidence.provider_reported_model != path_text(&model.path)? {
        return Err(SpeechError::InvalidOutput(
            "provider did not report the explicitly selected model path".to_owned(),
        ));
    }
    if transcription.transcript.cues.iter().any(|cue| cue.end_ms > duration_ms) {
        return Err(SpeechError::InvalidOutput(
            "provider returned a cue beyond the validated WAV duration".to_owned(),
        ));
    }
    ensure_not_cancelled(&cancellation)?;
    Ok(SpeechRun { transcription, provider_output: output })
}

fn transcription_arguments(
    source: &Path,
    model: &Path,
    output_base: &Path,
    options: &SpeechOptions,
) -> Result<Vec<String>, SpeechError> {
    options.validate()?;
    let mut arguments = vec![
        "--model".to_owned(), path_text(model)?.to_owned(),
        "--file".to_owned(), path_text(source)?.to_owned(),
        "--output-json-full".to_owned(),
        "--output-file".to_owned(), path_text(output_base)?.to_owned(),
        "--language".to_owned(), options.language.clone(),
        "--threads".to_owned(), options.threads.to_string(),
    ];
    arguments.extend(WHISPER_FIXED_ARGUMENTS.iter().map(|value| (*value).to_owned()));
    Ok(arguments)
}

pub(super) fn bounded_request(
    executable: &Path,
    work_directory: &Path,
    timeout: Duration,
    cancellation: &ProcessCancellationToken,
) -> Result<ProcessRequest, SpeechError> {
    Ok(ProcessRequest::direct(path_text(executable)?)
        .working_directory(work_directory)
        .stdout(ProcessOutputMode::capture(MAX_CAPTURE_BYTES))
        .stderr(ProcessOutputMode::capture(MAX_CAPTURE_BYTES))
        .timeout(timeout)
        .cancellation(cancellation.clone())
        .environment(ProcessEnvironment::clear().set("LC_ALL", "C"))
        .network_policy(ProcessNetworkPolicy::Deny))
}

pub(super) fn run_checked(executor: &ProcessExecutor, request: ProcessRequest) -> Result<ProcessResult, SpeechError> {
    let result = executor.execute(request).map_err(|error| match error {
        error @ ProcessError::MissingExecutable { .. } => SpeechError::ProviderUnavailable(error.to_string()),
        error => SpeechError::ProviderFailed(error.to_string()),
    })?;
    match result.termination() {
        ProcessTermination::Cancelled => return Err(SpeechError::Cancelled),
        ProcessTermination::TimedOut => return Err(SpeechError::Timeout),
        _ => {}
    }
    if result.stdout().truncated() || result.stderr().truncated() {
        return Err(SpeechError::ProviderFailed(
            "provider exceeded its configured stdout/stderr capture limit".to_owned(),
        ));
    }
    if !result.is_success() {
        return Err(SpeechError::ProviderFailed(result.failure_message()));
    }
    Ok(result)
}

fn parse_version(bytes: &[u8]) -> Result<String, SpeechError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| SpeechError::ProviderUnavailable("provider version is not UTF-8".to_owned()))?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let line = lines.next().unwrap_or("");
    let version = line.strip_prefix("whisper.cpp version: ").unwrap_or("");
    if version.trim().is_empty() || line.len() > 4096 || line.chars().any(char::is_control)
        || lines.next().is_some()
    {
        return Err(SpeechError::ProviderUnavailable(
            "expected one bounded 'whisper.cpp version: ...' stdout line".to_owned(),
        ));
    }
    Ok(line.to_owned())
}

pub(super) fn path_text(path: &Path) -> Result<&str, SpeechError> {
    let text = path.to_str().ok_or_else(|| SpeechError::InvalidRequest(
        "native paths must be UTF-8".to_owned(),
    ))?;
    if !path.is_absolute() || text.chars().any(char::is_control) {
        return Err(SpeechError::InvalidRequest(
            "native paths must be absolute and contain no control characters".to_owned(),
        ));
    }
    Ok(text)
}

pub(super) fn expected_digest(value: &str, name: &str) -> Result<(), SpeechError> {
    if value.len() != 64 || !value.bytes().all(|byte|
        byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SpeechError::InvalidRequest(format!(
            "{name} must be a lowercase SHA-256 digest",
        )));
    }
    Ok(())
}

pub(super) fn require_digest(snapshot: &FileSnapshot, expected: &str) -> Result<(), SpeechError> {
    if snapshot.sha256 != expected {
        return Err(SpeechError::InvalidRequest(format!(
            "{} SHA-256 does not match the explicitly supplied digest", snapshot.kind,
        )));
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct FileSnapshot {
    pub(super) path: PathBuf,
    kind: &'static str,
    maximum: u64,
    pub(super) metadata: Metadata,
    pub(super) sha256: String,
}

impl FileSnapshot {
    pub(super) fn read(
        input: &Path, kind: &'static str, maximum: u64,
        cancellation: &ProcessCancellationToken,
    ) -> Result<Self, SpeechError> {
        ensure_not_cancelled(cancellation)?;
        let initial = fs::symlink_metadata(input).map_err(|error| SpeechError::InvalidRequest(
            format!("cannot open {kind} as a local regular file: {error}"),
        ))?;
        if !initial.is_file() || initial.file_type().is_symlink() {
            return Err(SpeechError::InvalidRequest(format!(
                "{kind} must be a regular file, with no symlink at its final path component",
            )));
        }
        let path = fs::canonicalize(input)?;
        path_text(&path)?;
        let (metadata, sha256) = hash_file(&path, kind, maximum, cancellation)?;
        if !same_metadata(&initial, &metadata) {
            return Err(SpeechError::SourceChanged(format!("{kind} changed during path resolution")));
        }
        Ok(Self { path, kind, maximum, metadata, sha256 })
    }

    pub(super) fn recheck(&self, cancellation: &ProcessCancellationToken) -> Result<(), SpeechError> {
        let (metadata, digest) = hash_file(&self.path, self.kind, self.maximum, cancellation)?;
        if digest != self.sha256 || !same_metadata(&self.metadata, &metadata) {
            return Err(SpeechError::SourceChanged(format!("{} changed during the speech operation", self.kind)));
        }
        Ok(())
    }
}

fn hash_file(
    path: &Path, kind: &str, maximum: u64, cancellation: &ProcessCancellationToken,
) -> Result<(Metadata, String), SpeechError> {
    let before = regular_metadata(path)?;
    if before.len() == 0 || before.len() > maximum {
        return Err(SpeechError::InvalidRequest(format!(
            "{kind} must contain between 1 and {maximum} bytes",
        )));
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if !same_metadata(&before, &opened) {
        return Err(SpeechError::SourceChanged(format!("{kind} changed while opening")));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    let started = Instant::now();
    loop {
        ensure_not_cancelled(cancellation)?;
        if started.elapsed() >= HASH_TIMEOUT { return Err(SpeechError::Timeout); }
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        total += count as u64;
        if total > maximum {
            return Err(SpeechError::SourceChanged(format!("{kind} exceeded its byte limit while hashing")));
        }
        digest.update(&buffer[..count]);
    }
    ensure_not_cancelled(cancellation)?;
    if started.elapsed() >= HASH_TIMEOUT { return Err(SpeechError::Timeout); }
    let after = file.metadata()?;
    if total != before.len() || !same_metadata(&before, &after)
        || !same_metadata(&after, &regular_metadata(path)?)
    {
        return Err(SpeechError::SourceChanged(format!("{kind} changed while hashing")));
    }
    Ok((before, format!("{:x}", digest.finalize())))
}

pub(super) fn read_bounded(
    path: &Path, maximum: u64, cancellation: &ProcessCancellationToken,
) -> Result<Vec<u8>, SpeechError> {
    let before = regular_metadata(path)?;
    if before.len() == 0 || before.len() > maximum {
        return Err(SpeechError::InvalidOutput(format!(
            "file must contain between 1 and {maximum} bytes",
        )));
    }
    let mut file = File::open(path)?;
    if !same_metadata(&before, &file.metadata()?) {
        return Err(SpeechError::SourceChanged("file changed while opening bounded reader".to_owned()));
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    let mut buffer = [0_u8; 64 * 1024];
    let started = Instant::now();
    loop {
        ensure_not_cancelled(cancellation)?;
        if started.elapsed() >= HASH_TIMEOUT { return Err(SpeechError::Timeout); }
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        if bytes.len() as u64 + count as u64 > maximum {
            return Err(SpeechError::InvalidOutput("file exceeded its bounded read limit".to_owned()));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    ensure_not_cancelled(cancellation)?;
    if started.elapsed() >= HASH_TIMEOUT { return Err(SpeechError::Timeout); }
    let after = file.metadata()?;
    if bytes.len() as u64 != before.len() || !same_metadata(&before, &after)
        || !same_metadata(&after, &regular_metadata(path)?)
    {
        return Err(SpeechError::SourceChanged("file changed during bounded read".to_owned()));
    }
    Ok(bytes)
}

fn regular_metadata(path: &Path) -> Result<Metadata, SpeechError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(SpeechError::InvalidRequest("expected a non-symlink regular file".to_owned()));
    }
    Ok(metadata)
}

fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    if !left.is_file() || !right.is_file() || left.len() != right.len()
        || left.modified().ok() != right.modified().ok()
    { return false; }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev() || left.ino() != right.ino()
            || left.ctime() != right.ctime() || left.ctime_nsec() != right.ctime_nsec()
        { return false; }
    }
    true
}

pub(super) fn ensure_not_cancelled(cancellation: &ProcessCancellationToken) -> Result<(), SpeechError> {
    if cancellation.is_cancelled() { Err(SpeechError::Cancelled) } else { Ok(()) }
}

pub(super) fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

/// Parse RIFF boundaries before invoking native code. Ancillary chunks are
/// retained in the source identity but do not alter the single PCM data stream.
/// RF64, compressed/extensible WAVE, duplicate streams, and trailing data are
/// refused rather than decoded or normalized implicitly.
pub(super) fn inspect_pcm_wav(bytes: &[u8]) -> Result<u64, SpeechError> {
    let invalid = |message: &str| SpeechError::InvalidRequest(format!("unsupported WAV: {message}"));
    if bytes.len() as u64 > MAX_SOURCE_BYTES || bytes.len() < 44
        || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE"
    {
        return Err(invalid("expected bounded RIFF/WAVE PCM bytes"));
    }
    let riff_size = u32::from_le_bytes(bytes[4..8].try_into().expect("four-byte RIFF length")) as u64;
    if riff_size + 8 != bytes.len() as u64 { return Err(invalid("RIFF length does not match file size")); }
    let mut offset = 12_usize;
    let mut format_seen = false;
    let mut data_bytes = None;
    let mut chunks = 0_usize;
    while offset < bytes.len() {
        chunks += 1;
        if chunks > 256 || bytes.len() - offset < 8 { return Err(invalid("too many or incomplete chunks")); }
        let identifier = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into()
            .expect("four-byte chunk length")) as usize;
        let start = offset + 8;
        let end = start.checked_add(size).ok_or_else(|| invalid("chunk length overflow"))?;
        let next = end.checked_add(size & 1).ok_or_else(|| invalid("chunk padding overflow"))?;
        if next > bytes.len() { return Err(invalid("chunk extends beyond RIFF bounds")); }
        match identifier {
            b"fmt " => {
                if format_seen || data_bytes.is_some() || !matches!(size, 16 | 18) {
                    return Err(invalid("expected one PCM format chunk before data"));
                }
                let format = &bytes[start..end];
                let word = |index| u16::from_le_bytes([format[index], format[index + 1]]);
                let rate = u32::from_le_bytes(format[4..8].try_into().expect("four-byte rate"));
                let byte_rate = u32::from_le_bytes(format[8..12].try_into().expect("four-byte byte rate"));
                if word(0) != 1 || word(2) != 1 || rate != 16_000 || byte_rate != 32_000
                    || word(12) != 2 || word(14) != 16 || (size == 18 && word(16) != 0)
                {
                    return Err(invalid("requires mono 16000 Hz signed PCM16, without extensions"));
                }
                format_seen = true;
            }
            b"data" => {
                if !format_seen || data_bytes.is_some() || size == 0 || size % 2 != 0 {
                    return Err(invalid("requires exactly one nonempty, sample-aligned data chunk"));
                }
                data_bytes = Some(size as u64);
            }
            _ => {}
        }
        offset = next;
    }
    let data_bytes = data_bytes.ok_or_else(|| invalid("PCM data chunk is absent"))?;
    if data_bytes > MAX_DURATION_MS * 32 {
        return Err(invalid("duration exceeds the ten-minute profile limit"));
    }
    // Round up the final partial millisecond, preserving sub-ms sample tails.
    Ok((data_bytes + 31) / 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: u32) -> Vec<u8> {
        let data_bytes = samples * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&32_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_bytes.to_le_bytes());
        bytes.resize(44 + data_bytes as usize, 0);
        bytes
    }

    #[test]
    fn pcm_wav_accepts_bounded_samples_and_ancillary_padding() {
        assert_eq!(inspect_pcm_wav(&wav(16_000)).unwrap(), 1000);
        assert_eq!(inspect_pcm_wav(&wav(1)).unwrap(), 1);
        let mut bytes = wav(16_000);
        bytes.extend_from_slice(b"JUNK\x01\x00\x00\x00\x01\x00");
        let length = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&length.to_le_bytes());
        assert_eq!(inspect_pcm_wav(&bytes).unwrap(), 1000);
    }

    #[test]
    fn pcm_wav_refuses_implicit_conversion_and_malformed_boundaries() {
        for (offset, value) in [(20, 3), (22, 2), (24, 0), (28, 0), (32, 4), (34, 8)] {
            let mut bytes = wav(16);
            bytes[offset] = value;
            assert!(inspect_pcm_wav(&bytes).is_err());
        }
        let mut truncated = wav(16);
        truncated.pop();
        assert!(inspect_pcm_wav(&truncated).is_err());
        let mut duplicate = wav(16);
        duplicate.extend_from_slice(b"data\x02\x00\x00\x00\x00\x00");
        let length = (duplicate.len() - 8) as u32;
        duplicate[4..8].copy_from_slice(&length.to_le_bytes());
        assert!(inspect_pcm_wav(&duplicate).is_err());
        assert!(inspect_pcm_wav(&wav(0)).is_err());
        assert!(inspect_pcm_wav(&wav(16_000 * 601)).is_err());
    }

    #[test]
    fn provider_arguments_are_explicit_and_do_not_enable_translation_or_vad() {
        let directory = std::env::temp_dir();
        let arguments = transcription_arguments(
            &directory.join("source with spaces.wav"), &directory.join("model.bin"),
            &directory.join("transcription"), &SpeechOptions::default(),
        ).unwrap();
        for pair in [["--processors", "1"], ["--temperature", "0"],
            ["--temperature-inc", "0"], ["--beam-size", "1"], ["--best-of", "1"]]
        {
            assert!(arguments.windows(2).any(|actual| actual[0] == pair[0] && actual[1] == pair[1]));
        }
        for flag in ["--output-json-full", "--no-gpu", "--no-flash-attn", "--no-fallback"] {
            assert!(arguments.iter().any(|value| value == flag));
        }
        for flag in ["--translate", "--detect-language", "--vad", "--output-srt", "--output-vtt"] {
            assert!(!arguments.iter().any(|value| value == flag));
        }
        assert!(arguments.iter().any(|value| value.ends_with("source with spaces.wav")));
    }

    #[test]
    fn version_response_requires_one_unambiguous_upstream_line() {
        assert_eq!(parse_version(b"whisper.cpp version: 1.9.5-dev\n").unwrap(),
            "whisper.cpp version: 1.9.5-dev");
        for value in [b"help\n".as_slice(), b"whisper.cpp version: \n",
            b"whisper.cpp version: 1\nextra\n", b"whisper.cpp version: 1\tbad\n"]
        {
            assert!(parse_version(value).is_err());
        }
    }

    #[test]
    fn cancellation_precedes_filesystem_or_native_access() {
        let cancellation = ProcessCancellationToken::new();
        cancellation.cancel();
        let request = SpeechRequest {
            input: PathBuf::from("absent.wav"), provider_path: PathBuf::from("absent-whisper"),
            provider_sha256: "a".repeat(64), model_path: PathBuf::from("absent.bin"),
            model_id: "test-model".to_owned(), model_sha256: "b".repeat(64),
            options: SpeechOptions::default(),
        };
        assert!(matches!(transcribe(&request, cancellation), Err(SpeechError::Cancelled)));
    }

    #[test]
    fn pinned_digests_must_be_lowercase_complete_and_exact() {
        let uppercase = "A".repeat(64);
        let non_hex = "g".repeat(64);
        for digest in ["", "aa", &uppercase, &non_hex] {
            assert!(expected_digest(digest, "test").is_err());
        }
        assert!(expected_digest(&"a".repeat(64), "test").is_ok());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pinned.bin");
        fs::write(&path, b"synthetic identity").unwrap();
        let snapshot = FileSnapshot::read(&path, "test", 1024, &ProcessCancellationToken::new()).unwrap();
        assert!(require_digest(&snapshot, &sha256(b"synthetic identity")).is_ok());
        assert!(require_digest(&snapshot, &"a".repeat(64)).is_err());
    }
}
