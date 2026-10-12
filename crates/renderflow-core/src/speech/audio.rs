//! Explicit, pinned local media inspection and preparation for speech recognition.
//!
//! A selected global audio stream is decoded to a private, mono 16 kHz PCM16 WAV.
//! Timestamps count decoded samples from zero; they are not source-video times.
//! Demuxer/protocol restrictions reduce accidental external reads, but neither
//! the native process wrapper nor these arguments are an operating-system sandbox.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Map, Value};

use crate::process::{
    ProcessCancellationToken, ProcessExecutor, ProcessExpectedOutput, ProcessOutputMode,
};

use super::provider::{
    bounded_request, ensure_not_cancelled, expected_digest, inspect_pcm_wav,
    path_text, read_bounded, require_digest, run_checked, sha256, FileSnapshot,
    MAX_PROVIDER_BYTES,
};
use super::{
    audio_preparation_configuration_digest, AudioInventory, AudioPreparationEvidence,
    AudioPreparationLimits, AudioStream, AudioToolIdentity, SpeechError, StrictValue,
    AUDIO_INVENTORY_SCHEMA_V1, AUDIO_PREPARATION_PROFILE, AUDIO_TIMESTAMP_ORIGIN,
    MAX_AUDIO_SOURCE_BYTES, MAX_PREPARED_AUDIO_DURATION_MS, MAX_PREPARED_WAV_BYTES,
};

pub const MAX_MEDIA_BYTES: u64 = MAX_AUDIO_SOURCE_BYTES;
pub const MAX_PROBE_BYTES: usize = 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_STREAMS: usize = 256;

// The configuration digest is built from these same arrays used in argv.
const GLOBAL_ARGUMENTS: &[&str] = &[
    "-hide_banner", "-loglevel", "error", "-nostdin", "-n", "-xerror",
    "-filter_threads", "1",
];
const INPUT_ARGUMENTS: &[&str] = &[
    "-protocol_whitelist", "file", "-max_streams", "256",
    "-probesize", "5000000", "-analyzeduration", "5000000", "-threads", "1",
];
const PROBE_ARGUMENTS: &[&str] = &[
    "-hide_banner", "-loglevel", "error", "-select_streams", "a",
    "-show_entries", "format=format_name:stream=index,codec_name,codec_type,channels,sample_rate,duration,start_time:stream_tags=language,title",
    "-print_format", "json",
];
const MOV_ARGUMENTS: &[&str] = &["-enable_drefs", "0", "-use_absolute_path", "0"];
const FORMAT_WHITELIST_ARGUMENT: &str = "-format_whitelist";
const FORMAT_ARGUMENT: &str = "-f";
const SOURCE_ARGUMENT: &str = "-i";
const STREAM_ARGUMENT: &str = "-map";
const OUTPUT_ARGUMENTS: &[&str] = &[
    "-vn", "-sn", "-dn", "-map_metadata", "-1", "-map_metadata:s:a", "-1", "-map_chapters", "-1",
    "-filter:a", "aresample=16000:async=0,asetpts=N/SR/TB",
    "-codec:a", "pcm_s16le", "-ac", "1", "-ar", "16000", "-threads", "1",
    // 601s is a refusal sentinel, not a successful clipping mode: WAV validation
    // rejects every result longer than 600s, regardless of reported duration.
    "-t", "601", "-f", "wav", "-rf64", "never", "-write_bext", "0",
];

/// Fixed probe/extraction argv policy, including conditional MOV restrictions. The actual
/// demuxer, global stream index, tool identities, and timeout are hashed separately.
pub(crate) fn audio_fixed_arguments() -> Vec<&'static str> {
    let mut arguments: Vec<_> = [GLOBAL_ARGUMENTS, INPUT_ARGUMENTS, MOV_ARGUMENTS, OUTPUT_ARGUMENTS, PROBE_ARGUMENTS]
        .into_iter().flat_map(|arguments| arguments.iter().copied()).collect();
    arguments.extend([FORMAT_WHITELIST_ARGUMENT, FORMAT_ARGUMENT, SOURCE_ARGUMENT, STREAM_ARGUMENT]);
    arguments
}

