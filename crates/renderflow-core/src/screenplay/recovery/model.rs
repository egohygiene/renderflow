//! Deterministic, review-required recovery of a small screenplay body profile.
//!
//! This classifier measures no probabilities. Confidence labels rank the
//! strength of formatting cues only. Every candidate remains lossy and needs
//! human review, including candidates whose lines all have strong cues.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::screenplay::{
    validate, Screenplay, ScreenplayElement, ScreenplayElementKind,
    ScreenplayLossDirection, ScreenplayLossEvidence, ScreenplayLossKind, ScreenplaySource,
};

pub const CANDIDATE_SCHEMA_V1: &str = "renderflow.screenplay-recovery/v1";
pub const RECOVERY_HEURISTIC_V1: &str = "screenplay-body-lines/v1";
pub const MAX_RECOVERY_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_RECOVERY_PAGES: usize = 1_000;
pub const MAX_RECOVERY_LINES: usize = 100_000;
pub const MAX_RECOVERY_ELEMENTS: usize = 4_000;
pub const MAX_RECOVERY_LINE_BYTES: usize = 64 * 1024;
pub const MAX_RECOVERY_REPORT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionMethod {
    EmbeddedText,
    Ocr,
    PlainText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryInputFormat {
    Pdf,
    Text,
}

impl RecoveryInputFormat {
    pub fn as_str(self) -> &'static str {
        match self { Self::Pdf => "pdf", Self::Text => "text" }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedPage {
    pub page_number: u32,
    pub text: String,
    pub method: ExtractionMethod,
}

/// Ordinal formatting evidence, never a calibrated accuracy or probability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySourceSpan {
    pub page_number: u32,
    /// One-based physical line positions within this extracted page.
    pub line_start: u32,
    pub line_end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredElementEvidence {
    pub element_id: String,
    pub kind: ScreenplayElementKind,
    pub source: RecoverySourceSpan,
    /// Exact nonblank physical source line, including indentation/delimiters.
    pub source_text: String,
    pub confidence: RecoveryConfidence,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPageEvidence {
    pub page_number: u32,
    pub method: ExtractionMethod,
    /// Retained extraction, including blank lines; not a claim about PDF layout.
    pub text: String,
    pub text_sha256: String,
    pub line_count: u32,
    pub nonblank_line_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCandidate {
    pub schema_version: String,
    pub heuristic_version: String,
    pub review_required: bool,
    pub lossy: bool,
    /// Supplied upstream identity; callers must bind this to observed bytes.
    pub source_sha256: String,
    pub source_format: RecoveryInputFormat,
    pub extracted_text_sha256: String,
    pub pages: Vec<RecoveryPageEvidence>,
    pub elements: Vec<RecoveredElementEvidence>,
    pub screenplay: Screenplay,
}

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct RecoveryError {
    pub code: &'static str,
    pub message: String,
}

fn failure(code: &'static str, message: impl Into<String>) -> RecoveryError {
    RecoveryError { code, message: message.into() }
}

fn require(condition: bool, code: &'static str, message: impl Into<String>) -> Result<(), RecoveryError> {
    if condition { Ok(()) } else { Err(failure(code, message)) }
}

/// Split UTF-8 plain text using form feeds as explicit page boundaries.
/// A single final form feed is a terminator, not an invented empty page.
/// Interior empty pages remain present. CRLF is normalized; bare CR is refused.
pub fn text_pages(bytes: &[u8]) -> Result<Vec<ExtractedPage>, RecoveryError> {
    require(bytes.len() <= MAX_RECOVERY_INPUT_BYTES, "screenplay.recovery.bounds", "input exceeds 16 MiB")?;
    let input = std::str::from_utf8(bytes)
        .map_err(|error| failure("screenplay.recovery.encoding", format!("UTF-8 text is required: {error}")))?;
    let input = input.replace("\r\n", "\n");
    require(!input.contains('\r') && !input.starts_with('\u{feff}')
        && !input.chars().any(|character| character.is_control() && !matches!(character, '\n' | '\t' | '\u{000c}')),
        "screenplay.recovery.encoding", "only UTF-8 without BOM and LF/CRLF, tab, or form-feed controls is accepted")?;
    let paginated = input.strip_suffix('\u{000c}').unwrap_or(&input);
    let values: Vec<&str> = paginated.split('\u{000c}').take(MAX_RECOVERY_PAGES + 1).collect();
    require(values.len() <= MAX_RECOVERY_PAGES, "screenplay.recovery.bounds", "more than 1000 extracted pages")?;
    let pages: Vec<ExtractedPage> = values.into_iter().enumerate().map(|(index, text)| ExtractedPage {
        page_number: (index + 1) as u32,
        text: text.to_owned(),
        method: ExtractionMethod::PlainText,
    }).collect();
    validate_pages(&pages, RecoveryInputFormat::Text)?;
    Ok(pages)
}

fn lines(text: &str) -> Vec<&str> {
    if text.is_empty() { Vec::new() } else { text.split('\n').collect() }
}

fn validate_pages(pages: &[ExtractedPage], format: RecoveryInputFormat) -> Result<(), RecoveryError> {
    require(!pages.is_empty() && pages.len() <= MAX_RECOVERY_PAGES,
        "screenplay.recovery.bounds", "recovery requires between 1 and 1000 extracted pages")?;
    let mut total_bytes = 0_usize;
    let mut total_lines = 0_usize;
    let mut total_elements = 0_usize;
    for (index, page) in pages.iter().enumerate() {
        require(page.page_number as usize == index + 1, "screenplay.recovery.pages",
            "extracted pages must be complete, ordered, contiguous, and numbered from one")?;
        require(matches!((format, page.method),
            (RecoveryInputFormat::Text, ExtractionMethod::PlainText)
            | (RecoveryInputFormat::Pdf, ExtractionMethod::EmbeddedText | ExtractionMethod::Ocr)),
            "screenplay.recovery.source", "extraction method is inconsistent with the source format")?;
        total_bytes = total_bytes.checked_add(page.text.len())
            .ok_or_else(|| failure("screenplay.recovery.bounds", "page byte count overflow"))?;
        require(total_bytes <= MAX_RECOVERY_INPUT_BYTES, "screenplay.recovery.bounds", "extracted page text exceeds 16 MiB")?;
        require(!page.text.starts_with('\u{feff}')
            && !page.text.chars().any(|character| character.is_control() && !matches!(character, '\n' | '\t')),
            "screenplay.recovery.encoding", "extracted pages require normalized UTF-8 text without BOM and only LF/tab controls")?;
        if page.text.is_empty() { continue; }
        for line in page.text.split('\n') {
            total_lines += 1;
            require(total_lines <= MAX_RECOVERY_LINES && line.len() <= MAX_RECOVERY_LINE_BYTES,
                "screenplay.recovery.bounds", "extracted text exceeds 100000 lines or 65536 bytes per line")?;
            if !line.trim().is_empty() { total_elements += 1; }
            require(total_elements <= MAX_RECOVERY_ELEMENTS, "screenplay.recovery.bounds", "more than 4000 nonblank recovered lines")?;
        }
    }
    Ok(())
}

/// A length-framed digest includes page boundaries and extraction methods.
pub fn pages_digest(pages: &[ExtractedPage]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"renderflow.screenplay-recovery.pages/v1\0");
    digest.update((pages.len() as u64).to_be_bytes());
    for page in pages {
        digest.update(page.page_number.to_be_bytes());
        digest.update([match page.method {
            ExtractionMethod::EmbeddedText => 0_u8,
            ExtractionMethod::Ocr => 1_u8,
            ExtractionMethod::PlainText => 2_u8,
        }]);
        digest.update((page.text.len() as u64).to_be_bytes());
        digest.update(page.text.as_bytes());
    }
    format!("{digest:x}", digest = digest.finalize())
}

fn scene_heading(text: &str) -> bool {
    ["INT. ", "EXT. ", "INT./EXT. ", "EXT./INT. ", "INT/EXT. ", "EXT/INT. ", "I/E. "]
        .iter().any(|prefix| text.starts_with(prefix) && text.len() > prefix.len())
}

fn transition(text: &str) -> bool {
    matches!(text, "CUT TO:" | "DISSOLVE TO:" | "FADE IN:" | "FADE OUT." | "FADE OUT:"
        | "FADE TO BLACK." | "FADE TO BLACK:" | "CUT TO BLACK." | "SMASH CUT TO:"
        | "MATCH CUT TO:" | "BACK TO:")
}

fn page_marker(text: &str) -> bool {
    let digits = text.trim_end_matches('.');
    (!digits.is_empty() && digits.len() <= 6 && digits.bytes().all(|byte| byte.is_ascii_digit()))
        || matches!(text, "(MORE)" | "MORE" | "(CONTINUED)" | "CONTINUED:" | "(CONTINUED:)"
            | "CONTINUED" | "(CONT'D)" | "CONT'D")
        || text.strip_prefix("Page ").is_some_and(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn parenthetical(text: &str) -> Option<&str> {
    text.strip_prefix('(')?.strip_suffix(')').map(str::trim).filter(|value| !value.is_empty())
}

fn character_cue(text: &str) -> bool {
    !scene_heading(text) && !transition(text) && !page_marker(text)
        && text.chars().count() <= 60 && text.split_whitespace().count() <= 5
        && text.chars().any(|character| character.is_alphabetic() && character.is_uppercase())
        && text.to_uppercase() == text
        && text.chars().all(|character| character.is_alphabetic() || character.is_numeric()
            || matches!(character, ' ' | '-' | '\'' | '’' | '.' | '(' | ')'))
        && !text.ends_with('.') && !text.starts_with('(')
        && !["CHAPTER ", "SCENE ", "ACT ", "PART ", "TITLE "]
            .iter().any(|prefix| text.starts_with(prefix))
}

fn indentation(text: &str) -> usize {
    text.chars().take_while(|character| matches!(character, ' ' | '\t'))
        .map(|character| if character == '\t' { 4 } else { 1 }).sum()
}

fn has_dialogue_after(page_lines: &[&str], index: usize) -> bool {
    let Some(next) = page_lines.get(index + 1).map(|line| line.trim()) else { return false; };
    let next = if parenthetical(next).is_some() {
        let Some(dialogue) = page_lines.get(index + 2).map(|line| line.trim()) else { return false; };
        dialogue
    } else { next };
    !next.is_empty() && !scene_heading(next) && !transition(next) && !page_marker(next)
        && parenthetical(next).is_none() && !character_cue(next)
}

/// Recover one canonical element for every nonblank extracted physical line.
/// No OCR correction, header removal, reflow, dialogue completion, or semantic
/// rewriting occurs. Original lines and complete pages remain in the evidence.
pub fn recover_pages(
    pages: &[ExtractedPage],
    source_sha256: &str,
    source_format: RecoveryInputFormat,
) -> Result<RecoveryCandidate, RecoveryError> {
    require(source_sha256.len() == 64
        && source_sha256.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "screenplay.recovery.source", "source identity must be a lowercase SHA-256 digest")?;
    validate_pages(pages, source_format)?;
    let mut elements = Vec::new();
    let mut evidence = Vec::new();
    let mut page_evidence = Vec::new();
    let mut scene_count = 0_usize;
    let mut substantive_body_count = 0_usize;
    let mut character_names = BTreeSet::new();
    let mut dialogue_count = 0_usize;
    for page in pages {
        let page_lines = lines(&page.text);
        let mut active_character: Option<String> = None;
        let mut dialogue_indentation: Option<usize> = None;
        page_evidence.push(RecoveryPageEvidence {
            page_number: page.page_number,
            method: page.method,
            text: page.text.clone(),
            text_sha256: format!("{:x}", Sha256::digest(page.text.as_bytes())),
            line_count: page_lines.len() as u32,
            nonblank_line_count: page_lines.iter().filter(|line| !line.trim().is_empty()).count() as u32,
        });
        for (index, original) in page_lines.iter().enumerate() {
            let text = original.trim();
            if text.is_empty() {
                active_character = None;
                dialogue_indentation = None;
                continue;
            }
            let id = format!("recovered-p{:04}-l{:06}", page.page_number, index + 1);
            let source = RecoverySourceSpan { page_number: page.page_number, line_start: (index + 1) as u32, line_end: (index + 1) as u32 };
            let mut reasons: Vec<String> = Vec::new();
            let mut semantic_text = text.to_owned();
            let current_indentation = indentation(original);
            if dialogue_indentation.is_some_and(|previous| current_indentation < previous) {
                active_character = None;
                dialogue_indentation = None;
                reasons.push("indentation_ended_dialogue_context".to_owned());
            }
            let (kind, mut confidence) = if scene_heading(text) {
                scene_count += 1;
                reasons.push("conventional_scene_prefix".to_owned());
                (ScreenplayElementKind::SceneHeading, RecoveryConfidence::High)
            } else if transition(text) {
                reasons.push("recognized_transition_marker".to_owned());
                (ScreenplayElementKind::Transition, RecoveryConfidence::High)
            } else if page_marker(text) {
                reasons.push("pagination_or_continuation_marker_retained".to_owned());
                (ScreenplayElementKind::Action, RecoveryConfidence::Low)
            } else if active_character.is_none() && character_cue(text)
                && (index == 0 || page_lines[index - 1].trim().is_empty() || current_indentation >= 8)
                && has_dialogue_after(&page_lines, index)
            {
                character_names.insert(text.to_owned());
                reasons.push("uppercase_cue_followed_by_possible_dialogue".to_owned());
                reasons.push("cue_may_instead_be_action_or_heading".to_owned());
                (ScreenplayElementKind::Character, if current_indentation >= 8 { RecoveryConfidence::Medium } else { RecoveryConfidence::Low })
            } else if active_character.is_some() {
                if let Some(content) = parenthetical(text) {
                    semantic_text = content.to_owned();
                    reasons.push("parenthetical_in_character_block".to_owned());
                    reasons.push("parenthetical_delimiters_normalized".to_owned());
                    (ScreenplayElementKind::Parenthetical, RecoveryConfidence::Medium)
                } else {
                    dialogue_count += 1;
                    dialogue_indentation.get_or_insert(current_indentation);
                    reasons.push("contiguous_character_block_text".to_owned());
                    reasons.push("dialogue_or_action_requires_review".to_owned());
                    (ScreenplayElementKind::Dialogue, RecoveryConfidence::Low)
                }
            } else {
                reasons.push(if parenthetical(text).is_some() { "orphan_parenthetical_retained" }
                    else if character_cue(text) { "ambiguous_uppercase_text_retained" }
                    else { "unclassified_text_retained_as_action" }.to_owned());
                (ScreenplayElementKind::Action, RecoveryConfidence::Low)
            };
            if *original != text { reasons.push("surrounding_whitespace_normalized".to_owned()); }
            if page.method == ExtractionMethod::Ocr {
                confidence = RecoveryConfidence::Low;
                reasons.push("ocr_text_requires_source_review".to_owned());
            }
            let character_id = match kind {
                ScreenplayElementKind::Character => {
                    active_character = Some(id.clone());
                    dialogue_indentation = None;
                    None
                }
                ScreenplayElementKind::Dialogue | ScreenplayElementKind::Parenthetical => active_character.clone(),
                _ => {
                    active_character = None;
                    dialogue_indentation = None;
                    None
                }
            };
            if !matches!(kind, ScreenplayElementKind::SceneHeading | ScreenplayElementKind::Transition) && !page_marker(text) {
                substantive_body_count += 1;
            }
            let mut extensions = BTreeMap::new();
            extensions.insert("renderflow:recovery".to_owned(), json!({
                "page": source.page_number, "line_start": source.line_start, "line_end": source.line_end,
                "confidence": confidence, "reason_codes": reasons,
            }));
            elements.push(ScreenplayElement {
                id: id.clone(), kind, text: semantic_text, character_id,
                scene_number: None, section_level: None, extensions,
            });
            evidence.push(RecoveredElementEvidence {
                element_id: id, kind, source, source_text: (*original).to_owned(),
                confidence, reason_codes: reasons,
            });
        }
    }
    require(!elements.is_empty(), "screenplay.recovery.empty", "extracted pages contain no nonblank text")?;
    let cue_only = scene_count == 0 && character_names.len() >= 2 && dialogue_count >= 2;
    require((scene_count > 0 && substantive_body_count > 0) || cue_only,
        "screenplay.recovery.not_screenplay", "recovery requires a conventional scene heading with body text, or at least two distinct character cues with dialogue blocks; arbitrary prose is not coerced into a script")?;
    if cue_only {
        for (element, entry) in elements.iter_mut().zip(evidence.iter_mut()) {
            entry.confidence = RecoveryConfidence::Low;
            entry.reason_codes.push("missing_scene_heading_requires_review".to_owned());
            element.extensions.insert("renderflow:recovery".to_owned(), json!({
                "page": entry.source.page_number, "line_start": entry.source.line_start, "line_end": entry.source.line_end,
                "confidence": entry.confidence, "reason_codes": entry.reason_codes,
            }));
        }
    }
    let extracted_text_sha256 = pages_digest(pages);
    let mut screenplay = Screenplay::new(ScreenplaySource {
        sha256: source_sha256.to_owned(), format: source_format.as_str().to_owned(), extensions: BTreeMap::new(),
    }, elements);
    screenplay.extensions.insert("renderflow:recovery".to_owned(), json!({
        "schema_version": CANDIDATE_SCHEMA_V1,
        "heuristic_version": RECOVERY_HEURISTIC_V1,
        "review_required": true,
        "lossy": true,
        "extracted_text_sha256": extracted_text_sha256,
        "confidence_semantics": "ordinal_formatting_cues_not_measured_probabilities",
    }));
    for (kind, detail) in [
        (ScreenplayLossKind::ElementKinds, "Element kinds and dialogue ownership were inferred from formatting cues. Every recovered element requires review against its retained source line; no confidence band authorizes automatic approval."),
        (ScreenplayLossKind::Layout, "The canonical body trims surrounding whitespace and parenthetical delimiters and represents each nonblank physical line separately. Retained page and line evidence preserves extracted text, not original visual layout, typography, title-page structure, or reading-order certainty."),
        (ScreenplayLossKind::Pagination, "Page numbers and continuation markers are retained as uncertain action text. Page boundaries are provenance only; dialogue is never implicitly joined across pages."),
        (ScreenplayLossKind::UnknownSourceData, "Extraction may have omitted PDF visual content or misrecognized OCR text. Recovery does not reconstruct absent words, repair OCR, infer dual dialogue, or claim equivalence to the original screenplay."),
    ] {
        screenplay.losses.push(ScreenplayLossEvidence {
            direction: ScreenplayLossDirection::Import,
            format: source_format.as_str().to_owned(), kind, detail: detail.to_owned(),
        });
    }
    validate(&screenplay).map_err(|error| failure("screenplay.recovery.structure", format!("recovered candidate exceeds canonical bounds or structure: {error}")))?;
    let candidate = RecoveryCandidate {
        schema_version: CANDIDATE_SCHEMA_V1.to_owned(), heuristic_version: RECOVERY_HEURISTIC_V1.to_owned(),
        review_required: true, lossy: true, source_sha256: source_sha256.to_owned(), source_format,
        extracted_text_sha256, pages: page_evidence, elements: evidence, screenplay,
    };
    crate::screenplay::codec::bounded_encoded_len(&candidate, MAX_RECOVERY_REPORT_BYTES)
        .map_err(|_| failure("screenplay.recovery.bounds", "serialized recovery evidence exceeds 64 MiB"))?;
    Ok(candidate)
}

/// Replay deterministic recovery before exporting a deserialized candidate.
/// This detects edits to review state, semantics, lineage, spans, confidence,
/// reasons, and page evidence. It cannot independently authenticate the claimed
/// upstream PDF/text digest without the original bytes supplied by the caller.
pub fn validate_candidate(candidate: &RecoveryCandidate) -> Result<(), RecoveryError> {
    require(candidate.pages.len() <= MAX_RECOVERY_PAGES
        && candidate.elements.len() <= MAX_RECOVERY_ELEMENTS
        && candidate.screenplay.elements.len() <= MAX_RECOVERY_ELEMENTS,
        "screenplay.recovery.bounds", "candidate exceeds page or element bounds")?;
    let total_page_bytes = candidate.pages.iter().try_fold(0_usize, |total, page| total.checked_add(page.text.len()));
    require(total_page_bytes.is_some_and(|total| total <= MAX_RECOVERY_INPUT_BYTES),
        "screenplay.recovery.bounds", "candidate retained page text exceeds 16 MiB")?;
    let pages: Vec<ExtractedPage> = candidate.pages.iter().map(|page| ExtractedPage {
        page_number: page.page_number, text: page.text.clone(), method: page.method,
    }).collect();
    let regenerated = recover_pages(&pages, &candidate.source_sha256, candidate.source_format)?;
    require(candidate == &regenerated, "screenplay.recovery.candidate_mismatch",
        "candidate differs from deterministic recovery of its retained page text; review state, loss evidence and recovered semantics cannot be silently rewritten")
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
