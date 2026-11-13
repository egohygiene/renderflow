//! Manual timed lyrics: classification and derivatives over the shared codecs.
//!
//! There is no audio input, lyric recognition, timestamp inference or alignment.
//! SRT/WebVTT/timed-transcript JSON inputs need explicit lyrics policy selection.

pub mod graph;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::graph::Format;
use crate::timed_text::{self, TimedTextFormat, TimedTranscript};

pub const SCHEMA: &str = "renderflow.lyrics/v1";
pub const PROFILE: &str = "manual-timed-lyrics-v1";
pub const MAX_DOCUMENT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    #[default]
    Unreviewed,
    Reviewed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LyricsPolicy {
    /// Exact input bytes to which manual classification/review declarations apply.
    pub source_sha256: String,
    #[serde(default)]
    pub review_state: ReviewState,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub revision: Option<String>,
    /// Explicit cue-ID -> section labels; repeats remain separate observed cues.
    #[serde(default)]
    pub sections: BTreeMap<String, String>,
}

impl LyricsPolicy {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.source_sha256.len() == 64
                && self
                    .source_sha256
                    .bytes()
                    .all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch)),
            "lyrics.source.sha256: expected lowercase SHA-256"
        );
        if let Some(language) = &self.language {
            ensure!(
                language.len() <= 64
                    && language
                        .split('-')
                        .next()
                        .is_some_and(|part| (2..=8).contains(&part.len())
                            && part.bytes().all(|ch| ch.is_ascii_alphabetic()))
                    && language.split('-').all(|part| !part.is_empty()
                        && part.bytes().all(|ch| ch.is_ascii_alphanumeric())),
                "lyrics.language: expected an explicit language tag"
            );
        }
        if let Some(revision) = &self.revision {
            label(revision, 256)?;
        }
        ensure!(
            self.sections.len() <= timed_text::MAX_CUES,
            "lyrics.sections.limit"
        );
        ensure!(
            self.sections
                .iter()
                .map(|(id, section)| id.len() + section.len())
                .sum::<usize>()
                <= 1024 * 1024,
            "lyrics.sections.bytes.limit"
        );
        for (id, section) in &self.sections {
            label(id, 256)?;
            label(section, 1024)?;
        }
        Ok(())
    }
}

fn label(value: &str, limit: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control),
        "lyrics.label.invalid"
    );
    Ok(())
}

/// The nested transcript is the existing timing/text model, not another clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LyricsDocument {
    pub schema_version: String,
    pub source_format: String,
    pub policy: LyricsPolicy,
    pub transcript: TimedTranscript,
}

pub fn input_format(format: Format) -> Result<TimedTextFormat> {
    match format {
        Format::Srt => Ok(TimedTextFormat::Srt),
        Format::WebVtt => Ok(TimedTextFormat::WebVtt),
        Format::TimedTranscriptJson => Ok(TimedTextFormat::Json),
        _ => anyhow::bail!("lyrics.input: supply manual srt, vtt or timed_transcript_json; audio and untimed text are not inputs"),
    }
}

pub fn normalize(bytes: &[u8], format: Format, policy: &LyricsPolicy) -> Result<LyricsDocument> {
    policy.validate()?;
    ensure!(
        sha256(bytes) == policy.source_sha256,
        "lyrics.source.changed: manual input differs from policy binding"
    );
    let mut transcript = timed_text::parse(bytes, input_format(format)?)?;
    if let Some(language) = &policy.language {
        ensure!(
            transcript
                .language
                .as_ref()
                .is_none_or(|current| current == language),
            "lyrics.language.conflict: policy cannot overwrite a declared source language"
        );
        transcript.language = Some(language.clone());
    }
    let document = LyricsDocument {
        schema_version: SCHEMA.into(),
        source_format: format.to_string(),
        policy: policy.clone(),
        transcript,
    };
    validate(&document)?;
    Ok(document)
}

pub fn validate(document: &LyricsDocument) -> Result<()> {
    ensure!(
        document.schema_version == SCHEMA,
        "lyrics.schema: expected {SCHEMA}"
    );
    input_format(document.source_format.parse()?)?;
    document.policy.validate()?;
    timed_text::validate(&document.transcript)?;
    if document.source_format != Format::TimedTranscriptJson.to_string() {
        ensure!(
            document.transcript.source.sha256 == document.policy.source_sha256,
            "lyrics.source.binding"
        );
    }
    if let Some(language) = &document.policy.language {
        ensure!(
            document.transcript.language.as_ref() == Some(language),
            "lyrics.language.binding"
        );
    }
    if let Some(language) = &document.transcript.language {
        let mut checked = document.policy.clone();
        checked.language = Some(language.clone());
        checked.validate()?;
    }
    let cue_ids = document
        .transcript
        .cues
        .iter()
        .map(|cue| cue.id.as_str())
        .collect::<BTreeSet<_>>();
    for id in document.policy.sections.keys() {
        ensure!(
            cue_ids.contains(id.as_str()),
            "lyrics.section.cue: unknown cue ID {id}"
        );
    }
    Ok(())
}

