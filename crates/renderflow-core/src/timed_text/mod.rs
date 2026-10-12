//! Bounded, local timed-text interchange. This module never recognizes speech.
//!
//! The first profile accepts UTF-8 plain-text cues only. Styling, markup, cue
//! settings and WebVTT metadata are refused, so conversion cannot silently
//! discard them. Overlaps are retained under the explicit `preserve` policy;
//! cue starts must always be in nondecreasing order. Timestamps are integral
//! milliseconds in the shared SRT/WebVTT range 00:00:00.000–99:59:59.999.

mod codec;
pub mod provider;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use codec::{parse, render};

pub const TIMED_TRANSCRIPT_SCHEMA_V1: &str = "renderflow.timed-transcript/v1";
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CUES: usize = 100_000;
pub const MAX_CUE_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_TIMESTAMP_MS: u64 = 359_999_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimedTextFormat {
    Srt,
    #[serde(rename = "webvtt")]
    WebVtt,
    Json,
    Text,
}

impl TimedTextFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::WebVtt => "webvtt",
            Self::Json => "json",
            Self::Text => "text",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlapPolicy {
    Preserve,
    Reject,
}

/// Information that cannot be carried by a requested export format.
///
/// Renderers return only bytes; callers must present `export_losses` alongside
/// them (the CLI writes an evidence report to stdout). JSON preserves the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimedTextLoss {
    Timing,
    CueIds,
    Speakers,
    Language,
    SourceProvenance,
    OverlapPolicy,
    PriorLossEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamIdentity {
    /// Absolute container stream index, not the subtitle-only ordinal.
    pub index: u32,
    pub codec: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionProvenance {
    pub provider: String,
    pub ffprobe_version: String,
    pub ffmpeg_version: String,
    /// Digest of the extracted subtitle bytes before core parsing.
    pub subtitle_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptSource {
    /// Original subtitle bytes, or original media bytes for extracted streams.
    pub sha256: String,
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<StreamIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extraction: Option<ExtractionProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedCue {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedTranscript {
    pub schema_version: String,
    pub source: TranscriptSource,
    pub overlap_policy: OverlapPolicy,
    pub cues: Vec<TimedCue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub losses: Vec<TimedTextLoss>,
}

impl TimedTranscript {
    pub fn new(source: TranscriptSource, cues: Vec<TimedCue>) -> Self {
        Self {
            schema_version: TIMED_TRANSCRIPT_SCHEMA_V1.to_owned(),
            source,
            overlap_policy: OverlapPolicy::Preserve,
            cues,
            language: None,
            losses: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum TimedTextError {
    #[error("timed-text input exceeds the 16 MiB limit")]
    InputTooLarge,
    #[error("timed-text output exceeds the 16 MiB limit")]
    OutputTooLarge,
    #[error("plain text is an export format; it cannot supply cue timing")]
    UnsupportedInput,
    #[error("timed-text input must be UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("invalid timed-transcript JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid timed text: {context}")]
    Invalid { context: String },
    #[error("unsupported timed-text feature: {feature}")]
    UnsupportedFeature { feature: String },
}

fn invalid(context: impl Into<String>) -> TimedTextError {
    TimedTextError::Invalid { context: context.into() }
}

fn bounded_label(value: &str, name: &str, maximum: usize) -> Result<(), TimedTextError> {
    if value.trim().is_empty() || value.chars().count() > maximum || value.chars().any(char::is_control) {
        return Err(invalid(format!("{name} must be nonempty, at most {maximum} characters, and contain no controls")));
    }
    Ok(())
}

fn digest(value: &str, name: &str) -> Result<(), TimedTextError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(invalid(format!("{name} must be a lowercase SHA-256 hex digest")));
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), TimedTextError> {
    if value.is_empty() || value.len() > MAX_CUE_TEXT_BYTES || value.lines().any(|line| line.trim().is_empty()) {
        return Err(invalid("cue text must contain nonempty lines and fit within 65536 UTF-8 bytes"));
    }
    if value.ends_with('\n') || value.chars().any(|ch| ch.is_control() && ch != '\n' && ch != '\t') {
        return Err(invalid("cue text contains a control character or trailing newline"));
    }
    if value.contains(['<', '>', '&']) || value.contains("{\\") {
        return Err(TimedTextError::UnsupportedFeature {
            feature: "cue markup, entities or style overrides; this profile accepts plain text only".to_owned(),
        });
    }
    Ok(())
}

/// Validate the closed v1 model and the supported plain-text profile.
pub fn validate(transcript: &TimedTranscript) -> Result<(), TimedTextError> {
    if transcript.schema_version != TIMED_TRANSCRIPT_SCHEMA_V1 {
        return Err(invalid("unsupported timed-transcript schema_version"));
    }
    digest(&transcript.source.sha256, "source.sha256")?;
    bounded_label(&transcript.source.format, "source.format", 64)?;
    if let Some(stream) = &transcript.source.stream {
        bounded_label(&stream.codec, "source.stream.codec", 128)?;
        if let Some(language) = &stream.language {
            bounded_label(language, "source.stream.language", 128)?;
        }
    }
    if let Some(extraction) = &transcript.source.extraction {
        if transcript.source.stream.is_none() {
            return Err(invalid("extraction provenance requires a source stream identity"));
        }
        bounded_label(&extraction.provider, "source.extraction.provider", 256)?;
        bounded_label(&extraction.ffprobe_version, "source.extraction.ffprobe_version", 4096)?;
        bounded_label(&extraction.ffmpeg_version, "source.extraction.ffmpeg_version", 4096)?;
        digest(&extraction.subtitle_sha256, "source.extraction.subtitle_sha256")?;
    }
    if let Some(language) = &transcript.language {
        bounded_label(language, "language", 128)?;
    }
    if transcript.cues.is_empty() || transcript.cues.len() > MAX_CUES {
        return Err(invalid("transcript must contain between 1 and 100000 cues"));
    }
    if transcript.losses.len() > 7 {
        return Err(invalid("losses has more entries than defined loss kinds"));
    }
    for (index, loss) in transcript.losses.iter().enumerate() {
        if transcript.losses[..index].contains(loss) {
            return Err(invalid("losses contains a duplicate kind"));
        }
    }
    let mut ids = BTreeSet::new();
    let mut previous_start = None;
    let mut furthest_end = 0;
    let mut text_bytes = 0_usize;
    for cue in &transcript.cues {
        bounded_label(&cue.id, "cue.id", 128)?;
        if cue.id.contains("-->") || matches!(cue.id.as_str(), "WEBVTT" | "STYLE" | "REGION" | "NOTE") || cue.id.starts_with("NOTE ") || cue.id.starts_with("NOTE\t") {
            return Err(invalid("cue.id conflicts with a WebVTT delimiter or reserved block"));
        }
        if !ids.insert(&cue.id) {
            return Err(invalid(format!("duplicate cue id: {}", cue.id)));
        }
        if cue.start_ms >= cue.end_ms || cue.end_ms > MAX_TIMESTAMP_MS {
            return Err(invalid(format!("cue {} must have 0 <= start_ms < end_ms <= {MAX_TIMESTAMP_MS}", cue.id)));
        }
        if previous_start.is_some_and(|start| cue.start_ms < start) {
            return Err(invalid("cue starts must be in nondecreasing order"));
        }
        if transcript.overlap_policy == OverlapPolicy::Reject && cue.start_ms < furthest_end {
            return Err(invalid("overlapping cues are refused by the reject overlap policy"));
        }
        previous_start = Some(cue.start_ms);
        furthest_end = furthest_end.max(cue.end_ms);
        validate_text(&cue.text)?;
        text_bytes = text_bytes.checked_add(cue.text.len()).ok_or(TimedTextError::InputTooLarge)?;
        if text_bytes > MAX_INPUT_BYTES {
            return Err(TimedTextError::InputTooLarge);
        }
        if let Some(speaker) = &cue.speaker {
            bounded_label(speaker, "cue.speaker", 256)?;
        }
    }
    Ok(())
}

/// Explicit loss disclosure for an export; no transformations mutate the model.
pub fn export_losses(transcript: &TimedTranscript, format: TimedTextFormat) -> Vec<TimedTextLoss> {
    if format == TimedTextFormat::Json {
        return Vec::new();
    }
    let mut losses = vec![TimedTextLoss::SourceProvenance, TimedTextLoss::OverlapPolicy];
    if format == TimedTextFormat::Text {
        losses.push(TimedTextLoss::Timing);
        losses.push(TimedTextLoss::CueIds);
    } else if format == TimedTextFormat::Srt && transcript.cues.iter().enumerate().any(|(index, cue)| cue.id != (index + 1).to_string()) {
        losses.push(TimedTextLoss::CueIds);
    }
    if transcript.cues.iter().any(|cue| cue.speaker.is_some()) {
        losses.push(TimedTextLoss::Speakers);
    }
    if transcript.language.is_some() || transcript.source.stream.as_ref().is_some_and(|stream| stream.language.is_some()) {
        losses.push(TimedTextLoss::Language);
    }
    if !transcript.losses.is_empty() {
        losses.push(TimedTextLoss::PriorLossEvidence);
    }
    losses
}
