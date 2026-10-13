//! Explicit local PDF/text extraction for review-required screenplay recovery.
//!
//! Commands use private staged inputs, exact executable/data pins, cleared
//! environments, bounded outputs, and one page at a time. This is not an OS
//! sandbox: linked libraries, fonts, Poppler data, parser allocations, and native
//! tool memory are not attested or strictly capped. No tool is found on PATH,
//! installed, downloaded, or run by declaration validation.

use std::collections::HashSet;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lopdf::{Document, LoadOptions, Object, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::process::{ProcessCancellationToken, ProcessEnvironment, ProcessExecutor,
    ProcessExpectedOutput, ProcessNetworkPolicy, ProcessOutputMode, ProcessRequest, ProcessTermination};
use super::model::{text_pages, ExtractedPage, MAX_RECOVERY_INPUT_BYTES, MAX_RECOVERY_PAGES};
pub use super::model::ExtractionMethod;

pub const EXTRACTION_EVIDENCE_SCHEMA: &str = "renderflow.screenplay-recovery-extraction/v1";
const MAX_NATIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PAGE_TEXT_BYTES: usize = 1024 * 1024;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
const MAX_PDF_OBJECTS: usize = 100_000;
const MAX_PDF_VALUES: usize = 500_000;
const MAX_PDF_DEPTH: usize = 32;
const MAX_PDF_DECOMPRESSED_BYTES: usize = 4 * 1024 * 1024;
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionSourceFormat { Pdf, Text }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedRecoveryTool {
    pub executable: PathBuf,
    pub executable_sha256: String,
    /// Exact numeric version; an observation identifies supplied bytes only.
    pub expected_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryOcrConfig {
    pub pdftoppm: PinnedRecoveryTool,
    pub tesseract: PinnedRecoveryTool,
    /// One explicit language model, with no ambient OSD or config-file lookup.
    pub language: String,
    pub traineddata: PathBuf,
    pub traineddata_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionLimits {
    pub max_input_bytes: u64,
    pub max_pages: usize,
    pub max_page_text_bytes: usize,
    pub max_total_text_bytes: usize,
    /// A square upper bound: raster pixels cannot exceed this value squared.
    pub max_raster_side: u32,
    /// Shared wall budget for snapshotting, version probes, and every page.
    pub timeout_seconds: u64,
}
impl Default for ExtractionLimits {
    fn default() -> Self {
        Self { max_input_bytes: MAX_INPUT_BYTES, max_pages: MAX_RECOVERY_PAGES,
            max_page_text_bytes: MAX_PAGE_TEXT_BYTES, max_total_text_bytes: MAX_RECOVERY_INPUT_BYTES,
            max_raster_side: 3000, timeout_seconds: 300 }
    }
}
impl ExtractionLimits {
    pub fn validate(&self) -> Result<(), ExtractionError> {
        if self.max_input_bytes == 0 || self.max_input_bytes > MAX_INPUT_BYTES
            || self.max_pages == 0 || self.max_pages > MAX_RECOVERY_PAGES
            || self.max_page_text_bytes == 0 || self.max_page_text_bytes > MAX_PAGE_TEXT_BYTES
            || self.max_total_text_bytes == 0 || self.max_total_text_bytes > MAX_RECOVERY_INPUT_BYTES
            || self.max_page_text_bytes > self.max_total_text_bytes
            || !(64..=4096).contains(&self.max_raster_side)
            || !(1..=1800).contains(&self.timeout_seconds) {
            return Err(ExtractionError::InvalidRequest("extraction limits exceed the authored bounded contract".to_owned()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryExtractionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdftotext: Option<PinnedRecoveryTool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<RecoveryOcrConfig>,
    #[serde(default)]
    pub limits: ExtractionLimits,
    #[serde(default)]
    pub allow_unqualified: bool,
}
impl Default for RecoveryExtractionConfig {
    fn default() -> Self { Self { pdftotext: None, ocr: None, limits: ExtractionLimits::default(), allow_unqualified: false } }
}
impl RecoveryExtractionConfig {
    /// Declaration-only: no file reads, PATH searches, or native version probes.
    pub fn validate(&self) -> Result<(), ExtractionError> {
        self.limits.validate()?;
        if let Some(tool) = &self.pdftotext { validate_tool(tool, false)?; }
        if let Some(ocr) = &self.ocr {
            if self.pdftotext.is_none() { return Err(ExtractionError::InvalidRequest("OCR requires the explicit PDF embedded-text provider first".to_owned())); }
            validate_tool(&ocr.pdftoppm, false)?;
            validate_tool(&ocr.tesseract, true)?;
            absolute_text(&ocr.traineddata)?;
            expected_digest(&ocr.traineddata_sha256)?;
            if ocr.language.is_empty() || ocr.language.len() > 32
                || !ocr.language.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
                return Err(ExtractionError::InvalidRequest("OCR language must name one explicit alphanumeric/underscore model".to_owned()));
            }
        }
        if !self.allow_unqualified {
            return Err(ExtractionError::InvalidRequest("screenplay recovery requires allow_unqualified=true; qualification is deferred".to_owned()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ExtractionRequest {
    pub input: PathBuf,
    pub source_format: ExtractionSourceFormat,
    pub expected_source_sha256: Option<String>,
    pub config: RecoveryExtractionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionPageEvidence {
    pub page_number: u32,
    pub method: ExtractionMethod,
    pub text_sha256: String,
    pub text_bytes: usize,
    /// PDF extraction result before OCR selection, including blank text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded_text_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded_text: Option<String>,
    pub usable_text: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raster_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raster_dimensions: Option<[u32; 2]>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionProviderEvidence {
    pub provider_id: String,
    pub executable_sha256: String,
    pub observed_version: String,
    pub qualification: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionInvocationEvidence {
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    pub argv: Vec<String>,
    pub argv_sha256: String,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    pub stderr_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionEvidence {
    pub schema_version: String,
    pub source_format: ExtractionSourceFormat,
    pub source_sha256: String,
    pub source_bytes: u64,
    pub config_sha256: String,
    pub pages_sha256: String,
    pub pages: Vec<ExtractionPageEvidence>,
    pub providers: Vec<ExtractionProviderEvidence>,
    pub invocations: Vec<ExtractionInvocationEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traineddata_sha256: Option<String>,
    pub qualification: String,
    pub source_untouched: bool,
    pub review_required: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedDocument {
    pub source_sha256: String,
    pub pages: Vec<ExtractedPage>,
    pub evidence: ExtractionEvidence,
}

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("invalid screenplay extraction request: {0}")]
    InvalidRequest(String),
    #[error("invalid screenplay recovery input: {0}")]
    InvalidInput(String),
    #[error("malformed screenplay PDF: {0}")]
    MalformedPdf(String),
    #[error("encrypted PDFs require an explicitly decrypted source copy")]
    EncryptedPdf,
    #[error("unsupported screenplay recovery input: {0}")]
    UnsupportedInput(String),
    #[error("screenplay extraction provider unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("screenplay extraction provider failed: {0}")]
    ProviderFailed(String),
    #[error("screenplay extraction output rejected: {0}")]
    InvalidOutput(String),
    #[error("screenplay extraction limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("file changed during screenplay extraction: {0}")]
    SourceChanged(String),
    #[error("screenplay extraction cancelled")]
    Cancelled,
    #[error("screenplay extraction exceeded its shared time budget")]
    TimedOut,
    #[error("screenplay extraction I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("screenplay extraction JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}
impl ExtractionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "screenplay_recovery.invalid_request",
            Self::InvalidInput(_) => "screenplay_recovery.invalid_input",
            Self::MalformedPdf(_) => "screenplay_recovery.malformed_pdf",
            Self::EncryptedPdf => "screenplay_recovery.encrypted_pdf",
            Self::UnsupportedInput(_) => "screenplay_recovery.unsupported_input",
            Self::ProviderUnavailable(_) => "screenplay_recovery.provider_unavailable",
            Self::ProviderFailed(_) => "screenplay_recovery.provider_failed",
            Self::InvalidOutput(_) => "screenplay_recovery.invalid_output",
            Self::LimitExceeded(_) => "screenplay_recovery.limit_exceeded",
            Self::SourceChanged(_) => "screenplay_recovery.source_changed",
            Self::Cancelled => "screenplay_recovery.cancelled",
            Self::TimedOut => "screenplay_recovery.timed_out",
            Self::Io(_) => "screenplay_recovery.io",
            Self::Json(_) => "screenplay_recovery.json",
        }
    }
}

/// Injectable boundary for authored contract fixtures, not qualification.
pub trait RecoveryExtractionRunner {
    fn run(&self, invocation: &ExtractionInvocation, cancellation: &ProcessCancellationToken)
        -> Result<ExtractionProcessOutput, ExtractionError>;
}
#[derive(Debug)]
pub struct ExtractionInvocation {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    pub timeout: Duration,
    pub stdout_limit: usize,
    pub expected_output: Option<(PathBuf, u64)>,
}
#[derive(Debug)]
pub struct ExtractionProcessOutput {
    pub termination: ProcessTermination,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub capture_truncated: bool,
    pub output_failures: Vec<String>,
}
pub struct BoundedRecoveryExtractionRunner;
impl RecoveryExtractionRunner for BoundedRecoveryExtractionRunner {
    fn run(&self, invocation: &ExtractionInvocation, cancellation: &ProcessCancellationToken)
        -> Result<ExtractionProcessOutput, ExtractionError> {
        let mut request = ProcessRequest::direct(absolute_text(&invocation.executable)?)
            .args(invocation.arguments.clone()).working_directory(&invocation.working_directory)
            .stdout(ProcessOutputMode::capture(invocation.stdout_limit))
            .stderr(ProcessOutputMode::capture(MAX_CAPTURE_BYTES))
            .timeout(invocation.timeout).cancellation(cancellation.clone())
            .environment(ProcessEnvironment::clear().set("LC_ALL", "C").set("TZ", "UTC").set("OMP_THREAD_LIMIT", "1"))
            .network_policy(ProcessNetworkPolicy::Deny);
        if let Some((path, maximum)) = &invocation.expected_output {
            request = request.expect_output(ProcessExpectedOutput::file(path)
                .require_non_empty().require_change().max_bytes(*maximum));
        }
        let output = ProcessExecutor::new().execute(request)
            .map_err(|error| ExtractionError::ProviderFailed(error.to_string()))?;
        Ok(ExtractionProcessOutput { termination: output.termination(),
            stdout: output.stdout().bytes().to_vec(), stderr: output.stderr().bytes().to_vec(),
            capture_truncated: output.stdout().truncated() || output.stderr().truncated(),
            output_failures: output.output_failures().to_vec() })
    }
}

pub fn extract(request: &ExtractionRequest, cancellation: ProcessCancellationToken)
    -> Result<ExtractedDocument, ExtractionError> {
    extract_with_runner(request, &BoundedRecoveryExtractionRunner, cancellation)
}

/// Replay the evidence contract without reading files or launching a process.
/// This verifies internal binding to source/policy/pages, not native truth or
/// external provenance; the original execution remains authored/unqualified.
pub fn validate_extraction_evidence(pages: &[ExtractedPage], evidence: &ExtractionEvidence,
    config: &RecoveryExtractionConfig, expected_source_sha256: &str) -> Result<(), ExtractionError> {
    config.validate()?;
    expected_digest(expected_source_sha256)?;
    enforce_text_limits(pages, &config.limits)?;
    let invalid = |message: &str| ExtractionError::InvalidOutput(message.to_owned());
    if evidence.schema_version != EXTRACTION_EVIDENCE_SCHEMA
        || evidence.source_sha256 != expected_source_sha256
        || evidence.config_sha256 != digest(&serde_json::to_vec(config)?)
        || evidence.pages_sha256 != digest(&serde_json::to_vec(pages)?)
        || evidence.source_bytes == 0 || evidence.source_bytes > config.limits.max_input_bytes
        || evidence.pages.len() != pages.len() || !evidence.review_required || !evidence.source_untouched
        || evidence.qualification != "authored_unqualified" {
        return Err(invalid("extraction source, policy, page digest, or review contract differs"));
    }
    let mut total_text = 0_usize;
    let mut expected_providers: Vec<(&str, &PinnedRecoveryTool)> = Vec::new();
    let mut expected_invocations: Vec<(String, Option<u32>, Vec<String>)> = Vec::new();
    if evidence.source_format == ExtractionSourceFormat::Pdf {
        let tool = config.pdftotext.as_ref().ok_or_else(|| invalid("PDF evidence has no pinned text provider"))?;
        expected_providers.push(("pdftotext", tool));
        expected_invocations.push(("pdftotext".to_owned(), None, vec!["-v".to_owned()]));
    }
    let mut used_ocr = false;
    for (index, (page, recorded)) in pages.iter().zip(&evidence.pages).enumerate() {
        let number = (index + 1) as u32;
        if page.page_number != number || recorded.page_number != number || page.method != recorded.method
            || page.text.len() != recorded.text_bytes || digest(page.text.as_bytes()) != recorded.text_sha256
            || usable_text(&page.text) != recorded.usable_text || output_text(page.text.as_bytes())? != page.text {
            return Err(invalid("extraction page identity, method, text, or digest differs"));
        }
        total_text += page.text.len();
        match (evidence.source_format, page.method) {
            (ExtractionSourceFormat::Text, ExtractionMethod::PlainText) => {
                if recorded.embedded_text.is_some() || recorded.embedded_text_sha256.is_some()
                    || recorded.raster_sha256.is_some() || recorded.raster_dimensions.is_some() {
                    return Err(invalid("plain text evidence unexpectedly declares PDF/OCR content"));
                }
            }
            (ExtractionSourceFormat::Pdf, ExtractionMethod::EmbeddedText | ExtractionMethod::Ocr) => {
                expected_invocations.push(("pdftotext".to_owned(), Some(number), text_arguments(number)));
                if page.method == ExtractionMethod::EmbeddedText {
                    if recorded.embedded_text_sha256.as_deref() != Some(recorded.text_sha256.as_str())
                        || recorded.embedded_text.is_some() || recorded.raster_sha256.is_some() || recorded.raster_dimensions.is_some()
                        || (!recorded.usable_text && config.ocr.is_some()) {
                        return Err(invalid("embedded text evidence violates its selected extraction route"));
                    }
                } else {
                    let ocr = config.ocr.as_ref().ok_or_else(|| invalid("OCR evidence has no explicit policy"))?;
                    let embedded = recorded.embedded_text.as_ref().ok_or_else(|| invalid("OCR evidence lost its embedded extraction"))?;
                    if usable_text(embedded) || embedded.len() > config.limits.max_page_text_bytes
                        || output_text(embedded.as_bytes())? != *embedded
                        || recorded.embedded_text_sha256.as_deref() != Some(digest(embedded.as_bytes()).as_str()) {
                        return Err(invalid("OCR fallback lacks the exact non-usable embedded text"));
                    }
                    total_text += embedded.len();
                    expected_digest(recorded.raster_sha256.as_deref().ok_or_else(|| invalid("OCR raster digest missing"))?)?;
                    let [width, height] = recorded.raster_dimensions.ok_or_else(|| invalid("OCR raster dimensions missing"))?;
                    if width == 0 || height == 0 || width > config.limits.max_raster_side || height > config.limits.max_raster_side {
                        return Err(invalid("OCR raster dimensions exceed the policy"));
                    }
                    if !used_ocr {
                        expected_providers.push(("pdftoppm", &ocr.pdftoppm));
                        expected_providers.push(("tesseract", &ocr.tesseract));
                        expected_invocations.push(("pdftoppm".to_owned(), None, vec!["-v".to_owned()]));
                        expected_invocations.push(("tesseract".to_owned(), None, vec!["--version".to_owned()]));
                        used_ocr = true;
                    }
                    expected_invocations.push(("pdftoppm".to_owned(), Some(number), raster_arguments(number, config.limits.max_raster_side)));
                    expected_invocations.push(("tesseract".to_owned(), Some(number), ocr_arguments(number, &ocr.language)));
                }
            }
            _ => return Err(invalid("page extraction method does not belong to its source format")),
        }
        if total_text > config.limits.max_total_text_bytes { return Err(invalid("retained page text exceeds its total bound")); }
    }
    let expected_data = if used_ocr { config.ocr.as_ref().map(|ocr| ocr.traineddata_sha256.as_str()) } else { None };
    if evidence.traineddata_sha256.as_deref() != expected_data || evidence.providers.len() != expected_providers.len()
        || evidence.invocations.len() != expected_invocations.len() { return Err(invalid("extraction provider or invocation count differs")); }
    for (actual, (name, tool)) in evidence.providers.iter().zip(expected_providers) {
        if actual.provider_id != format!("tool.{name}") || actual.executable_sha256 != tool.executable_sha256
            || actual.observed_version != tool.expected_version || actual.qualification != "authored_unqualified" {
            return Err(invalid("extraction provider observation differs from its explicit pin"));
        }
    }
    for (actual, (name, page_number, argv)) in evidence.invocations.iter().zip(expected_invocations) {
        if actual.provider_id != format!("tool.{name}") || actual.page_number != page_number || actual.argv != argv
            || actual.argv_sha256 != digest(&serde_json::to_vec(&argv)?) || actual.stderr_bytes > MAX_CAPTURE_BYTES {
            return Err(invalid("extraction command differs from the fixed command contract"));
        }
        expected_digest(&actual.stdout_sha256)?;
        expected_digest(&actual.stderr_sha256)?;
    }
    Ok(())
}

pub fn extract_with_runner(request: &ExtractionRequest, runner: &dyn RecoveryExtractionRunner,
    cancellation: ProcessCancellationToken) -> Result<ExtractedDocument, ExtractionError> {
    request.config.validate()?;
    if let Some(expected) = &request.expected_source_sha256 { expected_digest(expected)?; }
    let budget = Budget { started: Instant::now(), timeout: Duration::from_secs(request.config.limits.timeout_seconds), cancellation: &cancellation };
    budget.check()?;
    let source = Snapshot::read(&request.input, request.config.limits.max_input_bytes, &budget)?;
    if source.bytes.is_empty() { return Err(ExtractionError::InvalidInput("source is empty".to_owned())); }
    if request.expected_source_sha256.as_ref().is_some_and(|expected| expected != &source.sha256) {
        return Err(ExtractionError::SourceChanged("source digest differs from the requested artifact".to_owned()));
    }
    let mut evidence = ExtractionEvidence {
        schema_version: EXTRACTION_EVIDENCE_SCHEMA.to_owned(), source_format: request.source_format,
        source_sha256: source.sha256.clone(), source_bytes: source.bytes.len() as u64,
        config_sha256: digest(&serde_json::to_vec(&request.config)?), pages_sha256: String::new(),
        pages: Vec::new(), providers: Vec::new(), invocations: Vec::new(), traineddata_sha256: None,
        qualification: "authored_unqualified".to_owned(), source_untouched: true, review_required: true,
        limitations: vec!["Text extraction and OCR may lose characters, indentation, reading order, and typography; all recovered structure requires review.".to_owned(),
            "Usable text means at least one alphabetic character; this is a deterministic routing heuristic, not recognition accuracy or page completeness.".to_owned(),
            "Executable hashes and observed versions do not attest linked libraries, native memory bounds, Poppler data, fonts, builds, or network isolation.".to_owned(),
            "PDF loading bounds source bytes and each decompressed stream; object/value limits apply after parsing and are not a process-wide memory or hard parser-time cap.".to_owned()],
    };
    let pages = match request.source_format {
        ExtractionSourceFormat::Text => {
            let pages = text_pages(&source.bytes).map_err(|error| ExtractionError::InvalidInput(error.to_string()))?;
            for page in &pages {
                evidence.pages.push(page_evidence(page, ExtractionMethod::PlainText));
            }
            pages
        }
        ExtractionSourceFormat::Pdf => extract_pdf(&source, request, runner, &budget, &mut evidence)?,
    };
    enforce_text_limits(&pages, &request.config.limits)?;
    source.recheck(&budget)?;
    evidence.pages_sha256 = digest(&serde_json::to_vec(&pages)?);
    validate_extraction_evidence(&pages, &evidence, &request.config, &source.sha256)?;
    budget.check()?;
    Ok(ExtractedDocument { source_sha256: source.sha256, pages, evidence })
}

fn extract_pdf(source: &Snapshot, request: &ExtractionRequest, runner: &dyn RecoveryExtractionRunner,
    budget: &Budget<'_>, evidence: &mut ExtractionEvidence) -> Result<Vec<ExtractedPage>, ExtractionError> {
    let page_count = inspect_pdf_source(&source.bytes, request.config.limits.max_pages, budget)?;
    let config = request.config.pdftotext.as_ref().ok_or_else(|| ExtractionError::ProviderUnavailable("PDF recovery requires an explicit pinned pdftotext configuration".to_owned()))?;
    let temporary = tempfile::Builder::new().prefix("renderflow-recovery-").tempdir()?;
    let directory = fs::canonicalize(temporary.path())?;
    let input_path = directory.join("source.pdf");
    write_new(&input_path, &source.bytes)?;
    let staged = Snapshot::read(&input_path, request.config.limits.max_input_bytes, budget)?;
    if staged.sha256 != source.sha256 { return Err(ExtractionError::SourceChanged("staged PDF differs from the source".to_owned())); }
    let tool = prepare_tool(config, "pdftotext", runner, &directory, budget, evidence)?;
    let mut ocr = None;
    let mut pages = Vec::new();
    let mut total_text = 0_usize;
    for page_number in 1..=page_count {
        budget.check()?;
        staged.recheck(budget)?;
        tool.recheck(budget)?;
        let arguments = text_arguments(page_number as u32);
        let output = invoke(runner, &tool, "pdftotext", arguments, &directory,
            request.config.limits.max_page_text_bytes, None, Some(page_number as u32), budget, evidence)?;
        let embedded = output_text(&output.stdout)?;
        let mut page = ExtractedPage { page_number: page_number as u32, text: embedded.clone(), method: ExtractionMethod::EmbeddedText };
        let mut item = page_evidence(&page, ExtractionMethod::EmbeddedText);
        item.embedded_text_sha256 = Some(digest(embedded.as_bytes()));
        if !usable_text(&embedded) {
            if let Some(config) = &request.config.ocr {
                if ocr.is_none() { ocr = Some(prepare_ocr(config, runner, &directory, budget, evidence)?); }
                let prepared = ocr.as_ref().expect("prepared OCR state");
                let (text, raster_hash, dimensions) = ocr_page(page_number as u32, prepared, config, request,
                    runner, &directory, &staged, budget, evidence)?;
                page.text = text;
                page.method = ExtractionMethod::Ocr;
                item.method = ExtractionMethod::Ocr;
                item.embedded_text = Some(embedded);
                item.raster_sha256 = Some(raster_hash);
                item.raster_dimensions = Some(dimensions);
                item.diagnostics.push("Embedded text lacked alphabetic characters; OCR supplied the candidate page text. The embedded extraction is retained for review.".to_owned());
            } else {
                item.diagnostics.push("No alphabetic embedded text was found. OCR is disabled; the page is retained as missing or uncertain content for review.".to_owned());
            }
        }
        item.text_sha256 = digest(page.text.as_bytes());
        item.text_bytes = page.text.len();
        item.usable_text = usable_text(&page.text);
        if !item.usable_text { item.diagnostics.push("This page has no usable recovered text; blank pages and unrecognized scans are not distinguished.".to_owned()); }
        total_text = total_text.checked_add(page.text.len()).and_then(|value| value.checked_add(item.embedded_text.as_ref().map_or(0, String::len)))
            .ok_or_else(|| ExtractionError::LimitExceeded("text byte count overflow".to_owned()))?;
        if total_text > request.config.limits.max_total_text_bytes { return Err(ExtractionError::LimitExceeded("total recovered and retained embedded text".to_owned())); }
        if page.text.len() > request.config.limits.max_page_text_bytes { return Err(ExtractionError::LimitExceeded("page text".to_owned())); }
        evidence.pages.push(item);
        pages.push(page);
    }
    tool.recheck(budget)?;
    staged.recheck(budget)?;
    if let Some(ocr) = &ocr { ocr.recheck(budget)?; }
    if request.config.ocr.is_none() && pages.iter().all(|page| !usable_text(&page.text)) {
        return Err(ExtractionError::ProviderUnavailable("PDF has no usable embedded text; OCR is disabled. Configure pinned pdftoppm, Tesseract 5, and a traineddata file to request OCR recovery".to_owned()));
    }
    Ok(pages)
}

struct PreparedOcr { rasterizer: Snapshot, recognizer: Snapshot, original_data: Snapshot, staged_data: Snapshot }
impl PreparedOcr {
    fn recheck(&self, budget: &Budget<'_>) -> Result<(), ExtractionError> {
        for snapshot in [&self.rasterizer, &self.recognizer, &self.original_data, &self.staged_data] { snapshot.recheck(budget)?; }
        Ok(())
    }
}
fn prepare_ocr(config: &RecoveryOcrConfig, runner: &dyn RecoveryExtractionRunner, directory: &Path,
    budget: &Budget<'_>, evidence: &mut ExtractionEvidence) -> Result<PreparedOcr, ExtractionError> {
    let original_data = provider_snapshot(&config.traineddata, &config.traineddata_sha256, budget)?;
    let data_directory = directory.join("tessdata");
    fs::create_dir(&data_directory)?;
    let data_path = data_directory.join(format!("{}.traineddata", config.language));
    write_new(&data_path, &original_data.bytes)?;
    let staged_data = Snapshot::read(&data_path, MAX_NATIVE_BYTES, budget)?;
    if staged_data.sha256 != original_data.sha256 { return Err(ExtractionError::SourceChanged("staged traineddata differs".to_owned())); }
    let rasterizer = prepare_tool(&config.pdftoppm, "pdftoppm", runner, directory, budget, evidence)?;
    let recognizer = prepare_tool(&config.tesseract, "tesseract", runner, directory, budget, evidence)?;
    evidence.traineddata_sha256 = Some(staged_data.sha256.clone());
    Ok(PreparedOcr { rasterizer, recognizer, original_data, staged_data })
}

fn ocr_page(page_number: u32, prepared: &PreparedOcr, config: &RecoveryOcrConfig, request: &ExtractionRequest,
    runner: &dyn RecoveryExtractionRunner, directory: &Path, source: &Snapshot,
    budget: &Budget<'_>, evidence: &mut ExtractionEvidence) -> Result<(String, String, [u32; 2]), ExtractionError> {
    prepared.recheck(budget)?;
    source.recheck(budget)?;
    let prefix = format!("page-{page_number}");
    let image_name = format!("{prefix}.pgm");
    let image_path = directory.join(&image_name);
    ensure_missing(&image_path)?;
    let side = request.config.limits.max_raster_side;
    let raster_limit = u64::from(side) * u64::from(side) + 4096;
    let arguments = raster_arguments(page_number, side);
    invoke(runner, &prepared.rasterizer, "pdftoppm", arguments, directory, MAX_CAPTURE_BYTES,
        Some((image_path.clone(), raster_limit)), Some(page_number), budget, evidence)?;
    let raster = Snapshot::read(&image_path, raster_limit, budget)?;
    let dimensions = inspect_pgm(&raster.bytes, side)?;
    prepared.recheck(budget)?;
    source.recheck(budget)?;
    let arguments = ocr_arguments(page_number, &config.language);
    let output = invoke(runner, &prepared.recognizer, "tesseract", arguments, directory,
        request.config.limits.max_page_text_bytes, None, Some(page_number), budget, evidence)?;
    raster.recheck(budget)?;
    prepared.recheck(budget)?;
    source.recheck(budget)?;
    let text = output_text(&output.stdout)?;
    fs::remove_file(&image_path)?;
    Ok((text, raster.sha256, dimensions))
}

fn prepare_tool(config: &PinnedRecoveryTool, name: &str, runner: &dyn RecoveryExtractionRunner,
    directory: &Path, budget: &Budget<'_>, evidence: &mut ExtractionEvidence) -> Result<Snapshot, ExtractionError> {
    let tool = provider_snapshot(&config.executable, &config.executable_sha256, budget)?;
    let arguments = vec![if name == "tesseract" { "--version" } else { "-v" }.to_owned()];
    let invocation = ExtractionInvocation { executable: tool.path.clone(), arguments: arguments.clone(), working_directory: directory.to_owned(),
        timeout: budget.remaining()?.min(VERSION_TIMEOUT), stdout_limit: MAX_CAPTURE_BYTES, expected_output: None };
    let result = run_checked(runner, &invocation, budget)?;
    let observed = parse_version(&result, name, &config.expected_version)?;
    tool.recheck(budget)?;
    evidence.providers.push(ExtractionProviderEvidence { provider_id: format!("tool.{name}"),
        executable_sha256: tool.sha256.clone(), observed_version: observed, qualification: "authored_unqualified".to_owned() });
    evidence.invocations.push(invocation_evidence(name, None, arguments, &result)?);
    Ok(tool)
}

fn invoke(runner: &dyn RecoveryExtractionRunner, tool: &Snapshot, name: &str, arguments: Vec<String>,
    directory: &Path, stdout_limit: usize, expected_output: Option<(PathBuf, u64)>, page_number: Option<u32>,
    budget: &Budget<'_>, evidence: &mut ExtractionEvidence) -> Result<ExtractionProcessOutput, ExtractionError> {
    tool.recheck(budget)?;
    if let Some((path, _)) = &expected_output { ensure_missing(path)?; }
    let invocation = ExtractionInvocation { executable: tool.path.clone(), arguments: arguments.clone(),
        working_directory: directory.to_owned(), timeout: budget.remaining()?, stdout_limit, expected_output };
    let result = run_checked(runner, &invocation, budget)?;
    tool.recheck(budget)?;
    evidence.invocations.push(invocation_evidence(name, page_number, arguments, &result)?);
    Ok(result)
}
fn invocation_evidence(name: &str, page_number: Option<u32>, argv: Vec<String>, output: &ExtractionProcessOutput)
    -> Result<ExtractionInvocationEvidence, ExtractionError> {
    Ok(ExtractionInvocationEvidence { provider_id: format!("tool.{name}"), page_number,
        argv_sha256: digest(&serde_json::to_vec(&argv)?), argv, stdout_sha256: digest(&output.stdout),
        stderr_sha256: digest(&output.stderr), stderr_bytes: output.stderr.len() })
}
fn run_checked(runner: &dyn RecoveryExtractionRunner, invocation: &ExtractionInvocation, budget: &Budget<'_>)
    -> Result<ExtractionProcessOutput, ExtractionError> {
    budget.check()?;
    let output = runner.run(invocation, budget.cancellation)?;
    budget.check()?;
    match output.termination {
        ProcessTermination::Cancelled => return Err(ExtractionError::Cancelled),
        ProcessTermination::TimedOut => return Err(ExtractionError::TimedOut),
        ProcessTermination::OutputLimitExceeded => return Err(ExtractionError::LimitExceeded("native output bytes".to_owned())),
        _ => {}
    }
    if output.capture_truncated || output.stdout.len() > invocation.stdout_limit || output.stderr.len() > MAX_CAPTURE_BYTES {
        return Err(ExtractionError::LimitExceeded("native output capture; no truncated text is accepted".to_owned()));
    }
    if !output.termination.is_success() || !output.output_failures.is_empty() {
        return Err(ExtractionError::ProviderFailed(format!("process terminated as {:?}; output failures: {}", output.termination, output.output_failures.len())));
    }
    Ok(output)
}

fn parse_version(output: &ExtractionProcessOutput, name: &str, expected: &str) -> Result<String, ExtractionError> {
    let bytes = if name == "tesseract" { &output.stdout } else { &output.stderr };
    let text = std::str::from_utf8(bytes).map_err(|_| ExtractionError::ProviderUnavailable("version output is not UTF-8".to_owned()))?;
    let expected_line = if name == "tesseract" { format!("tesseract {expected}") } else { format!("{name} version {expected}") };
    if text.lines().next() != Some(expected_line.as_str()) {
        return Err(ExtractionError::ProviderUnavailable(format!("expected exact version banner {expected_line}")));
    }
    Ok(expected.to_owned())
}

fn text_arguments(page_number: u32) -> Vec<String> {
    vec!["-f".to_owned(), page_number.to_string(), "-l".to_owned(), page_number.to_string(),
        "-layout".to_owned(), "-enc".to_owned(), "UTF-8".to_owned(), "-eol".to_owned(), "unix".to_owned(),
        "-nopgbrk".to_owned(), "source.pdf".to_owned(), "-".to_owned()]
}
fn raster_arguments(page_number: u32, side: u32) -> Vec<String> {
    vec!["-f".to_owned(), page_number.to_string(), "-l".to_owned(), page_number.to_string(),
        "-singlefile".to_owned(), "-scale-to".to_owned(), side.to_string(), "-gray".to_owned(),
        "source.pdf".to_owned(), format!("page-{page_number}")]
}
fn ocr_arguments(page_number: u32, language: &str) -> Vec<String> {
    vec![format!("page-{page_number}.pgm"), "stdout".to_owned(), "--tessdata-dir".to_owned(), "tessdata".to_owned(),
        "-l".to_owned(), language.to_owned(), "--oem".to_owned(), "1".to_owned(), "--psm".to_owned(), "3".to_owned()]
}

fn output_text(bytes: &[u8]) -> Result<String, ExtractionError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ExtractionError::InvalidOutput("extracted text is not UTF-8".to_owned()))?;
    if text.chars().any(|value| value.is_control() && value != '\n' && value != '\r' && value != '\t') {
        return Err(ExtractionError::InvalidOutput("one-page extraction contains a page break or unsupported control".to_owned()));
    }
    let normalized = text.replace("\r\n", "\n");
    if normalized.contains('\r') { return Err(ExtractionError::InvalidOutput("bare carriage returns are not supported".to_owned())); }
    Ok(normalized)
}
fn usable_text(value: &str) -> bool { value.chars().any(char::is_alphabetic) }
fn page_evidence(page: &ExtractedPage, method: ExtractionMethod) -> ExtractionPageEvidence {
    ExtractionPageEvidence { page_number: page.page_number, method, text_sha256: digest(page.text.as_bytes()),
        text_bytes: page.text.len(), embedded_text_sha256: None, embedded_text: None,
        usable_text: usable_text(&page.text), raster_sha256: None, raster_dimensions: None, diagnostics: Vec::new() }
}
fn enforce_text_limits(pages: &[ExtractedPage], limits: &ExtractionLimits) -> Result<(), ExtractionError> {
    if pages.is_empty() || pages.len() > limits.max_pages { return Err(ExtractionError::LimitExceeded("page count".to_owned())); }
    let mut total = 0_usize;
    for page in pages {
        if page.text.len() > limits.max_page_text_bytes { return Err(ExtractionError::LimitExceeded("page text".to_owned())); }
        total = total.checked_add(page.text.len()).ok_or_else(|| ExtractionError::LimitExceeded("text length overflow".to_owned()))?;
        if total > limits.max_total_text_bytes { return Err(ExtractionError::LimitExceeded("total text".to_owned())); }
    }
    Ok(())
}

/// Validate only the binary 8-bit grayscale profile requested from pdftoppm.
fn inspect_pgm(bytes: &[u8], side: u32) -> Result<[u32; 2], ExtractionError> {
    fn token<'a>(bytes: &'a [u8], position: &mut usize) -> Result<&'a [u8], ExtractionError> {
        loop {
            while *position < bytes.len() && bytes[*position].is_ascii_whitespace() { *position += 1; }
            if bytes.get(*position) == Some(&b'#') {
                while *position < bytes.len() && bytes[*position] != b'\n' { *position += 1; }
            } else { break; }
            if *position > 4096 { return Err(ExtractionError::InvalidOutput("PGM header exceeds bound".to_owned())); }
        }
        let start = *position;
        while *position < bytes.len() && !bytes[*position].is_ascii_whitespace() { *position += 1; }
        if *position > 4096 || start == *position { return Err(ExtractionError::InvalidOutput("invalid PGM header".to_owned())); }
        Ok(&bytes[start..*position])
    }
    let mut position = 0;
    if token(bytes, &mut position)? != b"P5" { return Err(ExtractionError::InvalidOutput("raster is not binary grayscale PGM".to_owned())); }
    let width = std::str::from_utf8(token(bytes, &mut position)?).ok().and_then(|value| value.parse::<u32>().ok());
    let height = std::str::from_utf8(token(bytes, &mut position)?).ok().and_then(|value| value.parse::<u32>().ok());
    if token(bytes, &mut position)? != b"255" { return Err(ExtractionError::InvalidOutput("raster must have 8-bit pixels".to_owned())); }
    let (Some(width), Some(height)) = (width, height) else { return Err(ExtractionError::InvalidOutput("PGM dimensions are invalid".to_owned())); };
    if width == 0 || height == 0 || width > side || height > side { return Err(ExtractionError::LimitExceeded("raster dimensions".to_owned())); }
    match bytes.get(position) {
        Some(b'\r') if bytes.get(position + 1) == Some(&b'\n') => position += 2,
        Some(value) if value.is_ascii_whitespace() => position += 1,
        _ => return Err(ExtractionError::InvalidOutput("PGM raster separator is missing".to_owned())),
    }
    let pixels = u64::from(width) * u64::from(height);
    if bytes.len().saturating_sub(position) as u64 != pixels {
        return Err(ExtractionError::InvalidOutput("PGM pixel count differs from its header".to_owned()));
    }
    Ok([width, height])
}

fn inspect_pdf_source(bytes: &[u8], max_pages: usize, budget: &Budget<'_>) -> Result<usize, ExtractionError> {
    budget.check()?;
    if !bytes.starts_with(b"%PDF-") { return Err(ExtractionError::MalformedPdf("missing PDF header".to_owned())); }
    let end = bytes.iter().rposition(|value| !value.is_ascii_whitespace()).map_or(0, |position| position + 1);
    if !bytes[..end].ends_with(b"%%EOF") { return Err(ExtractionError::MalformedPdf("missing PDF EOF marker".to_owned())); }
    let document = Document::load_mem_with_options(bytes, LoadOptions { strict: true,
        max_decompressed_size: Some(MAX_PDF_DECOMPRESSED_BYTES), ..LoadOptions::default() })
        .map_err(|error| ExtractionError::MalformedPdf(error.to_string()))?;
    budget.check()?;
    if document.is_encrypted() || document.trailer.has(b"Encrypt") { return Err(ExtractionError::EncryptedPdf); }
    if document.objects.len() > MAX_PDF_OBJECTS { return Err(ExtractionError::LimitExceeded("PDF object count".to_owned())); }
    let mut values = 0;
    for object in document.objects.values() { inspect_object(object, 0, &mut values, budget)?; }
    let root = document.trailer.get(b"Root").and_then(Object::as_reference).map_err(malformed)?;
    let catalog = document.objects.get(&root).ok_or_else(|| malformed("missing PDF catalog"))?.as_dict().map_err(malformed)?;
    if catalog.get(b"Type").and_then(Object::as_name).ok() != Some(b"Catalog") { return Err(malformed("PDF root is not a Catalog")); }
    let pages = catalog.get(b"Pages").and_then(Object::as_reference).map_err(malformed)?;
    collect_pages(&document, pages, None, 0, &mut HashSet::new(), max_pages, budget)
}
fn inspect_object(object: &Object, depth: usize, count: &mut usize, budget: &Budget<'_>) -> Result<(), ExtractionError> {
    budget.check()?;
    *count += 1;
    if depth > MAX_PDF_DEPTH || *count > MAX_PDF_VALUES { return Err(ExtractionError::LimitExceeded("PDF object nesting/value count".to_owned())); }
    match object {
        Object::Array(values) => { for value in values { inspect_object(value, depth + 1, count, budget)?; } }
        Object::Dictionary(dictionary) => { for (key, value) in dictionary.iter() {
            if matches!(key.as_slice(), b"OpenAction" | b"AA" | b"JS" | b"JavaScript" | b"XFA" | b"EmbeddedFiles" | b"RichMedia" | b"Launch") {
                return Err(ExtractionError::UnsupportedInput("active content or embedded documents are outside the recovery PDF profile".to_owned()));
            }
            if key.as_slice() == b"S" && value.as_name().is_ok_and(|name| matches!(name,
                b"Launch" | b"JavaScript" | b"SubmitForm" | b"ImportData" | b"Rendition" | b"Movie" | b"Sound")) {
                return Err(ExtractionError::UnsupportedInput("active PDF action is outside the recovery profile".to_owned()));
            }
            inspect_object(value, depth + 1, count, budget)?;
        } }
        Object::Stream(stream) => {
            if stream.dict.has(b"F") || stream.dict.has(b"FFilter") || stream.dict.has(b"FDecodeParms") {
                return Err(ExtractionError::UnsupportedInput("external PDF streams are unsupported".to_owned()));
            }
            inspect_object(&Object::Dictionary(stream.dict.clone()), depth + 1, count, budget)?;
        }
        _ => {}
    }
    Ok(())
}
fn collect_pages(document: &Document, id: ObjectId, parent: Option<ObjectId>, depth: usize,
    seen: &mut HashSet<ObjectId>, max_pages: usize, budget: &Budget<'_>) -> Result<usize, ExtractionError> {
    budget.check()?;
    if depth >= MAX_PDF_DEPTH || !seen.insert(id) || seen.len() > max_pages * 2 + MAX_PDF_DEPTH {
        return Err(malformed("cyclic, repeated, or excessive PDF page tree"));
    }
    let node = document.objects.get(&id).ok_or_else(|| malformed("missing PDF page tree node"))?.as_dict().map_err(malformed)?;
    match parent {
        Some(parent) if node.get(b"Parent").and_then(Object::as_reference).ok() == Some(parent) => {}
        None if !node.has(b"Parent") => {}
        _ => return Err(malformed("PDF page parent disagrees with the page tree")),
    }
    match node.get(b"Type").and_then(Object::as_name).map_err(malformed)? {
        b"Page" if parent.is_some() && !node.has(b"Kids") => Ok(1),
        b"Pages" => {
            let declared = node.get(b"Count").and_then(Object::as_i64).map_err(malformed)?;
            if declared < 1 || declared as u64 > max_pages as u64 { return Err(ExtractionError::LimitExceeded("PDF page count".to_owned())); }
            let children = node.get(b"Kids").and_then(Object::as_array).map_err(malformed)?;
            if children.is_empty() || children.len() > max_pages { return Err(malformed("invalid PDF page children")); }
            let mut actual = 0;
            for child in children {
                actual += collect_pages(document, child.as_reference().map_err(malformed)?, Some(id), depth + 1, seen, max_pages, budget)?;
                if actual > max_pages { return Err(ExtractionError::LimitExceeded("PDF page count".to_owned())); }
            }
            if actual as i64 != declared { return Err(malformed("PDF declared page count differs from page tree")); }
            Ok(actual)
        }
        _ => Err(malformed("invalid PDF page node type")),
    }
}
fn malformed(message: impl std::fmt::Display) -> ExtractionError { ExtractionError::MalformedPdf(message.to_string()) }

struct Budget<'a> { started: Instant, timeout: Duration, cancellation: &'a ProcessCancellationToken }
impl Budget<'_> {
    fn check(&self) -> Result<(), ExtractionError> {
        if self.cancellation.is_cancelled() { return Err(ExtractionError::Cancelled); }
        if self.started.elapsed() >= self.timeout { return Err(ExtractionError::TimedOut); }
        Ok(())
    }
    fn remaining(&self) -> Result<Duration, ExtractionError> { self.check()?; Ok(self.timeout.saturating_sub(self.started.elapsed())) }
}
struct Snapshot { original: PathBuf, path: PathBuf, maximum: u64, metadata: Metadata, sha256: String, bytes: Vec<u8> }
impl Snapshot {
    fn read(path: &Path, maximum: u64, budget: &Budget<'_>) -> Result<Self, ExtractionError> {
        budget.check()?;
        let initial = regular_metadata(path)?;
        if initial.len() > maximum { return Err(ExtractionError::LimitExceeded("file byte limit".to_owned())); }
        let resolved = fs::canonicalize(path)?;
        let mut file = File::open(&resolved)?;
        if !same_metadata(&initial, &file.metadata()?) { return Err(ExtractionError::SourceChanged("file identity changed while opening".to_owned())); }
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            budget.check()?;
            let count = file.read(&mut buffer)?;
            if count == 0 { break; }
            if bytes.len() as u64 + count as u64 > maximum { return Err(ExtractionError::LimitExceeded("file grew beyond byte limit".to_owned())); }
            bytes.extend_from_slice(&buffer[..count]);
        }
        if bytes.len() as u64 != initial.len() || !same_metadata(&initial, &file.metadata()?)
            || !same_metadata(&initial, &regular_metadata(path)?) || !same_metadata(&initial, &regular_metadata(&resolved)?) {
            return Err(ExtractionError::SourceChanged("file changed while reading".to_owned()));
        }
        budget.check()?;
        Ok(Self { original: path.to_owned(), path: resolved, maximum, metadata: initial, sha256: digest(&bytes), bytes })
    }
    fn recheck(&self, budget: &Budget<'_>) -> Result<(), ExtractionError> {
        let current = Self::read(&self.original, self.maximum, budget)?;
        if current.path != self.path || current.sha256 != self.sha256 || !same_metadata(&self.metadata, &current.metadata) {
            return Err(ExtractionError::SourceChanged("source, pinned tool/data, or staged file changed".to_owned()));
        }
        Ok(())
    }
}
fn provider_snapshot(path: &Path, expected: &str, budget: &Budget<'_>) -> Result<Snapshot, ExtractionError> {
    let snapshot = Snapshot::read(path, MAX_NATIVE_BYTES, budget).map_err(|error| match error {
        ExtractionError::Io(error) => ExtractionError::ProviderUnavailable(error.to_string()), other => other,
    })?;
    if snapshot.bytes.is_empty() || snapshot.sha256 != expected {
        return Err(ExtractionError::ProviderUnavailable("configured tool/data bytes do not match the explicit SHA-256 pin".to_owned()));
    }
    Ok(snapshot)
}
fn regular_metadata(path: &Path) -> Result<Metadata, ExtractionError> {
    let value = fs::symlink_metadata(path)?;
    if !value.is_file() || value.file_type().is_symlink() { return Err(ExtractionError::InvalidRequest("input and configured files must be regular with non-symlink final components".to_owned())); }
    Ok(value)
}
fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    if left.len() != right.len() || left.modified().ok() != right.modified().ok() || !left.is_file() || !right.is_file() { return false; }
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev() || left.ino() != right.ino() || left.ctime() != right.ctime() || left.ctime_nsec() != right.ctime_nsec() { return false; }
    }
    true
}
fn validate_tool(tool: &PinnedRecoveryTool, tesseract: bool) -> Result<(), ExtractionError> {
    absolute_text(&tool.executable)?;
    expected_digest(&tool.executable_sha256)?;
    let numbers: Vec<&str> = tool.expected_version.split('.').collect();
    // Poppler uses zero-padded calendar releases (for example 25.01.0).
    if numbers.len() != 3 || !numbers.iter().all(|part| !part.is_empty() && part.len() <= 4
        && part.bytes().all(|byte| byte.is_ascii_digit()))
        || (tesseract && numbers.first() != Some(&"5")) {
        return Err(ExtractionError::InvalidRequest("provider version must be one exact numeric release; OCR supports the authored Tesseract 5 command contract".to_owned()));
    }
    Ok(())
}
fn absolute_text(path: &Path) -> Result<&str, ExtractionError> {
    let value = path.to_str().ok_or_else(|| ExtractionError::InvalidRequest("configured paths must be UTF-8".to_owned()))?;
    if !path.is_absolute() || value.chars().any(char::is_control) { return Err(ExtractionError::InvalidRequest("configured paths must be absolute and contain no controls".to_owned())); }
    Ok(value)
}
fn expected_digest(value: &str) -> Result<(), ExtractionError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(ExtractionError::InvalidRequest("pins must be lowercase SHA-256 digests".to_owned()));
    }
    Ok(())
}
fn ensure_missing(path: &Path) -> Result<(), ExtractionError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(ExtractionError::InvalidOutput("staged output path already exists".to_owned())),
    }
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), ExtractionError> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    Ok(())
}
fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
