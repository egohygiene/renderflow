//! Explicit local speech intent for the render-spec planner.
//!
//! Validation is structural. It never opens a model, probes a binary, selects
//! a stream, or treats a supplied digest as observed runtime evidence.

use std::marker::PhantomData;
use std::path::{Component, Path};

use serde::{Deserialize, Deserializer, Serialize};

use crate::graph::Format;

use super::{is_label, is_sha256, SpeechError, SpeechOptions};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechToolPolicy {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechModelPolicy {
    pub path: String,
    pub sha256: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechAudioPolicy {
    /// Global stream index returned by speech inspect; never defaulted.
    pub stream_index: u32,
    pub ffprobe: SpeechToolPolicy,
    pub ffmpeg: SpeechToolPolicy,
    pub timeout_seconds: u64,
}

/// Explicit historical result selection. Verification never falls back to a
/// fresh native run when the selected receipt or its retained files differ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechRecoveryPolicy {
    pub receipt_path: String,
    pub receipt_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechPolicy {
    /// Digest of the original graph input, before any audio preparation.
    pub source_sha256: String,
    pub provider: SpeechToolPolicy,
    pub model: SpeechModelPolicy,
    pub options: SpeechOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<SpeechAudioPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<SpeechRecoveryPolicy>,
}

fn default_audio_timeout() -> u64 { 300 }

// Derive's struct visitor also accepts positional arrays. Keep this public
// opt-in contract object-only, including all nested provider/model/options.
fn deserialize_object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where D: Deserializer<'de>, T: Deserialize<'de> {
    struct ObjectVisitor<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for ObjectVisitor<T> {
        type Value = T;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a speech policy object")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<T, M::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(ObjectVisitor(PhantomData))
}

impl<'de> Deserialize<'de> for SpeechToolPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields { path: String, sha256: String }
        let fields: Fields = deserialize_object(deserializer)?;
        Ok(Self { path: fields.path, sha256: fields.sha256 })
    }
}

impl<'de> Deserialize<'de> for SpeechModelPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields { path: String, sha256: String, id: String }
        let fields: Fields = deserialize_object(deserializer)?;
        Ok(Self { path: fields.path, sha256: fields.sha256, id: fields.id })
    }
}

impl<'de> Deserialize<'de> for SpeechAudioPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            stream_index: u32,
            ffprobe: SpeechToolPolicy,
            ffmpeg: SpeechToolPolicy,
            #[serde(default = "default_audio_timeout")]
            timeout_seconds: u64,
        }
        let fields: Fields = deserialize_object(deserializer)?;
        Ok(Self { stream_index: fields.stream_index, ffprobe: fields.ffprobe,
            ffmpeg: fields.ffmpeg, timeout_seconds: fields.timeout_seconds })
    }
}

impl<'de> Deserialize<'de> for SpeechRecoveryPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields { receipt_path: String, receipt_sha256: String }
        let fields: Fields = deserialize_object(deserializer)?;
        Ok(Self { receipt_path: fields.receipt_path, receipt_sha256: fields.receipt_sha256 })
    }
}

impl<'de> Deserialize<'de> for SpeechPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            source_sha256: String,
            provider: SpeechToolPolicy,
            model: SpeechModelPolicy,
            #[serde(default, deserialize_with = "deserialize_object")]
            options: SpeechOptions,
            #[serde(default)]
            audio: Option<SpeechAudioPolicy>,
            #[serde(default)]
            recovery: Option<SpeechRecoveryPolicy>,
        }
        let fields: Fields = deserialize_object(deserializer)?;
        Ok(Self { source_sha256: fields.source_sha256, provider: fields.provider,
            model: fields.model, options: fields.options, audio: fields.audio,
            recovery: fields.recovery })
    }
}

impl SpeechToolPolicy {
    fn validate(&self, name: &str) -> Result<(), SpeechError> {
        validate_path(&self.path, name)?;
        validate_digest(&self.sha256, name)
    }
}

impl SpeechPolicy {
    pub fn validate(&self) -> Result<(), SpeechError> {
        validate_digest(&self.source_sha256, "source_sha256")?;
        self.provider.validate("provider")?;
        validate_path(&self.model.path, "model")?;
        validate_digest(&self.model.sha256, "model")?;
        if !is_label(&self.model.id, 256) {
            return Err(invalid("model.id must be nonempty, at most 256 characters, and contain no controls"));
        }
        self.options.validate()?;
        if let Some(audio) = &self.audio {
            audio.ffprobe.validate("audio.ffprobe")?;
            audio.ffmpeg.validate("audio.ffmpeg")?;
            if !(1..=3600).contains(&audio.timeout_seconds) {
                return Err(invalid("audio.timeout_seconds must be between 1 and 3600"));
            }
        }
        if let Some(recovery) = &self.recovery {
            validate_path(&recovery.receipt_path, "recovery.receipt_path")?;
            validate_digest(&recovery.receipt_sha256, "recovery.receipt_sha256")?;
        }
        Ok(())
    }

    pub fn validate_source(&self, format: Format, collection: bool) -> Result<(), SpeechError> {
        self.validate()?;
        if collection {
            return Err(invalid("speech.policy.collection: speech recognition requires one original source artifact"));
        }
        if self.audio.is_none() && format != Format::Wav {
            return Err(invalid("speech.audio.required: non-WAV sources require explicit audio preparation and a selected global stream index"));
        }
        if self.audio.is_some() && !supports_prepared_source_format(format) {
            return Err(invalid("speech.policy.source: audio preparation does not support this original source format"));
        }
        Ok(())
    }
}

pub fn supports_prepared_source_format(format: Format) -> bool {
    matches!(format, Format::Wav | Format::Flac | Format::Mp3 | Format::M4aAlac
        | Format::M4aAac | Format::Aac | Format::Ogg | Format::Opus | Format::Mkv
        | Format::WebM | Format::Mp4 | Format::Mov | Format::Avi)
}

fn validate_path(value: &str, name: &str) -> Result<(), SpeechError> {
    let path = Path::new(value);
    if !is_label(value, 4096) || !path.is_absolute()
        || path.file_name().is_none()
        || path.components().any(|component| matches!(component, Component::ParentDir))
    {
        return Err(invalid(&format!("{name}.path must be an absolute file path without parent traversal or control characters")));
    }
    Ok(())
}

fn validate_digest(value: &str, name: &str) -> Result<(), SpeechError> {
    if !is_sha256(value) {
        return Err(invalid(&format!("{name} must carry a lowercase 64-character SHA-256")));
    }
    Ok(())
}

fn invalid(message: &str) -> SpeechError { SpeechError::InvalidRequest(message.to_owned()) }
