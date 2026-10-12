//! Read-only subtitle discovery and explicit selected-stream extraction.
//!
//! Only local Matroska/WebM and MOV/MP4 files enter the native provider. Input
//! demuxers are forced, network protocols and MOV external tracks are disabled,
//! and stream-info decoding is disabled. Extraction copies one SubRip/WebVTT
//! stream to bounded stdout; this adapter never creates an output file.
//!
//! Source hashes and metadata are checked before and after native work. This is
//! change detection, not protection against a hostile process changing a path
//! temporarily while FFmpeg opens it. Use a stable, user-controlled source.

use std::collections::BTreeSet;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::graph::Format;
use crate::process::{
    ProcessCancellationToken, ProcessEnvironment, ProcessError, ProcessExecutor,
    ProcessNetworkPolicy, ProcessOutputMode, ProcessRequest, ProcessResult, ProcessTermination,
    ToolProbeEvidence, ToolProbeStatus,
};
use crate::toolchain::{ToolDiscovery, ToolProbe, ToolRegistry, ToolRuntimeContext};

use super::{
    parse, validate, ExtractionProvenance, StreamIdentity, TimedTextFormat, TimedTranscript,
};

pub const SUBTITLE_INSPECTION_SCHEMA: &str = "renderflow.subtitle-inspection/v1";
pub const SUBTITLE_PROVIDER_ID: &str = "adapter.subtitles.ffmpeg";
pub const MAX_SUBTITLE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_PROBE_BYTES: usize = 256 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;
const MAX_STREAMS: usize = 256;
const HASH_TIMEOUT: Duration = Duration::from_secs(120);
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const EXTRACT_TIMEOUT: Duration = Duration::from_secs(300);

/// Discovered streams include unsupported codecs so users can make an explicit
/// choice without mistaking an unsupported stream for an absent subtitle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubtitleStream {
    pub index: u32,
    pub codec: String,
    pub language: Option<String>,
    pub supported: bool,
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubtitleInspection {
    pub schema: String,
    pub source_path: PathBuf,
    pub source_sha256: String,
    pub ffprobe_version: String,
    pub streams: Vec<SubtitleStream>,
}

#[derive(Debug, Error)]
pub enum SubtitleProviderError {
    #[error("input must be an existing, non-symlink local regular file: {0}")]
    InvalidSource(String),
    #[error("unsupported media container '{0}'; use MKV, MKA, WebM, MP4, M4V, or MOV")]
    UnsupportedContainer(String),
    #[error("source is larger than the {MAX_SOURCE_BYTES}-byte subtitle input limit")]
    SourceTooLarge,
    #[error("source changed while subtitles were being inspected or extracted")]
    SourceChanged,
    #[error("subtitle operation was cancelled")]
    Cancelled,
    #[error("source hashing exceeded the 120-second limit")]
    HashTimeout,
    #[error("required subtitle provider '{0}' is missing; install FFmpeg including ffprobe")]
    MissingProvider(String),
    #[error("subtitle provider configuration is unavailable: {0}")]
    ProviderConfiguration(String),
    #[error("subtitle provider '{tool}' failed: {diagnostic}")]
    ProviderFailed { tool: String, diagnostic: String },
    #[error("subtitle provider '{0}' exceeded the bounded capture limit")]
    TruncatedCapture(String),
    #[error("subtitle provider returned malformed stream metadata: {0}")]
    MalformedProbe(String),
    #[error("no embedded subtitle streams; speech transcription is a separate operation")]
    NoSubtitleStreams,
    #[error("subtitle stream index {0} was not present in the source")]
    StreamNotFound(u32),
    #[error("subtitle stream {index} uses unsupported codec '{codec}': {reason}")]
    UnsupportedCodec { index: u32, codec: String, reason: String },
    #[error("extracted subtitle data is unsupported or invalid: {0}")]
    InvalidSubtitle(String),
    #[error("subtitle source I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

impl SubtitleProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidSource(_) => "invalid_source",
            Self::UnsupportedContainer(_) => "unsupported_container",
            Self::SourceTooLarge => "source_too_large",
            Self::SourceChanged => "source_changed",
            Self::Cancelled => "cancelled",
            Self::HashTimeout => "hash_timeout",
            Self::MissingProvider(_) => "missing_provider",
            Self::ProviderConfiguration(_) => "provider_configuration",
            Self::ProviderFailed { .. } => "provider_failed",
            Self::TruncatedCapture(_) => "truncated_capture",
            Self::MalformedProbe(_) => "malformed_probe",
            Self::NoSubtitleStreams => "no_subtitle_streams",
            Self::StreamNotFound(_) => "stream_not_found",
            Self::UnsupportedCodec { .. } => "unsupported_codec",
            Self::InvalidSubtitle(_) => "invalid_subtitle",
            Self::Io(_) => "source_io",
        }
    }
}

