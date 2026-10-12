//! Provider-neutral speech results and a bounded whisper.cpp JSON adapter.
//!
//! Speech evidence lives in a separate envelope around the unchanged timed-text
//! contract. Recognition is an unreviewed draft: provider segments supply the
//! timestamps, token probabilities are not accuracy estimates, and this module
//! neither invents missing words nor performs alignment or diarization.

pub mod provider;

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
        },
    };
    if serde_json::to_vec(&transcription).map_err(|error| invalid(error.to_string()))?.len() > MAX_INPUT_BYTES {
        return Err(invalid("normalized speech envelope exceeds the 16 MiB limit"));
    }
    Ok(transcription)
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
}