pub fn parse_document(bytes: &[u8]) -> Result<LyricsDocument> {
    ensure!(bytes.len() <= MAX_DOCUMENT_BYTES, "lyrics.document.limit");
    let document = serde_json::from_slice(bytes).context("lyrics.document.json")?;
    validate(&document)?;
    Ok(document)
}

pub const OUTPUTS: &[Format] = &[
    Format::LyricsJson,
    Format::Yaml,
    Format::Srt,
    Format::WebVtt,
    Format::Text,
    Format::Markdown,
    Format::Lrc,
    Format::Ttml,
];

pub fn losses(document: &LyricsDocument, format: Format) -> Vec<String> {
    if matches!(format, Format::LyricsJson | Format::Yaml) {
        return Vec::new();
    }
    let mut result = vec![
        "classification_review_revision_and_section_metadata_in_companion".into(),
        "original_source_provenance_and_prior_losses_in_companion".into(),
    ];
    let core = match format {
        Format::Srt => Some(TimedTextFormat::Srt),
        Format::WebVtt => Some(TimedTextFormat::WebVtt),
        Format::Text | Format::Markdown => Some(TimedTextFormat::Text),
        _ => None,
    };
    if let Some(core) = core {
        result.extend(
            timed_text::export_losses(&document.transcript, core)
                .into_iter()
                .map(|loss| format!("timed_text.{loss:?}")),
        );
    } else {
        result.push(if format == Format::Ttml {
            "cue_ids_speakers_and_overlap_policy_in_companion".into()
        } else {
            "cue_ids_speakers_language_and_overlap_policy_in_companion".into()
        });
    }
    if format == Format::Lrc {
        result.extend([
            "cue_end_times_not_represented".into(),
            "multiline_cue_grouping_not_represented".into(),
        ]);
    }
    result
}

pub fn eligibility(document: &LyricsDocument) -> Value {
    let mut targets = OUTPUTS.iter().map(|format| json!({"format":format.to_string(),
        "status":if format.to_string() == document.source_format { "source_equivalent" } else { "eligible" },
        "losses":losses(document,*format),"runtime_qualified":false})).collect::<Vec<_>>();
    targets.extend([
        json!({"format":"chordpro","status":"unsupported","reason":"No chord representation or qualified ChordPro adapter is supplied by this timed-lyrics profile."}),
        json!({"format":"musicxml","status":"unsupported","reason":"No notes, score alignment or qualified MusicXML adapter is supplied; melody is never inferred."}),
    ]);
    json!({"schema_version":"renderflow.lyrics-eligibility/v1","profile":PROFILE,
        "source_sha256":document.policy.source_sha256,"review_state":document.policy.review_state,
        "review_is_declared":true,"targets":targets,"recognition_or_alignment":false,
        "runtime_qualified":false})
}

