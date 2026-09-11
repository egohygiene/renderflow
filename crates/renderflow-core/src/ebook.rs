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

use crate::process::{
    ProcessExecutor, ProcessNetworkPolicy, ProcessRequest, ToolProbeStatus,
    DEFAULT_CAPTURE_LIMIT_BYTES,
};

pub const EBOOK_EVIDENCE_SCHEMA_V1: &str = "renderflow.ebook-evidence/v1";
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
}

impl EbookCapabilityContract {
    pub fn builtin() -> Self {
        Self {
            schema: EBOOK_EVIDENCE_SCHEMA_V1.to_string(),
            epub_reflow_generation: true,
            epub_fixed_layout_generation: false,
            kepub_reflow_generation: true,
            kepub_fixed_layout_generation: false,
            structural_inspection: true,
            page_list_evidence: true,
            accessibility_evidence: true,
            retailer_acceptance_requires_provider_profile: true,
        }
    }
}

pub fn inspect_ebook(path: &Path, run_epubcheck: bool) -> Result<EbookInspection> {
    let digest = sha256_file(path)?;
    let file = File::open(path)
        .with_context(|| format!("failed to open e-book source '{}'", path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("'{}' is not a readable EPUB ZIP container", path.display()))?;
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
    let valid = !diagnostics
        .iter()
        .any(|item| item.severity == EbookDiagnosticSeverity::Error);
    let epubcheck = run_epubcheck
        .then(|| run_epubcheck_provider(path))
        .transpose()?;
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
    })
}

pub fn run_epubcheck_provider(path: &Path) -> Result<EpubCheckEvidence> {
    let executor = ProcessExecutor::new();
    let probe = executor.probe_version("epubcheck");
    if probe.status != ToolProbeStatus::Available {
        return Ok(EpubCheckEvidence {
            provider_id: "tool.epubcheck".to_string(),
            available: false,
            version: probe.version_line,
            passed: None,
            duration_ms: Some(probe.duration_ms),
            report: None,
            diagnostic: probe.diagnostic,
        });
    }
    let directory = tempfile::tempdir().context("failed to create EPUBCheck evidence directory")?;
    let report_path = directory.path().join("epubcheck.json");
    let result = executor.execute(
        ProcessRequest::direct("epubcheck")
            .arg(path.to_string_lossy().into_owned())
            .arg("--json")
            .arg(report_path.to_string_lossy().into_owned())
            .timeout(Duration::from_secs(5 * 60))
            .capture_limit(DEFAULT_CAPTURE_LIMIT_BYTES)
            .network_policy(ProcessNetworkPolicy::Deny),
    )?;
    let report = std::fs::read(&report_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let diagnostic = (!result.is_success()).then(|| {
        let stderr = result.stderr().redacted_text().trim();
        if stderr.is_empty() {
            "EPUBCheck reported conformance errors".to_string()
        } else {
            stderr.to_string()
        }
    });
    Ok(EpubCheckEvidence {
        provider_id: "tool.epubcheck".to_string(),
        available: true,
        version: probe.version_line,
        passed: Some(result.is_success()),
        duration_ms: Some(result.duration_ms()),
        report,
        diagnostic,
    })
}

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
