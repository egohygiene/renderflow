//! Recovery-only body exports, independently read back before publication.
//!
//! The native bytes cannot carry the complete recovery record. Callers must
//! retain the candidate and canonical JSON alongside this evidence. Removing
//! the envelope for the body comparison below never changes the candidate.

use std::io::{Cursor, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::{write::SimpleFileOptions, CompressionMethod, DateTime, ZipWriter};

use super::model::{validate_candidate, RecoveryCandidate};
use crate::screenplay::native::{equivalent_semantics, parse_native, NativeScreenplayFormat};
use crate::screenplay::{
    render_json, validate, Screenplay, ScreenplayElementKind, ScreenplayLossDirection,
    ScreenplayLossEvidence, ScreenplayLossKind, MAX_INPUT_BYTES, MAX_LOSS_ENTRIES, MAX_TEXT_BYTES,
};

pub const RECOVERY_EXPORT_SCHEMA_V1: &str = "renderflow.screenplay-recovery-export/v1";
pub const RECOVERY_EXPORT_PROFILE_V1: &str = "renderflow.recovery-native-body/v1";

/// Physical dialogue lines joined for the narrow Fountain body profile. Every
/// original line, span, and element ID remains in the retained candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryDialogueJoin {
    pub source_element_ids: Vec<String>,
    pub serialized_element_id: String,
    pub separator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryExportEvidence {
    pub schema_version: String,
    pub profile: String,
    pub target_format: NativeScreenplayFormat,
    pub source_sha256: String,
    /// SHA-256 of compact `serde_json::to_vec(candidate)`, independent of a
    /// retained sidecar's whitespace. The bundle receipt binds sidecar bytes.
    pub candidate_sha256: String,
    pub candidate_digest_scope: String,
    pub canonical_sha256: String,
    pub output_sha256: String,
    pub output_size_bytes: u64,
    pub source_element_count: usize,
    pub serialized_element_count: usize,
    pub dialogue_joins: Vec<RecoveryDialogueJoin>,
    pub losses: Vec<ScreenplayLossEvidence>,
    pub lossy: bool,
    pub review_required: bool,
    pub runtime_qualified: bool,
    pub semantic_validation: String,
    pub limitations: String,
}

#[derive(Debug, Clone)]
pub struct RecoveryExport {
    pub bytes: Vec<u8>,
    pub evidence: RecoveryExportEvidence,
}

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct RecoveryExportError {
    pub code: &'static str,
    pub message: String,
}

type Checked<T> = Result<T, RecoveryExportError>;

fn failure(code: &'static str, message: impl Into<String>) -> RecoveryExportError {
    RecoveryExportError { code, message: message.into() }
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

/// Export a verified recovery candidate into the adopted native body profile.
///
/// No provider runs here. Unsupported structure fails explicitly; no character,
/// paragraph, or unsupported construct is silently dropped. The independent
/// native reader must reproduce the chosen body projection before bytes leave
/// this function. This is structural evidence, not native-application or
/// original-source fidelity qualification.
pub fn export(candidate: &RecoveryCandidate, format: NativeScreenplayFormat) -> Checked<RecoveryExport> {
    validate_candidate(candidate).map_err(|error| failure("screenplay_recovery.export.candidate", error.to_string()))?;
    let canonical = render_json(&candidate.screenplay)
        .map_err(|error| failure("screenplay_recovery.export.canonical", error.to_string()))?;
    let candidate_bytes = serde_json::to_vec(candidate)
        .map_err(|error| failure("screenplay_recovery.export.candidate", error.to_string()))?;
    let (body, dialogue_joins) = body_projection(&candidate.screenplay, format)?;
    let bytes = match format {
        NativeScreenplayFormat::Fountain => fountain_bytes(&body)?,
        NativeScreenplayFormat::Fdx => xml_bytes(&body, format)?,
        NativeScreenplayFormat::Osf => xml_bytes(&body, format)?,
        NativeScreenplayFormat::FadeIn => fadein_bytes(&body)?,
    };
    validate_body_bytes(&body, format, &bytes)?;
    let mut losses = candidate.screenplay.losses.clone();
    let directional = [
        (ScreenplayLossKind::StableIds, "Native body bytes do not retain canonical stable element IDs. Retain the candidate and canonical JSON identified by this evidence."),
        (ScreenplayLossKind::SourceExtensions, "Per-page and per-element recovery spans, confidence labels, ambiguities, and extraction details remain in the companion recovery record, not in native body bytes."),
        (ScreenplayLossKind::PriorLossEvidence, "Recovery loss and review declarations remain mandatory in the companion evidence. Native body bytes alone do not attest to source fidelity or review completion."),
        (ScreenplayLossKind::Layout, "The exported body uses new formatting. Source indentation, line wrapping, pagination, and native application layout are not restored."),
    ];
    if losses.len().saturating_add(directional.len()) > MAX_LOSS_ENTRIES {
        return Err(failure("screenplay_recovery.export.bounds", "export loss evidence exceeds its bound"));
    }
    losses.extend(directional.into_iter().map(|(kind, detail)| ScreenplayLossEvidence {
        direction: ScreenplayLossDirection::Export, format: format.as_str().to_owned(), kind, detail: detail.to_owned(),
    }));
    let evidence = RecoveryExportEvidence {
        schema_version: RECOVERY_EXPORT_SCHEMA_V1.to_owned(),
        profile: RECOVERY_EXPORT_PROFILE_V1.to_owned(),
        target_format: format,
        source_sha256: candidate.source_sha256.clone(),
        candidate_sha256: digest(&candidate_bytes),
        candidate_digest_scope: "compact_serde_json_recovery_candidate/v1".to_owned(),
        canonical_sha256: digest(&canonical),
        output_sha256: digest(&bytes),
        output_size_bytes: bytes.len() as u64,
        source_element_count: candidate.screenplay.elements.len(),
        serialized_element_count: body.elements.len(),
        dialogue_joins,
        losses,
        lossy: true,
        review_required: true,
        runtime_qualified: false,
        semantic_validation: "independent_native_body_reparse".to_owned(),
        limitations: "Retain the full recovery candidate and canonical JSON with this derivative. Structural body comparison does not establish extraction accuracy, correct screenplay classification, completed human review, original layout, or acceptance by a native screenplay application.".to_owned(),
    };
    Ok(RecoveryExport { bytes, evidence })
}

/// Recreate deterministic bytes/evidence and repeat the independent body read.
/// Both the native payload and every evidence claim must still match the
/// supplied, independently replayed candidate before terminal publication.
pub fn validate_export(
    candidate: &RecoveryCandidate,
    format: NativeScreenplayFormat,
    bytes: &[u8],
    evidence: &RecoveryExportEvidence,
) -> Checked<()> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(failure("screenplay_recovery.export.bounds", "native output is empty or exceeds 16 MiB"));
    }
    let expected = export(candidate, format)?;
    if bytes != expected.bytes.as_slice() || evidence != &expected.evidence {
        return Err(failure("screenplay_recovery.export.identity", "native output or recovery export evidence differs from deterministic replay"));
    }
    Ok(())
}

