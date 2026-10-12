//! Provider-neutral speech results and a bounded whisper.cpp JSON adapter.
//!
//! Speech evidence lives in a separate envelope around the unchanged timed-text
//! contract. Recognition is an unreviewed draft: provider segments supply the
//! timestamps, token probabilities are not accuracy estimates, and this module
//! neither invents missing words nor performs alignment or diarization.

pub mod audio;
pub mod graph;
pub mod policy;
pub mod provider;

use std::collections::BTreeSet;
use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::timed_text::{self, TimedCue, TimedTranscript, TranscriptSource, MAX_CUES, MAX_INPUT_BYTES};

pub const SPEECH_TRANSCRIPTION_SCHEMA_V1: &str = "renderflow.speech-transcription/v1";
pub const WHISPER_PROVIDER_ID: &str = "adapter.speech.whisper_cpp";
pub const MAX_PROVIDER_JSON_BYTES: usize = MAX_INPUT_BYTES;
pub const AUDIO_INVENTORY_SCHEMA_V1: &str = "renderflow.audio-inventory/v1";
pub const AUDIO_PREPARATION_PROFILE: &str = "mono_16000_hz_pcm_s16le";
pub const AUDIO_TIMESTAMP_ORIGIN: &str = "selected_stream_decoded_samples_from_zero";
pub const MAX_AUDIO_SOURCE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_PREPARED_WAV_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_PREPARED_AUDIO_DURATION_MS: u64 = 600_000;
// Shared with the native argv builder so changes to fixed decoding options also
// change configuration identity. The profile covers PCM input/full-JSON output,
// translation/VAD remaining disabled, and the bounded process environment.
pub(crate) const WHISPER_FIXED_ARGUMENTS: &[&str] = &[
    "--processors", "1", "--temperature", "0", "--temperature-inc", "0",
    "--beam-size", "1", "--best-of", "1", "--no-fallback", "--no-gpu",
    "--no-flash-attn", "--no-prints",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechOptions {
    /// `auto` or a lowercase two/three-letter provider language code.
    pub language: String,
    pub threads: u32,
    pub timeout_seconds: u64,
}

impl Default for SpeechOptions {
    fn default() -> Self {
        Self { language: "en".to_owned(), threads: 4, timeout_seconds: 900 }
    }
}

impl SpeechOptions {
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.language != "auto" && !is_language(&self.language) {
            return Err(SpeechError::InvalidRequest("language must be auto or a lowercase two/three-letter code".to_owned()));
        }
        if !(1..=64).contains(&self.threads) {
            return Err(SpeechError::InvalidRequest("threads must be between 1 and 64".to_owned()));
        }
        if !(1..=3600).contains(&self.timeout_seconds) {
            return Err(SpeechError::InvalidRequest("timeout_seconds must be between 1 and 3600".to_owned()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechIdentity {
    pub provider_id: String,
    pub provider_version: String,
    pub provider_sha256: String,
    pub model_id: String,
    pub model_sha256: String,
    pub source_sha256: String,
}

impl SpeechIdentity {
    pub fn validate(&self) -> Result<(), SpeechError> {
        for (name, value, maximum) in [
            ("provider_id", self.provider_id.as_str(), 256),
            ("provider_version", self.provider_version.as_str(), 4096),
            ("model_id", self.model_id.as_str(), 256),
        ] {
            if !is_label(value, maximum) {
                return Err(SpeechError::InvalidRequest(format!("{name} must be nonempty, bounded, and contain no controls")));
            }
        }
        for (name, value) in [
            ("provider_sha256", &self.provider_sha256),
            ("model_sha256", &self.model_sha256),
            ("source_sha256", &self.source_sha256),
        ] {
            if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
                return Err(SpeechError::InvalidRequest(format!("{name} must be a lowercase SHA-256 digest")));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechTimestampPolicy {
    ProviderSegmentMilliseconds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechTextNormalization {
    TrimSegmentEdges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechConfidence {
    Unavailable,
}

/// Identity of the exact local binary used to inspect or decode the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioToolIdentity {
    pub tool_id: String,
    pub version: String,
    pub sha256: String,
}

impl AudioToolIdentity {
    pub fn validate(&self, expected_id: &str) -> Result<(), SpeechError> {
        if self.tool_id != expected_id || !is_label(&self.version, 4096) || !is_sha256(&self.sha256) {
            return Err(invalid("audio tool identity must name the expected tool, a bounded version, and its SHA-256"));
        }
        Ok(())
    }
}

/// Audio stream metadata from ffprobe. Index is global, not audio-relative.
/// Missing timing stays unknown; no container duration is substituted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioStream {
    pub index: u32,
    pub codec_name: String,
    pub channels: u32,
    pub sample_rate: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl AudioStream {
    pub fn validate(&self) -> Result<(), SpeechError> {
        if !is_label(&self.codec_name, 256) || !(1..=64).contains(&self.channels)
            || !(1..=768_000).contains(&self.sample_rate)
        {
            return Err(invalid("audio stream codec, channel count, or sample rate is outside this profile"));
        }
        if self.language.as_ref().is_some_and(|value| !is_label(value, 64))
            || self.title.as_ref().is_some_and(|value| !is_label(value, 1024))
        {
            return Err(invalid("audio stream language/title must be bounded labels without controls"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInventory {
    pub schema_version: String,
    pub source_sha256: String,
    pub source_size_bytes: u64,
    pub container_format: String,
    pub demuxer: String,
    pub ffprobe: AudioToolIdentity,
    pub probe_output_sha256: String,
    pub streams: Vec<AudioStream>,
}

impl AudioInventory {
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.schema_version != AUDIO_INVENTORY_SCHEMA_V1
            || !is_sha256(&self.source_sha256) || !is_sha256(&self.probe_output_sha256)
            || !(1..=MAX_AUDIO_SOURCE_BYTES).contains(&self.source_size_bytes)
            || !is_label(&self.container_format, 256) || !is_audio_demuxer(&self.demuxer)
            || self.streams.len() > 256
        {
            return Err(invalid("audio inventory identity, format, size, or stream count is invalid"));
        }
        self.ffprobe.validate("tool.ffprobe")?;
        let mut indexes = BTreeSet::new();
        for stream in &self.streams {
            stream.validate()?;
            if !indexes.insert(stream.index) {
                return Err(invalid("audio inventory contains duplicate global stream indexes"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPreparationLimits {
    pub max_source_bytes: u64,
    pub max_duration_ms: u64,
    pub max_prepared_wav_bytes: u64,
}

impl Default for AudioPreparationLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: MAX_AUDIO_SOURCE_BYTES,
            max_duration_ms: MAX_PREPARED_AUDIO_DURATION_MS,
            max_prepared_wav_bytes: MAX_PREPARED_WAV_BYTES,
        }
    }
}

/// Provenance of bounded, explicitly selected container audio preparation.
/// The transcript still addresses the prepared WAV, starting at sample zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPreparationEvidence {
    pub source_sha256: String,
    pub source_size_bytes: u64,
    pub container_format: String,
    pub demuxer: String,
    pub selected_stream: AudioStream,
    pub ffprobe: AudioToolIdentity,
    pub ffmpeg: AudioToolIdentity,
    pub probe_output_sha256: String,
    pub prepared_wav_sha256: String,
    pub prepared_wav_size_bytes: u64,
    pub prepared_wav_duration_ms: u64,
    pub profile: String,
    pub configuration_sha256: String,
    pub timestamp_origin: String,
    pub timeout_seconds: u64,
    pub limits: AudioPreparationLimits,
    pub limitations: Vec<String>,
}

impl AudioPreparationEvidence {
    pub fn validate(&self) -> Result<(), SpeechError> {
        for value in [&self.source_sha256, &self.probe_output_sha256, &self.prepared_wav_sha256, &self.configuration_sha256] {
            if !is_sha256(value) {
                return Err(invalid("audio preparation digests must be lowercase SHA-256 values"));
            }
        }
        if !(1..=MAX_AUDIO_SOURCE_BYTES).contains(&self.source_size_bytes)
            || !(46..=MAX_PREPARED_WAV_BYTES).contains(&self.prepared_wav_size_bytes)
            || !(1..=MAX_PREPARED_AUDIO_DURATION_MS).contains(&self.prepared_wav_duration_ms)
            || !is_label(&self.container_format, 256) || !is_audio_demuxer(&self.demuxer)
            || self.profile != AUDIO_PREPARATION_PROFILE || self.timestamp_origin != AUDIO_TIMESTAMP_ORIGIN
            || !(1..=3600).contains(&self.timeout_seconds) || self.limits != AudioPreparationLimits::default()
        {
            return Err(invalid("audio preparation must match the bounded PCM profile and declared limits"));
        }
        self.ffprobe.validate("tool.ffprobe")?;
        self.ffmpeg.validate("tool.ffmpeg")?;
        self.selected_stream.validate()?;
        if self.selected_stream.duration_ms.is_some_and(|duration| duration == 0 || duration > MAX_PREPARED_AUDIO_DURATION_MS) {
            return Err(invalid("known selected audio duration must be nonzero and at most ten minutes"));
        }
        let unique_limitations: BTreeSet<_> = self.limitations.iter().collect();
        if self.limitations.is_empty() || self.limitations.len() > 16
            || unique_limitations.len() != self.limitations.len()
            || self.limitations.iter().any(|value| !is_label(value, 4096))
        {
            return Err(invalid("audio preparation limitations must be nonempty, unique, bounded labels"));
        }
        if self.configuration_sha256 != audio_preparation_configuration_digest(self)? {
            return Err(invalid("audio preparation configuration digest does not match its policy and identities"));
        }
        Ok(())
    }
}

/// Hash the versioned extraction policy and actual pinned tool/selection inputs.
/// Original bytes and probe bytes belong to the enclosing reuse identity.
pub fn audio_preparation_configuration_digest(evidence: &AudioPreparationEvidence) -> Result<String, SpeechError> {
    json_digest(&(
        "renderflow.audio-preparation-configuration/v1",
        audio::audio_fixed_arguments(),
        &evidence.profile,
        &evidence.timestamp_origin,
        &evidence.demuxer,
        &evidence.selected_stream,
        &evidence.ffprobe,
        &evidence.ffmpeg,
        evidence.timeout_seconds,
        &evidence.limits,
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechEvidence {
    pub identity: SpeechIdentity,
    pub options: SpeechOptions,
    pub provider_output_sha256: String,
    /// Stable identity of the adapter policy, provider, model, and options.
    pub configuration_sha256: String,
    /// Includes source identity and configuration; this does not enable caching.
    pub reuse_key: String,
    /// The provider's model path; the runtime adapter must verify its argv match.
    pub provider_reported_model: String,
    pub provider_reported_language: String,
    pub requested_language: String,
    pub detected_language: String,
    pub timestamp_policy: SpeechTimestampPolicy,
    pub text_normalization: SpeechTextNormalization,
    pub trimmed_segment_count: usize,
    pub confidence: SpeechConfidence,
    pub human_reviewed: bool,
    pub limitations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_preparation: Option<AudioPreparationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechTranscription {
    pub schema_version: String,
    pub transcript: TimedTranscript,
    pub evidence: SpeechEvidence,
}

#[derive(Debug, Error)]
pub enum SpeechError {
    #[error("invalid speech request: {0}")]
    InvalidRequest(String),
    #[error("invalid speech provider output: {0}")]
    InvalidOutput(String),
    #[error("speech provider unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("speech provider failed: {0}")]
    ProviderFailed(String),
    #[error("speech input changed during execution: {0}")]
    SourceChanged(String),
    #[error("speech operation was cancelled")]
    Cancelled,
    #[error("speech operation exceeded its time limit")]
    Timeout,
    #[error("speech operation I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

impl SpeechError {
    /// Stable diagnostic suffix used by graph execution evidence.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::InvalidOutput(_) => "invalid_output",
            Self::ProviderUnavailable(_) => "provider_unavailable",
            Self::ProviderFailed(_) => "provider_failed",
            Self::SourceChanged(_) => "source_changed",
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::Io(_) => "io_failed",
        }
    }
}

/// Convert whisper.cpp full JSON into unreviewed, plain-text segment cues.
///
/// Offsets are already integer milliseconds in the provider format; formatted
/// timestamp strings and token probabilities are deliberately not substituted.
/// Extra provider fields are tolerated within the total input/depth bounds, but
/// duplicate keys anywhere, translation, empty segments, and unsupported plain
/// text are refused. Unicode whitespace is trimmed only at each segment's edges,
/// after rejecting unsupported controls; that normalization is explicit evidence.
pub fn normalize_whisper_json(
    bytes: &[u8],
    identity: &SpeechIdentity,
    options: &SpeechOptions,
) -> Result<SpeechTranscription, SpeechError> {
    identity.validate()?;
    options.validate()?;
    if bytes.len() > MAX_PROVIDER_JSON_BYTES {
        return Err(invalid("provider JSON exceeds the 16 MiB limit"));
    }
    let StrictValue(value) = serde_json::from_slice::<StrictValue>(bytes)
        .map_err(|error| invalid(format!("malformed or duplicate-key JSON: {error}")))?;
    let root = object(&value, "root")?;
    let params = object(field(root, "params")?, "params")?;
    let result = object(field(root, "result")?, "result")?;
    if field(params, "translate")?.as_bool() != Some(false) {
        return Err(invalid("params.translate must be false; this profile transcribes without translation"));
    }
    let provider_model = string(params, "model")?;
    if !is_label(provider_model, 4096) {
        return Err(invalid("params.model must be a nonempty, bounded model path without controls"));
    }
    let reported_language = string(params, "language")?;
    let detected_language = string(result, "language")?;
    if !is_language(detected_language) {
        return Err(invalid("result.language must identify a lowercase two/three-letter language"));
    }
    if options.language == "auto" {
        if reported_language != "auto" && reported_language != detected_language {
            return Err(invalid("auto-detection params.language must be auto or match result.language"));
        }
    } else if reported_language != options.language || detected_language != options.language {
        return Err(invalid("provider requested and detected languages must match the explicit language request"));
    }
    let segments = field(root, "transcription")?.as_array()
        .ok_or_else(|| invalid("transcription must be an array"))?;
    if segments.is_empty() || segments.len() > MAX_CUES {
        return Err(invalid("transcription must contain between 1 and 100000 segments"));
    }
    let mut cues = Vec::with_capacity(segments.len());
    let mut trimmed_segment_count = 0;
    for (index, segment) in segments.iter().enumerate() {
        let segment = object(segment, "transcription segment")?;
        let offsets = object(field(segment, "offsets")?, "segment offsets")?;
        let start_ms = field(offsets, "from")?.as_u64()
            .ok_or_else(|| invalid("segment offsets.from must be a nonnegative integer millisecond offset"))?;
        let end_ms = field(offsets, "to")?.as_u64()
            .ok_or_else(|| invalid("segment offsets.to must be a nonnegative integer millisecond offset"))?;
        let text = string(segment, "text")?;
        if text.chars().any(|character| character.is_control() && character != '\n' && character != '\t') {
            return Err(invalid("segment text contains an unsupported control character"));
        }
        let normalized = text.trim();
        if normalized != text {
            trimmed_segment_count += 1;
        }
        cues.push(TimedCue {
            id: format!("speech-{:06}", index + 1),
            start_ms,
            end_ms,
            text: normalized.to_owned(),
            speaker: None,
        });
    }
    let mut transcript = TimedTranscript::new(TranscriptSource {
        sha256: identity.source_sha256.clone(),
        format: "wav".to_owned(),
        stream: None,
        extraction: None,
    }, cues);
    transcript.language = Some(detected_language.to_owned());
    timed_text::validate(&transcript).map_err(|error| invalid(error.to_string()))?;

    let configuration_sha256 = configuration_digest(identity, options)?;
    let reuse_key = json_digest(&(
        "renderflow.speech-reuse/v1",
        &identity.source_sha256,
        &configuration_sha256,
    ))?;
    let transcription = SpeechTranscription {
        schema_version: SPEECH_TRANSCRIPTION_SCHEMA_V1.to_owned(),
        transcript,
        evidence: SpeechEvidence {
            identity: identity.clone(),
            options: options.clone(),
            provider_output_sha256: format!("{:x}", Sha256::digest(bytes)),
            configuration_sha256,
            reuse_key,
            provider_reported_model: provider_model.to_owned(),
            provider_reported_language: reported_language.to_owned(),
            requested_language: options.language.clone(),
            detected_language: detected_language.to_owned(),
            timestamp_policy: SpeechTimestampPolicy::ProviderSegmentMilliseconds,
            text_normalization: SpeechTextNormalization::TrimSegmentEdges,
            trimmed_segment_count,
            confidence: SpeechConfidence::Unavailable,
            human_reviewed: false,
            limitations: vec![
                "Unreviewed recognition draft; words and segment timing require human review.".to_owned(),
                "No calibrated confidence, word alignment, diarization, translation, or singing-accuracy guarantee.".to_owned(),
                "Provider metadata beyond language/model and segment text/offsets is not represented in the transcript.".to_owned(),
                "Reuse identity is recorded only; no speech cache lookup or reuse is enabled.".to_owned(),
            ],
            audio_preparation: None,
        },
    };
    if serde_json::to_vec(&transcription).map_err(|error| invalid(error.to_string()))?.len() > MAX_INPUT_BYTES {
        return Err(invalid("normalized speech envelope exceeds the 16 MiB limit"));
    }
    Ok(transcription)
}

/// Bind prepared audio provenance to an already normalized recognition result.
/// This preserves the WAV-relative timed transcript and does not enable caching
/// or reinterpret cues against the original media clock. Reattaching identical
/// evidence is idempotent; replacing evidence is refused.
pub fn attach_audio_preparation(
    transcription: &mut SpeechTranscription,
    preparation: &AudioPreparationEvidence,
) -> Result<(), SpeechError> {
    preparation.validate()?;
    transcription.evidence.identity.validate()?;
    transcription.evidence.options.validate()?;
    timed_text::validate(&transcription.transcript).map_err(|error| invalid(error.to_string()))?;
    if transcription.schema_version != SPEECH_TRANSCRIPTION_SCHEMA_V1
        || transcription.transcript.source.format != "wav"
        || transcription.transcript.source.stream.is_some()
        || transcription.transcript.source.extraction.is_some()
        || transcription.evidence.human_reviewed
        || transcription.evidence.identity.source_sha256 != preparation.prepared_wav_sha256
        || transcription.transcript.source.sha256 != preparation.prepared_wav_sha256
        || transcription.transcript.cues.iter().any(|cue| cue.end_ms > preparation.prepared_wav_duration_ms)
    {
        return Err(invalid("audio preparation must describe the same unreviewed WAV transcript and cover every cue"));
    }
    let speech_configuration = configuration_digest(&transcription.evidence.identity, &transcription.evidence.options)?;
    let combined_configuration = json_digest(&(
        "renderflow.prepared-speech-configuration/v1",
        &speech_configuration,
        &preparation.configuration_sha256,
    ))?;
    let combined_reuse_key = json_digest(&(
        "renderflow.prepared-speech-reuse/v1",
        &preparation.source_sha256,
        preparation.source_size_bytes,
        &preparation.container_format,
        &preparation.selected_stream,
        &preparation.probe_output_sha256,
        &preparation.prepared_wav_sha256,
        &combined_configuration,
    ))?;
    match &transcription.evidence.audio_preparation {
        Some(existing) => {
            if existing != preparation || transcription.evidence.configuration_sha256 != combined_configuration
                || transcription.evidence.reuse_key != combined_reuse_key
            {
                return Err(invalid("existing audio preparation evidence or identity cannot be replaced"));
            }
            return Ok(());
        }
        None => {
            let expected_reuse_key = json_digest(&(
                "renderflow.speech-reuse/v1",
                &transcription.evidence.identity.source_sha256,
                &speech_configuration,
            ))?;
            if transcription.evidence.configuration_sha256 != speech_configuration
                || transcription.evidence.reuse_key != expected_reuse_key
            {
                return Err(invalid("speech identity must match the normalized result before attaching preparation"));
            }
        }
    }
    // Check the complete prospective envelope before changing the caller's value.
    let mut enriched = transcription.clone();
    enriched.evidence.configuration_sha256 = combined_configuration;
    enriched.evidence.reuse_key = combined_reuse_key;
    enriched.evidence.audio_preparation = Some(preparation.clone());
    if serde_json::to_vec(&enriched).map_err(|error| invalid(error.to_string()))?.len() > MAX_INPUT_BYTES {
        return Err(invalid("normalized speech envelope exceeds the 16 MiB limit"));
    }
    *transcription = enriched;
    Ok(())
}

fn configuration_digest(identity: &SpeechIdentity, options: &SpeechOptions) -> Result<String, SpeechError> {
    // A tuple has a stable serialization order. The domain separates policy
    // revisions from any other digest; source identity belongs in reuse_key.
    json_digest(&(
        "renderflow.speech-configuration/v1",
        "whisper-full-json-segments/v1",
        "whisper-cli-pcm16-mono-16000/full-json/no-translate/no-vad/cleared-environment/v1",
        WHISPER_FIXED_ARGUMENTS,
        &identity.provider_id,
        &identity.provider_version,
        &identity.provider_sha256,
        &identity.model_id,
        &identity.model_sha256,
        options,
        SpeechTimestampPolicy::ProviderSegmentMilliseconds,
        SpeechTextNormalization::TrimSegmentEdges,
    ))
}

fn json_digest(value: &impl Serialize) -> Result<String, SpeechError> {
    let bytes = serde_json::to_vec(value).map_err(|error| invalid(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn invalid(message: impl Into<String>) -> SpeechError {
    SpeechError::InvalidOutput(message.into())
}

fn is_language(value: &str) -> bool {
    (2..=3).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn is_label(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_audio_demuxer(value: &str) -> bool {
    matches!(value, "mov" | "matroska" | "avi" | "wav" | "flac" | "mp3" | "ogg" | "aac")
}

fn object<'a>(value: &'a Value, name: &str) -> Result<&'a Map<String, Value>, SpeechError> {
    value.as_object().ok_or_else(|| invalid(format!("{name} must be an object")))
}

fn field<'a>(value: &'a Map<String, Value>, name: &str) -> Result<&'a Value, SpeechError> {
    value.get(name).ok_or_else(|| invalid(format!("missing required field: {name}")))
}

fn string<'a>(value: &'a Map<String, Value>, name: &str) -> Result<&'a str, SpeechError> {
    field(value, name)?.as_str().ok_or_else(|| invalid(format!("{name} must be a string")))
}

/// serde_json::Value alone silently overwrites duplicate keys. Inspect every
/// map, including ignored extension metadata, while retaining JSON's ordinary
/// recursion limit and the caller's byte cap.
struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictValue;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON value without duplicate object keys")
            }

            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Bool(value)))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Number(value.into())))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Number(value.into())))
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value).map(|number| StrictValue(Value::Number(number)))
                    .ok_or_else(|| E::custom("nonfinite JSON number"))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::String(value.to_owned())))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::String(value)))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Null))
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Null))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(StrictValue(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(StrictValue(Value::Array(values)))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate JSON object key"));
                    }
                    let StrictValue(value) = map.next_value()?;
                    values.insert(key, value);
                }
                Ok(StrictValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(StrictVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../../../tests/fixtures/speech/whisper-segments.json");

    fn identity() -> SpeechIdentity {
        SpeechIdentity {
            provider_id: WHISPER_PROVIDER_ID.to_owned(),
            provider_version: "whisper.cpp synthetic fixture".to_owned(),
            provider_sha256: "1".repeat(64),
            model_id: "synthetic".to_owned(),
            model_sha256: "2".repeat(64),
            source_sha256: "3".repeat(64),
        }
    }

    fn options() -> SpeechOptions {
        SpeechOptions { language: "en".to_owned(), threads: 2, timeout_seconds: 60 }
    }

    fn changed(mut change: impl FnMut(&mut Value)) -> Result<SpeechTranscription, SpeechError> {
        let mut value: Value = serde_json::from_slice(FIXTURE).unwrap();
        change(&mut value);
        normalize_whisper_json(&serde_json::to_vec(&value).unwrap(), &identity(), &options())
    }

    #[test]
    fn normalizes_segment_milliseconds_and_records_unreviewed_evidence() {
        let result = normalize_whisper_json(FIXTURE, &identity(), &options()).unwrap();
        assert_eq!(result.transcript.cues[0].id, "speech-000001");
        assert_eq!(result.transcript.cues[0].text, "Hello, world.");
        assert_eq!(result.transcript.cues[1].end_ms, 2500);
        assert_eq!(result.transcript.source.format, "wav");
        assert_eq!(result.transcript.source.sha256, identity().source_sha256);
        assert_eq!(result.evidence.trimmed_segment_count, 1);
        assert_eq!(result.evidence.confidence, SpeechConfidence::Unavailable);
        assert!(!result.evidence.human_reviewed);
        assert!(result.transcript.cues.iter().all(|cue| cue.speaker.is_none()));
        assert_eq!(result.evidence.provider_output_sha256, format!("{:x}", Sha256::digest(FIXTURE)));
        assert_eq!(result, normalize_whisper_json(FIXTURE, &identity(), &options()).unwrap());
    }

    #[test]
    fn reuse_identity_changes_with_source_provider_model_and_options() {
        let baseline = normalize_whisper_json(FIXTURE, &identity(), &options()).unwrap();
        let mut source = identity();
        source.source_sha256 = "4".repeat(64);
        let changed_source = normalize_whisper_json(FIXTURE, &source, &options()).unwrap();
        assert_eq!(baseline.evidence.configuration_sha256, changed_source.evidence.configuration_sha256);
        assert_ne!(baseline.evidence.reuse_key, changed_source.evidence.reuse_key);
        for changed_identity in [
            SpeechIdentity { provider_sha256: "5".repeat(64), ..identity() },
            SpeechIdentity { model_sha256: "6".repeat(64), ..identity() },
            SpeechIdentity { provider_version: "another build".to_owned(), ..identity() },
        ] {
            let result = normalize_whisper_json(FIXTURE, &changed_identity, &options()).unwrap();
            assert_ne!(baseline.evidence.configuration_sha256, result.evidence.configuration_sha256);
            assert_ne!(baseline.evidence.reuse_key, result.evidence.reuse_key);
        }
        let result = normalize_whisper_json(FIXTURE, &identity(), &SpeechOptions { threads: 3, ..options() }).unwrap();
        assert_ne!(baseline.evidence.reuse_key, result.evidence.reuse_key);
    }

    #[test]
    fn refuses_language_rewrites_and_translation_but_preserves_auto_detection() {
        assert!(changed(|value| value["params"]["translate"] = true.into()).is_err());
        assert!(changed(|value| value["params"]["language"] = "ru".into()).is_err());
        assert!(changed(|value| value["result"]["language"] = "ru".into()).is_err());
        let mut value: Value = serde_json::from_slice(FIXTURE).unwrap();
        let auto = SpeechOptions { language: "auto".to_owned(), ..options() };
        for reported in ["auto", "en"] {
            value["params"]["language"] = reported.into();
            let result = normalize_whisper_json(&serde_json::to_vec(&value).unwrap(), &identity(), &auto).unwrap();
            assert_eq!(result.evidence.requested_language, "auto");
            assert_eq!(result.evidence.detected_language, "en");
            assert_eq!(result.evidence.provider_reported_language, reported);
        }
    }

    #[test]
    fn refuses_duplicate_fields_even_in_ignored_metadata_and_positional_records() {
        let fixture = std::str::from_utf8(FIXTURE).unwrap();
        for duplicate in [
            fixture.replacen("\"language\": \"en\"", "\"language\": \"en\", \"language\": \"en\"", 1),
            fixture.replacen("\"p\": 0.42", "\"p\": 0.42, \"p\": 0.99", 1),
        ] {
            assert!(normalize_whisper_json(duplicate.as_bytes(), &identity(), &options()).is_err());
        }
        assert!(changed(|value| value["params"] = serde_json::json!(["model", "en", false])).is_err());
        assert!(changed(|value| value["transcription"][0]["offsets"] = serde_json::json!([0, 1000])).is_err());
    }

    #[test]
    fn refuses_missing_empty_invalid_or_unsupported_segments_without_dropping_them() {
        assert!(changed(|value| value["transcription"] = serde_json::json!([])).is_err());
        for text in ["", " \t ", "<b>styled</b>", "one & two", "\u{000b}hidden", "a\n\nb"] {
            assert!(changed(|value| value["transcription"][0]["text"] = text.into()).is_err());
        }
        for end in [serde_json::json!(0), serde_json::json!(-1), serde_json::json!(1.5), serde_json::json!(360000000)] {
            assert!(changed(|value| value["transcription"][0]["offsets"]["to"] = end.clone()).is_err());
        }
        assert!(changed(|value| value["transcription"][1]["offsets"]["from"] = serde_json::json!(0)).is_ok());
        assert!(changed(|value| {
            value["transcription"][0]["offsets"]["from"] = serde_json::json!(500);
            value["transcription"][1]["offsets"]["from"] = serde_json::json!(0);
        }).is_err());
    }

    #[test]
    fn bounds_requests_identity_and_provider_input_before_normalizing() {
        for options in [
            SpeechOptions { threads: 0, ..options() },
            SpeechOptions { threads: 65, ..options() },
            SpeechOptions { timeout_seconds: 0, ..options() },
            SpeechOptions { timeout_seconds: 3601, ..options() },
            SpeechOptions { language: "EN".to_owned(), ..options() },
        ] {
            assert!(matches!(normalize_whisper_json(FIXTURE, &identity(), &options), Err(SpeechError::InvalidRequest(_))));
        }
        let invalid_identity = SpeechIdentity { model_sha256: "not-a-digest".to_owned(), ..identity() };
        assert!(matches!(normalize_whisper_json(FIXTURE, &invalid_identity, &options()), Err(SpeechError::InvalidRequest(_))));
        assert!(normalize_whisper_json(&vec![b' '; MAX_PROVIDER_JSON_BYTES + 1], &identity(), &options()).is_err());
    }

    fn preparation() -> AudioPreparationEvidence {
        let mut evidence = AudioPreparationEvidence {
            source_sha256: "4".repeat(64),
            source_size_bytes: 100_000,
            container_format: "matroska,webm".to_owned(),
            demuxer: "matroska".to_owned(),
            selected_stream: AudioStream {
                index: 2,
                codec_name: "opus".to_owned(),
                channels: 2,
                sample_rate: 48_000,
                duration_ms: None,
                start_time_ms: Some(-7),
                language: Some("eng".to_owned()),
                title: Some("Synthetic narration".to_owned()),
            },
            ffprobe: AudioToolIdentity {
                tool_id: "tool.ffprobe".to_owned(),
                version: "ffprobe synthetic fixture".to_owned(),
                sha256: "5".repeat(64),
            },
            ffmpeg: AudioToolIdentity {
                tool_id: "tool.ffmpeg".to_owned(),
                version: "ffmpeg synthetic fixture".to_owned(),
                sha256: "6".repeat(64),
            },
            probe_output_sha256: "7".repeat(64),
            prepared_wav_sha256: identity().source_sha256,
            prepared_wav_size_bytes: 96_044,
            prepared_wav_duration_ms: 3000,
            profile: AUDIO_PREPARATION_PROFILE.to_owned(),
            configuration_sha256: String::new(),
            timestamp_origin: AUDIO_TIMESTAMP_ORIGIN.to_owned(),
            timeout_seconds: 300,
            limits: AudioPreparationLimits::default(),
            limitations: vec!["Synthetic metadata only; no native decoding or accuracy claim.".to_owned()],
        };
        evidence.configuration_sha256 = audio_preparation_configuration_digest(&evidence).unwrap();
        evidence
    }

    #[test]
    fn optional_audio_preparation_preserves_wav_wire_and_attaches_idempotently() {
        let mut result = normalize_whisper_json(FIXTURE, &identity(), &options()).unwrap();
        let original_transcript = result.transcript.clone();
        let before = serde_json::to_value(&result).unwrap();
        assert!(before["evidence"].get("audio_preparation").is_none());
        let old_wire: SpeechTranscription = serde_json::from_value(before).unwrap();
        assert_eq!(old_wire.evidence.audio_preparation, None);
        let evidence = preparation();
        attach_audio_preparation(&mut result, &evidence).unwrap();
        assert_eq!(result.transcript, original_transcript);
        assert_eq!(result.evidence.audio_preparation, Some(evidence.clone()));
        assert_ne!(result.evidence.configuration_sha256, old_wire.evidence.configuration_sha256);
        assert_ne!(result.evidence.reuse_key, old_wire.evidence.reuse_key);
        let once = result.clone();
        attach_audio_preparation(&mut result, &evidence).unwrap();
        assert_eq!(result, once);
    }

    #[test]
    fn preparation_reuse_identity_tracks_original_media_stream_and_decoding_policy() {
        let recognize = || normalize_whisper_json(FIXTURE, &identity(), &options()).unwrap();
        let baseline_evidence = preparation();
        let mut baseline = recognize();
        attach_audio_preparation(&mut baseline, &baseline_evidence).unwrap();
        let mut other_source = baseline_evidence.clone();
        other_source.source_sha256 = "8".repeat(64);
        let mut source_result = recognize();
        attach_audio_preparation(&mut source_result, &other_source).unwrap();
        assert_eq!(baseline.evidence.configuration_sha256, source_result.evidence.configuration_sha256);
        assert_ne!(baseline.evidence.reuse_key, source_result.evidence.reuse_key);
        for mut changed in [
            AudioPreparationEvidence { selected_stream: AudioStream { index: 3, ..baseline_evidence.selected_stream.clone() }, ..baseline_evidence.clone() },
            AudioPreparationEvidence { timeout_seconds: 301, ..baseline_evidence.clone() },
            AudioPreparationEvidence { ffmpeg: AudioToolIdentity { sha256: "9".repeat(64), ..baseline_evidence.ffmpeg.clone() }, ..baseline_evidence.clone() },
        ] {
            changed.configuration_sha256 = audio_preparation_configuration_digest(&changed).unwrap();
            let mut result = recognize();
            attach_audio_preparation(&mut result, &changed).unwrap();
            assert_ne!(baseline.evidence.configuration_sha256, result.evidence.configuration_sha256);
            assert_ne!(baseline.evidence.reuse_key, result.evidence.reuse_key);
        }
    }

    #[test]
    fn preparation_refusals_leave_the_transcript_unchanged() {
        let baseline = normalize_whisper_json(FIXTURE, &identity(), &options()).unwrap();
        for mut evidence in [
            AudioPreparationEvidence { prepared_wav_sha256: "a".repeat(64), ..preparation() },
            AudioPreparationEvidence { prepared_wav_duration_ms: 2499, ..preparation() },
            AudioPreparationEvidence { timestamp_origin: "original_video_clock".to_owned(), ..preparation() },
            AudioPreparationEvidence { selected_stream: AudioStream { duration_ms: Some(600_001), ..preparation().selected_stream }, ..preparation() },
        ] {
            evidence.configuration_sha256 = audio_preparation_configuration_digest(&evidence).unwrap();
            let mut result = baseline.clone();
            assert!(attach_audio_preparation(&mut result, &evidence).is_err());
            assert_eq!(result, baseline);
        }
        let mut changed = preparation();
        changed.timeout_seconds += 1;
        assert!(changed.validate().is_err());
    }

    #[test]
    fn audio_inventory_preserves_unknown_timing_and_refuses_ambiguous_indexes() {
        let preparation = preparation();
        let mut inventory = AudioInventory {
            schema_version: AUDIO_INVENTORY_SCHEMA_V1.to_owned(),
            source_sha256: preparation.source_sha256,
            source_size_bytes: preparation.source_size_bytes,
            container_format: preparation.container_format,
            demuxer: preparation.demuxer,
            ffprobe: preparation.ffprobe,
            probe_output_sha256: preparation.probe_output_sha256,
            streams: vec![preparation.selected_stream],
        };
        inventory.validate().unwrap();
        let encoded = serde_json::to_value(&inventory).unwrap();
        assert!(encoded["streams"][0].get("duration_ms").is_none());
        assert_eq!(encoded["streams"][0]["start_time_ms"], -7);
        inventory.streams.push(inventory.streams[0].clone());
        assert!(inventory.validate().is_err());
        inventory.streams.clear();
        inventory.validate().unwrap();
    }
}