pub fn inspect(input: &Path) -> Result<SubtitleInspection, SubtitleProviderError> {
    inspect_with_cancellation(input, ProcessCancellationToken::new())
}

pub fn inspect_with_cancellation(
    input: &Path,
    cancellation: ProcessCancellationToken,
) -> Result<SubtitleInspection, SubtitleProviderError> {
    let source = SourceSnapshot::read(input, &cancellation)?;
    let executor = ProcessExecutor::new();
    let inspection = inspect_source(&source, &executor, &cancellation)?;
    source.recheck(&cancellation)?;
    Ok(inspection)
}

pub fn extract(input: &Path, stream_index: u32) -> Result<TimedTranscript, SubtitleProviderError> {
    extract_with_cancellation(input, stream_index, ProcessCancellationToken::new())
}

pub fn extract_with_cancellation(
    input: &Path,
    stream_index: u32,
    cancellation: ProcessCancellationToken,
) -> Result<TimedTranscript, SubtitleProviderError> {
    let source = SourceSnapshot::read(input, &cancellation)?;
    extract_snapshot(source, stream_index, cancellation)
}

/// Extract from an immutable artifact-store object, whose filename has no
/// extension. Only the graph's closed format enum may select this demuxer.
/// The supplied object digest is checked before any native process is started;
/// no original source path is reopened and no media copy is staged for a suffix.
pub(crate) fn extract_stored(
    input: &Path,
    format: Format,
    expected_sha256: &str,
    stream_index: u32,
    cancellation: ProcessCancellationToken,
) -> Result<TimedTranscript, SubtitleProviderError> {
    let extension = match format {
        Format::Mkv => "mkv",
        Format::WebM => "webm",
        Format::Mp4 => "mp4",
        Format::Mov => "mov",
        _ => return Err(SubtitleProviderError::UnsupportedContainer(format.to_string())),
    };
    let source = SourceSnapshot::read_as(input, Some(extension), &cancellation)?;
    if source.sha256 != expected_sha256 {
        return Err(SubtitleProviderError::SourceChanged);
    }
    extract_snapshot(source, stream_index, cancellation)
}

