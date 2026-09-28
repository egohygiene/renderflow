//! Provider-neutral EPUB/KEPUB contracts, structural inspection, and EPUBCheck evidence.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::evidence::{
    ArtifactRole, RunManifest, RunState, ValidationState, ARTIFACT_MANIFEST_SCHEMA_V1,
    RUN_MANIFEST_SCHEMA_V1,
};
use crate::fixed_layout_epub_validate::inspect_fixed_layout;
pub use crate::fixed_layout_epub_validate::{
    FixedLayoutEvidence, FixedLayoutPageEvidence, FixedLayoutStatus,
};
use crate::process::{
    ProcessExecutor, ProcessNetworkPolicy, ProcessRequest, ToolProbeStatus,
    DEFAULT_CAPTURE_LIMIT_BYTES,
};

pub const EBOOK_EVIDENCE_SCHEMA_V1: &str = "renderflow.ebook-evidence/v1";
pub const EBOOK_CAPABILITIES_SCHEMA_V1: &str = "renderflow.ebook-capabilities/v1";
const MAX_INSPECTION_MEMBER_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOTAL_XHTML_INSPECTION_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EbookVariant {
    Epub,
    Kepub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EbookLayout {
    Reflowable,
    PrePaginated,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EbookDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EbookDiagnostic {
    pub severity: EbookDiagnosticSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EbookMetadataEvidence {
    pub title: bool,
    pub creator: bool,
    pub language: bool,
    pub identifier: bool,
    pub rights: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EbookAccessibilityEvidence {
    pub access_modes: usize,
    pub accessibility_features: usize,
    pub accessibility_hazards: usize,
    pub accessibility_summary: bool,
    pub conforms_to: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpubCheckEvidence {
    pub provider_id: String,
    pub available: bool,
    pub version: Option<String>,
    pub passed: Option<bool>,
    pub duration_ms: Option<u64>,
    pub report: Option<Value>,
    pub diagnostic: Option<String>,
    /// These fields are optional so historical v1 inspection records remain readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<EpubCheckStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpubCheckStatus {
    Passed,
    Invalid,
    Unavailable,
    ProviderError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EbookInspection {
    pub schema: String,
    pub source: String,
    pub source_sha256: String,
    pub variant: EbookVariant,
    pub valid: bool,
    pub epub_version: Option<String>,
    pub package_document: Option<String>,
    pub layout: EbookLayout,
    pub xhtml_documents: usize,
    pub spine_items: usize,
    pub navigation: bool,
    pub page_list: bool,
    pub metadata: EbookMetadataEvidence,
    pub accessibility: EbookAccessibilityEvidence,
    pub retailer_acceptance: String,
    pub diagnostics: Vec<EbookDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epubcheck: Option<EpubCheckEvidence>,
    /// Independent inspection of the exact ordered PNG/JPEG package route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_layout: Option<FixedLayoutEvidence>,
    /// Optional binding to a canonical run manifest, never inferred from the ZIP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<EbookProvenanceEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EbookProvenanceStatus {
    Verified,
    Stale,
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EbookProvenanceEvidence {
    pub status: EbookProvenanceStatus,
    pub run_id: Option<String>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EbookCapabilityContract {
    pub schema: String,
    pub epub_reflow_generation: bool,
    pub epub_fixed_layout_generation: bool,
    pub kepub_reflow_generation: bool,
    pub kepub_fixed_layout_generation: bool,
    pub structural_inspection: bool,
    pub page_list_evidence: bool,
    pub accessibility_evidence: bool,
    pub retailer_acceptance_requires_provider_profile: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_layout_epub_route: Option<FixedLayoutEpubRouteCapability>,
}

/// The exact supported fixed-layout generation route, without implying generic
/// EPUB or fixed-layout KEPUB capability for arbitrary source collections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedLayoutEpubRouteCapability {
    pub capability: String,
    pub provider_id: String,
    pub source_formats: Vec<String>,
    pub target_format: String,
    pub ordered_collection_required: bool,
    pub homogeneous_local_sources_required: bool,
    pub explicit_execution_policy_required: bool,
    pub page_progression_directions: Vec<String>,
    pub spread_policies: Vec<String>,
    pub native_validation: bool,
    pub optional_epubcheck_v5: bool,
}

impl EbookCapabilityContract {
    pub fn builtin() -> Self {
        Self {
            schema: EBOOK_CAPABILITIES_SCHEMA_V1.to_string(),
            epub_reflow_generation: true,
            epub_fixed_layout_generation: true,
            kepub_reflow_generation: true,
            kepub_fixed_layout_generation: false,
            structural_inspection: true,
            page_list_evidence: true,
            accessibility_evidence: true,
            retailer_acceptance_requires_provider_profile: true,
            fixed_layout_epub_route: Some(FixedLayoutEpubRouteCapability {
                capability: crate::fixed_layout_epub::FIXED_EPUB_CAPABILITY.to_string(),
                provider_id: crate::fixed_layout_epub::FIXED_EPUB_PROVIDER.to_string(),
                source_formats: vec!["png".to_string(), "jpeg".to_string()],
                target_format: "epub".to_string(),
                ordered_collection_required: true,
                homogeneous_local_sources_required: true,
                explicit_execution_policy_required: true,
                page_progression_directions: vec!["ltr".to_string(), "rtl".to_string()],
                spread_policies: vec!["none".to_string()],
                native_validation: true,
                optional_epubcheck_v5: true,
            }),
        }
    }
}

pub fn inspect_ebook(path: &Path, run_epubcheck: bool) -> Result<EbookInspection> {
    let digest = sha256_file(path)?;
    let file = File::open(path)
        .with_context(|| format!("failed to open e-book source '{}'", path.display()))?;
    let mut archive = match ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(error) => {
            let path_lower = path.to_string_lossy().to_ascii_lowercase();
            let variant = if path_lower.ends_with(".kepub") || path_lower.ends_with(".kepub.epub") {
                EbookVariant::Kepub
            } else {
                EbookVariant::Epub
            };
            return Ok(EbookInspection {
                schema: EBOOK_EVIDENCE_SCHEMA_V1.to_string(),
                source: path.display().to_string(),
                source_sha256: digest,
                variant,
                valid: false,
                epub_version: None,
                package_document: None,
                layout: EbookLayout::Unknown,
                xhtml_documents: 0,
                spine_items: 0,
                navigation: false,
                page_list: false,
                metadata: EbookMetadataEvidence {
                    title: false,
                    creator: false,
                    language: false,
                    identifier: false,
                    rights: false,
                },
                accessibility: EbookAccessibilityEvidence {
                    access_modes: 0,
                    accessibility_features: 0,
                    accessibility_hazards: 0,
                    accessibility_summary: false,
                    conforms_to: false,
                },
                retailer_acceptance: "requires_provider_profile".to_string(),
                diagnostics: vec![EbookDiagnostic {
                    severity: EbookDiagnosticSeverity::Error,
                    code: "ebook.container.invalid".to_string(),
                    message: format!("EPUB ZIP container is unreadable: {error}"),
                }],
                epubcheck: run_epubcheck
                    .then(|| run_epubcheck_provider(path))
                    .transpose()?,
                fixed_layout: Some(FixedLayoutEvidence {
                    status: FixedLayoutStatus::Invalid,
                    pages: Vec::new(),
                }),
                provenance: None,
            });
        }
    };
    let mimetype_envelope = archive.by_index(0).ok().is_some_and(|entry| {
        entry.name() == "mimetype" && entry.compression() == zip::CompressionMethod::Stored
    });
    let names = (0..archive.len())
        .filter_map(|index| {
            archive
                .by_index(index)
                .ok()
                .map(|entry| entry.name().to_string())
        })
        .collect::<Vec<_>>();
    let mimetype = read_member(&mut archive, "mimetype").unwrap_or_default();
    let container = read_member(&mut archive, "META-INF/container.xml").unwrap_or_default();
    let package_document = attribute_value(&container, "full-path");
    let opf = package_document
        .as_deref()
        .and_then(|member| read_member(&mut archive, member));
    let opf_text = opf.as_deref().unwrap_or_default();
    let epub_version =
        tag_fragment(opf_text, "package").and_then(|tag| attribute_value(tag, "version"));
    let xhtml_names = names
        .iter()
        .filter(|name| name.ends_with(".xhtml") || name.ends_with(".html"))
        .cloned()
        .collect::<Vec<_>>();
    let mut xhtml_members = Vec::new();
    let mut inspected_xhtml_bytes = 0_usize;
    let mut xhtml_scan_truncated = false;
    for name in &xhtml_names {
        let Some(member) = read_member(&mut archive, name) else {
            continue;
        };
        inspected_xhtml_bytes = inspected_xhtml_bytes.saturating_add(member.len());
        if inspected_xhtml_bytes > MAX_TOTAL_XHTML_INSPECTION_BYTES {
            xhtml_scan_truncated = true;
            break;
        }
        xhtml_members.push(member);
    }
    let xhtml_documents = xhtml_members
        .iter()
        .filter(|member| member.contains("<html"))
        .count();
    let spine_items = count_token(opf_text, "<itemref");
    let nav_href = manifest_href_with_property(opf_text, "nav");
    let nav_path = nav_href.as_deref().and_then(|href| {
        package_document
            .as_deref()
            .map(|package| resolve_member_path(package, href))
    });
    let nav = nav_path
        .as_deref()
        .and_then(|member| read_member(&mut archive, member));
    let nav_text = nav.as_deref().unwrap_or_default();
    let navigation = nav.is_some();
    let page_list = contains_token(nav_text, "page-list");
    let layout = inspect_layout(opf_text);
    let kepub_marker = names.iter().any(|name| name.ends_with("kepubify.css"))
        || xhtml_members
            .iter()
            .any(|member| member.contains("koboSpan") || member.contains("kobo.1.1"));
    let path_lower = path.to_string_lossy().to_ascii_lowercase();
    let variant =
        if kepub_marker || path_lower.ends_with(".kepub") || path_lower.ends_with(".kepub.epub") {
            EbookVariant::Kepub
        } else {
            EbookVariant::Epub
        };
    let metadata = EbookMetadataEvidence {
        title: contains_element(opf_text, "dc:title"),
        creator: contains_element(opf_text, "dc:creator"),
        language: contains_element(opf_text, "dc:language"),
        identifier: contains_element(opf_text, "dc:identifier"),
        rights: contains_element(opf_text, "dc:rights"),
    };
    let accessibility = EbookAccessibilityEvidence {
        access_modes: count_token(opf_text, "schema:accessMode"),
        accessibility_features: count_token(opf_text, "schema:accessibilityFeature"),
        accessibility_hazards: count_token(opf_text, "schema:accessibilityHazard"),
        accessibility_summary: contains_token(opf_text, "schema:accessibilitySummary"),
        conforms_to: contains_token(opf_text, "dcterms:conformsTo"),
    };
    let mut diagnostics = Vec::new();
    require(
        &mut diagnostics,
        mimetype_envelope,
        "ebook.mimetype.envelope",
        "The mimetype member must be the first ZIP entry and stored without compression",
    );
    require(
        &mut diagnostics,
        mimetype.trim() == "application/epub+zip",
        "ebook.mimetype",
        "The EPUB mimetype member is missing or invalid",
    );
    require(
        &mut diagnostics,
        !container.is_empty(),
        "ebook.container",
        "META-INF/container.xml is missing",
    );
    require(
        &mut diagnostics,
        opf.is_some(),
        "ebook.package_document",
        "The root package document declared by container.xml is missing",
    );
    require(
        &mut diagnostics,
        epub_version
            .as_deref()
            .is_some_and(|version| version.starts_with('3')),
        "ebook.epub3",
        "The package does not declare EPUB 3.x",
    );
    require(
        &mut diagnostics,
        metadata.title,
        "ebook.metadata.title",
        "Required internal title metadata is missing",
    );
    require(
        &mut diagnostics,
        metadata.language,
        "ebook.metadata.language",
        "Required internal language metadata is missing",
    );
    require(
        &mut diagnostics,
        metadata.identifier,
        "ebook.metadata.identifier",
        "Required internal identifier metadata is missing",
    );
    require(
        &mut diagnostics,
        xhtml_documents > 0,
        "ebook.xhtml",
        "No XHTML/HTML content documents were found",
    );
    require(
        &mut diagnostics,
        spine_items > 0,
        "ebook.spine",
        "The package has no ordered spine items",
    );
    require(
        &mut diagnostics,
        navigation,
        "ebook.navigation",
        "No EPUB 3 navigation document was declared",
    );
    if !page_list {
        diagnostics.push(warning(
            "ebook.page_list.absent",
            "No page-list navigation was found; downstream print-page correlation is unavailable",
        ));
    }
    if xhtml_scan_truncated {
        diagnostics.push(warning(
            "ebook.xhtml.scan_bounded",
            "XHTML marker inspection stopped at the 64 MiB safety bound",
        ));
    }
    if accessibility.access_modes == 0
        && accessibility.accessibility_features == 0
        && !accessibility.accessibility_summary
    {
        diagnostics.push(warning(
            "ebook.accessibility.sparse",
            "No accessibility metadata was found in the package document",
        ));
    }
    if variant == EbookVariant::Kepub && !kepub_marker {
        diagnostics.push(warning(
            "ebook.kepub.marker",
            "The filename implies KEPUB, but no Kepubify content marker was found",
        ));
    }
    if layout == EbookLayout::PrePaginated {
        diagnostics.push(info(
            "ebook.fixed_layout",
            "Fixed-layout is declared; retailer/channel acceptance requires a provider profile",
        ));
    }
    let fixed_candidate = layout == EbookLayout::PrePaginated
        || names.iter().any(|name| {
            name.starts_with("EPUB/pages/page-") || name.starts_with("EPUB/images/page-")
        });
    let fixed_layout = if variant == EbookVariant::Epub && fixed_candidate {
        let (evidence, mut failures) = inspect_fixed_layout(&mut archive, path);
        diagnostics.append(&mut failures);
        Some(evidence)
    } else if variant == EbookVariant::Kepub && fixed_candidate {
        Some(FixedLayoutEvidence {
            status: FixedLayoutStatus::Unsupported,
            pages: Vec::new(),
        })
    } else {
        None
    };
    let epubcheck = run_epubcheck
        .then(|| run_epubcheck_provider(path))
        .transpose()?;
    if sha256_file(path)? != digest {
        diagnostics.push(EbookDiagnostic {
            severity: EbookDiagnosticSeverity::Error,
            code: "ebook.container.changed".to_string(),
            message: "EPUB bytes changed during inspection".to_string(),
        });
    }
    let valid = !diagnostics
        .iter()
        .any(|item| item.severity == EbookDiagnosticSeverity::Error);
    Ok(EbookInspection {
        schema: EBOOK_EVIDENCE_SCHEMA_V1.to_string(),
        source: path.display().to_string(),
        source_sha256: digest,
        variant,
        valid,
        epub_version,
        package_document,
        layout,
        xhtml_documents,
        spine_items,
        navigation,
        page_list,
        metadata,
        accessibility,
        retailer_acceptance: "requires_provider_profile".to_string(),
        diagnostics,
        epubcheck,
        fixed_layout,
        provenance: None,
    })
}

/// Bind a native fixed-layout inspection to an explicit local canonical run
/// manifest. This is digest/lineage evidence, not a signature or publication
/// approval. Unreadable or incompatible evidence is a typed invalid result.
pub fn inspect_ebook_with_manifest(
    path: &Path,
    run_epubcheck: bool,
    manifest_path: &Path,
) -> Result<EbookInspection> {
    let mut inspection = inspect_ebook(path, run_epubcheck)?;
    let binding = verify_run_manifest(path, manifest_path, &inspection);
    let (status, run_id, diagnostic) = match binding {
        Ok(run_id) => (EbookProvenanceStatus::Verified, Some(run_id), None),
        Err((status, message)) => {
            inspection.valid = false;
            inspection.diagnostics.push(EbookDiagnostic {
                severity: EbookDiagnosticSeverity::Error,
                code: match status {
                    EbookProvenanceStatus::Corrupt => "ebook.provenance.corrupt",
                    EbookProvenanceStatus::Stale => "ebook.provenance.stale",
                    EbookProvenanceStatus::Verified => unreachable!(),
                }
                .to_string(),
                message: message.clone(),
            });
            (status, None, Some(message))
        }
    };
    inspection.provenance = Some(EbookProvenanceEvidence {
        status,
        run_id,
        diagnostic,
    });
    if sha256_file(path)? != inspection.source_sha256 {
        inspection.valid = false;
        inspection.diagnostics.push(EbookDiagnostic {
            severity: EbookDiagnosticSeverity::Error,
            code: "ebook.provenance.stale".to_string(),
            message: "EPUB bytes changed during run-manifest binding".to_string(),
        });
        inspection.provenance = Some(EbookProvenanceEvidence {
            status: EbookProvenanceStatus::Stale,
            run_id: None,
            diagnostic: Some("EPUB bytes changed during run-manifest binding".to_string()),
        });
    }
    Ok(inspection)
}

fn verify_run_manifest(
    path: &Path,
    manifest_path: &Path,
    inspection: &EbookInspection,
) -> std::result::Result<String, (EbookProvenanceStatus, String)> {
    let corrupt = |message: String| (EbookProvenanceStatus::Corrupt, message);
    let stale = |message: String| (EbookProvenanceStatus::Stale, message);
    let size = std::fs::metadata(manifest_path)
        .map_err(|error| corrupt(format!("run manifest is unavailable: {error}")))?
        .len();
    if size == 0 || size > 32 * 1024 * 1024 {
        return Err(corrupt(
            "run manifest is empty or exceeds 32 MiB".to_string(),
        ));
    }
    let manifest_bytes = std::fs::read(manifest_path)
        .map_err(|error| corrupt(format!("run manifest cannot be read: {error}")))?;
    let manifest: RunManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| corrupt(format!("run manifest JSON/contract is corrupt: {error}")))?;
    if manifest.schema_version != RUN_MANIFEST_SCHEMA_V1
        || manifest.artifact_manifest.schema_version != ARTIFACT_MANIFEST_SCHEMA_V1
        || manifest.run_id != manifest.artifact_manifest.run_id
        || manifest.state != RunState::Complete
    {
        return Err(corrupt(
            "run manifest schema, identity, or completed state is inconsistent".to_string(),
        ));
    }
    let terminals = manifest
        .artifact_manifest
        .artifacts
        .iter()
        .filter(|artifact| artifact.lifecycle == ArtifactRole::Terminal && artifact.role == "ebook")
        .collect::<Vec<_>>();
    if terminals.len() != 1 {
        return Err(corrupt(
            "run manifest must identify exactly one terminal ebook".to_string(),
        ));
    }
    let terminal = terminals[0];
    if terminal.format != "epub"
        || terminal.media_type != "application/epub+zip"
        || terminal.producer.capability.as_deref()
            != Some(crate::fixed_layout_epub::FIXED_EPUB_CAPABILITY)
        || terminal.producer.provider.as_deref()
            != Some(crate::fixed_layout_epub::FIXED_EPUB_PROVIDER)
        || !matches!(
            terminal.validation,
            ValidationState::Valid | ValidationState::ValidWithWarnings
        )
    {
        return Err(corrupt(
            "run manifest terminal does not describe the exact validated EPUB route".to_string(),
        ));
    }
    let locator = terminal
        .locator
        .strip_prefix("bundle:")
        .ok_or_else(|| corrupt("terminal EPUB has no bundle locator".to_string()))?;
    if locator.is_empty()
        || !Path::new(locator)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(corrupt("terminal EPUB locator is unsafe".to_string()));
    }
    let manifest_root = manifest_path
        .parent()
        .ok_or_else(|| corrupt("run manifest has no output directory".to_string()))?
        .canonicalize()
        .map_err(|error| {
            corrupt(format!(
                "run manifest output directory cannot be resolved: {error}"
            ))
        })?;
    let declared_root = Path::new(&manifest.artifact_manifest.output_dir);
    if (declared_root.is_absolute()
        && declared_root.canonicalize().map_err(|error| {
            stale(format!(
                "declared output directory cannot be resolved: {error}"
            ))
        })? != manifest_root)
        || (declared_root.is_relative()
            && declared_root != Path::new(".")
            && !manifest_root.ends_with(declared_root))
    {
        return Err(stale(
            "run manifest output directory differs from its location".to_string(),
        ));
    }
    let expected_path = manifest_root.join(locator);
    let actual = path
        .canonicalize()
        .map_err(|error| stale(format!("EPUB path cannot be resolved: {error}")))?;
    let expected = expected_path
        .canonicalize()
        .map_err(|error| stale(format!("manifest EPUB path cannot be resolved: {error}")))?;
    if actual != expected
        || terminal.digest.algorithm != "sha256"
        || terminal.digest.value != inspection.source_sha256
        || terminal.size_bytes
            != std::fs::metadata(path)
                .map_err(|error| stale(error.to_string()))?
                .len()
    {
        return Err(stale(
            "EPUB path, size, or SHA-256 differs from the run manifest".to_string(),
        ));
    }
    let pages = inspection
        .fixed_layout
        .as_ref()
        .filter(|evidence| evidence.status == FixedLayoutStatus::Validated)
        .map(|evidence| &evidence.pages)
        .ok_or_else(|| stale("native fixed-layout validation did not pass".to_string()))?;
    let sources = manifest
        .artifact_manifest
        .artifacts
        .iter()
        .filter(|artifact| artifact.lifecycle == ArtifactRole::Source)
        .collect::<Vec<_>>();
    let declared = terminal
        .metadata
        .get("renderflow.fixed_epub.ordered_pages")
        .and_then(Value::as_array)
        .ok_or_else(|| corrupt("ordered page provenance is missing".to_string()))?;
    if pages.len() != sources.len()
        || pages.len() != declared.len()
        || terminal.sources
            != sources
                .iter()
                .map(|source| source.artifact_id.clone())
                .collect::<Vec<_>>()
    {
        return Err(stale(
            "ordered source lineage differs from validated pages".to_string(),
        ));
    }
    for (index, ((page, source), declared)) in pages.iter().zip(&sources).zip(declared).enumerate()
    {
        if declared.get("index").and_then(Value::as_u64) != Some(index as u64)
            || declared.get("source_id").and_then(Value::as_str) != Some(source.role.as_str())
            || declared.get("sha256").and_then(Value::as_str) != Some(page.image_sha256.as_str())
            || source.digest.algorithm != "sha256"
            || source.digest.value != page.image_sha256
        {
            return Err(stale(format!(
                "page {} differs from ordered source provenance",
                index + 1
            )));
        }
    }
    Ok(manifest.run_id)
}

pub fn run_epubcheck_provider(path: &Path) -> Result<EpubCheckEvidence> {
    let executable = epubcheck_executable().unwrap_or_else(|| "epubcheck".to_string());
    run_epubcheck_with_executable(path, &executable)
}

fn epubcheck_executable() -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for name in if cfg!(windows) {
            &["epubcheck.exe", "epubcheck.cmd", "epubcheck.bat"][..]
        } else {
            &["epubcheck"][..]
        } {
            let candidate = directory.join(name);
            if candidate.is_file() {
                // Preserve the invoked symlink path: some launcher scripts use
                // their own directory to locate the EPUBCheck JAR.
                let absolute = if candidate.is_absolute() {
                    candidate
                } else {
                    std::env::current_dir().ok()?.join(candidate)
                };
                return Some(absolute.display().to_string());
            }
        }
    }
    None
}

fn epubcheck_major(version: &str) -> Option<(u32, String)> {
    let lower = version.to_ascii_lowercase();
    let after_name = lower
        .find("epubcheck")
        .map_or(lower.as_str(), |index| &lower[index + "epubcheck".len()..]);
    let start = after_name.find(|character: char| character.is_ascii_digit())?;
    let token: String = after_name[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    let major = token.split('.').next()?.parse().ok()?;
    Some((major, token.trim_end_matches('.').to_string()))
}

fn run_epubcheck_with_executable(path: &Path, executable: &str) -> Result<EpubCheckEvidence> {
    let input_sha256 = sha256_file(path)?;
    let executor = ProcessExecutor::new();
    let probe = executor.probe_version(executable);
    let version = probe.version_line.clone();
    if probe.status != ToolProbeStatus::Available
        || !version.as_deref().is_some_and(|line| {
            line.to_ascii_lowercase().contains("epubcheck")
                && epubcheck_major(line).is_some_and(|(major, _)| major == 5)
        })
    {
        let diagnostic = if probe.status == ToolProbeStatus::Available {
            Some(format!(
                "EPUBCheck v5 is required; version probe returned '{}'",
                version.as_deref().unwrap_or("no version")
            ))
        } else {
            probe.diagnostic
        };
        return Ok(EpubCheckEvidence {
            provider_id: "tool.epubcheck".to_string(),
            available: false,
            version,
            passed: None,
            duration_ms: Some(probe.duration_ms),
            report: None,
            diagnostic,
            status: Some(
                if probe.status == ToolProbeStatus::Missing
                    || probe.status == ToolProbeStatus::Available
                {
                    EpubCheckStatus::Unavailable
                } else {
                    EpubCheckStatus::ProviderError
                },
            ),
            executable: Some(executable.to_string()),
            arguments: vec![],
            input_sha256: Some(input_sha256),
            report_sha256: None,
            exit_code: None,
        });
    }
    let directory = tempfile::tempdir().context("failed to create EPUBCheck evidence directory")?;
    let report_path = directory.path().join("epubcheck.json");
    let arguments = vec![
        path.to_string_lossy().into_owned(),
        "--json".to_string(),
        report_path.to_string_lossy().into_owned(),
    ];
    let request = ProcessRequest::direct(executable)
        .args(arguments.iter().cloned())
        .timeout(Duration::from_secs(5 * 60))
        .capture_limit(DEFAULT_CAPTURE_LIMIT_BYTES)
        .network_policy(ProcessNetworkPolicy::Deny);
    let result = match executor.execute(request) {
        Ok(result) => result,
        Err(error) => {
            let missing = matches!(
                &error,
                crate::process::ProcessError::MissingExecutable { .. }
            );
            return Ok(EpubCheckEvidence {
                provider_id: "tool.epubcheck".to_string(),
                available: false,
                version,
                passed: None,
                duration_ms: None,
                report: None,
                diagnostic: Some(error.to_string()),
                status: Some(if missing {
                    EpubCheckStatus::Unavailable
                } else {
                    EpubCheckStatus::ProviderError
                }),
                executable: Some(executable.to_string()),
                arguments,
                input_sha256: Some(input_sha256),
                report_sha256: None,
                exit_code: None,
            });
        }
    };
    let exit_code = match result.termination() {
        crate::process::ProcessTermination::Exited { code } => Some(code),
        _ => None,
    };
    let raw_report = std::fs::metadata(&report_path)
        .ok()
        .filter(|metadata| metadata.is_file() && metadata.len() <= 8 * 1024 * 1024)
        .and_then(|_| std::fs::read(&report_path).ok());
    let report_sha256 = raw_report
        .as_deref()
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
    let report: Option<Value> = raw_report
        .as_deref()
        .and_then(|bytes| serde_json::from_slice(bytes).ok());
    let report_error = match report.as_ref() {
        Some(report) => {
            epubcheck_report_error(report, version.as_deref().unwrap_or_default(), path)
        }
        None => Some("EPUBCheck JSON report is missing, oversized, or malformed".to_string()),
    };
    let source_changed = sha256_file(path).ok().as_deref() != Some(input_sha256.as_str());
    let (status, passed, diagnostic) = if source_changed {
        (
            EpubCheckStatus::ProviderError,
            None,
            Some("EPUB source changed while EPUBCheck was running".to_string()),
        )
    } else if !matches!(
        result.termination(),
        crate::process::ProcessTermination::Exited { .. }
    ) {
        (
            EpubCheckStatus::ProviderError,
            None,
            Some(result.failure_message()),
        )
    } else if let Some(error) = report_error {
        (EpubCheckStatus::ProviderError, None, Some(error))
    } else if result.is_success() && report.as_ref().is_some_and(epubcheck_report_has_errors) {
        (
            EpubCheckStatus::ProviderError,
            None,
            Some("EPUBCheck exited successfully but its JSON report contains errors".to_string()),
        )
    } else if result.is_success() {
        (EpubCheckStatus::Passed, Some(true), None)
    } else if exit_code.is_some() && report.as_ref().is_some_and(epubcheck_report_has_errors) {
        (
            EpubCheckStatus::Invalid,
            Some(false),
            Some("EPUBCheck reported conformance errors".to_string()),
        )
    } else {
        (
            EpubCheckStatus::ProviderError,
            None,
            Some(result.failure_message()),
        )
    };
    Ok(EpubCheckEvidence {
        provider_id: "tool.epubcheck".to_string(),
        available: true,
        version,
        passed,
        duration_ms: Some(result.duration_ms()),
        report,
        diagnostic,
        status: Some(status),
        executable: Some(executable.to_string()),
        arguments,
        input_sha256: Some(input_sha256),
        report_sha256,
        exit_code,
    })
}

fn epubcheck_report_error(report: &Value, probe_version: &str, path: &Path) -> Option<String> {
    let Some(checker) = report.get("checker").and_then(Value::as_object) else {
        return Some("EPUBCheck JSON report has no checker metadata".to_string());
    };
    let Some((_, probed)) = epubcheck_major(probe_version) else {
        return Some("EPUBCheck version probe was not parseable".to_string());
    };
    let Some((_, reported)) = checker
        .get("checkerVersion")
        .and_then(Value::as_str)
        .and_then(epubcheck_major)
    else {
        return Some("EPUBCheck JSON report has no checker version".to_string());
    };
    if probed != reported {
        return Some("EPUBCheck report version differs from the executable probe".to_string());
    }
    if checker.get("filename").and_then(Value::as_str)
        != path.file_name().and_then(|name| name.to_str())
    {
        return Some("EPUBCheck report filename differs from the checked input".to_string());
    }
    for key in ["nFatal", "nError"] {
        if checker.get(key).and_then(Value::as_u64).is_none() {
            return Some(format!("EPUBCheck JSON report has no valid {key} count"));
        }
    }
    if report
        .get("publication")
        .and_then(Value::as_object)
        .is_none()
        || report.get("items").and_then(Value::as_array).is_none()
        || report.get("messages").and_then(Value::as_array).is_none()
    {
        return Some(
            "EPUBCheck JSON report is missing publication, items, or messages".to_string(),
        );
    }
    None
}

fn epubcheck_report_has_errors(report: &Value) -> bool {
    let checker = &report["checker"];
    checker["nFatal"].as_u64().unwrap_or(0) > 0
        || checker["nError"].as_u64().unwrap_or(0) > 0
        || report["messages"].as_array().is_some_and(|messages| {
            messages.iter().any(|message| {
                message["severity"].as_str().is_some_and(|severity| {
                    matches!(severity.to_ascii_lowercase().as_str(), "fatal" | "error")
                })
            })
        })
}

#[cfg(all(test, unix))]
#[path = "ebook_provider_tests.rs"]
mod epubcheck_provider_tests;

fn read_member<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let mut member = archive.by_name(name).ok()?;
    if member.size() > MAX_INSPECTION_MEMBER_BYTES {
        return None;
    }
    let mut text = String::new();
    member.read_to_string(&mut text).ok()?;
    Some(text)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to read e-book source '{}'", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn attribute_value(text: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(start) = text.find(&needle) {
            let value = &text[start + needle.len()..];
            if let Some(end) = value.find(quote) {
                return Some(value[..end].to_string());
            }
        }
    }
    None
}

