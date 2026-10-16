//! Provider-neutral, bounded screenplay interchange.
//!
//! The canonical JSON codec preserves this semantic model, not the original
//! source bytes or native application layout. Canonical inspection treats
//! upstream source identity as a declaration. The separate native/provider
//! modules implement explicit, unqualified body-profile conversions only.

mod codec;
pub mod afterwriting;
pub mod afterwriting_graph;
pub mod analysis;
pub mod body_graph;
pub mod graph;
mod installation;
pub mod native;
pub mod pdf;
pub mod provider;
pub mod recovery;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use codec::{parse_json, render_json};

pub const SCREENPLAY_SCHEMA_V1: &str = "renderflow.screenplay/v1";
pub const SCREENPLAY_INSPECTION_SCHEMA_V1: &str = "renderflow.screenplay-inspection/v1";
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ELEMENTS: usize = 100_000;
pub const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_TITLE_PAGE_FIELDS: usize = 256;
pub const MAX_DUAL_DIALOGUE_GROUPS: usize = 10_000;
pub const MAX_LOSS_ENTRIES: usize = 1_024;
pub const MAX_EXTENSION_ENTRIES: usize = 64;
pub const MAX_EXTENSION_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_EXTENSION_TOTAL_BYTES: usize = 1024 * 1024;
pub const MAX_EXTENSION_DEPTH: usize = 16;
pub const MAX_EXTENSION_NODES: usize = 100_000;

