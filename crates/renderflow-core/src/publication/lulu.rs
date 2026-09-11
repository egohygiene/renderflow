//! Pinned, offline Lulu candidate rules and provider-specific preflight evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ebook::{inspect_ebook, EbookDiagnosticSeverity, EbookLayout};
use crate::publication::{PublicationContract, PUBLICATION_CONTRACT_V1};
use crate::spec::load_spec;

pub const LULU_RULES_SCHEMA_V1: &str = "renderflow.lulu-rules/v1";
pub const LULU_REQUEST_SCHEMA_V1: &str = "renderflow.lulu-request/v1";
pub const LULU_REPORT_SCHEMA_V1: &str = "renderflow.lulu-conformance/v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluRuleSource {
    pub url: String,
    pub observed_modified: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluBarcodeRules {
    pub width_mm: f64,
    pub height_mm: f64,
    pub edge_clearance_mm: f64,
    pub black_on_white: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluPrintRules {
    pub recommended_max_bytes: u64,
    pub amazon_minimum_color_pages: u32,
    pub spine_text_minimum_pages: u32,
    pub maximum_consecutive_blank_pages: u32,
    pub barcode: LuluBarcodeRules,
    pub required_distribution_attestations: Vec<String>,
    pub amazon_excluded_formats: Vec<String>,
    pub ingram_excluded_formats: Vec<String>,
    pub amazon_excluded_bindings: Vec<String>,
    pub amazon_excluded_papers: Vec<String>,
    pub global_distribution_review_minimum_weeks: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluEbookRules {
    pub epubcheck_major: u32,
    pub epub_version: String,
    pub recommended_max_bytes: u64,
    pub rejection_risk_bytes: u64,
    pub global_distribution_languages: Vec<String>,
    pub cover_width_px: u32,
    pub cover_height_px: u32,
    pub cover_minimum_ppi: u32,
    pub cover_maximum_ppi: u32,
    pub apple_max_image_pixels: u64,
    pub required_distribution_attestations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluRulePack {
    pub schema: String,
    pub id: String,
    pub observed_on: String,
    pub sources: Vec<LuluRuleSource>,
    pub print: LuluPrintRules,
    pub ebook: LuluEbookRules,
}

impl LuluRulePack {
    pub fn builtin() -> Result<Self> {
        let rules: Self = serde_yaml_ng::from_str(include_str!(
            "../../data/publication/lulu-rules-2026-09-11.yaml"
        ))
        .context("bundled Lulu rule pack is invalid")?;
        if rules.schema != LULU_RULES_SCHEMA_V1 {
            anyhow::bail!(
                "unsupported Lulu rule schema '{}'; expected '{}'",
                rules.schema,
                LULU_RULES_SCHEMA_V1
            );
        }
        Ok(rules)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LuluChannel {
    PrintDirect,
    LuluBookstore,
    GlobalDistribution,
    PdfEbook,
    EpubDistribution,
    Amazon,
    Ingram,
    BarnesAndNoble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LuluEligibility {
    Eligible,
    Ineligible,
    Unknown,
    NotRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LuluFindingSeverity {
    Info,
    Warning,
    Error,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluFinding {
    pub code: String,
    pub severity: LuluFindingSeverity,
    pub message: String,
    pub channels: Vec<LuluChannel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluProductRequest {
    pub format: String,
    pub binding: String,
    pub paper: String,
    pub color: String,
    pub page_count: u32,
    pub trim_width_mm: f64,
    pub trim_height_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluArtifactRequest {
    #[serde(default)]
    pub interior_pdf: Option<String>,
    #[serde(default)]
    pub bookstore_cover_pdf: Option<String>,
    #[serde(default)]
    pub distribution_cover_pdf: Option<String>,
    #[serde(default)]
    pub bookstore_cover_template: Option<String>,
    #[serde(default)]
    pub distribution_cover_template: Option<String>,
    #[serde(default)]
    pub pdf_ebook: Option<String>,
    #[serde(default)]
    pub epub: Option<String>,
    #[serde(default)]
    pub ebook_cover: Option<String>,
    #[serde(default)]
    pub print_proof_pdf: Option<String>,
    #[serde(default)]
    pub proof_contact_sheet: Option<String>,
    #[serde(default)]
    pub ace_report: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluMetadataRequest {
    #[serde(default)]
    pub print_isbn: Option<String>,
    #[serde(default)]
    pub ebook_isbn: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub proof_approved: bool,
    #[serde(default)]
    pub fixed_layout_compatibility_verified: bool,
    #[serde(default)]
    pub accessible_epub_claimed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluPreflightRequest {
    pub schema: String,
    pub rule_pack: String,
    pub publication_spec: String,
    pub channels: Vec<LuluChannel>,
    pub product: LuluProductRequest,
    #[serde(default)]
    pub artifacts: LuluArtifactRequest,
    #[serde(default)]
    pub metadata: LuluMetadataRequest,
    #[serde(default)]
    pub attestations: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluArtifactEvidence {
    pub role: String,
    pub path: String,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub inspected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluChannelResult {
    pub channel: LuluChannel,
    pub eligibility: LuluEligibility,
    pub blocking_findings: Vec<String>,
    pub warning_findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuluConformanceReport {
    pub schema: String,
    pub provider_id: String,
    pub rule_pack: String,
    pub observed_on: String,
    pub deterministic_offline_evaluation: bool,
    pub publication_schema: String,
    pub publication_spec_sha256: String,
    pub artifacts: Vec<LuluArtifactEvidence>,
    pub channels: Vec<LuluChannelResult>,
    pub findings: Vec<LuluFinding>,
    pub sources: Vec<LuluRuleSource>,
    pub upload_performed: bool,
}

pub fn load_request(path: &Path) -> Result<LuluPreflightRequest> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read Lulu request '{}'", path.display()))?;
    let request: LuluPreflightRequest = serde_yaml_ng::from_slice(&bytes)
        .with_context(|| format!("failed to parse Lulu request '{}'", path.display()))?;
    if request.schema != LULU_REQUEST_SCHEMA_V1 {
        anyhow::bail!(
            "unsupported Lulu request schema '{}'; expected '{}'",
            request.schema,
            LULU_REQUEST_SCHEMA_V1
        );
    }
    Ok(request)
}

pub fn evaluate_request(request_path: &Path, run_epubcheck: bool) -> Result<LuluConformanceReport> {
    let rules = LuluRulePack::builtin()?;
    let request = load_request(request_path)?;
    if request.rule_pack != rules.id {
        anyhow::bail!(
            "Lulu request pins rule pack '{}' but this build provides '{}'; refusing to substitute changing rules",
            request.rule_pack,
            rules.id
        );
    }
    let root = request_path.parent().unwrap_or_else(|| Path::new("."));
    let publication_path = root.join(&request.publication_spec);
    let publication_spec_sha256 = format!(
        "{:x}",
        Sha256::digest(fs::read(&publication_path).with_context(|| {
            format!(
                "failed to read publication spec '{}'",
                publication_path.display()
            )
        })?)
    );
    let loaded = load_spec(
        publication_path
            .to_str()
            .context("non-UTF8 publication path")?,
    )?;
    let publication = loaded
        .spec
        .publication
        .context("the referenced Renderflow spec has no publication contract")?;
    evaluate(
        &rules,
        &request,
        &publication,
        &publication_spec_sha256,
        root,
        run_epubcheck,
    )
}

fn evaluate(
    rules: &LuluRulePack,
    request: &LuluPreflightRequest,
    publication: &PublicationContract,
    publication_spec_sha256: &str,
    root: &Path,
    run_epubcheck: bool,
) -> Result<LuluConformanceReport> {
    let requested = request.channels.iter().copied().collect::<BTreeSet<_>>();
    let mut findings = Vec::new();
    let mut artifacts = Vec::new();
    for (role, path) in artifact_paths(&request.artifacts) {
        artifacts.push(inspect_artifact(role, path, root));
    }

    require(
        publication.schema == PUBLICATION_CONTRACT_V1,
        "lulu.publication.schema",
        "The publication contract must use renderflow.publication/v1",
        LuluFindingSeverity::Error,
        all_channels(),
        &mut findings,
    );
    require(
        request.product.page_count > 0,
        "lulu.print.page_count",
        "A positive interior page count is required",
        LuluFindingSeverity::Error,
        print_channels(),
        &mut findings,
    );
    validate_declared_geometry(request, publication, &mut findings);
    validate_print(rules, request, root, &requested, &mut findings)?;
    validate_ebook(
        rules,
        request,
        publication,
        root,
        run_epubcheck,
        &requested,
        &mut findings,
    )?;
    validate_distribution_metadata(request, publication, &requested, &mut findings);

    let channels = all_channels()
        .into_iter()
        .map(|channel| channel_result(channel, requested.contains(&channel), &findings))
        .collect();
    Ok(LuluConformanceReport {
        schema: LULU_REPORT_SCHEMA_V1.to_string(),
        provider_id: "adapter.publication.lulu".to_string(),
        rule_pack: rules.id.clone(),
        observed_on: rules.observed_on.clone(),
        deterministic_offline_evaluation: true,
        publication_schema: publication.schema.clone(),
        publication_spec_sha256: publication_spec_sha256.to_string(),
        artifacts,
        channels,
        findings,
        sources: rules.sources.clone(),
        upload_performed: false,
    })
}

fn validate_print(
    rules: &LuluRulePack,
    request: &LuluPreflightRequest,
    root: &Path,
    requested: &BTreeSet<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) -> Result<()> {
    if !requested
        .iter()
        .any(|channel| print_channels().contains(channel))
    {
        return Ok(());
    }
    require_path(
        request.artifacts.interior_pdf.as_deref(),
        root,
        "lulu.print.interior_pdf",
        "A single-page-layout interior PDF is required",
        print_channels(),
        findings,
    );
    validate_pdf(
        request.artifacts.interior_pdf.as_deref(),
        root,
        "interior",
        print_channels(),
        findings,
    )?;
    if let Some(path) = resolved_existing(request.artifacts.interior_pdf.as_deref(), root) {
        let size = fs::metadata(path)?.len();
        if size >= rules.print.recommended_max_bytes {
            finding(
                "lulu.print.file_size.recommended",
                LuluFindingSeverity::Warning,
                "Interior PDF is at or above Lulu's pinned 500 MB recommendation",
                print_channels(),
                findings,
            );
        }
    }
    if requested.contains(&LuluChannel::LuluBookstore) {
        validate_cover_pair(
            "bookstore",
            request.artifacts.bookstore_cover_pdf.as_deref(),
            request.artifacts.bookstore_cover_template.as_deref(),
            root,
            vec![LuluChannel::LuluBookstore],
            findings,
        )?;
    }
    if requested.contains(&LuluChannel::GlobalDistribution)
        || requested.contains(&LuluChannel::Amazon)
        || requested.contains(&LuluChannel::Ingram)
    {
        validate_cover_pair(
            "distribution",
            request.artifacts.distribution_cover_pdf.as_deref(),
            request.artifacts.distribution_cover_template.as_deref(),
            root,
            vec![
                LuluChannel::GlobalDistribution,
                LuluChannel::Amazon,
                LuluChannel::Ingram,
            ],
            findings,
        )?;
        for attestation in &rules.print.required_distribution_attestations {
            require_attestation(
                attestation,
                request,
                print_distribution_channels(),
                findings,
            );
        }
        require(
            request.metadata.proof_approved,
            "lulu.print.proof_approval",
            "Global Distribution requires creator approval of a print proof",
            LuluFindingSeverity::Error,
            print_distribution_channels(),
            findings,
        );
        require_path(
            request.artifacts.print_proof_pdf.as_deref(),
            root,
            "lulu.print.proof_pdf",
            "A reviewed print proof PDF is required for distribution evidence",
            print_distribution_channels(),
            findings,
        );
        require_path(
            request.artifacts.proof_contact_sheet.as_deref(),
            root,
            "lulu.print.proof_contact_sheet",
            "A proof contact sheet is required for distribution evidence",
            print_distribution_channels(),
            findings,
        );
        finding(
            "lulu.distribution.timeline",
            LuluFindingSeverity::Info,
            &format!(
                "Pinned Lulu guidance says Global Distribution can take at least {} weeks",
                rules.print.global_distribution_review_minimum_weeks
            ),
            print_distribution_channels(),
            findings,
        );
    }
    let color = request.product.color != "black_and_white";
    if requested.contains(&LuluChannel::Amazon)
        && color
        && request.product.page_count < rules.print.amazon_minimum_color_pages
    {
        finding(
            "lulu.amazon.color_page_minimum",
            LuluFindingSeverity::Error,
            &format!(
                "Amazon requires at least {} pages for Standard or Premium Color; this request declares {}",
                rules.print.amazon_minimum_color_pages, request.product.page_count
            ),
            vec![LuluChannel::Amazon],
            findings,
        );
    }
    if requested.contains(&LuluChannel::Amazon)
        && rules
            .print
            .amazon_excluded_formats
            .contains(&request.product.format)
    {
        finding(
            "lulu.amazon.product_format_excluded",
            LuluFindingSeverity::Error,
            "The selected product format is excluded from Amazon in the pinned Lulu table",
            vec![LuluChannel::Amazon],
            findings,
        );
    }
    if requested.contains(&LuluChannel::Amazon)
        && rules
            .print
            .amazon_excluded_papers
            .contains(&request.product.paper)
    {
        finding(
            "lulu.amazon.paper_excluded",
            LuluFindingSeverity::Error,
            "The selected 80# paper is excluded from Amazon in the pinned Lulu table",
            vec![LuluChannel::Amazon],
            findings,
        );
    }
    if request.product.page_count < rules.print.spine_text_minimum_pages
        && request.attestations.get("spine_text_present") == Some(&true)
    {
        finding(
            "lulu.print.spine_text_below_minimum",
            LuluFindingSeverity::Error,
            &format!(
                "Spine text is not permitted below {} pages in the pinned Lulu rules",
                rules.print.spine_text_minimum_pages
            ),
            print_channels(),
            findings,
        );
    }
    if requested.contains(&LuluChannel::LuluBookstore)
        && requested.iter().any(|channel| {
            matches!(
                channel,
                LuluChannel::GlobalDistribution | LuluChannel::Amazon | LuluChannel::Ingram
            )
        })
        && request.artifacts.bookstore_cover_template
            == request.artifacts.distribution_cover_template
    {
        finding(
            "lulu.print.cover_templates.not_distinct",
            LuluFindingSeverity::Unknown,
            "Bookstore and distribution cover templates are identical; their spine geometries must be independently verified",
            vec![
                LuluChannel::LuluBookstore,
                LuluChannel::GlobalDistribution,
                LuluChannel::Amazon,
                LuluChannel::Ingram,
            ],
            findings,
        );
    }
    if requested.contains(&LuluChannel::Amazon)
        && rules
            .print
            .amazon_excluded_bindings
            .contains(&request.product.binding)
    {
        finding(
            "lulu.amazon.binding_excluded",
            LuluFindingSeverity::Error,
            "The selected binding is excluded from Amazon in the pinned Lulu table",
            vec![LuluChannel::Amazon],
            findings,
        );
    }
    if requested.contains(&LuluChannel::Ingram)
        && rules
            .print
            .ingram_excluded_formats
            .contains(&request.product.format)
    {
        finding(
            "lulu.ingram.product_format_excluded",
            LuluFindingSeverity::Error,
            "The selected product format is excluded from Ingram in the pinned Lulu table",
            vec![LuluChannel::Ingram],
            findings,
        );
    }
    if requested.contains(&LuluChannel::BarnesAndNoble) {
        finding(
            "lulu.barnes_and_noble.not_inferred",
            LuluFindingSeverity::Unknown,
            "Ingram availability does not establish Barnes & Noble listing or shelf placement",
            vec![LuluChannel::BarnesAndNoble],
            findings,
        );
    }
    Ok(())
}

fn validate_ebook(
    rules: &LuluRulePack,
    request: &LuluPreflightRequest,
    publication: &PublicationContract,
    root: &Path,
    run_epubcheck: bool,
    requested: &BTreeSet<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) -> Result<()> {
    if requested.contains(&LuluChannel::PdfEbook) {
        require_path(
            request.artifacts.pdf_ebook.as_deref(),
            root,
            "lulu.ebook.pdf",
            "The Lulu PDF ebook channel requires a PDF candidate",
            vec![LuluChannel::PdfEbook],
            findings,
        );
        validate_pdf(
            request.artifacts.pdf_ebook.as_deref(),
            root,
            "ebook",
            vec![LuluChannel::PdfEbook],
            findings,
        )?;
        finding(
            "lulu.ebook.pdf.accessibility_not_inferred",
            LuluFindingSeverity::Info,
            "A PDF ebook is not automatically treated as accessible",
            vec![LuluChannel::PdfEbook],
            findings,
        );
    }
    if !requested.contains(&LuluChannel::EpubDistribution) {
        return Ok(());
    }
    let channels = vec![LuluChannel::EpubDistribution];
    require_path(
        request.artifacts.epub.as_deref(),
        root,
        "lulu.epub.file",
        "EPUB Distribution requires an EPUB candidate",
        channels.clone(),
        findings,
    );
    require_path(
        request.artifacts.ebook_cover.as_deref(),
        root,
        "lulu.epub.cover",
        "EPUB Distribution requires a separate flat retailer cover image",
        channels.clone(),
        findings,
    );
    require(
        rules
            .ebook
            .global_distribution_languages
            .contains(&publication.language),
        "lulu.epub.language",
        "The pinned Lulu rule pack only verifies listed English language tags for EPUB distribution",
        LuluFindingSeverity::Error,
        channels.clone(),
        findings,
    );
    for attestation in &rules.ebook.required_distribution_attestations {
        require_attestation(attestation, request, channels.clone(), findings);
    }
    if let Some(path) = resolved_existing(request.artifacts.epub.as_deref(), root) {
        let inspection = inspect_ebook(&path, run_epubcheck)?;
        for diagnostic in inspection.diagnostics {
            finding(
                &format!("lulu.{}", diagnostic.code),
                match diagnostic.severity {
                    EbookDiagnosticSeverity::Info => LuluFindingSeverity::Info,
                    EbookDiagnosticSeverity::Warning => LuluFindingSeverity::Warning,
                    EbookDiagnosticSeverity::Error => LuluFindingSeverity::Error,
                },
                &diagnostic.message,
                channels.clone(),
                findings,
            );
        }
        require(
            inspection.epub_version.as_deref() == Some(rules.ebook.epub_version.as_str()),
            "lulu.epub.version",
            "EPUB Distribution requires the pinned EPUB 3.3 profile",
            LuluFindingSeverity::Error,
            channels.clone(),
            findings,
        );
        if inspection.layout == EbookLayout::PrePaginated
            && !request.metadata.fixed_layout_compatibility_verified
        {
            finding(
                "lulu.epub.fixed_layout_unverified",
                LuluFindingSeverity::Unknown,
                "Fixed-layout retailer compatibility was not explicitly verified",
                channels.clone(),
                findings,
            );
        }
        if run_epubcheck {
            match inspection.epubcheck {
                Some(check)
                    if check.passed == Some(true)
                        && check.version.as_deref().is_some_and(|version| {
                            version.contains(&format!("{}.", rules.ebook.epubcheck_major))
                        }) => {}
                Some(check) if check.passed == Some(true) => finding(
                    "lulu.epub.epubcheck_version",
                    LuluFindingSeverity::Unknown,
                    "EPUBCheck passed, but its reported version does not verify the pinned v5 requirement",
                    channels.clone(),
                    findings,
                ),
                Some(check) if !check.available => finding(
                    "lulu.epub.epubcheck_unavailable",
                    LuluFindingSeverity::Unknown,
                    "EPUBCheck v5 was requested but is unavailable",
                    channels.clone(),
                    findings,
                ),
                _ => finding(
                    "lulu.epub.epubcheck_failed",
                    LuluFindingSeverity::Error,
                    "EPUBCheck v5 did not pass",
                    channels.clone(),
                    findings,
                ),
            }
        } else {
            finding(
                "lulu.epub.epubcheck_not_run",
                LuluFindingSeverity::Unknown,
                "EPUBCheck v5 evidence is required before upload-ready status",
                channels.clone(),
                findings,
            );
        }
        let size = fs::metadata(path)?.len();
        if size >= rules.ebook.rejection_risk_bytes {
            finding(
                "lulu.epub.file_size.rejection_risk",
                LuluFindingSeverity::Error,
                "EPUB is at or above the pinned 500 MB rejection-risk threshold",
                channels.clone(),
                findings,
            );
        } else if size >= rules.ebook.recommended_max_bytes {
            finding(
                "lulu.epub.file_size.recommended",
                LuluFindingSeverity::Warning,
                "EPUB is at or above Lulu's pinned 50 MB recommendation",
                channels.clone(),
                findings,
            );
        }
        if request.metadata.accessible_epub_claimed
            && inspection.accessibility.accessibility_features == 0
            && !inspection.accessibility.conforms_to
        {
            finding(
                "lulu.epub.accessibility_unverified",
                LuluFindingSeverity::Unknown,
                "The EPUB is claimed accessible but structural accessibility evidence is sparse; attach Ace by DAISY evidence",
                channels.clone(),
                findings,
            );
        }
        if request.metadata.accessible_epub_claimed {
            require_path(
                request.artifacts.ace_report.as_deref(),
                root,
                "lulu.epub.accessibility.ace_report",
                "An Ace by DAISY report is required to support the accessibility claim",
                channels,
                findings,
            );
        }
    }
    Ok(())
}

fn validate_distribution_metadata(
    request: &LuluPreflightRequest,
    publication: &PublicationContract,
    requested: &BTreeSet<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) {
    let print = vec![
        LuluChannel::GlobalDistribution,
        LuluChannel::Amazon,
        LuluChannel::Ingram,
    ];
    if requested.iter().any(|channel| print.contains(channel)) {
        require(
            valid_isbn13(request.metadata.print_isbn.as_deref()),
            "lulu.print.isbn",
            "A valid 13-digit print ISBN is required for distribution",
            LuluFindingSeverity::Error,
            print.clone(),
            findings,
        );
    }
    if requested.contains(&LuluChannel::EpubDistribution) {
        require(
            valid_undashed_isbn13(request.metadata.ebook_isbn.as_deref()),
            "lulu.epub.isbn",
            "A separate valid 13-digit ebook ISBN is required for distribution",
            LuluFindingSeverity::Error,
            vec![LuluChannel::EpubDistribution],
            findings,
        );
    }
    let distribution_requested = requested.contains(&LuluChannel::GlobalDistribution)
        || requested.contains(&LuluChannel::EpubDistribution);
    if distribution_requested {
        let description = request.metadata.description.as_deref().unwrap_or_default();
        require(
            description
                .chars()
                .filter(|character| !character.is_whitespace())
                .count()
                >= 50,
            "lulu.metadata.description",
            "Distribution description must contain at least 50 non-whitespace characters",
            LuluFindingSeverity::Error,
            vec![
                LuluChannel::GlobalDistribution,
                LuluChannel::EpubDistribution,
            ],
            findings,
        );
        require(
            publication.title.len() + publication.subtitle.as_deref().map(str::len).unwrap_or(0)
                <= 200,
            "lulu.metadata.title_length",
            "Combined title and subtitle must not exceed 200 characters",
            LuluFindingSeverity::Error,
            vec![
                LuluChannel::GlobalDistribution,
                LuluChannel::EpubDistribution,
            ],
            findings,
        );
        for keyword in &request.metadata.keywords {
            if keyword.chars().count() >= 50 {
                finding(
                    "lulu.metadata.keyword_length",
                    LuluFindingSeverity::Warning,
                    "A keyword is 50 characters or longer and may be removed by distribution partners",
                    vec![LuluChannel::GlobalDistribution, LuluChannel::EpubDistribution],
                    findings,
                );
            }
        }
    }
}

fn validate_cover_pair(
    label: &str,
    cover: Option<&str>,
    template: Option<&str>,
    root: &Path,
    channels: Vec<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) -> Result<()> {
    require(
        cover != template,
        &format!("lulu.print.{label}_cover_template_distinct"),
        "The cover candidate and user-supplied template must be separate files",
        LuluFindingSeverity::Error,
        channels.clone(),
        findings,
    );
    require_path(
        cover,
        root,
        &format!("lulu.print.{label}_cover"),
        "A separate one-piece cover PDF is required",
        channels.clone(),
        findings,
    );
    validate_pdf(
        cover,
        root,
        &format!("{label} cover"),
        channels.clone(),
        findings,
    )?;
    require_path(
        template,
        root,
        &format!("lulu.print.{label}_template"),
        "A current user-supplied Lulu cover template is required; geometry is never guessed",
        channels,
        findings,
    );
    Ok(())
}

fn validate_pdf(
    path: Option<&str>,
    root: &Path,
    label: &str,
    channels: Vec<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) -> Result<()> {
    let Some(path) = resolved_existing(path, root) else {
        return Ok(());
    };
    let bytes = fs::read(path)?;
    require(
        bytes.starts_with(b"%PDF-")
            && bytes
                .windows(5)
                .rev()
                .take(2048)
                .any(|item| item == b"%%EOF"),
        &format!("lulu.pdf.{label}.envelope"),
        &format!("The {label} PDF envelope is invalid"),
        LuluFindingSeverity::Error,
        channels,
        findings,
    );
    Ok(())
}

fn require_attestation(
    id: &str,
    request: &LuluPreflightRequest,
    channels: Vec<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) {
    require(
        request.attestations.get(id) == Some(&true),
        &format!("lulu.attestation.{id}"),
        &format!("Required reviewed attestation '{id}' is missing or false"),
        LuluFindingSeverity::Unknown,
        channels,
        findings,
    );
}

fn require_path(
    value: Option<&str>,
    root: &Path,
    code: &str,
    message: &str,
    channels: Vec<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) {
    require(
        resolved_existing(value, root).is_some(),
        code,
        message,
        LuluFindingSeverity::Error,
        channels,
        findings,
    );
}

fn resolved_existing<'a>(value: Option<&'a str>, root: &'a Path) -> Option<PathBuf> {
    value
        .map(|value| root.join(value))
        .filter(|path| path.is_file())
}

fn artifact_paths(artifacts: &LuluArtifactRequest) -> Vec<(&'static str, Option<&str>)> {
    vec![
        ("interior_pdf", artifacts.interior_pdf.as_deref()),
        (
            "bookstore_cover_pdf",
            artifacts.bookstore_cover_pdf.as_deref(),
        ),
        (
            "distribution_cover_pdf",
            artifacts.distribution_cover_pdf.as_deref(),
        ),
        (
            "bookstore_cover_template",
            artifacts.bookstore_cover_template.as_deref(),
        ),
        (
            "distribution_cover_template",
            artifacts.distribution_cover_template.as_deref(),
        ),
        ("pdf_ebook", artifacts.pdf_ebook.as_deref()),
        ("epub", artifacts.epub.as_deref()),
        ("ebook_cover", artifacts.ebook_cover.as_deref()),
        ("print_proof_pdf", artifacts.print_proof_pdf.as_deref()),
        (
            "proof_contact_sheet",
            artifacts.proof_contact_sheet.as_deref(),
        ),
        ("ace_report", artifacts.ace_report.as_deref()),
    ]
}

fn inspect_artifact(role: &str, value: Option<&str>, root: &Path) -> LuluArtifactEvidence {
    let Some(value) = value else {
        return LuluArtifactEvidence {
            role: role.to_string(),
            path: String::new(),
            sha256: None,
            size_bytes: None,
            inspected: false,
        };
    };
    let path = root.join(value);
    match fs::read(&path) {
        Ok(bytes) => LuluArtifactEvidence {
            role: role.to_string(),
            path: path.display().to_string(),
            sha256: Some(format!("{:x}", Sha256::digest(&bytes))),
            size_bytes: Some(bytes.len() as u64),
            inspected: true,
        },
        Err(_) => LuluArtifactEvidence {
            role: role.to_string(),
            path: path.display().to_string(),
            sha256: None,
            size_bytes: None,
            inspected: false,
        },
    }
}

fn channel_result(
    channel: LuluChannel,
    requested: bool,
    findings: &[LuluFinding],
) -> LuluChannelResult {
    if !requested {
        return LuluChannelResult {
            channel,
            eligibility: LuluEligibility::NotRequested,
            blocking_findings: Vec::new(),
            warning_findings: Vec::new(),
        };
    }
    let relevant = findings
        .iter()
        .filter(|finding| finding.channels.contains(&channel))
        .collect::<Vec<_>>();
    let blocking_findings = relevant
        .iter()
        .filter(|finding| finding.severity == LuluFindingSeverity::Error)
        .map(|finding| finding.code.clone())
        .collect::<Vec<_>>();
    let unknown = relevant
        .iter()
        .filter(|finding| finding.severity == LuluFindingSeverity::Unknown)
        .map(|finding| finding.code.clone())
        .collect::<Vec<_>>();
    let warning_findings = relevant
        .iter()
        .filter(|finding| finding.severity == LuluFindingSeverity::Warning)
        .map(|finding| finding.code.clone())
        .collect::<Vec<_>>();
    let eligibility = if !blocking_findings.is_empty() {
        LuluEligibility::Ineligible
    } else if !unknown.is_empty() {
        LuluEligibility::Unknown
    } else {
        LuluEligibility::Eligible
    };
    LuluChannelResult {
        channel,
        eligibility,
        blocking_findings: blocking_findings.into_iter().chain(unknown).collect(),
        warning_findings,
    }
}

fn valid_isbn13(value: Option<&str>) -> bool {
    let Some(value) = value else { return false };
    let digits = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.len() != 13 {
        return false;
    }
    let sum = digits
        .bytes()
        .take(12)
        .enumerate()
        .map(|(index, digit)| u32::from(digit - b'0') * if index % 2 == 0 { 1 } else { 3 })
        .sum::<u32>();
    let check = (10 - (sum % 10)) % 10;
    u32::from(digits.as_bytes()[12] - b'0') == check
}

fn valid_undashed_isbn13(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value.len() == 13
            && value.chars().all(|character| character.is_ascii_digit())
            && valid_isbn13(Some(value))
    })
}

fn validate_declared_geometry(
    request: &LuluPreflightRequest,
    publication: &PublicationContract,
    findings: &mut Vec<LuluFinding>,
) {
    if publication.geometry.unit != "mm" {
        finding(
            "lulu.print.geometry.unit_unverified",
            LuluFindingSeverity::Unknown,
            "The provider pack only compares declared trim geometry expressed in millimetres",
            print_channels(),
            findings,
        );
        return;
    }
    let tolerance_mm = 0.01;
    require(
        (publication.geometry.width - request.product.trim_width_mm).abs() <= tolerance_mm
            && (publication.geometry.height - request.product.trim_height_mm).abs() <= tolerance_mm,
        "lulu.print.geometry.contract_mismatch",
        "Requested Lulu trim geometry does not match the reviewed publication contract",
        LuluFindingSeverity::Error,
        print_channels(),
        findings,
    );
}

fn require(
    condition: bool,
    code: &str,
    message: &str,
    severity: LuluFindingSeverity,
    channels: Vec<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) {
    if !condition {
        finding(code, severity, message, channels, findings);
    }
}

fn finding(
    code: &str,
    severity: LuluFindingSeverity,
    message: &str,
    channels: Vec<LuluChannel>,
    findings: &mut Vec<LuluFinding>,
) {
    findings.push(LuluFinding {
        code: code.to_string(),
        severity,
        message: message.to_string(),
        channels,
    });
}

fn all_channels() -> Vec<LuluChannel> {
    vec![
        LuluChannel::PrintDirect,
        LuluChannel::LuluBookstore,
        LuluChannel::GlobalDistribution,
        LuluChannel::PdfEbook,
        LuluChannel::EpubDistribution,
        LuluChannel::Amazon,
        LuluChannel::Ingram,
        LuluChannel::BarnesAndNoble,
    ]
}

fn print_channels() -> Vec<LuluChannel> {
    vec![
        LuluChannel::PrintDirect,
        LuluChannel::LuluBookstore,
        LuluChannel::GlobalDistribution,
        LuluChannel::Amazon,
        LuluChannel::Ingram,
    ]
}

fn print_distribution_channels() -> Vec<LuluChannel> {
    vec![
        LuluChannel::GlobalDistribution,
        LuluChannel::Amazon,
        LuluChannel::Ingram,
    ]
}