fn body_projection(screenplay: &Screenplay, format: NativeScreenplayFormat) -> Checked<(Screenplay, Vec<RecoveryDialogueJoin>)> {
    validate(screenplay).map_err(|error| failure("screenplay_recovery.export.structure", error.to_string()))?;
    if !screenplay.title_page.is_empty() || !screenplay.dual_dialogue.is_empty() {
        return Err(failure("screenplay_recovery.export.unsupported", "the recovery body export does not serialize title pages or dual dialogue"));
    }
    let mut body = screenplay.clone();
    body.losses.clear();
    body.extensions.clear();
    body.source.extensions.clear();
    for element in &mut body.elements {
        if !matches!(element.kind, ScreenplayElementKind::SceneHeading | ScreenplayElementKind::Action
            | ScreenplayElementKind::Character | ScreenplayElementKind::Dialogue
            | ScreenplayElementKind::Parenthetical | ScreenplayElementKind::Transition)
            || element.scene_number.is_some() || element.section_level.is_some()
        {
            return Err(failure("screenplay_recovery.export.unsupported", "the recovery body export supports only six unnumbered, unstyled paragraph kinds"));
        }
        if element.text.is_empty() || element.text.trim() != element.text
            || element.text.chars().any(char::is_control)
        {
            return Err(failure("screenplay_recovery.export.text", "native recovery paragraphs must contain one nonempty trimmed line without controls"));
        }
        // These extensions have already been verified as a generated recovery
        // envelope. The original model and retained candidate are untouched.
        element.extensions.clear();
    }
    let mut joins = Vec::new();
    if format == NativeScreenplayFormat::Fountain {
        let mut elements = Vec::with_capacity(body.elements.len());
        let mut cursor = 0;
        while cursor < body.elements.len() {
            let mut element = body.elements[cursor].clone();
            cursor += 1;
            if element.kind == ScreenplayElementKind::Dialogue {
                let mut ids = vec![element.id.clone()];
                while let Some(next) = body.elements.get(cursor) {
                    if next.kind != ScreenplayElementKind::Dialogue || next.character_id != element.character_id { break; }
                    if element.text.len().saturating_add(1).saturating_add(next.text.len()) > MAX_TEXT_BYTES {
                        return Err(failure("screenplay_recovery.export.bounds", "joined Fountain dialogue exceeds the bounded native paragraph profile"));
                    }
                    element.text.push(' ');
                    element.text.push_str(&next.text);
                    ids.push(next.id.clone());
                    cursor += 1;
                }
                if ids.len() > 1 {
                    joins.push(RecoveryDialogueJoin {
                        source_element_ids: ids, serialized_element_id: element.id.clone(), separator: " ".to_owned(),
                    });
                }
            }
            elements.push(element);
        }
        body.elements = elements;
    }
    validate(&body).map_err(|error| failure("screenplay_recovery.export.structure", error.to_string()))?;
    Ok((body, joins))
}

fn append(output: &mut String, value: &str) -> Checked<()> {
    if output.len().saturating_add(value.len()) > MAX_INPUT_BYTES {
        return Err(failure("screenplay_recovery.export.bounds", "native output exceeds 16 MiB"));
    }
    output.push_str(value);
    Ok(())
}