/// Export-only LRC and TTML use the shared transcript's integral milliseconds.
/// No end time, word clock, chord, pitch, translation or review state is inferred.
pub fn render(document: &LyricsDocument, format: Format) -> Result<Vec<u8>> {
    validate(document)?;
    let transcript = &document.transcript;
    let bytes = match format {
        Format::LyricsJson => {
            let mut bytes = serde_json::to_vec_pretty(document)?;
            bytes.push(b'\n');
            bytes
        }
        Format::Yaml => serde_yaml_ng::to_string(document)?.into_bytes(),
        Format::Srt => timed_text::render(transcript, TimedTextFormat::Srt)?,
        Format::WebVtt => timed_text::render(transcript, TimedTextFormat::WebVtt)?,
        Format::Text => timed_text::render(transcript, TimedTextFormat::Text)?,
        Format::Markdown => {
            let plain = timed_text::render(transcript, TimedTextFormat::Text)?;
            let plain = std::str::from_utf8(&plain)?;
            let mut output = String::new();
            for line in plain.split_terminator('\n') {
                output.push_str("    ");
                output.push_str(line);
                output.push('\n');
            }
            output.into_bytes()
        }
        Format::Lrc => {
            let mut output = String::new();
            for cue in &transcript.cues {
                for line in cue.text.split('\n') {
                    output.push_str(&format!(
                        "[{:02}:{:02}.{:03}]{}\n",
                        cue.start_ms / 60_000,
                        cue.start_ms / 1000 % 60,
                        cue.start_ms % 1000,
                        line
                    ));
                }
            }
            output.into_bytes()
        }
        Format::Ttml => {
            let language = transcript
                .language
                .as_ref()
                .map(|language| format!(" xml:lang=\"{}\"", xml(language)))
                .unwrap_or_default();
            let mut output = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<tt xmlns=\"http://www.w3.org/ns/ttml\" xml:space=\"preserve\"{language}><body><div>\n");
            for (index, cue) in transcript.cues.iter().enumerate() {
                output.push_str(&format!(
                    "<p xml:id=\"cue-{}\" begin=\"{}\" end=\"{}\">{}</p>\n",
                    index + 1,
                    timestamp(cue.start_ms),
                    timestamp(cue.end_ms),
                    cue.text
                        .split('\n')
                        .map(xml)
                        .collect::<Vec<_>>()
                        .join("<br/>")
                ));
            }
            output.push_str("</div></body></tt>\n");
            output.into_bytes()
        }
        _ => anyhow::bail!("lyrics.output: unsupported derivative {format}"),
    };
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_DOCUMENT_BYTES,
        "lyrics.output.limit"
    );
    Ok(bytes)
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn timestamp(ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn policy_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["source_sha256"],"properties":{
        "source_sha256":{"type":"string","pattern":"^[a-f0-9]{64}$"},
        "review_state":{"enum":["unreviewed","reviewed"],"default":"unreviewed"},
        "language":{"type":["string","null"],"maxLength":64},
        "revision":{"type":["string","null"],"maxLength":256},
        "sections":{"type":"object","maxProperties":100000,"additionalProperties":{"type":"string","minLength":1,"maxLength":1024},"default":{}}
    }})
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &[u8] = "1\n00:00:01,123 --> 00:00:03,456\nSous la lune\nEncore ici\n\n2\n00:00:03,456 --> 00:00:05,000\nSous la lune\n\n".as_bytes();
    fn policy() -> LyricsPolicy {
        LyricsPolicy {
            source_sha256: sha256(SRT),
            review_state: ReviewState::Unreviewed,
            language: Some("fr".into()),
            revision: Some("manual-r1".into()),
            sections: BTreeMap::new(),
        }
    }

    #[test]
    fn shared_codecs_preserve_authored_words_repeats_and_exact_millisecond_clocks() {
        let document = normalize(SRT, Format::Srt, &policy()).unwrap();
        for format in [Format::Srt, Format::WebVtt] {
            let parsed = timed_text::parse(
                &render(&document, format).unwrap(),
                input_format(format).unwrap(),
            )
            .unwrap();
            for (left, right) in document.transcript.cues.iter().zip(parsed.cues) {
                assert_eq!(
                    (&left.text, left.start_ms, left.end_ms),
                    (&right.text, right.start_ms, right.end_ms)
                );
            }
        }
        let lrc = String::from_utf8(render(&document, Format::Lrc).unwrap()).unwrap();
        assert!(lrc.contains("[00:01.123]Sous la lune\n[00:01.123]Encore ici"));
        assert_eq!(lrc.matches("Sous la lune").count(), 2);
        assert!(
            losses(&document, Format::Lrc).contains(&"cue_end_times_not_represented".to_string())
        );
    }

    #[test]
    fn structured_forms_retain_the_existing_transcript_and_explicit_policy() {
        let document = normalize(SRT, Format::Srt, &policy()).unwrap();
        assert_eq!(
            parse_document(&render(&document, Format::LyricsJson).unwrap()).unwrap(),
            document
        );
        let yaml: LyricsDocument =
            serde_yaml_ng::from_slice(&render(&document, Format::Yaml).unwrap()).unwrap();
        assert_eq!(yaml, document);
        let ttml = String::from_utf8(render(&document, Format::Ttml).unwrap()).unwrap();
        let xml = roxmltree::Document::parse(&ttml).unwrap();
        let first = xml
            .descendants()
            .find(|node| node.has_tag_name("p"))
            .unwrap();
        assert_eq!(first.attribute("begin"), Some("00:00:01.123"));
        assert_eq!(first.attribute("end"), Some("00:00:03.456"));
    }

    #[test]
    fn manual_source_binding_and_section_ids_are_required_and_no_audio_is_admitted() {
        let mut policy = policy();
        policy.source_sha256 = "a".repeat(64);
        assert!(normalize(SRT, Format::Srt, &policy).is_err());
        policy.source_sha256 = sha256(SRT);
        policy.sections.insert("absent".into(), "Chorus".into());
        assert!(normalize(SRT, Format::Srt, &policy).is_err());
        assert!(input_format(Format::Wav).is_err());
        assert!(input_format(Format::Text).is_err());
    }

    #[test]
    fn unsupported_score_formats_stay_diagnostics_and_do_not_invent_review() {
        let document = normalize(SRT, Format::Srt, &policy()).unwrap();
        let report = eligibility(&document);
        assert_eq!(report["review_state"], "unreviewed");
        assert!(report["targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["format"] == "musicxml" && entry["status"] == "unsupported"));
        assert!(render(&document, Format::Xml).is_err());
    }
}