/// Namespaced source-specific values are preserved but never interpreted.
pub type ScreenplayExtensions = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplaySource {
    /// Claimed digest of the upstream source, not of this JSON serialization.
    pub sha256: String,
    pub format: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: ScreenplayExtensions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TitlePageField {
    pub id: String,
    /// Original field label; repeated labels and their order are preserved.
    pub key: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: ScreenplayExtensions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenplayElementKind {
    SceneHeading,
    Action,
    Character,
    Dialogue,
    Parenthetical,
    Transition,
    Lyrics,
    Centered,
    Section,
    Synopsis,
    Note,
    /// Opaque source constructs require an extension carrying their meaning.
    Unknown,
}

impl ScreenplayElementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SceneHeading => "scene_heading",
            Self::Action => "action",
            Self::Character => "character",
            Self::Dialogue => "dialogue",
            Self::Parenthetical => "parenthetical",
            Self::Transition => "transition",
            Self::Lyrics => "lyrics",
            Self::Centered => "centered",
            Self::Section => "section",
            Self::Synopsis => "synopsis",
            Self::Note => "note",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayElement {
    pub id: String,
    pub kind: ScreenplayElementKind,
    /// Unicode semantic text. Native styling belongs in explicit extensions.
    pub text: String,
    /// Required on dialogue/parenthetical; refers to the preceding character
    /// block, never a global character-name dictionary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_level: Option<u8>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: ScreenplayExtensions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DualDialogue {
    pub id: String,
    pub left_character_id: String,
    pub right_character_id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: ScreenplayExtensions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenplayLossDirection {
    Import,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenplayLossKind {
    TitlePage,
    ElementKinds,
    StableIds,
    DualDialogue,
    TextStyling,
    Layout,
    Pagination,
    SourceExtensions,
    SourceProvenance,
    UnknownSourceData,
    PriorLossEvidence,
}

/// Historical adapter/operator declarations, not new findings from inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayLossEvidence {
    pub direction: ScreenplayLossDirection,
    pub format: String,
    pub kind: ScreenplayLossKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screenplay {
    pub schema_version: String,
    pub source: ScreenplaySource,
    pub title_page: Vec<TitlePageField>,
    pub elements: Vec<ScreenplayElement>,
    pub dual_dialogue: Vec<DualDialogue>,
    pub losses: Vec<ScreenplayLossEvidence>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: ScreenplayExtensions,
}

impl Screenplay {
    pub fn new(source: ScreenplaySource, elements: Vec<ScreenplayElement>) -> Self {
        Self {
            schema_version: SCREENPLAY_SCHEMA_V1.to_owned(),
            source,
            title_page: Vec::new(),
            elements,
            dual_dialogue: Vec::new(),
            losses: Vec::new(),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ScreenplayError {
    #[error("screenplay input exceeds the 16 MiB limit")]
    InputTooLarge,
    #[error("screenplay output exceeds the 16 MiB limit")]
    OutputTooLarge,
    #[error("screenplay JSON must be UTF-8: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("invalid screenplay JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid screenplay: {context}")]
    Invalid { context: String },
}

pub(crate) fn invalid(context: impl Into<String>) -> ScreenplayError {
    ScreenplayError::Invalid { context: context.into() }
}

fn label(value: &str, name: &str, maximum: usize) -> Result<(), ScreenplayError> {
    if value.trim().is_empty() || value.chars().count() > maximum || value.chars().any(char::is_control) {
        return Err(invalid(format!("{name} must be nonempty, at most {maximum} characters, and contain no controls")));
    }
    Ok(())
}

fn text(value: &str, name: &str, allow_empty: bool) -> Result<(), ScreenplayError> {
    if (!allow_empty && value.trim().is_empty()) || value.len() > MAX_TEXT_BYTES
        || value.chars().any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    {
        return Err(invalid(format!("{name} must fit within 65536 UTF-8 bytes and contain only text, LF, or tab controls")));
    }
    Ok(())
}

#[derive(Default)]
struct ExtensionBudget {
    entries: usize,
    bytes: usize,
    nodes: usize,
}

fn extension_value(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), ScreenplayError> {
    *nodes = nodes.checked_add(1).ok_or_else(|| invalid("extension node budget overflow"))?;
    if depth > MAX_EXTENSION_DEPTH || *nodes > MAX_EXTENSION_NODES {
        return Err(invalid("extensions exceed the depth or node budget"));
    }
    match value {
        Value::Number(number) if number.is_f64() => {
            return Err(invalid("extension numbers must be exact i64/u64 integers; preserve other numeric tokens as strings"));
        }
        Value::Array(values) => {
            for value in values {
                extension_value(value, depth + 1, nodes)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                extension_value(value, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn extensions(values: &ScreenplayExtensions, budget: &mut ExtensionBudget) -> Result<(), ScreenplayError> {
    if values.len() > MAX_EXTENSION_ENTRIES {
        return Err(invalid("an extension bag exceeds 64 entries"));
    }
    for (key, value) in values {
        label(key, "extension key", 128)?;
        let Some((namespace, name)) = key.split_once(':') else {
            return Err(invalid("extension keys must use a namespace:name label"));
        };
        if namespace.trim().is_empty() || name.trim().is_empty() {
            return Err(invalid("extension keys require a nonempty namespace and name"));
        }
        extension_value(value, 0, &mut budget.nodes)?;
        let size = codec::bounded_encoded_len(value, MAX_EXTENSION_VALUE_BYTES)
            .map_err(|_| invalid("an extension value exceeds 65536 encoded bytes"))?;
        budget.entries += 1;
        budget.bytes = budget.bytes.checked_add(key.len() + size).ok_or_else(|| invalid("extension byte budget overflow"))?;
        if budget.bytes > MAX_EXTENSION_TOTAL_BYTES {
            return Err(invalid("extensions exceed the aggregate 1 MiB byte budget"));
        }
    }
    Ok(())
}

/// Validate semantic references, preserved ordering, and resource bounds.
pub fn validate(screenplay: &Screenplay) -> Result<(), ScreenplayError> {
    if screenplay.schema_version != SCREENPLAY_SCHEMA_V1 {
        return Err(invalid("unsupported schema_version"));
    }
    if screenplay.source.sha256.len() != 64 || !screenplay.source.sha256.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(invalid("source.sha256 must be a lowercase SHA-256 hex digest"));
    }
    label(&screenplay.source.format, "source.format", 64)?;
    if screenplay.elements.is_empty() || screenplay.elements.len() > MAX_ELEMENTS {
        return Err(invalid("elements must contain between 1 and 100000 entries"));
    }
    if screenplay.title_page.len() > MAX_TITLE_PAGE_FIELDS || screenplay.dual_dialogue.len() > MAX_DUAL_DIALOGUE_GROUPS || screenplay.losses.len() > MAX_LOSS_ENTRIES {
        return Err(invalid("title-page, dual-dialogue, or loss-evidence count exceeds its bound"));
    }
    let mut ids = BTreeSet::new();
    let mut budget = ExtensionBudget::default();
    extensions(&screenplay.extensions, &mut budget)?;
    extensions(&screenplay.source.extensions, &mut budget)?;
    let mut total_text_bytes = 0_usize;
    for field in &screenplay.title_page {
        label(&field.id, "title_page.id", 128)?;
        if !ids.insert(field.id.as_str()) {
            return Err(invalid(format!("duplicate stable id: {}", field.id)));
        }
        label(&field.key, "title_page.key", 128)?;
        text(&field.value, "title_page.value", true)?;
        total_text_bytes = total_text_bytes.checked_add(field.value.len()).ok_or(ScreenplayError::InputTooLarge)?;
        extensions(&field.extensions, &mut budget)?;
    }
    let mut characters = BTreeMap::new();
    let mut active_character: Option<&str> = None;
    let mut dialogue_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut block_ends: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, element) in screenplay.elements.iter().enumerate() {
        label(&element.id, "element.id", 128)?;
        if !ids.insert(element.id.as_str()) {
            return Err(invalid(format!("duplicate stable id: {}", element.id)));
        }
        text(&element.text, "element.text", element.kind == ScreenplayElementKind::Unknown)?;
        total_text_bytes = total_text_bytes.checked_add(element.text.len()).ok_or(ScreenplayError::InputTooLarge)?;
        if total_text_bytes > MAX_INPUT_BYTES {
            return Err(ScreenplayError::InputTooLarge);
        }
        if element.scene_number.is_some() && element.kind != ScreenplayElementKind::SceneHeading {
            return Err(invalid("scene_number is allowed only on scene_heading elements"));
        }
        if let Some(number) = &element.scene_number {
            label(number, "scene_number", 128)?;
        }
        match (element.kind, element.section_level) {
            (ScreenplayElementKind::Section, Some(1..=6)) => {}
            (ScreenplayElementKind::Section, _) => return Err(invalid("section elements require section_level between 1 and 6")),
            (_, Some(_)) => return Err(invalid("section_level is allowed only on section elements")),
            (_, None) => {}
        }
        match element.kind {
            ScreenplayElementKind::Character => {
                if element.character_id.is_some() {
                    return Err(invalid("character elements cannot refer to another character block"));
                }
                active_character = Some(&element.id);
                characters.insert(element.id.as_str(), index);
                block_ends.insert(element.id.as_str(), index);
            }
            ScreenplayElementKind::Dialogue | ScreenplayElementKind::Parenthetical => {
                let owner = element.character_id.as_deref().ok_or_else(|| invalid("dialogue and parenthetical elements require character_id"))?;
                label(owner, "character_id", 128)?;
                if active_character != Some(owner) {
                    return Err(invalid("character_id must refer to the current preceding character block"));
                }
                block_ends.insert(owner, index);
                if element.kind == ScreenplayElementKind::Dialogue {
                    *dialogue_counts.entry(owner).or_default() += 1;
                }
            }
            _ => {
                if element.character_id.is_some() {
                    return Err(invalid("character_id is allowed only on dialogue and parenthetical elements"));
                }
                active_character = None;
            }
        }
        if element.kind == ScreenplayElementKind::Unknown && element.extensions.is_empty() {
            return Err(invalid("unknown elements require source-specific extensions"));
        }
        extensions(&element.extensions, &mut budget)?;
    }
    let mut paired_characters = BTreeSet::new();
    for group in &screenplay.dual_dialogue {
        label(&group.id, "dual_dialogue.id", 128)?;
        if !ids.insert(group.id.as_str()) {
            return Err(invalid(format!("duplicate stable id: {}", group.id)));
        }
        label(&group.left_character_id, "left_character_id", 128)?;
        label(&group.right_character_id, "right_character_id", 128)?;
        let left = group.left_character_id.as_str();
        let right = group.right_character_id.as_str();
        let left_start = characters.get(left).ok_or_else(|| invalid("dual dialogue left reference is not a character element"))?;
        let right_start = characters.get(right).ok_or_else(|| invalid("dual dialogue right reference is not a character element"))?;
        if left_start >= right_start || block_ends.get(left).copied().map(|end| end + 1) != Some(*right_start) {
            return Err(invalid("dual dialogue requires adjacent left-then-right character blocks"));
        }
        if dialogue_counts.get(left).copied().unwrap_or_default() == 0 || dialogue_counts.get(right).copied().unwrap_or_default() == 0 {
            return Err(invalid("each side of dual dialogue must contain dialogue"));
        }
        if !paired_characters.insert(left) || !paired_characters.insert(right) {
            return Err(invalid("a character block may belong to only one dual-dialogue group"));
        }
        extensions(&group.extensions, &mut budget)?;
    }
    for loss in &screenplay.losses {
        label(&loss.format, "loss.format", 64)?;
        text(&loss.detail, "loss.detail", false)?;
        total_text_bytes = total_text_bytes.checked_add(loss.detail.len()).ok_or(ScreenplayError::InputTooLarge)?;
        if total_text_bytes > MAX_INPUT_BYTES {
            return Err(ScreenplayError::InputTooLarge);
        }
    }
    codec::bounded_encoded_len(screenplay, MAX_INPUT_BYTES).map_err(|_| ScreenplayError::InputTooLarge)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenplayExportTarget {
    ScreenplayJson,
    Fountain,
    Fdx,
    #[serde(rename = "fadein")]
    FadeIn,
    Osf,
    Pdf,
    Text,
}

impl ScreenplayExportTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScreenplayJson => "screenplay_json",
            Self::Fountain => "fountain",
            Self::Fdx => "fdx",
            Self::FadeIn => "fadein",
            Self::Osf => "osf",
            Self::Pdf => "pdf",
            Self::Text => "text",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenplayPreservation {
    CanonicalModel,
    Unassessed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenplayImportFormat {
    ScreenplayJson,
    Fountain,
    Fdx,
    #[serde(rename = "fadein")]
    FadeIn,
    Osf,
}

impl ScreenplayImportFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScreenplayJson => "screenplay_json",
            Self::Fountain => "fountain",
            Self::Fdx => "fdx",
            Self::FadeIn => "fadein",
            Self::Osf => "osf",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayImportAssessment {
    pub source_format: ScreenplayImportFormat,
    pub adapter_available: bool,
    pub preservation: ScreenplayPreservation,
    pub review_required: Vec<ScreenplayLossKind>,
    pub detail: String,
}

/// General canonical-model import policy, independent of provider-local body
/// validation. It does not configure or qualify a native conversion route.
pub fn assess_import(format: ScreenplayImportFormat) -> ScreenplayImportAssessment {
    if format == ScreenplayImportFormat::ScreenplayJson {
        return ScreenplayImportAssessment {
            source_format: format,
            adapter_available: true,
            preservation: ScreenplayPreservation::CanonicalModel,
            review_required: Vec::new(),
            detail: "The bounded canonical JSON reader validates and retains the declared model. It does not reobserve the upstream source or attest to the completeness of earlier imports.".to_owned(),
        };
    }
    ScreenplayImportAssessment {
        source_format: format,
        adapter_available: false,
        preservation: ScreenplayPreservation::Unassessed,
        review_required: vec![ScreenplayLossKind::TitlePage, ScreenplayLossKind::ElementKinds, ScreenplayLossKind::StableIds, ScreenplayLossKind::DualDialogue, ScreenplayLossKind::TextStyling, ScreenplayLossKind::Layout, ScreenplayLossKind::Pagination, ScreenplayLossKind::SourceExtensions, ScreenplayLossKind::SourceProvenance, ScreenplayLossKind::UnknownSourceData, ScreenplayLossKind::PriorLossEvidence],
        detail: "This canonical inspection API does not import native sources. The separate opt-in scripttool adapter validates a restricted body profile for native-to-Fountain conversion; it supplies no general native-to-canonical import or preservation qualification.".to_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayExportAssessment {
    pub target: ScreenplayExportTarget,
    pub adapter_available: bool,
    pub preservation: ScreenplayPreservation,
    /// Features a future writer must explicitly account for. This is not a
    /// declaration that any particular native format necessarily loses them.
    pub review_required: Vec<ScreenplayLossKind>,
    pub detail: String,
}

/// Directional model-to-target policy. It never infers reverse conversion or
/// lossless native round trips from the model's ability to retain data.
pub fn assess_export(screenplay: &Screenplay, target: ScreenplayExportTarget) -> Result<ScreenplayExportAssessment, ScreenplayError> {
    validate(screenplay)?;
    if target == ScreenplayExportTarget::ScreenplayJson {
        return Ok(ScreenplayExportAssessment {
            target,
            adapter_available: true,
            preservation: ScreenplayPreservation::CanonicalModel,
            review_required: Vec::new(),
            detail: "Canonical JSON preserves the validated semantic model, including declared prior losses and opaque extensions; original source bytes, native layout, and unrecorded source data are not recovered.".to_owned(),
        });
    }
    let mut review_required = vec![ScreenplayLossKind::ElementKinds, ScreenplayLossKind::StableIds, ScreenplayLossKind::SourceProvenance, ScreenplayLossKind::TextStyling, ScreenplayLossKind::Layout, ScreenplayLossKind::Pagination];
    if !screenplay.title_page.is_empty() { review_required.push(ScreenplayLossKind::TitlePage); }
    if !screenplay.dual_dialogue.is_empty() { review_required.push(ScreenplayLossKind::DualDialogue); }
    if extension_entry_count(screenplay) > 0 { review_required.push(ScreenplayLossKind::SourceExtensions); }
    if screenplay.elements.iter().any(|element| element.kind == ScreenplayElementKind::Unknown) { review_required.push(ScreenplayLossKind::UnknownSourceData); }
    if !screenplay.losses.is_empty() { review_required.push(ScreenplayLossKind::PriorLossEvidence); }
    Ok(ScreenplayExportAssessment {
        target,
        adapter_available: false,
        preservation: ScreenplayPreservation::Unassessed,
        review_required,
        detail: "No canonical-model writer is implemented for this target. The separate native-to-Fountain adapter does not export this canonical document. Preservation and losses require a directional adapter contract and qualification; this assessment authorizes no conversion.".to_owned(),
    })
}

fn extension_entry_count(screenplay: &Screenplay) -> usize {
    screenplay.extensions.len() + screenplay.source.extensions.len()
        + screenplay.title_page.iter().map(|field| field.extensions.len()).sum::<usize>()
        + screenplay.elements.iter().map(|element| element.extensions.len()).sum::<usize>()
        + screenplay.dual_dialogue.iter().map(|group| group.extensions.len()).sum::<usize>()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayInspection {
    pub schema_version: String,
    pub input_sha256: String,
    pub input_bytes: usize,
    pub source: ScreenplaySource,
    pub source_identity_verified: bool,
    pub title_page_fields: usize,
    pub elements: usize,
    pub element_counts: BTreeMap<String, usize>,
    pub dual_dialogue_groups: usize,
    /// Count of top-level namespaced entries across all extension bags.
    pub extension_entries: usize,
    pub prior_losses: Vec<ScreenplayLossEvidence>,
    /// No native adapter is selected by canonical JSON inspection. This field
    /// does not inventory separately configured provider conversion routes.
    pub native_adapters_available: bool,
    pub import_assessments: Vec<ScreenplayImportAssessment>,
    pub export_assessments: Vec<ScreenplayExportAssessment>,
}

pub fn inspect_json(bytes: &[u8]) -> Result<ScreenplayInspection, ScreenplayError> {
    let screenplay = parse_json(bytes)?;
    let mut element_counts = BTreeMap::new();
    for element in &screenplay.elements {
        *element_counts.entry(element.kind.as_str().to_owned()).or_default() += 1;
    }
    let export_assessments = [ScreenplayExportTarget::ScreenplayJson, ScreenplayExportTarget::Fountain, ScreenplayExportTarget::Fdx, ScreenplayExportTarget::FadeIn, ScreenplayExportTarget::Osf, ScreenplayExportTarget::Pdf, ScreenplayExportTarget::Text]
        .into_iter().map(|target| assess_export(&screenplay, target)).collect::<Result<Vec<_>, _>>()?;
    Ok(ScreenplayInspection {
        schema_version: SCREENPLAY_INSPECTION_SCHEMA_V1.to_owned(),
        input_sha256: format!("{:x}", Sha256::digest(bytes)),
        input_bytes: bytes.len(),
        source: screenplay.source.clone(),
        source_identity_verified: false,
        title_page_fields: screenplay.title_page.len(),
        elements: screenplay.elements.len(),
        element_counts,
        dual_dialogue_groups: screenplay.dual_dialogue.len(),
        extension_entries: extension_entry_count(&screenplay),
        prior_losses: screenplay.losses,
        native_adapters_available: false,
        import_assessments: [ScreenplayImportFormat::ScreenplayJson, ScreenplayImportFormat::Fountain, ScreenplayImportFormat::Fdx, ScreenplayImportFormat::FadeIn, ScreenplayImportFormat::Osf].into_iter().map(assess_import).collect(),
        export_assessments,
    })
}