fn append_xml(output: &mut String, value: &str) -> Checked<()> {
    for character in value.chars() {
        match character {
            '&' => append(output, "&amp;")?,
            '<' => append(output, "&lt;")?,
            '>' => append(output, "&gt;")?,
            '"' => append(output, "&quot;")?,
            '\'' => append(output, "&apos;")?,
            other => append(output, other.encode_utf8(&mut [0; 4]))?,
        }
    }
    Ok(())
}

fn kind_label(kind: ScreenplayElementKind) -> Checked<&'static str> {
    match kind {
        ScreenplayElementKind::SceneHeading => Ok("Scene Heading"),
        ScreenplayElementKind::Action => Ok("Action"),
        ScreenplayElementKind::Character => Ok("Character"),
        ScreenplayElementKind::Dialogue => Ok("Dialogue"),
        ScreenplayElementKind::Parenthetical => Ok("Parenthetical"),
        ScreenplayElementKind::Transition => Ok("Transition"),
        _ => Err(failure("screenplay_recovery.export.unsupported", "unsupported native paragraph kind")),
    }
}

fn xml_bytes(body: &Screenplay, format: NativeScreenplayFormat) -> Checked<Vec<u8>> {
    let fdx = format == NativeScreenplayFormat::Fdx;
    let mut output = String::new();
    append(&mut output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
    append(&mut output, if fdx {
        "<FinalDraft DocumentType=\"Script\" Template=\"No\" Version=\"1\"><Content>\n"
    } else {
        "<document type=\"Open Screenplay Format document\" version=\"20\"><paragraphs>\n"
    })?;
    for element in &body.elements {
        append(&mut output, if fdx { "<Paragraph Type=\"" } else { "<para><style basestylename=\"" })?;
        append(&mut output, kind_label(element.kind)?)?;
        append(&mut output, if fdx { "\"><Text>" } else { "\"/><text>" })?;
        if element.kind == ScreenplayElementKind::Parenthetical {
            if element.text.contains(['(', ')']) {
                return Err(failure("screenplay_recovery.export.unsupported", "nested parentheses cannot be represented by the verified native parenthetical profile"));
            }
            append(&mut output, "(")?;
        }
        append_xml(&mut output, &element.text)?;
        if element.kind == ScreenplayElementKind::Parenthetical { append(&mut output, ")")?; }
        append(&mut output, if fdx { "</Text></Paragraph>\n" } else { "</text></para>\n" })?;
    }
    append(&mut output, if fdx { "</Content></FinalDraft>\n" } else { "</paragraphs></document>\n" })?;
    Ok(output.into_bytes())
}

fn fountain_bytes(body: &Screenplay) -> Checked<Vec<u8>> {
    let mut output = String::new();
    for element in &body.elements {
        if !matches!(element.kind, ScreenplayElementKind::Dialogue | ScreenplayElementKind::Parenthetical)
            && !output.is_empty()
        {
            append(&mut output, "\n")?;
        }
        if element.kind == ScreenplayElementKind::Parenthetical { append(&mut output, "(")?; }
        append(&mut output, &element.text)?;
        if element.kind == ScreenplayElementKind::Parenthetical { append(&mut output, ")")?; }
        append(&mut output, "\n")?;
    }
    Ok(output.into_bytes())
}

fn fadein_bytes(body: &Screenplay) -> Checked<Vec<u8>> {
    let xml = xml_bytes(body, NativeScreenplayFormat::Osf)?;
    // Reserve more than the fixed one-member ZIP header/directory overhead.
    if xml.len() > MAX_INPUT_BYTES - 1024 {
        return Err(failure("screenplay_recovery.export.bounds", "Fade In XML leaves insufficient bounded archive space"));
    }
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    archive.start_file("document.xml", options)
        .map_err(|error| failure("screenplay_recovery.export.archive", error.to_string()))?;
    archive.write_all(&xml).map_err(|error| failure("screenplay_recovery.export.archive", error.to_string()))?;
    let output = archive.finish()
        .map_err(|error| failure("screenplay_recovery.export.archive", error.to_string()))?.into_inner();
    if output.len() > MAX_INPUT_BYTES {
        return Err(failure("screenplay_recovery.export.bounds", "Fade In archive exceeds 16 MiB"));
    }
    Ok(output)
}

fn validate_body_bytes(body: &Screenplay, format: NativeScreenplayFormat, bytes: &[u8]) -> Checked<()> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(failure("screenplay_recovery.export.bounds", "native output is empty or exceeds 16 MiB"));
    }
    let parsed = parse_native(bytes, format).map_err(|error| failure(
        "screenplay_recovery.export.unsupported",
        format!("{} body could not be independently accepted; retain the canonical candidate or choose another native target: {error}", format.as_str()),
    ))?;
    equivalent_semantics(body, &parsed).map_err(|error| failure(
        "screenplay_recovery.export.semantic_mismatch",
        format!("{} representation changed the recovery body projection: {error}", format.as_str()),
    ))
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