fn extract_snapshot(
    source: SourceSnapshot,
    stream_index: u32,
    cancellation: ProcessCancellationToken,
) -> Result<TimedTranscript, SubtitleProviderError> {
    let executor = ProcessExecutor::new();
    let inspection = inspect_source(&source, &executor, &cancellation)?;
    let stream = selected_stream(&inspection.streams, stream_index)?;
    let format = match stream.codec.as_str() {
        "subrip" => TimedTextFormat::Srt,
        "webvtt" => TimedTextFormat::WebVtt,
        _ => unreachable!("selected_stream admits only supported codecs"),
    };
    let (executable, ffmpeg_version) = provider_version("tool.ffmpeg", &executor, &cancellation)?;
    source.recheck(&cancellation)?;
    let request = bounded_request(&executable, MAX_SUBTITLE_BYTES, EXTRACT_TIMEOUT, &cancellation)
        .args(extraction_arguments(&source.path, source.container, stream_index, &stream.codec)?);
    let result = run_checked(&executor, request, &executable)?;
    refuse_media_errors(&result, &executable)?;
    source.recheck(&cancellation)?;
    let mut transcript = parse(result.stdout().bytes(), format)
        .map_err(|error| SubtitleProviderError::InvalidSubtitle(error.to_string()))?;
    let subtitle_sha256 = transcript.source.sha256.clone();
    transcript.source.sha256 = source.sha256;
    transcript.source.format = source.extension;
    transcript.source.stream = Some(StreamIdentity {
        index: stream.index,
        codec: stream.codec.clone(),
        language: stream.language.clone(),
    });
    transcript.source.extraction = Some(ExtractionProvenance {
        provider: SUBTITLE_PROVIDER_ID.to_string(),
        ffprobe_version: inspection.ffprobe_version,
        ffmpeg_version,
        subtitle_sha256,
    });
    transcript.language = stream.language.clone();
    validate(&transcript)
        .map_err(|error| SubtitleProviderError::InvalidSubtitle(error.to_string()))?;
    Ok(transcript)
}

fn inspect_source(
    source: &SourceSnapshot,
    executor: &ProcessExecutor,
    cancellation: &ProcessCancellationToken,
) -> Result<SubtitleInspection, SubtitleProviderError> {
    let (executable, ffprobe_version) = provider_version("tool.ffprobe", executor, cancellation)?;
    let request = bounded_request(&executable, MAX_PROBE_BYTES, PROBE_TIMEOUT, cancellation)
        .args(probe_arguments(&source.path, source.container)?);
    let result = run_checked(executor, request, &executable)?;
    refuse_media_errors(&result, &executable)?;
    Ok(SubtitleInspection {
        schema: SUBTITLE_INSPECTION_SCHEMA.to_string(),
        source_path: source.path.clone(),
        source_sha256: source.sha256.clone(),
        ffprobe_version,
        streams: parse_probe(result.stdout().bytes())?,
    })
}

fn bounded_request(
    executable: &str,
    stdout_limit: usize,
    timeout: Duration,
    cancellation: &ProcessCancellationToken,
) -> ProcessRequest {
    ProcessRequest::direct(executable)
        .stdout(ProcessOutputMode::capture(stdout_limit))
        .stderr(ProcessOutputMode::capture(MAX_DIAGNOSTIC_BYTES))
        .timeout(timeout)
        .cancellation(cancellation.clone())
        .environment(ProcessEnvironment::filtered_inherit()
            .deny("FFREPORT")
            .deny("AV_LOG_FORCE_COLOR")
            .set("AV_LOG_FORCE_NOCOLOR", "1")
            .set("LC_ALL", "C"))
        .network_policy(ProcessNetworkPolicy::Deny)
}