#[derive(Debug, Clone)]
pub struct AudioTool {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct AudioInspectRequest {
    pub input: PathBuf,
    pub ffprobe: AudioTool,
}

#[derive(Debug, Clone)]
pub struct AudioPrepareRequest {
    pub inspection: AudioInspectRequest,
    /// Digest copied from inspection. A replaced input must be inspected again.
    pub expected_source_sha256: String,
    pub ffmpeg: AudioTool,
    /// Global container stream index, not the audio-only ordinal.
    pub stream_index: u32,
    pub timeout_seconds: u64,
}

#[derive(Debug)]
pub struct AudioInspection {
    pub inventory: AudioInventory,
    pub provider_output: Vec<u8>,
}

#[derive(Debug)]
pub struct PreparedAudio {
    pub path: PathBuf,
    pub evidence: AudioPreparationEvidence,
    pub probe_output: Vec<u8>,
    source: FileSnapshot,
    ffprobe: FileSnapshot,
    ffmpeg: FileSnapshot,
    _temporary: tempfile::TempDir,
}

impl PreparedAudio {
    /// Recheck the original media and preparation executables after recognition.
    /// Hashes detect persistent changes, not adversarial swaps between checks.
    pub fn recheck(&self, cancellation: &ProcessCancellationToken) -> Result<(), SpeechError> {
        for snapshot in [&self.source, &self.ffprobe, &self.ffmpeg] {
            snapshot.recheck(cancellation)?;
        }
        ensure_not_cancelled(cancellation)
    }

