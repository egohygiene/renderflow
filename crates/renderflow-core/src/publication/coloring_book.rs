//! Deterministic, provider-neutral coloring-book source and preflight contracts.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::evidence::DigestEvidence;

pub const COLORING_BOOK_SCHEMA_V1: &str = "renderflow.coloring-book/v1";
pub const COLORING_BOOK_REPORT_SCHEMA_V1: &str = "renderflow.coloring-book-validation/v1";
pub const COLORING_BOOK_VALIDATOR_ID: &str = "renderflow.builtin.coloring-book-preflight";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColoringBookUse {
    Private,
    NonCommercial,
    Public,
    Commercial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    FrontCover,
    Interior,
    IntentionalBlank,
    BackMatter,
    BackCover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkOrigin {
    ReviewedSource,
    GeneratedCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateState {
    Candidate,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLocality {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringBookAudience {
    pub label: String,
    pub complexity: String,
    #[serde(default)]
    pub minimum_age: Option<u8>,
    #[serde(default)]
    pub maximum_age: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringBookGeometry {
    pub width_mm: f64,
    pub height_mm: f64,
    pub margin_mm: f64,
    pub bleed_mm: f64,
    pub safe_area_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineArtPolicy {
    pub minimum_stroke_pt: f64,
    pub minimum_contrast_ratio: f64,
    pub minimum_dpi: u32,
    pub foreground: String,
    pub background: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaginationPolicy {
    pub expected_page_count: u32,
    pub first_interior_side: String,
    pub binding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringBookPrintMetadata {
    pub color_space: String,
    pub interior_color: String,
    pub paper: String,
    pub duplex: bool,
    pub embedded_fonts_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedSourceRef {
    pub path: String,
    pub sha256: String,
    pub approval_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RightsEvidence {
    pub license: String,
    pub rights_holder: String,
    pub source: String,
    pub approval_reference: String,
    pub reviewed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalEvidence {
    pub state: CandidateState,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub approved_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationEvidence {
    pub candidate_id: String,
    pub provider_id: String,
    pub provider_locality: ProviderLocality,
    pub model_id: String,
    pub skill_id: String,
    pub skill_version: String,
    pub settings_sha256: String,
    pub prompt_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineArtInspection {
    pub effective_dpi: u32,
    pub contrast_ratio: f64,
    pub minimum_stroke_pt: f64,
    pub trim_verified: bool,
    pub bleed_verified: bool,
    pub safe_area_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HygieneReview {
    pub privacy_reviewed: bool,
    pub copyright_reviewed: bool,
    pub protected_references_reviewed: bool,
    pub prompt_hygiene_reviewed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringBookArtwork {
    pub path: String,
    pub sha256: String,
    pub origin: ArtworkOrigin,
    pub approval: ApprovalEvidence,
    pub rights: RightsEvidence,
    pub inspection: LineArtInspection,
    #[serde(default)]
    pub generation: Option<GenerationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessibleEquivalent {
    pub alt_text: String,
    pub caption: String,
    #[serde(default)]
    pub source_links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringBookPage {
    pub id: String,
    pub order: u32,
    pub kind: PageKind,
    #[serde(default)]
    pub teaching_objective: Option<String>,
    #[serde(default)]
    pub reviewed_text: Option<ReviewedSourceRef>,
    #[serde(default)]
    pub artwork: Option<ColoringBookArtwork>,
    #[serde(default)]
    pub accessibility: Option<AccessibleEquivalent>,
    #[serde(default)]
    pub continuity_references: Vec<String>,
    #[serde(default)]
    pub blank_intent: Option<String>,
    #[serde(default)]
    pub allow_duplicate_artwork: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontAssetRef {
    pub path: String,
    pub sha256: String,
    pub license: String,
    pub embedding_approved: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringBookContract {
    pub schema: String,
    pub book_id: String,
    pub version: String,
    pub title: String,
    pub language: String,
    pub intended_use: ColoringBookUse,
    pub audience: ColoringBookAudience,
    pub geometry: ColoringBookGeometry,
    pub line_art: LineArtPolicy,
    pub pagination: PaginationPolicy,
    pub print: ColoringBookPrintMetadata,
    pub hygiene: HygieneReview,
    #[serde(default)]
    pub fonts: Vec<FontAssetRef>,
    pub pages: Vec<ColoringBookPage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColoringFindingSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringFinding {
    pub severity: ColoringFindingSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringArtifactEvidence {
    pub page_id: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_text: Option<ColoringReviewedTextEvidence>,
    pub locator: String,
    pub digest: DigestEvidence,
    pub origin: ArtworkOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub candidate_state: CandidateState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
    pub rights: RightsEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringReviewedTextEvidence {
    pub locator: String,
    pub digest: DigestEvidence,
    pub approval_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringPolicyEvidence {
    pub deterministic_offline: bool,
    pub network_accessed: bool,
    pub ai_invoked: bool,
    pub remote_provider_opt_in: bool,
    pub candidate_output_authoritative: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColoringValidationStatus {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoringBookValidationReport {
    pub schema: String,
    pub validator_id: String,
    pub validator_version: String,
    pub contract_digest: DigestEvidence,
    pub settings_digest: DigestEvidence,
    pub policy: ColoringPolicyEvidence,
    pub status: ColoringValidationStatus,
    pub release_eligible: bool,
    pub artifacts: Vec<ColoringArtifactEvidence>,
    pub findings: Vec<ColoringFinding>,
}

impl ColoringBookValidationReport {
    pub fn is_valid(&self) -> bool {
        self.status == ColoringValidationStatus::Valid
    }
}

pub fn evaluate_coloring_book(
    contract_path: &Path,
    allow_remote: bool,
) -> Result<ColoringBookValidationReport> {
    let bytes = fs::read(contract_path).with_context(|| {
        format!(
            "failed to read coloring-book contract '{}'",
            contract_path.display()
        )
    })?;
    let contract: ColoringBookContract = serde_yaml_ng::from_slice(&bytes).with_context(|| {
        format!(
            "invalid coloring-book contract '{}'",
            contract_path.display()
        )
    })?;
    let root = contract_path.parent().unwrap_or_else(|| Path::new("."));
    validate_coloring_book(&contract, root, allow_remote)
}

pub fn validate_coloring_book(
    contract: &ColoringBookContract,
    root: &Path,
    allow_remote: bool,
) -> Result<ColoringBookValidationReport> {
    let mut findings = Vec::new();
    let mut artifacts = Vec::new();
    let mut page_ids = BTreeSet::new();
    let mut orders = BTreeSet::new();
    let mut artwork_digests: BTreeMap<String, String> = BTreeMap::new();

    if contract.schema != COLORING_BOOK_SCHEMA_V1 {
        error(
            &mut findings,
            "coloring.schema.unsupported",
            "$.schema",
            "supported schema is renderflow.coloring-book/v1",
        );
    }
    required(
        &mut findings,
        &contract.book_id,
        "$.book_id",
        "coloring.book_id.empty",
    );
    required(
        &mut findings,
        &contract.version,
        "$.version",
        "coloring.version.empty",
    );
    required(
        &mut findings,
        &contract.title,
        "$.title",
        "coloring.title.empty",
    );
    required(
        &mut findings,
        &contract.language,
        "$.language",
        "coloring.language.empty",
    );
    required(
        &mut findings,
        &contract.audience.label,
        "$.audience.label",
        "coloring.audience.label_empty",
    );
    required(
        &mut findings,
        &contract.audience.complexity,
        "$.audience.complexity",
        "coloring.audience.complexity_empty",
    );
    if contract
        .audience
        .minimum_age
        .zip(contract.audience.maximum_age)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        error(
            &mut findings,
            "coloring.audience.age_range",
            "$.audience",
            "minimum_age must not exceed maximum_age",
        );
    }
    validate_geometry(contract, &mut findings);
    required(
        &mut findings,
        &contract.print.color_space,
        "$.print.color_space",
        "coloring.print.color_space_empty",
    );
    required(
        &mut findings,
        &contract.print.interior_color,
        "$.print.interior_color",
        "coloring.print.interior_color_empty",
    );
    required(
        &mut findings,
        &contract.print.paper,
        "$.print.paper",
        "coloring.print.paper_empty",
    );
    if contract.pagination.expected_page_count as usize != contract.pages.len() {
        error(
            &mut findings,
            "coloring.pagination.count",
            "$.pagination.expected_page_count",
            "expected_page_count must equal the number of declared pages",
        );
    }

    for (index, font) in contract.fonts.iter().enumerate() {
        let path = format!("$.fonts[{index}]");
        validate_file_ref(root, &font.path, &font.sha256, &path, &mut findings)?;
        required(
            &mut findings,
            &font.license,
            &format!("{path}.license"),
            "coloring.font.license_missing",
        );
        if !font.embedding_approved {
            error(
                &mut findings,
                "coloring.font.embedding_unapproved",
                &format!("{path}.embedding_approved"),
                "font embedding must be explicitly approved",
            );
        }
    }

    for (index, page) in contract.pages.iter().enumerate() {
        let path = format!("$.pages[{index}]");
        required(
            &mut findings,
            &page.id,
            &format!("{path}.id"),
            "coloring.page.id_empty",
        );
        if !page_ids.insert(page.id.clone()) {
            error(
                &mut findings,
                "coloring.page.id_duplicate",
                &format!("{path}.id"),
                "page IDs must be unique",
            );
        }
        if !orders.insert(page.order) {
            error(
                &mut findings,
                "coloring.page.order_duplicate",
                &format!("{path}.order"),
                "page order values must be unique",
            );
        }
        if page.order != index as u32 + 1 {
            error(
                &mut findings,
                "coloring.page.order_noncontiguous",
                &format!("{path}.order"),
                "pages must be listed in contiguous one-based order",
            );
        }

        if page.kind == PageKind::IntentionalBlank {
            required_option(
                &mut findings,
                page.blank_intent.as_deref(),
                &format!("{path}.blank_intent"),
                "coloring.page.blank_intent_missing",
            );
            if page.reviewed_text.is_some() || page.artwork.is_some() {
                error(
                    &mut findings,
                    "coloring.page.blank_has_content",
                    &path,
                    "an intentional blank must not declare reviewed text or artwork",
                );
            }
            continue;
        }

        required_option(
            &mut findings,
            page.teaching_objective.as_deref(),
            &format!("{path}.teaching_objective"),
            "coloring.page.objective_missing",
        );
        let reviewed_text = match &page.reviewed_text {
            Some(source) => {
                let digest = validate_file_ref(
                    root,
                    &source.path,
                    &source.sha256,
                    &format!("{path}.reviewed_text"),
                    &mut findings,
                )?;
                required(
                    &mut findings,
                    &source.approval_reference,
                    &format!("{path}.reviewed_text.approval_reference"),
                    "coloring.page.text_unapproved",
                );
                digest.map(|digest| ColoringReviewedTextEvidence {
                    locator: source.path.clone(),
                    digest,
                    approval_reference: source.approval_reference.clone(),
                })
            }
            None => {
                error(
                    &mut findings,
                    "coloring.page.text_missing",
                    &format!("{path}.reviewed_text"),
                    "non-blank pages require reviewed source text evidence",
                );
                None
            }
        };
        match &page.accessibility {
            Some(accessibility) => {
                required(
                    &mut findings,
                    &accessibility.alt_text,
                    &format!("{path}.accessibility.alt_text"),
                    "coloring.accessibility.alt_text_missing",
                );
                required(
                    &mut findings,
                    &accessibility.caption,
                    &format!("{path}.accessibility.caption"),
                    "coloring.accessibility.caption_missing",
                );
                if accessibility.source_links.is_empty() {
                    error(
                        &mut findings,
                        "coloring.accessibility.source_link_missing",
                        &format!("{path}.accessibility.source_links"),
                        "non-blank pages require at least one source link",
                    );
                }
            }
            None => error(
                &mut findings,
                "coloring.accessibility.missing",
                &format!("{path}.accessibility"),
                "non-blank pages require an accessible equivalent",
            ),
        }
        match &page.artwork {
            Some(artwork) => validate_artwork(
                contract,
                page,
                artwork,
                root,
                allow_remote,
                &path,
                reviewed_text,
                &mut artwork_digests,
                &mut artifacts,
                &mut findings,
            )?,
            None => error(
                &mut findings,
                "coloring.page.artwork_missing",
                &format!("{path}.artwork"),
                "non-blank pages require reviewed or explicitly approved artwork",
            ),
        }
    }

    findings.sort_by(|left, right| {
        (&left.path, &left.code, &left.message).cmp(&(&right.path, &right.code, &right.message))
    });
    let invalid = findings
        .iter()
        .any(|finding| finding.severity == ColoringFindingSeverity::Error);
    let contract_digest = digest_json(contract)?;
    let settings_digest = digest_json(&serde_json::json!({
        "validator_id": COLORING_BOOK_VALIDATOR_ID,
        "validator_version": env!("CARGO_PKG_VERSION"),
        "schema": COLORING_BOOK_REPORT_SCHEMA_V1,
        "allow_remote": allow_remote,
        "geometry": contract.geometry,
        "line_art": contract.line_art,
        "pagination": contract.pagination,
        "print": contract.print,
    }))?;
    Ok(ColoringBookValidationReport {
        schema: COLORING_BOOK_REPORT_SCHEMA_V1.to_string(),
        validator_id: COLORING_BOOK_VALIDATOR_ID.to_string(),
        validator_version: env!("CARGO_PKG_VERSION").to_string(),
        contract_digest,
        settings_digest,
        policy: ColoringPolicyEvidence {
            deterministic_offline: true,
            network_accessed: false,
            ai_invoked: false,
            remote_provider_opt_in: allow_remote,
            candidate_output_authoritative: false,
        },
        status: if invalid {
            ColoringValidationStatus::Invalid
        } else {
            ColoringValidationStatus::Valid
        },
        release_eligible: !invalid,
        artifacts,
        findings,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_artwork(
    contract: &ColoringBookContract,
    page: &ColoringBookPage,
    artwork: &ColoringBookArtwork,
    root: &Path,
    allow_remote: bool,
    path: &str,
    reviewed_text: Option<ColoringReviewedTextEvidence>,
    artwork_digests: &mut BTreeMap<String, String>,
    artifacts: &mut Vec<ColoringArtifactEvidence>,
    findings: &mut Vec<ColoringFinding>,
) -> Result<()> {
    let art_path = format!("{path}.artwork");
    let observed = validate_file_ref(root, &artwork.path, &artwork.sha256, &art_path, findings)?;
    if let Some(first_page) = artwork_digests.insert(artwork.sha256.clone(), page.id.clone()) {
        if !page.allow_duplicate_artwork {
            error(findings, "coloring.page.artwork_duplicate", &art_path, &format!("artwork digest duplicates page '{first_page}'; set allow_duplicate_artwork only for an intentional reuse"));
        }
    }
    required(
        findings,
        &artwork.rights.license,
        &format!("{art_path}.rights.license"),
        "coloring.rights.license_missing",
    );
    required(
        findings,
        &artwork.rights.rights_holder,
        &format!("{art_path}.rights.rights_holder"),
        "coloring.rights.holder_missing",
    );
    required(
        findings,
        &artwork.rights.source,
        &format!("{art_path}.rights.source"),
        "coloring.rights.source_missing",
    );
    required(
        findings,
        &artwork.rights.approval_reference,
        &format!("{art_path}.rights.approval_reference"),
        "coloring.rights.approval_missing",
    );
    if !artwork.rights.reviewed {
        error(
            findings,
            "coloring.rights.unreviewed",
            &format!("{art_path}.rights.reviewed"),
            "artwork rights must be explicitly reviewed",
        );
    }
    if matches!(
        contract.intended_use,
        ColoringBookUse::Public | ColoringBookUse::Commercial
    ) && [
        artwork.rights.license.as_str(),
        artwork.rights.rights_holder.as_str(),
        artwork.rights.source.as_str(),
        artwork.rights.approval_reference.as_str(),
    ]
    .iter()
    .any(|value| is_ambiguous(value))
    {
        error(
            findings,
            "coloring.release.rights_ambiguous",
            &format!("{art_path}.rights"),
            "public and commercial release is blocked by placeholder or ambiguous rights evidence",
        );
    }
    if artwork.approval.state != CandidateState::Approved {
        error(
            findings,
            "coloring.artwork.candidate_unapproved",
            &format!("{art_path}.approval.state"),
            "candidate artwork is never authoritative without explicit approval",
        );
    }
    required_option(
        findings,
        artwork.approval.reference.as_deref(),
        &format!("{art_path}.approval.reference"),
        "coloring.artwork.approval_missing",
    );
    if artwork.approval.approved_sha256.as_deref() != Some(artwork.sha256.as_str()) {
        error(
            findings,
            "coloring.artwork.approved_digest_mismatch",
            &format!("{art_path}.approval.approved_sha256"),
            "approval must bind the exact artwork digest",
        );
    }
    let inspection = &artwork.inspection;
    if inspection.effective_dpi < contract.line_art.minimum_dpi {
        error(
            findings,
            "coloring.line_art.dpi",
            &format!("{art_path}.inspection.effective_dpi"),
            "effective DPI is below the profile minimum",
        );
    }
    if inspection.contrast_ratio < contract.line_art.minimum_contrast_ratio {
        error(
            findings,
            "coloring.line_art.contrast",
            &format!("{art_path}.inspection.contrast_ratio"),
            "contrast ratio is below the profile minimum",
        );
    }
    if inspection.minimum_stroke_pt < contract.line_art.minimum_stroke_pt {
        error(
            findings,
            "coloring.line_art.stroke",
            &format!("{art_path}.inspection.minimum_stroke_pt"),
            "observed line weight is below the profile minimum",
        );
    }
    for (field, verified) in [
        ("trim_verified", inspection.trim_verified),
        ("bleed_verified", inspection.bleed_verified),
        ("safe_area_verified", inspection.safe_area_verified),
    ] {
        if !verified {
            error(
                findings,
                "coloring.line_art.geometry_unverified",
                &format!("{art_path}.inspection.{field}"),
                "trim, bleed, and safe-area evidence must be explicit",
            );
        }
    }

    match artwork.origin {
        ArtworkOrigin::ReviewedSource if artwork.generation.is_some() => error(findings, "coloring.artwork.unexpected_generation", &format!("{art_path}.generation"), "reviewed-source artwork must not claim generator provenance"),
        ArtworkOrigin::ReviewedSource => {}
        ArtworkOrigin::GeneratedCandidate => match &artwork.generation {
            None => error(findings, "coloring.artwork.generation_missing", &format!("{art_path}.generation"), "generated candidates require provider, model, skill, prompt, and settings provenance"),
            Some(generation) => {
                for (field, value) in [("candidate_id", generation.candidate_id.as_str()), ("provider_id", generation.provider_id.as_str()), ("model_id", generation.model_id.as_str()), ("skill_id", generation.skill_id.as_str()), ("skill_version", generation.skill_version.as_str())] {
                    required(findings, value, &format!("{art_path}.generation.{field}"), "coloring.artwork.generation_incomplete");
                }
                validate_sha256(findings, &generation.settings_sha256, &format!("{art_path}.generation.settings_sha256"));
                validate_sha256(findings, &generation.prompt_sha256, &format!("{art_path}.generation.prompt_sha256"));
                if generation.provider_locality == ProviderLocality::Remote && !allow_remote {
                    error(findings, "coloring.provider.remote_opt_in_required", &format!("{art_path}.generation.provider_locality"), "remote provider provenance requires the explicit --allow-remote review opt-in");
                }
                let hygiene = &contract.hygiene;
                for (field, reviewed) in [("privacy_reviewed", hygiene.privacy_reviewed), ("copyright_reviewed", hygiene.copyright_reviewed), ("protected_references_reviewed", hygiene.protected_references_reviewed), ("prompt_hygiene_reviewed", hygiene.prompt_hygiene_reviewed)] {
                    if !reviewed {
                        error(findings, "coloring.hygiene.review_required", &format!("$.hygiene.{field}"), "generated candidates require all privacy, copyright, protected-reference, and prompt-hygiene reviews");
                    }
                }
            }
        },
    }

    if matches!(
        contract.intended_use,
        ColoringBookUse::Public | ColoringBookUse::Commercial
    ) && !artwork.rights.reviewed
    {
        error(
            findings,
            "coloring.release.rights_blocked",
            &format!("{art_path}.rights"),
            "public and commercial release is blocked without reviewed rights evidence",
        );
    }
    if let Some(digest) = observed {
        artifacts.push(ColoringArtifactEvidence {
            page_id: page.id.clone(),
            role: format!("pages/{}/line-art", page.id),
            reviewed_text,
            locator: artwork.path.clone(),
            digest,
            origin: artwork.origin,
            provider_id: artwork
                .generation
                .as_ref()
                .map(|value| value.provider_id.clone()),
            model_id: artwork
                .generation
                .as_ref()
                .map(|value| value.model_id.clone()),
            candidate_state: artwork.approval.state,
            approval_reference: artwork.approval.reference.clone(),
            rights: artwork.rights.clone(),
        });
    }
    Ok(())
}

fn validate_geometry(contract: &ColoringBookContract, findings: &mut Vec<ColoringFinding>) {
    let geometry = &contract.geometry;
    if geometry.width_mm <= 0.0 || geometry.height_mm <= 0.0 {
        error(
            findings,
            "coloring.geometry.trim",
            "$.geometry",
            "trim width and height must be greater than zero",
        );
    }
    if geometry.margin_mm < 0.0 || geometry.bleed_mm < 0.0 || geometry.safe_area_mm < 0.0 {
        error(
            findings,
            "coloring.geometry.negative",
            "$.geometry",
            "margin, bleed, and safe area must not be negative",
        );
    }
    if geometry.margin_mm + geometry.safe_area_mm >= geometry.width_mm / 2.0
        || geometry.margin_mm + geometry.safe_area_mm >= geometry.height_mm / 2.0
    {
        error(
            findings,
            "coloring.geometry.safe_area",
            "$.geometry.safe_area_mm",
            "margin plus safe area must leave a positive content region",
        );
    }
    if contract.line_art.minimum_dpi == 0
        || contract.line_art.minimum_stroke_pt <= 0.0
        || contract.line_art.minimum_contrast_ratio < 1.0
    {
        error(
            findings,
            "coloring.line_art.policy_invalid",
            "$.line_art",
            "DPI and stroke must be positive and contrast ratio must be at least 1.0",
        );
    }
}

fn validate_file_ref(
    root: &Path,
    locator: &str,
    expected: &str,
    path: &str,
    findings: &mut Vec<ColoringFinding>,
) -> Result<Option<DigestEvidence>> {
    validate_sha256(findings, expected, &format!("{path}.sha256"));
    let relative = Path::new(locator);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        error(
            findings,
            "coloring.asset.path_unsafe",
            &format!("{path}.path"),
            "asset paths must be relative and contained by the contract directory",
        );
        return Ok(None);
    }
    let resolved: PathBuf = root.join(relative);
    let canonical_root = fs::canonicalize(root).with_context(|| {
        format!(
            "failed to resolve coloring-book contract directory '{}'",
            root.display()
        )
    })?;
    let canonical_asset = match fs::canonicalize(&resolved) {
        Ok(path) => path,
        Err(_) => {
            error(
                findings,
                "coloring.asset.missing",
                &format!("{path}.path"),
                &format!(
                    "referenced asset '{}' could not be read",
                    resolved.display()
                ),
            );
            return Ok(None);
        }
    };
    if !canonical_asset.starts_with(&canonical_root) {
        error(
            findings,
            "coloring.asset.path_unsafe",
            &format!("{path}.path"),
            "asset symlinks must remain contained by the contract directory",
        );
        return Ok(None);
    }
    let bytes = fs::read(&canonical_asset).with_context(|| {
        format!(
            "failed to read coloring-book asset '{}'",
            canonical_asset.display()
        )
    })?;
    let observed = format!("{:x}", Sha256::digest(&bytes));
    if observed != expected {
        error(
            findings,
            "coloring.asset.digest_mismatch",
            &format!("{path}.sha256"),
            "declared SHA-256 does not match the referenced bytes",
        );
    }
    Ok(Some(DigestEvidence {
        algorithm: "sha256".to_string(),
        value: observed,
    }))
}

fn validate_sha256(findings: &mut Vec<ColoringFinding>, value: &str, path: &str) {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        error(
            findings,
            "coloring.digest.invalid",
            path,
            "digest must be 64 lowercase hexadecimal SHA-256 characters",
        );
    }
}

fn digest_json(value: &impl Serialize) -> Result<DigestEvidence> {
    Ok(DigestEvidence {
        algorithm: "sha256".to_string(),
        value: format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)),
    })
}

fn required(findings: &mut Vec<ColoringFinding>, value: &str, path: &str, code: &str) {
    if value.trim().is_empty() {
        error(findings, code, path, "value must not be empty");
    }
}

fn required_option(
    findings: &mut Vec<ColoringFinding>,
    value: Option<&str>,
    path: &str,
    code: &str,
) {
    if value.is_none_or(|value| value.trim().is_empty()) {
        error(findings, code, path, "value must be present and non-empty");
    }
}

fn is_ambiguous(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "unknown" | "tbd" | "unspecified" | "pending" | "none"
    )
}

fn error(findings: &mut Vec<ColoringFinding>, code: &str, path: &str, message: &str) {
    findings.push(ColoringFinding {
        severity: ColoringFindingSeverity::Error,
        code: code.to_string(),
        path: path.to_string(),
        message: message.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (ColoringBookContract, PathBuf) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/coloring-book/book.yaml");
        let contract = serde_yaml_ng::from_slice(&fs::read(&path).unwrap()).unwrap();
        (contract, path.parent().unwrap().to_path_buf())
    }

    #[test]
    fn synthetic_local_fixture_is_valid_and_deterministic() {
        let (contract, root) = fixture();
        let first = validate_coloring_book(&contract, &root, false).unwrap();
        let second = validate_coloring_book(&contract, &root, false).unwrap();
        assert!(first.is_valid(), "{:?}", first.findings);
        assert!(first.release_eligible);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert!(first.policy.deterministic_offline);
        assert!(!first.policy.network_accessed);
        assert!(!first.policy.ai_invoked);
        assert!(!first.policy.candidate_output_authoritative);
    }

    #[test]
    fn bundled_profile_is_offline_and_exposes_release_roles() {
        let profile: crate::spec::DerivativeProfile =
            serde_yaml_ng::from_str(include_str!("../../data/profiles/coloring-book-v1.yaml"))
                .unwrap();
        let roles = profile
            .targets
            .iter()
            .filter_map(|target| target.role.as_deref())
            .collect::<BTreeSet<_>>();
        assert!(roles.contains("print/book"));
        assert!(roles.contains("print/proof"));
        assert!(roles.contains("pages/raster"));
        assert!(roles.contains("pages/vector"));
        assert_eq!(
            profile.policy.network,
            Some(crate::spec::NetworkPolicy::Deny)
        );
        assert_eq!(profile.policy.ai, Some(crate::spec::AiPolicy::Deny));
    }

    #[test]
    fn candidate_and_remote_provider_require_explicit_approval() {
        let (mut contract, root) = fixture();
        let artwork = contract.pages[0].artwork.as_mut().unwrap();
        artwork.origin = ArtworkOrigin::GeneratedCandidate;
        artwork.approval.state = CandidateState::Candidate;
        artwork.generation = Some(GenerationEvidence {
            candidate_id: "candidate-1".to_string(),
            provider_id: "provider.example".to_string(),
            provider_locality: ProviderLocality::Remote,
            model_id: "model.example".to_string(),
            skill_id: "skill.line-art".to_string(),
            skill_version: "1.0.0".to_string(),
            settings_sha256: "a".repeat(64),
            prompt_sha256: "b".repeat(64),
        });
        let report = validate_coloring_book(&contract, &root, false).unwrap();
        assert!(!report.is_valid());
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "coloring.artwork.candidate_unapproved"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "coloring.provider.remote_opt_in_required"));
    }

    #[test]
    fn ambiguous_rights_block_public_release() {
        let (mut contract, root) = fixture();
        contract.intended_use = ColoringBookUse::Public;
        contract.pages[0].artwork.as_mut().unwrap().rights.license = "TBD".to_string();
        let report = validate_coloring_book(&contract, &root, false).unwrap();
        assert!(!report.release_eligible);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "coloring.release.rights_ambiguous"));
    }
}
