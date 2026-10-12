//! Explicit, unqualified afterwriting PDF output with pinned Node/package bytes.
//!
//! This is a bounded process adapter, not an OS sandbox. Clearing environment,
//! staging the package, and disabling global module paths/native addons limit
//! accidental ambient configuration; they do not attest arbitrary JavaScript,
//! dependency closure, dynamic libraries, network isolation, or source builds.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::process::{ProcessCancellationToken, ProcessEnvironment, ProcessExecutor,
    ProcessExpectedOutput, ProcessNetworkPolicy, ProcessOutputMode, ProcessRequest, ProcessTermination};
use super::analysis::{analyze_fountain, ScreenplayAnalysis};
use super::installation::{absolute_text, check_cancelled, digest, expected_digest, FileSnapshot, TreeSnapshot};
use super::pdf::{inspect_pdf, ExpectedScreenplayPdf, ScreenplayPdfInspection, MAX_SCREENPLAY_PDF_BYTES};
pub use super::installation::{package_fingerprint, PackageFingerprint, PACKAGE_TREE_ALGORITHM};

pub const AFTERWRITING_PROVIDER_ID: &str = "tool.afterwriting";
pub const NODE_PROVIDER_ID: &str = "tool.node";
pub const AFTERWRITING_VERSION: &str = "1.17.3";
pub const AFTERWRITING_SOURCE_REF: &str = "f1912c08d0905bac42e1805830a1e72f98b88c2f";
pub const AFTERWRITING_EVIDENCE_SCHEMA: &str = "renderflow.afterwriting-evidence/v1";
pub const AFTERWRITING_CAPABILITY: &str = "screenplay.render.fountain.pdf";
const MAX_NODE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenplayPdfProfile {
    #[serde(rename = "screenplay-a4-v1")]
    A4,
    #[serde(rename = "screenplay-usletter-v1")]
    UsLetter,
}
impl ScreenplayPdfProfile {
    pub fn as_str(self) -> &'static str {
        match self { Self::A4 => "screenplay-a4-v1", Self::UsLetter => "screenplay-usletter-v1" }
    }
    pub fn dimensions(self) -> (f64, f64) {
        match self { Self::A4 => (595.28, 841.89), Self::UsLetter => (612.0, 792.0) }
    }
    /// Fixed, versioned provider settings. Callers cannot supply extra settings.
    pub fn settings(self) -> Value {
        json!({
            "print_profile": match self { Self::A4 => "a4", Self::UsLetter => "usletter" },
            "font_family": "Courier", "print_title_page": false,
            "show_page_numbers": true, "scenes_numbers": "none",
            "print_actions": true, "print_headers": true, "print_dialogues": true,
            "print_sections": false, "print_synopsis": false, "print_notes": false,
            "snippets": {}, "print_header": "", "print_footer": "", "print_watermark": "",
            "use_dual_dialogue": false, "split_dialogue": true,
            "double_space_between_scenes": false, "each_scene_on_new_page": false,
            "number_sections": false, "embolden_scene_headers": false,
            "underline_scene_headers": false, "scene_continuation_top": false,
            "scene_continuation_bottom": false, "text_more": "(MORE)",
            "text_contd": "(CONT'D)", "text_scene_continued": "CONTINUED",
            "merge_empty_lines": true
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AfterwritingConfig {
    pub node_executable: PathBuf,
    pub node_sha256: String,
    pub expected_node_version: String,
    pub package_root: PathBuf,
    pub package_tree_sha256: String,
    pub expected_provider_version: String,
    pub profile: ScreenplayPdfProfile,
    pub timeout_seconds: u64,
    pub allow_unqualified: bool,
}
impl AfterwritingConfig {
    pub fn validate(&self) -> Result<(), AfterwritingError> {
        absolute_text(&self.node_executable)?;
        absolute_text(&self.package_root)?;
        expected_digest(&self.node_sha256, "node_sha256")?;
        expected_digest(&self.package_tree_sha256, "package_tree_sha256")?;
        if !supported_node_version(&self.expected_node_version) {
            return Err(AfterwritingError::InvalidRequest("expected_node_version must be one exact v22.x.y or v24.x.y candidate; no prerelease/range/suffix".to_owned()));
        }
        if self.expected_provider_version != AFTERWRITING_VERSION {
            return Err(AfterwritingError::InvalidRequest("only the authored afterwriting 1.17.3 package contract is supported".to_owned()));
        }
        if !(1..=300).contains(&self.timeout_seconds) {
            return Err(AfterwritingError::InvalidRequest("timeout_seconds must be between 1 and 300".to_owned()));
        }
        if !self.allow_unqualified {
            return Err(AfterwritingError::InvalidRequest("afterwriting requires explicit allow_unqualified=true; native and visual qualification are deferred".to_owned()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AfterwritingRequest {
    pub input: PathBuf,
    pub config: AfterwritingConfig,
    pub expected_source_sha256: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AfterwritingPreflight {
    pub node_version: String,
    pub provider_version: String,
    pub package_tree_sha256: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AfterwritingEvidence {
    pub schema_version: String,
    pub provider_id: String,
    pub provider_version: String,
    pub node_provider_id: String,
    pub node_version: String,
    pub node_sha256: String,
    pub package_tree_sha256: String,
    pub package_tree_algorithm: String,
    pub package_file_count: usize,
    pub package_total_bytes: u64,
    pub adapter_reference_commit: String,
    pub executable_source_attested: bool,
    pub dependency_closure_attested: bool,
    pub source_sha256: String,
    pub output_sha256: String,
    /// SHA-256 of compact JSON serialization of the independent analysis.
    pub analysis_sha256: String,
    pub source_format: String,
    pub target_format: String,
    pub profile: ScreenplayPdfProfile,
    pub config_sha256: String,
    pub settings: Value,
    pub argv: Vec<String>,
    pub argv_sha256: String,
    pub source_element_count: u64,
    pub input_structure_validated: bool,
    pub output_structure_validated: bool,
    pub qualification: String,
    pub fidelity: String,
    pub inspection: ScreenplayPdfInspection,
    pub limitations: Vec<String>,
}
#[derive(Debug)]
pub struct AfterwritingRun {
    pub output: Vec<u8>,
    pub analysis: ScreenplayAnalysis,
    pub evidence: AfterwritingEvidence,
}
#[derive(Debug, Error)]
pub enum AfterwritingError {
    #[error("invalid afterwriting request: {0}")]
    InvalidRequest(String),
    #[error("unsupported screenplay PDF input: {0}")]
    InvalidInput(String),
    #[error("afterwriting provider unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("afterwriting provider failed: {0}")]
    ProviderFailed(String),
    #[error("afterwriting PDF rejected: {0}")]
    InvalidOutput(String),
    #[error("file changed during afterwriting operation: {0}")]
    SourceChanged(String),
    #[error("afterwriting operation was cancelled")]
    Cancelled,
    #[error("afterwriting operation exceeded its time bound")]
    TimedOut,
    #[error("afterwriting I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("afterwriting JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn supported_node_version(value: &str) -> bool {
    let Some(numbers) = value.strip_prefix('v') else { return false; };
    let pieces: Vec<&str> = numbers.split('.').collect();
    pieces.len() == 3 && matches!(pieces[0], "22" | "24")
        && pieces.iter().all(|part| !part.is_empty() && part.len() <= 10
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && (part.len() == 1 || !part.starts_with('0'))
            && part.parse::<u32>().is_ok())
}

pub trait AfterwritingRunner {
    fn run(&self, invocation: &AfterwritingInvocation, cancellation: &ProcessCancellationToken)
        -> Result<AfterwritingProcessOutput, AfterwritingError>;
}
#[derive(Debug)]
pub struct AfterwritingInvocation {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    pub timeout: Duration,
    pub expected_output: Option<PathBuf>,
}
#[derive(Debug)]
pub struct AfterwritingProcessOutput {
    pub termination: ProcessTermination,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub capture_truncated: bool,
    pub output_failures: Vec<String>,
}
pub struct BoundedAfterwritingRunner;
impl AfterwritingRunner for BoundedAfterwritingRunner {
    fn run(&self, invocation: &AfterwritingInvocation, cancellation: &ProcessCancellationToken)
        -> Result<AfterwritingProcessOutput, AfterwritingError> {
        let mut request = ProcessRequest::direct(absolute_text(&invocation.executable)?)
            .args(invocation.arguments.clone()).working_directory(&invocation.working_directory)
            .stdout(ProcessOutputMode::capture(MAX_CAPTURE_BYTES)).stderr(ProcessOutputMode::capture(MAX_CAPTURE_BYTES))
            .timeout(invocation.timeout).cancellation(cancellation.clone())
            .environment(ProcessEnvironment::clear().set("LC_ALL", "C").set("TZ", "UTC"))
            .network_policy(ProcessNetworkPolicy::Deny);
        if let Some(output) = &invocation.expected_output {
            request = request.expect_output(ProcessExpectedOutput::file(output).require_non_empty()
                .require_change().max_bytes(MAX_SCREENPLAY_PDF_BYTES as u64));
        }
        let result = ProcessExecutor::new().execute(request)
            .map_err(|error| AfterwritingError::ProviderFailed(error.to_string()))?;
        Ok(AfterwritingProcessOutput { termination: result.termination(),
            stdout: result.stdout().bytes().to_vec(), stderr: result.stderr().bytes().to_vec(),
            capture_truncated: result.stdout().truncated() || result.stderr().truncated(),
            output_failures: result.output_failures().to_vec() })
    }
}

/// Explicit planner preflight. It hashes supplied bytes and probes only Node,
/// with cleared environment; it never invokes afterwriting or claims qualification.
pub fn preflight(config: &AfterwritingConfig, cancellation: ProcessCancellationToken)
    -> Result<AfterwritingPreflight, AfterwritingError> {
    preflight_with_runner(config, &BoundedAfterwritingRunner, cancellation)
}
pub fn preflight_with_runner(config: &AfterwritingConfig, runner: &dyn AfterwritingRunner,
    cancellation: ProcessCancellationToken) -> Result<AfterwritingPreflight, AfterwritingError> {
    let (node, package) = pinned_installation(config, &cancellation)?;
    let temporary = tempfile::Builder::new().prefix("renderflow-afterwriting-probe-").tempdir()?;
    let working_directory = fs::canonicalize(temporary.path())?;
    let version = probe_node(config, &node, runner, &working_directory, &cancellation)?;
    node.recheck(&cancellation)?;
    package.recheck(&cancellation)?;
    Ok(AfterwritingPreflight { node_version: version, provider_version: AFTERWRITING_VERSION.to_owned(),
        package_tree_sha256: package.fingerprint.tree_sha256,
        file_count: package.fingerprint.file_count, total_bytes: package.fingerprint.total_bytes })
}

pub fn render(request: &AfterwritingRequest, cancellation: ProcessCancellationToken)
    -> Result<AfterwritingRun, AfterwritingError> {
    render_with_runner(request, &BoundedAfterwritingRunner, cancellation)
}
/// Injectable boundary for authored contract coverage, never native qualification.
pub fn render_with_runner(request: &AfterwritingRequest, runner: &dyn AfterwritingRunner,
    cancellation: ProcessCancellationToken) -> Result<AfterwritingRun, AfterwritingError> {
    check_cancelled(&cancellation)?;
    request.config.validate()?;
    if let Some(expected) = &request.expected_source_sha256 { expected_digest(expected, "expected_source_sha256")?; }
    let source = FileSnapshot::read(&request.input, super::MAX_INPUT_BYTES as u64, true, Instant::now(), &cancellation)?;
    if request.expected_source_sha256.as_ref().is_some_and(|expected| expected != &source.sha256) {
        return Err(AfterwritingError::InvalidRequest("source SHA-256 does not match selected artifact".to_owned()));
    }
    let source_bytes = source.bytes(Instant::now(), &cancellation)?;
    let analysis = analyze_fountain(&source_bytes).map_err(|error| AfterwritingError::InvalidInput(error.to_string()))?;
    let (node, package) = pinned_installation(&request.config, &cancellation)?;
    let temporary = tempfile::Builder::new().prefix("renderflow-afterwriting-").tempdir()?;
    let working_directory = fs::canonicalize(temporary.path())?;
    let staged_package = package.stage(&working_directory.join("package"), &cancellation)?;
    fs::write(working_directory.join("source.fountain"), &source_bytes)?;
    let staged_source = FileSnapshot::read(&working_directory.join("source.fountain"), super::MAX_INPUT_BYTES as u64,
        true, Instant::now(), &cancellation)?;
    if staged_source.sha256 != source.sha256 { return Err(AfterwritingError::SourceChanged("staged source differs".to_owned())); }
    let settings = request.config.profile.settings();
    let config_bytes = serde_json::to_vec(&settings)?;
    fs::write(working_directory.join("profile.json"), &config_bytes)?;
    let staged_config = FileSnapshot::read(&working_directory.join("profile.json"), 64 * 1024, true, Instant::now(), &cancellation)?;
    let version = probe_node(&request.config, &node, runner, &working_directory, &cancellation)?;
    for snapshot in [&source, &node, &staged_source, &staged_config] { snapshot.recheck(&cancellation)?; }
    package.recheck(&cancellation)?;
    staged_package.recheck(&cancellation)?;
    let output_path = working_directory.join("output.pdf");
    match fs::symlink_metadata(&output_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(AfterwritingError::InvalidOutput("output path must not exist before rendering".to_owned())),
    }
    let arguments: Vec<String> = ["--no-addons", "--no-global-search-paths", "package/awc.js", "--source",
        "source.fountain", "--pdf", "output.pdf", "--config", "profile.json"].into_iter().map(str::to_owned).collect();
    run_checked(runner, &AfterwritingInvocation { executable: node.path.clone(), arguments: arguments.clone(),
        working_directory, timeout: Duration::from_secs(request.config.timeout_seconds), expected_output: Some(output_path.clone()) }, &cancellation)?;
    for snapshot in [&source, &node, &staged_source, &staged_config] { snapshot.recheck(&cancellation)?; }
    package.recheck(&cancellation)?;
    staged_package.recheck(&cancellation)?;
    let output_snapshot = FileSnapshot::read(&output_path, MAX_SCREENPLAY_PDF_BYTES as u64, true, Instant::now(), &cancellation)?;
    let output = output_snapshot.bytes(Instant::now(), &cancellation)?;
    let (paper_width_pt, paper_height_pt) = request.config.profile.dimensions();
    let inspection = inspect_pdf(&output, ExpectedScreenplayPdf { paper_width_pt, paper_height_pt })
        .map_err(|error| AfterwritingError::InvalidOutput(error.to_string()))?;
    let evidence = AfterwritingEvidence {
        schema_version: AFTERWRITING_EVIDENCE_SCHEMA.to_owned(), provider_id: AFTERWRITING_PROVIDER_ID.to_owned(),
        provider_version: AFTERWRITING_VERSION.to_owned(), node_provider_id: NODE_PROVIDER_ID.to_owned(),
        node_version: version, node_sha256: node.sha256, package_tree_sha256: package.fingerprint.tree_sha256,
        package_tree_algorithm: PACKAGE_TREE_ALGORITHM.to_owned(), package_file_count: package.fingerprint.file_count,
        package_total_bytes: package.fingerprint.total_bytes, adapter_reference_commit: AFTERWRITING_SOURCE_REF.to_owned(),
        executable_source_attested: false, dependency_closure_attested: false,
        source_sha256: source.sha256, output_sha256: digest(&output), analysis_sha256: digest(&serde_json::to_vec(&analysis)?),
        source_format: "fountain".to_owned(), target_format: "pdf".to_owned(), profile: request.config.profile,
        config_sha256: digest(&config_bytes), settings, argv_sha256: digest(&serde_json::to_vec(&arguments)?), argv: arguments,
        source_element_count: analysis.total_elements, input_structure_validated: true, output_structure_validated: true,
        qualification: "authored_unqualified".to_owned(), fidelity: "layout_derivative".to_owned(), inspection,
        limitations: vec![
            "Independent analysis describes accepted source text, not PDF text equivalence, visual correctness, or provider analytics.".to_owned(),
            "Only unstyled ASCII body content is accepted. Title pages, custom fonts, non-ASCII, dual dialogue, notes, and native files are refused.".to_owned(),
            "Courier uses nonembedded base14 PDF fonts; viewer substitution, pagination, wrapping, and typography require visual qualification.".to_owned(),
            "PDF metadata dates and document IDs may vary. Pinned inputs/settings do not guarantee byte-reproducible output.".to_owned(),
            "Pinned bytes do not attest upstream source builds, transitive runtime dependency closure, native libraries, or native application acceptance.".to_owned(),
            "Direct process bounds, clear environment, and network-deny policy are not operating-system or network isolation.".to_owned(),
        ],
    };
    check_cancelled(&cancellation)?;
    Ok(AfterwritingRun { output, analysis, evidence })
}

fn pinned_installation(config: &AfterwritingConfig, cancellation: &ProcessCancellationToken)
    -> Result<(FileSnapshot, TreeSnapshot), AfterwritingError> {
    check_cancelled(cancellation)?;
    config.validate()?;
    let node = FileSnapshot::read(&config.node_executable, MAX_NODE_BYTES, true, Instant::now(), cancellation)?;
    if node.sha256 != config.node_sha256 { return Err(AfterwritingError::ProviderUnavailable("Node SHA-256 does not match its explicit pin".to_owned())); }
    let package = TreeSnapshot::read(&config.package_root, cancellation)?;
    if package.fingerprint.tree_sha256 != config.package_tree_sha256 {
        return Err(AfterwritingError::ProviderUnavailable("installed package tree SHA-256 does not match its explicit pin".to_owned()));
    }
    validate_package(&package, cancellation)?;
    Ok((node, package))
}
fn validate_package(package: &TreeSnapshot, cancellation: &ProcessCancellationToken) -> Result<(), AfterwritingError> {
    let bytes = package.file_bytes("package.json", cancellation)?;
    if bytes.len() > 1024 * 1024 { return Err(AfterwritingError::ProviderUnavailable("package manifest exceeds 1 MiB".to_owned())); }
    let manifest: Value = serde_json::from_slice(&bytes)?;
    if manifest.get("name").and_then(Value::as_str) != Some("afterwriting")
        || manifest.get("bin").and_then(|value| value.get("afterwriting")).and_then(Value::as_str) != Some("./awc.js")
        || manifest.get("version").and_then(Value::as_str) != Some(AFTERWRITING_VERSION) {
        return Err(AfterwritingError::ProviderUnavailable("package.json must declare afterwriting 1.17.3 with bin.afterwriting=./awc.js; no provider --version command exists".to_owned()));
    }
    for path in ["awc.js", "js/bootstrap.js", "js/client/client-config.js", "js/client/options.js", "js/client/awrequire.js",
        "node_modules/pdfkit/package.json", "node_modules/pdfkit/js/pdfkit.js",
        "node_modules/pdfkit/js/data/Courier.afm", "node_modules/pdfkit/js/data/Courier-Bold.afm",
        "node_modules/pdfkit/js/data/Courier-Oblique.afm", "node_modules/pdfkit/js/data/Courier-BoldOblique.afm",
        "node_modules/pdfkit/js/data/Helvetica.afm"] {
        if !package.has_file(path) || package.file_bytes(path, cancellation)?.is_empty() {
            return Err(AfterwritingError::ProviderUnavailable(format!("installed package is missing nonempty required runtime file {path}")));
        }
    }
    let dependencies = manifest.get("dependencies").and_then(Value::as_object)
        .ok_or_else(|| AfterwritingError::ProviderUnavailable("package manifest lacks runtime dependencies".to_owned()))?;
    for name in ["aw-liner", "aw-parser", "d3", "handlebars", "jquery", "lodash", "pdfkit", "protoplast", "requirejs-text", "stdio"] {
        if !dependencies.contains_key(name) {
            return Err(AfterwritingError::ProviderUnavailable(format!("package manifest is missing required runtime dependency {name}")));
        }
    }
    for name in dependencies.keys() {
        if !safe_package_name(name) { return Err(AfterwritingError::ProviderUnavailable("invalid dependency package name".to_owned())); }
        let relative = format!("node_modules/{name}/package.json");
        let dependency_bytes = package.file_bytes(&relative, cancellation)?;
        if dependency_bytes.len() > 1024 * 1024 { return Err(AfterwritingError::ProviderUnavailable("dependency manifest exceeds 1 MiB".to_owned())); }
        let dependency: Value = serde_json::from_slice(&dependency_bytes)?;
        let main = dependency.get("main").and_then(Value::as_str).unwrap_or("index.js");
        let main = main.strip_prefix("./").unwrap_or(main);
        if main.is_empty() || main.starts_with('/') || main.contains(['\\', ':']) || main.chars().any(char::is_control)
            || main.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
            return Err(AfterwritingError::ProviderUnavailable(format!("unsupported dependency entrypoint for {name}")));
        }
        let base = format!("node_modules/{name}/{main}");
        if ![base.clone(), format!("{base}.js"), format!("{base}.json"), format!("{base}/index.js")].iter().any(|path| package.has_file(path)) {
            return Err(AfterwritingError::ProviderUnavailable(format!("dependency {name} lacks a local regular entrypoint")));
        }
    }
    Ok(())
}
fn safe_package_name(name: &str) -> bool {
    let parts: Vec<_> = name.split('/').collect();
    ((parts.len() == 1 && !name.starts_with('@')) || (parts.len() == 2 && name.starts_with('@')))
        && parts.iter().all(|part| !part.is_empty() && *part != "." && *part != ".."
            && part.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'@' | b'-' | b'_' | b'.')))
}
fn probe_node(config: &AfterwritingConfig, node: &FileSnapshot, runner: &dyn AfterwritingRunner,
    working_directory: &Path, cancellation: &ProcessCancellationToken) -> Result<String, AfterwritingError> {
    let result = run_checked(runner, &AfterwritingInvocation { executable: node.path.clone(),
        arguments: vec!["--no-addons".to_owned(), "--no-global-search-paths".to_owned(), "--version".to_owned()],
        working_directory: working_directory.to_owned(), timeout: VERSION_TIMEOUT, expected_output: None }, cancellation)?;
    let output = std::str::from_utf8(&result.stdout).map_err(|_| AfterwritingError::ProviderUnavailable("Node version must be UTF-8".to_owned()))?;
    let line = output.strip_suffix("\n").unwrap_or(output);
    let line = line.strip_suffix("\r").unwrap_or(line);
    if line != config.expected_node_version || !supported_node_version(line) || !result.stderr.is_empty() {
        return Err(AfterwritingError::ProviderUnavailable("expected exactly the declared Node version line and empty stderr".to_owned()));
    }
    Ok(line.to_owned())
}
fn run_checked(runner: &dyn AfterwritingRunner, invocation: &AfterwritingInvocation,
    cancellation: &ProcessCancellationToken) -> Result<AfterwritingProcessOutput, AfterwritingError> {
    check_cancelled(cancellation)?;
    let result = runner.run(invocation, cancellation)?;
    check_cancelled(cancellation)?;
    match result.termination {
        ProcessTermination::Cancelled => return Err(AfterwritingError::Cancelled),
        ProcessTermination::TimedOut => return Err(AfterwritingError::TimedOut),
        _ => {}
    }
    if result.capture_truncated || result.stdout.len() > MAX_CAPTURE_BYTES || result.stderr.len() > MAX_CAPTURE_BYTES {
        return Err(AfterwritingError::ProviderFailed("stdout or stderr exceeded bounded capture".to_owned()));
    }
    if !result.termination.is_success() || !result.output_failures.is_empty() {
        return Err(AfterwritingError::ProviderFailed(format!("process terminated as {:?}; expected-output failures: {}",
            result.termination, result.output_failures.len())));
    }
    Ok(result)
}

#[cfg(test)]
#[path = "afterwriting_tests.rs"]
mod tests;