    /// Retain the exact prepared waveform with the final transcript bundle.
    pub fn read_wav(&self, cancellation: &ProcessCancellationToken) -> Result<Vec<u8>, SpeechError> {
        let bytes = read_bounded(&self.path, MAX_PREPARED_WAV_BYTES, cancellation)?;
        if bytes.len() as u64 != self.evidence.prepared_wav_size_bytes
            || sha256(&bytes) != self.evidence.prepared_wav_sha256
            || inspect_pcm_wav(&bytes)? != self.evidence.prepared_wav_duration_ms
        {
            return Err(SpeechError::SourceChanged("prepared WAV changed after extraction".to_owned()));
        }
        Ok(bytes)
    }
}

/// Discover audio streams without choosing one or invoking a speech model.
pub fn inspect(
    request: &AudioInspectRequest,
    cancellation: ProcessCancellationToken,
) -> Result<AudioInspection, SpeechError> {
    ensure_not_cancelled(&cancellation)?;
    expected_digest(&request.ffprobe.sha256, "ffprobe_sha256")?;
    let container = Container::from_path(&request.input)?;
    let source = FileSnapshot::read(&request.input, "media source", MAX_MEDIA_BYTES, &cancellation)?;
    let ffprobe = pinned_tool(&request.ffprobe, "ffprobe", &cancellation)?;
    let temporary = tempfile::Builder::new().prefix("renderflow-audio-probe-").tempdir()?;
    let work_directory = fs::canonicalize(temporary.path())?;
    let inspection = inspect_snapshot(&source, &ffprobe, container, &work_directory, &cancellation)?;
    source.recheck(&cancellation)?;
    ffprobe.recheck(&cancellation)?;
    Ok(inspection)
}

/// Decode exactly the selected stream. Unknown stream duration is allowed only
/// because the decoded-sample sentinel and RIFF inspection refuse overlong audio.
/// No successful preparation intentionally trims, segments, or preserves gaps.
pub fn prepare(
    request: &AudioPrepareRequest,
    cancellation: ProcessCancellationToken,
) -> Result<PreparedAudio, SpeechError> {
    ensure_not_cancelled(&cancellation)?;
    expected_digest(&request.expected_source_sha256, "input_sha256")?;
    expected_digest(&request.inspection.ffprobe.sha256, "ffprobe_sha256")?;
    expected_digest(&request.ffmpeg.sha256, "ffmpeg_sha256")?;
    if !(1..=3600).contains(&request.timeout_seconds) {
        return Err(SpeechError::InvalidRequest("audio timeout must be between 1 and 3600 seconds".to_owned()));
    }
    let container = Container::from_path(&request.inspection.input)?;
    let source = FileSnapshot::read(
        &request.inspection.input, "media source", MAX_MEDIA_BYTES, &cancellation,
    )?;
    require_digest(&source, &request.expected_source_sha256)?;
    let ffprobe = pinned_tool(&request.inspection.ffprobe, "ffprobe", &cancellation)?;
    let ffmpeg = pinned_tool(&request.ffmpeg, "ffmpeg", &cancellation)?;
    let temporary = tempfile::Builder::new().prefix("renderflow-audio-prepare-").tempdir()?;
    let work_directory = fs::canonicalize(temporary.path())?;
    let inspection = inspect_snapshot(&source, &ffprobe, container, &work_directory, &cancellation)?;
    let selected = inspection.inventory.streams.iter()
        .find(|stream| stream.index == request.stream_index).cloned()
        .ok_or_else(|| SpeechError::InvalidRequest(format!(
            "global audio stream index {} is not present; inspect and select an audio stream", request.stream_index,
        )))?;
    if selected.duration_ms.is_some_and(|duration| duration == 0 || duration > MAX_PREPARED_AUDIO_DURATION_MS) {
        return Err(SpeechError::InvalidRequest(
            "selected audio stream duration is outside the nonempty ten-minute profile".to_owned(),
        ));
    }
    let executor = ProcessExecutor::new();
    let ffmpeg_identity = tool_identity(&ffmpeg, "ffmpeg", &executor, &work_directory, &cancellation)?;
    for snapshot in [&source, &ffprobe, &ffmpeg] { snapshot.recheck(&cancellation)?; }
    let output_path = work_directory.join("prepared.wav");
    let process = bounded_request(
        &ffmpeg.path, &work_directory, Duration::from_secs(request.timeout_seconds), &cancellation,
    )?.args(extraction_arguments(&source.path, container, request.stream_index, &output_path)?)
        .expect_output(ProcessExpectedOutput::file(&output_path).require_non_empty().max_bytes(MAX_PREPARED_WAV_BYTES));
    run_checked(&executor, process)?;
    for snapshot in [&source, &ffprobe, &ffmpeg] { snapshot.recheck(&cancellation)?; }
    let waveform = read_bounded(&output_path, MAX_PREPARED_WAV_BYTES, &cancellation)?;
    let duration_ms = inspect_pcm_wav(&waveform)?;
    let mut evidence = AudioPreparationEvidence {
        source_sha256: source.sha256.clone(),
        source_size_bytes: source.metadata.len(),
        container_format: inspection.inventory.container_format,
        demuxer: container.demuxer().to_owned(),
        selected_stream: selected,
        ffprobe: inspection.inventory.ffprobe,
        ffmpeg: ffmpeg_identity,
        probe_output_sha256: inspection.inventory.probe_output_sha256,
        prepared_wav_sha256: sha256(&waveform),
        prepared_wav_size_bytes: waveform.len() as u64,
        prepared_wav_duration_ms: duration_ms,
        profile: AUDIO_PREPARATION_PROFILE.to_owned(),
        configuration_sha256: String::new(),
        timestamp_origin: AUDIO_TIMESTAMP_ORIGIN.to_owned(),
        timeout_seconds: request.timeout_seconds,
        limits: AudioPreparationLimits {
            max_source_bytes: MAX_MEDIA_BYTES,
            max_duration_ms: MAX_PREPARED_AUDIO_DURATION_MS,
            max_prepared_wav_bytes: MAX_PREPARED_WAV_BYTES,
        },
        limitations: vec![
            "Audio is downmixed and resampled; transcript times count decoded samples from zero, not source-video timestamps.".to_owned(),
            "Stream start and duration metadata are provider reports; absent duration is guarded by decoded PCM sample count.".to_owned(),
            "A 601-second output sentinel causes refusal above 600 seconds; successful output is not an intentionally clipped excerpt.".to_owned(),
            "Native libraries, codec implementations, and temporary path races are not attested by executable hashes.".to_owned(),
            "File-only protocols and forced demuxers are restrictions, not an operating-system resource or network sandbox.".to_owned(),
        ],
    };
    evidence.configuration_sha256 = audio_preparation_configuration_digest(&evidence)?;
    evidence.validate()?;
    ensure_not_cancelled(&cancellation)?;
    Ok(PreparedAudio {
        path: output_path, evidence, probe_output: inspection.provider_output,
        source, ffprobe, ffmpeg, _temporary: temporary,
    })
}

fn pinned_tool(
    tool: &AudioTool, name: &'static str, cancellation: &ProcessCancellationToken,
) -> Result<FileSnapshot, SpeechError> {
    expected_digest(&tool.sha256, name)?;
    let snapshot = FileSnapshot::read(&tool.path, name, MAX_PROVIDER_BYTES, cancellation)?;
    require_digest(&snapshot, &tool.sha256)?;
    Ok(snapshot)
}

fn tool_identity(
    snapshot: &FileSnapshot,
    name: &str,
    executor: &ProcessExecutor,
    work_directory: &Path,
    cancellation: &ProcessCancellationToken,
) -> Result<AudioToolIdentity, SpeechError> {
    let output = run_checked(executor, bounded_request(
        &snapshot.path, work_directory, VERSION_TIMEOUT, cancellation,
    )?.arg("-version"))?;
    let text = std::str::from_utf8(output.stdout().bytes())
        .map_err(|_| SpeechError::ProviderUnavailable(format!("{name} version is not UTF-8")))?;
    let line = text.lines().next().unwrap_or("");
    let prefix = format!("{name} version ");
    if line.len() > 4096 || line.chars().any(char::is_control)
        || line.strip_prefix(prefix.as_str()).is_none_or(|version| version.trim().is_empty())
    {
        return Err(SpeechError::ProviderUnavailable(format!("expected a bounded '{name} version ...' first line")));
    }
    snapshot.recheck(cancellation)?;
    Ok(AudioToolIdentity { tool_id: format!("tool.{name}"), version: line.to_owned(), sha256: snapshot.sha256.clone() })
}

fn inspect_snapshot(
    source: &FileSnapshot,
    ffprobe: &FileSnapshot,
    container: Container,
    work_directory: &Path,
    cancellation: &ProcessCancellationToken,
) -> Result<AudioInspection, SpeechError> {
    let executor = ProcessExecutor::new();
    let identity = tool_identity(ffprobe, "ffprobe", &executor, work_directory, cancellation)?;
    source.recheck(cancellation)?;
    let result = run_checked(&executor, bounded_request(
        &ffprobe.path, work_directory, PROBE_TIMEOUT, cancellation,
    )?.stdout(ProcessOutputMode::capture(MAX_PROBE_BYTES))
        .args(probe_arguments(&source.path, container)?))?;
    source.recheck(cancellation)?;
    ffprobe.recheck(cancellation)?;
    let bytes = result.stdout().bytes().to_vec();
    let (format, streams) = parse_probe(&bytes, container)?;
    let inventory = AudioInventory {
        schema_version: AUDIO_INVENTORY_SCHEMA_V1.to_owned(),
        source_sha256: source.sha256.clone(),
        source_size_bytes: source.metadata.len(),
        container_format: format,
        demuxer: container.demuxer().to_owned(),
        ffprobe: identity,
        probe_output_sha256: sha256(&bytes),
        streams,
    };
    inventory.validate()?;
    Ok(AudioInspection { inventory, provider_output: bytes })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container { Matroska, Mov, Avi, Wav, Flac, Mp3, Ogg, Aac }

impl Container {
    fn from_path(path: &Path) -> Result<Self, SpeechError> {
        let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
        match extension.as_str() {
            "mkv" | "mka" | "webm" => Ok(Self::Matroska),
            "mp4" | "m4v" | "m4a" | "mov" => Ok(Self::Mov),
            "avi" => Ok(Self::Avi), "wav" => Ok(Self::Wav), "flac" => Ok(Self::Flac),
            "mp3" => Ok(Self::Mp3), "ogg" | "oga" | "opus" => Ok(Self::Ogg), "aac" => Ok(Self::Aac),
            _ => Err(SpeechError::InvalidRequest(
                "audio preparation requires a supported local MP4/MOV/MKV/WebM/AVI/WAV/FLAC/MP3/Ogg/AAC filename".to_owned(),
            )),
        }
    }

    fn demuxer(self) -> &'static str {
        match self {
            Self::Matroska => "matroska", Self::Mov => "mov", Self::Avi => "avi", Self::Wav => "wav",
            Self::Flac => "flac", Self::Mp3 => "mp3", Self::Ogg => "ogg", Self::Aac => "aac",
        }
    }
}

fn input_arguments(path: &Path, container: Container) -> Result<Vec<String>, SpeechError> {
    let mut arguments: Vec<String> = INPUT_ARGUMENTS.iter().map(|value| (*value).to_owned()).collect();
    arguments.extend([
        FORMAT_WHITELIST_ARGUMENT.to_owned(), container.demuxer().to_owned(),
        FORMAT_ARGUMENT.to_owned(), container.demuxer().to_owned(),
    ]);
    if container == Container::Mov {
        arguments.extend(MOV_ARGUMENTS.iter().map(|value| (*value).to_owned()));
    }
    arguments.extend([SOURCE_ARGUMENT.to_owned(), path_text(path)?.to_owned()]);
    Ok(arguments)
}

fn probe_arguments(path: &Path, container: Container) -> Result<Vec<String>, SpeechError> {
    let mut arguments: Vec<String> = PROBE_ARGUMENTS.iter().map(|value| (*value).to_owned()).collect();
    arguments.extend(input_arguments(path, container)?);
    Ok(arguments)
}

fn extraction_arguments(
    source: &Path, container: Container, stream_index: u32, output: &Path,
) -> Result<Vec<String>, SpeechError> {
    let mut arguments: Vec<String> = GLOBAL_ARGUMENTS.iter().map(|value| (*value).to_owned()).collect();
    arguments.extend(input_arguments(source, container)?);
    arguments.extend([STREAM_ARGUMENT.to_owned(), format!("0:{stream_index}")]);
    arguments.extend(OUTPUT_ARGUMENTS.iter().map(|value| (*value).to_owned()));
    arguments.push(path_text(output)?.to_owned());
    Ok(arguments)
}

fn parse_probe(bytes: &[u8], container: Container) -> Result<(String, Vec<AudioStream>), SpeechError> {
    let invalid = |message: &str| SpeechError::InvalidOutput(format!("invalid audio probe: {message}"));
    if bytes.is_empty() || bytes.len() > MAX_PROBE_BYTES { return Err(invalid("JSON byte limit")); }
    let StrictValue(value) = serde_json::from_slice::<StrictValue>(bytes)
        .map_err(|error| SpeechError::InvalidOutput(format!("invalid audio probe JSON: {error}")))?;
    let root = value.as_object().ok_or_else(|| invalid("root must be an object"))?;
    let format = root.get("format").and_then(Value::as_object)
        .and_then(|format| format.get("format_name")).and_then(Value::as_str)
        .ok_or_else(|| invalid("format_name is missing"))?;
    if format.len() > 256 || !format.split(',').any(|name| name == container.demuxer())
        || !format.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b',' | b'_'))
    { return Err(invalid("container does not match the forced demuxer")); }
    let array = root.get("streams").and_then(Value::as_array)
        .ok_or_else(|| invalid("streams must be an array"))?;
    if array.len() > MAX_STREAMS { return Err(invalid("more than 256 audio streams")); }
    let mut seen = BTreeSet::new();
    let mut streams = Vec::with_capacity(array.len());
    for value in array {
        let stream = value.as_object().ok_or_else(|| invalid("stream must be an object"))?;
        if stream.get("codec_type").and_then(Value::as_str) != Some("audio") {
            return Err(invalid("non-audio stream in audio selection"));
        }
        let index = stream.get("index").and_then(Value::as_u64).and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid("stream index must be an unsigned 32-bit integer"))?;
        if !seen.insert(index) { return Err(invalid("duplicate global stream index")); }
        let codec = stream.get("codec_name").and_then(Value::as_str)
            .ok_or_else(|| invalid("codec_name is missing"))?;
        let channels = stream.get("channels").and_then(Value::as_u64).and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid("channels must be an integer"))?;
        let sample_rate = stream.get("sample_rate").and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|value| value.parse::<u32>().ok()).ok_or_else(|| invalid("sample_rate must be an integer string"))?;
        let tags = match stream.get("tags") {
            None => None,
            Some(value) => Some(value.as_object().ok_or_else(|| invalid("tags must be an object"))?),
        };
        let parsed = AudioStream {
            index, codec_name: codec.to_owned(), channels, sample_rate,
            duration_ms: optional_seconds(stream, "duration", true)?.map(|value| value as u64),
            start_time_ms: optional_seconds(stream, "start_time", false)?,
            language: optional_tag(tags, "language")?,
            title: optional_tag(tags, "title")?,
        };
        parsed.validate()?;
        streams.push(parsed);
    }
    streams.sort_by_key(|stream| stream.index);
    Ok((format.to_owned(), streams))
}