fn manifest_href_with_property(opf: &str, property: &str) -> Option<String> {
    opf.split('<').find_map(|fragment| {
        if !fragment.starts_with("item ") {
            return None;
        }
        let properties = attribute_value(fragment, "properties")?;
        properties
            .split_ascii_whitespace()
            .any(|candidate| candidate == property)
            .then(|| attribute_value(fragment, "href"))
            .flatten()
    })
}

fn tag_fragment<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    text.split('<').find(|fragment| {
        fragment.starts_with(tag)
            && fragment[tag.len()..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_whitespace() || character == '>')
    })
}

fn resolve_member_path(package: &str, href: &str) -> String {
    let mut path = PathBuf::from(package);
    path.pop();
    path.push(href.split('#').next().unwrap_or(href));
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn inspect_layout(opf: &str) -> EbookLayout {
    let fixed = contains_token(opf, "pre-paginated");
    let reflow = contains_token(opf, "reflowable");
    match (fixed, reflow) {
        (true, true) => EbookLayout::Mixed,
        (true, false) => EbookLayout::PrePaginated,
        (false, true) => EbookLayout::Reflowable,
        (false, false) if !opf.is_empty() => EbookLayout::Reflowable,
        _ => EbookLayout::Unknown,
    }
}

fn contains_element(text: &str, element: &str) -> bool {
    text.contains(&format!("<{element}"))
}

fn contains_token(text: &str, token: &str) -> bool {
    text.contains(token)
}

fn count_token(text: &str, token: &str) -> usize {
    text.match_indices(token).count()
}

fn require(diagnostics: &mut Vec<EbookDiagnostic>, condition: bool, code: &str, message: &str) {
    if !condition {
        diagnostics.push(EbookDiagnostic {
            severity: EbookDiagnosticSeverity::Error,
            code: code.to_string(),
            message: message.to_string(),
        });
    }
}

fn warning(code: &str, message: &str) -> EbookDiagnostic {
    EbookDiagnostic {
        severity: EbookDiagnosticSeverity::Warning,
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn info(code: &str, message: &str) -> EbookDiagnostic {
    EbookDiagnostic {
        severity: EbookDiagnosticSeverity::Info,
        code: code.to_string(),
        message: message.to_string(),
    }
}