fn provider_version(
    id: &str,
    executor: &ProcessExecutor,
    cancellation: &ProcessCancellationToken,
) -> Result<(String, String), SubtitleProviderError> {
    ensure_not_cancelled(cancellation)?;
    let registry = ToolRegistry::builtins();
    let descriptor = registry.get(id)
        .ok_or_else(|| SubtitleProviderError::ProviderConfiguration(id.to_string()))?;
    let ToolDiscovery::Executable { candidates, version_args } = &descriptor.discovery else {
        return Err(SubtitleProviderError::ProviderConfiguration(id.to_string()));
    };
    let executable = candidates.first()
        .ok_or_else(|| SubtitleProviderError::ProviderConfiguration(id.to_string()))?;
    let result = run_checked(executor,
        bounded_request(executable, MAX_DIAGNOSTIC_BYTES, Duration::from_secs(5), cancellation)
            .args(version_args.iter().cloned()), executable)?;
    let line = result.stdout().redacted_text().lines()
        .chain(result.stderr().redacted_text().lines())
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| SubtitleProviderError::ProviderFailed {
            tool: executable.clone(), diagnostic: "version response was empty".to_string(),
        })?;
    if line.len() > 4096 || line.chars().any(char::is_control) {
        return Err(SubtitleProviderError::ProviderFailed {
            tool: executable.clone(), diagnostic: "version response is not a bounded plain-text line".to_string(),
        });
    }
    let version = line.trim().to_string();
    // Reuse the registry's version/platform assessment without a second native
    // probe, preserving this operation's timeout/cancellation/capture policy.
    let recorded = RecordedVersion(ToolProbeEvidence {
        executable: executable.clone(),
        status: ToolProbeStatus::Available,
        version_line: Some(version.clone()),
        duration_ms: result.duration_ms(),
        platform: *result.platform(),
        diagnostic: None,
    });
    let inventory = registry.assess_ids_with([id], &recorded, &ToolRuntimeContext::current());
    let availability = inventory.get(id)
        .ok_or_else(|| SubtitleProviderError::ProviderConfiguration(id.to_string()))?;
    if !availability.is_available() {
        return Err(SubtitleProviderError::ProviderFailed {
            tool: executable.clone(), diagnostic: availability.summary(),
        });
    }
    Ok((executable.clone(), version))
}

struct RecordedVersion(ToolProbeEvidence);

impl ToolProbe for RecordedVersion {
    fn probe(&self, executable: &str, _version_args: &[String]) -> ToolProbeEvidence {
        let mut evidence = self.0.clone();
        if executable != evidence.executable {
            evidence.status = ToolProbeStatus::Missing;
            evidence.version_line = None;
            evidence.diagnostic = Some("candidate was not probed by this operation".to_string());
        }
        evidence
    }
}

fn run_checked(
    executor: &ProcessExecutor,
    request: ProcessRequest,
    tool: &str,
) -> Result<ProcessResult, SubtitleProviderError> {
    let result = executor.execute(request).map_err(|error| match error {
        ProcessError::MissingExecutable { .. } => SubtitleProviderError::MissingProvider(tool.to_string()),
        error => SubtitleProviderError::ProviderFailed { tool: tool.to_string(), diagnostic: error.to_string() },
    })?;
    if result.termination() == ProcessTermination::Cancelled {
        return Err(SubtitleProviderError::Cancelled);
    }
    if result.stdout().truncated() || result.stderr().truncated() {
        return Err(SubtitleProviderError::TruncatedCapture(tool.to_string()));
    }
    if !result.is_success() {
        return Err(SubtitleProviderError::ProviderFailed {
            tool: tool.to_string(), diagnostic: result.failure_message(),
        });
    }
    Ok(result)
}