fn optional_tag(tags: Option<&Map<String, Value>>, name: &str) -> Result<Option<String>, SpeechError> {
    match tags.and_then(|tags| tags.get(name)) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(SpeechError::InvalidOutput(format!("audio {name} tag must be a string"))),
    }
}

/// Decimal provider seconds are parsed without floats. Duration rounds up a
/// partial millisecond; reported start time truncates toward zero. Raw provider
/// JSON remains available when a caller needs the original timestamp precision.
fn optional_seconds(
    object: &Map<String, Value>, name: &str, duration: bool,
) -> Result<Option<i64>, SpeechError> {
    let Some(value) = object.get(name) else { return Ok(None); };
    let invalid = || SpeechError::InvalidOutput(format!("invalid audio {name} timestamp"));
    let value = value.as_str().ok_or_else(invalid)?;
    if value == "N/A" { return Ok(None); }
    if value.len() > 32 { return Err(invalid()); }
    let (negative, unsigned) = match value.strip_prefix('-') {
        Some(unsigned) if !duration => (true, unsigned),
        Some(_) => return Err(invalid()),
        None => (false, value),
    };
    let mut pieces = unsigned.split('.');
    let whole = pieces.next().unwrap_or("");
    let fraction = pieces.next();
    if whole.is_empty() || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || pieces.next().is_some()
        || fraction.is_some_and(|fraction| fraction.is_empty() || fraction.len() > 9
            || !fraction.bytes().all(|byte| byte.is_ascii_digit()))
    { return Err(invalid()); }
    let seconds = whole.parse::<i64>().map_err(|_| invalid())?;
    let mut milliseconds = seconds.checked_mul(1000).ok_or_else(invalid)?;
    if let Some(fraction) = fraction {
        let first = &fraction[..fraction.len().min(3)];
        let units = first.parse::<i64>().map_err(|_| invalid())?
            * 10_i64.pow((3 - first.len()) as u32);
        milliseconds = milliseconds.checked_add(units).ok_or_else(invalid)?;
        if duration && fraction.len() > 3 && fraction[3..].bytes().any(|byte| byte != b'0') {
            milliseconds = milliseconds.checked_add(1).ok_or_else(invalid)?;
        }
    }
    Ok(Some(if negative { -milliseconds } else { milliseconds }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROBE: &[u8] = br#"{"format":{"format_name":"matroska,webm"},"streams":[{"index":2,"codec_type":"audio","codec_name":"aac","channels":2,"sample_rate":"48000","start_time":"-0.021333","tags":{"language":"eng","title":"Original"}},{"index":4,"codec_type":"audio","codec_name":"opus","channels":2,"sample_rate":"48000","duration":"1.000001"}]}"#;

    #[test]
    fn inventory_keeps_global_indices_and_unknown_durations() {
        let (format, streams) = parse_probe(PROBE, Container::Matroska).unwrap();
        assert_eq!(format, "matroska,webm");
        assert_eq!(streams[0].index, 2);
        assert_eq!(streams[0].duration_ms, None);
        assert_eq!(streams[0].start_time_ms, Some(-21));
        assert_eq!(streams[1].duration_ms, Some(1001));
    }

    #[test]
    fn malformed_probe_and_ambiguous_selection_are_refused() {
        let text = std::str::from_utf8(PROBE).unwrap();
        for malformed in [
            text.replace("\"index\":4", "\"index\":2"),
            text.replace("\"index\":2", "\"index\":2,\"index\":3"),
            text.replace("\"codec_type\":\"audio\"", "\"codec_type\":\"video\""),
            text.replace("\"48000\"", "48000"),
            text.replace("\"1.000001\"", "\"-1\""),
            text.replace("\"1.000001\"", "\"1e3\""),
            text.replace("\"channels\":2", "\"channels\":0"),
        ] { assert!(parse_probe(malformed.as_bytes(), Container::Matroska).is_err()); }
        assert!(parse_probe(PROBE, Container::Mov).is_err());
        assert!(Container::from_path(Path::new("playlist.m3u8")).is_err());
    }

    #[test]
    fn extraction_maps_only_explicit_global_stream_and_guards_decoded_duration() {
        let directory = std::env::temp_dir();
        let arguments = extraction_arguments(
            &directory.join("source video.mp4"), Container::Mov, 4, &directory.join("prepared.wav"),
        ).unwrap();
        for pair in [["-map", "0:4"], ["-t", "601"], ["-protocol_whitelist", "file"],
            ["-enable_drefs", "0"], ["-use_absolute_path", "0"], ["-f", "mov"],
            ["-codec:a", "pcm_s16le"], ["-filter:a", "aresample=16000:async=0,asetpts=N/SR/TB"]]
        {
            assert!(arguments.windows(2).any(|actual| actual[0] == pair[0] && actual[1] == pair[1]));
        }
        for forbidden in ["-shortest", "-ss", "-fs", "-copyts", "-y"] {
            assert!(!arguments.iter().any(|value| value == forbidden));
        }
        assert!(arguments.iter().any(|value| value == "-xerror"));
    }

    #[test]
    fn precancellation_precedes_media_tool_access() {
        let cancellation = ProcessCancellationToken::new();
        cancellation.cancel();
        let request = AudioInspectRequest {
            input: PathBuf::from("absent.mp4"),
            ffprobe: AudioTool { path: PathBuf::from("absent-ffprobe"), sha256: "a".repeat(64) },
        };
        assert!(matches!(inspect(&request, cancellation), Err(SpeechError::Cancelled)));
    }
}