fn refuse_media_errors(result: &ProcessResult, tool: &str) -> Result<(), SubtitleProviderError> {
    // Media commands capture warning-and-error logging. In particular the SRT
    // muxer can skip packets without timestamps with only a warning and return
    // success. Reject all such diagnostics rather than accept partial output.
    // Version probes may print to stderr and do not use this media-only check.
    if result.stderr().bytes().iter().any(|byte| !byte.is_ascii_whitespace()) {
        return Err(SubtitleProviderError::ProviderFailed {
            tool: tool.to_string(),
            diagnostic: format!(
                "provider reported warnings or errors; potentially partial subtitle output was refused: {}",
                result.stderr().redacted_text().trim(),
            ),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container { Matroska, Mov }

impl Container {
    fn from_extension(extension: &str) -> Result<Self, SubtitleProviderError> {
        match extension {
            "mkv" | "mka" | "webm" => Ok(Self::Matroska),
            "mp4" | "m4v" | "mov" => Ok(Self::Mov),
            _ => Err(SubtitleProviderError::UnsupportedContainer(extension.to_string())),
        }
    }

    fn demuxer(self) -> &'static str {
        match self { Self::Matroska => "matroska", Self::Mov => "mov" }
    }
}

fn input_arguments(path: &Path, container: Container) -> Result<Vec<String>, SubtitleProviderError> {
    if !path.is_absolute() {
        return Err(SubtitleProviderError::InvalidSource("native input must be absolute".to_string()));
    }
    let input = path.to_str().ok_or_else(|| SubtitleProviderError::InvalidSource(
        "native input path must be UTF-8".to_string()))?;
    let mut arguments: Vec<String> = [
        "-protocol_whitelist", "file,pipe", "-format_whitelist", container.demuxer(),
        "-f", container.demuxer(), "-nofind_stream_info", "-max_streams", "256",
    ].into_iter().map(str::to_string).collect();
    if container == Container::Mov {
        arguments.extend(["-enable_drefs", "0", "-use_absolute_path", "0"].map(str::to_string));
    }
    arguments.extend(["-i".to_string(), input.to_string()]);
    Ok(arguments)
}

fn probe_arguments(path: &Path, container: Container) -> Result<Vec<String>, SubtitleProviderError> {
    let mut arguments: Vec<String> = [
        "-hide_banner", "-loglevel", "warning", "-select_streams", "s",
        "-show_entries", "stream=index,codec_name,codec_type:stream_tags=language",
        "-print_format", "json",
    ].into_iter().map(str::to_string).collect();
    arguments.extend(input_arguments(path, container)?);
    Ok(arguments)
}

fn extraction_arguments(
    path: &Path,
    container: Container,
    stream_index: u32,
    codec: &str,
) -> Result<Vec<String>, SubtitleProviderError> {
    let muxer = match codec {
        "subrip" => "srt",
        "webvtt" => "webvtt",
        _ => return Err(unsupported_codec(stream_index, codec)),
    };
    let mut arguments: Vec<String> = [
        "-hide_banner", "-loglevel", "warning", "-nostdin", "-xerror", "-copyts",
    ].into_iter().map(str::to_string).collect();
    arguments.extend(input_arguments(path, container)?);
    arguments.extend([
        "-map".to_string(), format!("0:{stream_index}"),
        "-vn".to_string(), "-an".to_string(), "-dn".to_string(),
        "-codec:s".to_string(), "copy".to_string(),
        "-map_metadata".to_string(), "-1".to_string(),
        "-map_chapters".to_string(), "-1".to_string(),
        "-avoid_negative_ts".to_string(), "disabled".to_string(),
        "-f".to_string(), muxer.to_string(), "pipe:1".to_string(),
    ]);
    Ok(arguments)
}

#[derive(Deserialize)]
struct ProbeDocument { streams: Vec<ProbeStream> }

#[derive(Deserialize)]
struct ProbeStream {
    index: u32,
    codec_name: String,
    codec_type: String,
    #[serde(default)]
    tags: ProbeTags,
}

#[derive(Default, Deserialize)]
struct ProbeTags { language: Option<String> }

fn parse_probe(bytes: &[u8]) -> Result<Vec<SubtitleStream>, SubtitleProviderError> {
    if bytes.len() > MAX_PROBE_BYTES {
        return Err(SubtitleProviderError::TruncatedCapture("ffprobe".to_string()));
    }
    let document: ProbeDocument = serde_json::from_slice(bytes)
        .map_err(|error| SubtitleProviderError::MalformedProbe(error.to_string()))?;
    if document.streams.len() > MAX_STREAMS {
        return Err(SubtitleProviderError::MalformedProbe("more than 256 subtitle streams".to_string()));
    }
    let mut seen = BTreeSet::new();
    let mut streams = Vec::with_capacity(document.streams.len());
    for stream in document.streams {
        if stream.codec_type != "subtitle" || !seen.insert(stream.index) {
            return Err(SubtitleProviderError::MalformedProbe("non-subtitle or duplicate stream index".to_string()));
        }
        if stream.codec_name.is_empty() || stream.codec_name.len() > 128
            || !stream.codec_name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(SubtitleProviderError::MalformedProbe("invalid codec name".to_string()));
        }
        if stream.tags.language.as_ref().is_some_and(|language|
            language.is_empty() || language.len() > 64 || !language.bytes().all(|byte|
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
        {
            return Err(SubtitleProviderError::MalformedProbe("invalid stream language tag".to_string()));
        }
        let reason = unsupported_reason(&stream.codec_name);
        streams.push(SubtitleStream {
            index: stream.index,
            codec: stream.codec_name,
            language: stream.tags.language,
            supported: reason.is_none(),
            unsupported_reason: reason.map(str::to_string),
        });
    }
    streams.sort_by_key(|stream| stream.index);
    Ok(streams)
}

fn unsupported_reason(codec: &str) -> Option<&'static str> {
    match codec {
        "subrip" | "webvtt" => None,
        "hdmv_pgs_subtitle" | "dvd_subtitle" | "dvb_subtitle" | "xsub" =>
            Some("bitmap subtitles require a separate OCR provider; OCR is not performed"),
        "ass" | "ssa" => Some("styled ASS/SSA needs a style-preserving adapter or explicit loss policy"),
        "mov_text" => Some("MOV timed text is not supported by this stream-copy profile"),
        _ => Some("only embedded SubRip and WebVTT are supported by this profile"),
    }
}

fn unsupported_codec(index: u32, codec: &str) -> SubtitleProviderError {
    SubtitleProviderError::UnsupportedCodec {
        index, codec: codec.to_string(),
        reason: unsupported_reason(codec).unwrap_or("codec is not supported").to_string(),
    }
}

fn selected_stream(streams: &[SubtitleStream], index: u32) -> Result<&SubtitleStream, SubtitleProviderError> {
    if streams.is_empty() { return Err(SubtitleProviderError::NoSubtitleStreams); }
    let stream = streams.iter().find(|stream| stream.index == index)
        .ok_or(SubtitleProviderError::StreamNotFound(index))?;
    if unsupported_reason(&stream.codec).is_some() {
        return Err(unsupported_codec(index, &stream.codec));
    }
    Ok(stream)
}

struct SourceSnapshot {
    path: PathBuf,
    extension: String,
    container: Container,
    metadata: Metadata,
    sha256: String,
}

impl SourceSnapshot {
    fn read(input: &Path, cancellation: &ProcessCancellationToken) -> Result<Self, SubtitleProviderError> {
        Self::read_as(input, None, cancellation)
    }

    fn read_as(
        input: &Path,
        declared_extension: Option<&str>,
        cancellation: &ProcessCancellationToken,
    ) -> Result<Self, SubtitleProviderError> {
        ensure_not_cancelled(cancellation)?;
        let metadata = fs::symlink_metadata(input)
            .map_err(|error| SubtitleProviderError::InvalidSource(error.to_string()))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(SubtitleProviderError::InvalidSource(input.display().to_string()));
        }
        let path = fs::canonicalize(input)?;
        let extension = declared_extension
            .or_else(|| path.extension().and_then(|extension| extension.to_str()))
            .unwrap_or("").to_ascii_lowercase();
        let container = Container::from_extension(&extension)?;
        // Validate representability before hashing a potentially large file.
        input_arguments(&path, container)?;
        let (metadata, sha256) = hash_source(&path, cancellation)?;
        Ok(Self { path, extension, container, metadata, sha256 })
    }

    fn recheck(&self, cancellation: &ProcessCancellationToken) -> Result<(), SubtitleProviderError> {
        let (metadata, digest) = hash_source(&self.path, cancellation)?;
        if digest != self.sha256 || !same_metadata(&self.metadata, &metadata) {
            return Err(SubtitleProviderError::SourceChanged);
        }
        Ok(())
    }
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

fn hash_source(path: &Path, cancellation: &ProcessCancellationToken) -> Result<(Metadata, String), SubtitleProviderError> {
    let path_metadata = fs::symlink_metadata(path)?;
    if !path_metadata.is_file() || path_metadata.file_type().is_symlink() {
        return Err(SubtitleProviderError::SourceChanged);
    }
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if !same_metadata(&path_metadata, &metadata) { return Err(SubtitleProviderError::SourceChanged); }
    if metadata.len() > MAX_SOURCE_BYTES { return Err(SubtitleProviderError::SourceTooLarge); }
    let started = Instant::now();
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        ensure_not_cancelled(cancellation)?;
        if started.elapsed() >= HASH_TIMEOUT { return Err(SubtitleProviderError::HashTimeout); }
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        total += count as u64;
        if total > MAX_SOURCE_BYTES { return Err(SubtitleProviderError::SourceTooLarge); }
        hash.update(&buffer[..count]);
    }
    let after = file.metadata()?;
    if total != metadata.len() || !same_metadata(&metadata, &after)
        || !same_metadata(&after, &fs::symlink_metadata(path)?)
    { return Err(SubtitleProviderError::SourceChanged); }
    Ok((metadata, format!("{:x}", hash.finalize())))
}

fn ensure_not_cancelled(cancellation: &ProcessCancellationToken) -> Result<(), SubtitleProviderError> {
    if cancellation.is_cancelled() { Err(SubtitleProviderError::Cancelled) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_preserves_global_index_language_and_unsupported_codecs() {
        let streams = parse_probe(br#"{"streams":[{"index":4,"codec_name":"hdmv_pgs_subtitle","codec_type":"subtitle"},{"index":2,"codec_name":"subrip","codec_type":"subtitle","tags":{"language":"eng"}}]}"#).unwrap();
        assert_eq!(streams[0].index, 2);
        assert_eq!(streams[0].language.as_deref(), Some("eng"));
        assert!(streams[0].supported);
        assert!(!streams[1].supported);
        assert!(streams[1].unsupported_reason.as_deref().unwrap().contains("OCR"));
    }

    #[test]
    fn empty_discovery_is_distinct_from_extracting_absent_stream() {
        let streams = parse_probe(br#"{"streams":[]}"#).unwrap();
        assert!(streams.is_empty());
        assert!(matches!(selected_stream(&streams, 0), Err(SubtitleProviderError::NoSubtitleStreams)));
    }

    #[test]
    fn malformed_and_duplicate_stream_metadata_is_refused() {
        for bytes in [
            br#"{"streams":[{"index":0,"codec_name":"subrip","codec_type":"audio"}]}"#.as_slice(),
            br#"{"streams":[{"index":0,"codec_name":"subrip","codec_type":"subtitle"},{"index":0,"codec_name":"webvtt","codec_type":"subtitle"}]}"#,
            br#"{"streams":[{"index":-1,"codec_name":"subrip","codec_type":"subtitle"}]}"#,
            br#"{"streams":[{"index":2,"codec_name":"subrip","codec_type":"subtitle","tags":{"language":"en\n-injected"}}]}"#,
            br#"{"streams":[{"index":0,"codec_type":"subtitle"}]}"#,
        ] { assert!(parse_probe(bytes).is_err()); }
    }

    #[test]
    fn absent_selected_stream_never_falls_back_to_first_stream() {
        let streams = parse_probe(br#"{"streams":[{"index":3,"codec_name":"subrip","codec_type":"subtitle"}]}"#).unwrap();
        assert!(matches!(selected_stream(&streams, 0), Err(SubtitleProviderError::StreamNotFound(0))));
    }

    #[test]
    fn bitmap_styled_and_mov_text_are_typed_refusals() {
        for codec in ["hdmv_pgs_subtitle", "ass", "ssa", "mov_text", "unknown"] {
            let streams = vec![SubtitleStream { index: 2, codec: codec.to_string(), language: None,
                supported: false, unsupported_reason: None }];
            assert!(matches!(selected_stream(&streams, 2), Err(SubtitleProviderError::UnsupportedCodec { .. })));
        }
    }

    #[test]
    fn extraction_is_explicit_stream_copy_with_no_timestamp_shift_or_file_output() {
        let path = std::env::temp_dir().join("--source with spaces.mkv");
        let args = extraction_arguments(&path, Container::Matroska, 7, "subrip").unwrap();
        assert!(args.windows(2).any(|pair| pair == ["-map", "0:7"]));
        assert!(args.windows(2).any(|pair| pair == ["-codec:s", "copy"]));
        assert!(args.windows(2).any(|pair| pair == ["-avoid_negative_ts", "disabled"]));
        assert!(args.contains(&"-copyts".to_string()));
        assert!(args.contains(&"-nofind_stream_info".to_string()));
        assert!(args.contains(&"-vn".to_string()) && args.contains(&"-an".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("pipe:1"));
        assert!(args.contains(&path.to_str().unwrap().to_string()));
    }

    #[test]
    fn mov_input_disables_external_tracks_and_forces_local_demuxing() {
        let path = std::env::temp_dir().join("source.mp4");
        let args = probe_arguments(&path, Container::Mov).unwrap();
        for pair in [["-enable_drefs", "0"], ["-use_absolute_path", "0"],
            ["-format_whitelist", "mov"], ["-protocol_whitelist", "file,pipe"]]
        { assert!(args.windows(2).any(|window| window == pair)); }
    }

    #[test]
    fn playlists_and_unknown_containers_are_refused_before_launch() {
        for extension in ["m3u8", "mpd", "ffconcat", "avi", "", "srt"] {
            assert!(matches!(Container::from_extension(extension), Err(SubtitleProviderError::UnsupportedContainer(_))));
        }
        assert!(input_arguments(Path::new("relative.mkv"), Container::Matroska).is_err());
    }

    #[test]
    fn source_digest_change_is_detected_without_provider_execution() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("synthetic.mkv");
        fs::write(&path, b"synthetic-one").unwrap();
        let token = ProcessCancellationToken::new();
        let snapshot = SourceSnapshot::read(&path, &token).unwrap();
        fs::write(&path, b"synthetic-two").unwrap();
        assert!(matches!(snapshot.recheck(&token), Err(SubtitleProviderError::SourceChanged)));
    }

    #[test]
    fn cancellation_prevents_source_and_provider_work() {
        let token = ProcessCancellationToken::new();
        token.cancel();
        assert!(matches!(inspect_with_cancellation(Path::new("missing.mkv"), token), Err(SubtitleProviderError::Cancelled)));
    }

    #[test]
    fn overlarge_probe_capture_is_not_parsed_as_complete() {
        let bytes = vec![b' '; MAX_PROBE_BYTES + 1];
        assert!(matches!(parse_probe(&bytes), Err(SubtitleProviderError::TruncatedCapture(_))));
    }

    #[test]
    fn stored_media_uses_declared_format_without_a_suffix_or_media_copy() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("0123456789abcdef");
        fs::write(&path, b"synthetic container").unwrap();
        let token = ProcessCancellationToken::new();
        let snapshot = SourceSnapshot::read_as(&path, Some("mkv"), &token).unwrap();
        assert_eq!(snapshot.path, fs::canonicalize(&path).unwrap());
        assert_eq!(snapshot.extension, "mkv");
        assert_eq!(snapshot.container, Container::Matroska);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert!(matches!(extract_stored(&path, Format::Mkv, &"0".repeat(64), 2, token),
            Err(SubtitleProviderError::SourceChanged)));
    }
}
