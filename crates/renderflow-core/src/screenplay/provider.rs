//! Explicit, unqualified scripttool imports into the independently parsed
//! Fountain subset. No executable is selected from PATH or downloaded here.
//!
//! Private staging and file identity checks detect persistent modification;
//! they are not an operating-system sandbox, do not freeze linked libraries,
//! and cannot attest that a supplied executable was built from the source
//! reference below. Network denial is recorded process policy, not isolation.

use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::process::{
    ProcessCancellationToken, ProcessEnvironment, ProcessExecutor,
    ProcessExpectedOutput, ProcessNetworkPolicy, ProcessOutputMode,
    ProcessRequest, ProcessTermination,
};

use super::native::{equivalent_semantics, parse_native, NativeScreenplayFormat};
use super::{Screenplay, ScreenplayLossDirection, ScreenplayLossEvidence, ScreenplayLossKind};

pub const SCRIPTTOOL_PROVIDER_ID: &str = "tool.scripttool";
pub const SCRIPTTOOL_VERSION: &str = "0.0.10";
/// Authored adapter reference, not an assertion about any supplied binary.
pub const SCRIPTTOOL_SOURCE_REF: &str = "ea8ed24020b6f6989e01df44eef64febdfa7bfb2";
pub const SCRIPTTOOL_EVIDENCE_SCHEMA: &str = "renderflow.scripttool-evidence/v1";
pub const MAX_PROVIDER_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_SCREENPLAY_BYTES: u64 = super::MAX_INPUT_BYTES as u64;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const FILE_TIMEOUT: Duration = Duration::from_secs(120);

/// Every field is explicit: this adapter is not enabled by a default provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScripttoolConfig {
    pub executable: PathBuf,
    pub executable_sha256: String,
    pub expected_version: String,
    pub timeout_seconds: u64,
    pub allow_unqualified: bool,
}

impl ScripttoolConfig {
    /// Validate declarations without reading files or launching a probe.
    pub fn validate(&self) -> Result<(), ScripttoolError> {
        path_text(&self.executable)?;
        expected_digest(&self.executable_sha256, "executable_sha256")?;
        if self.expected_version != SCRIPTTOOL_VERSION {
            return Err(ScripttoolError::InvalidRequest(format!(
                "only the authored scripttool {SCRIPTTOOL_VERSION} command contract is supported",
            )));
        }
        if !(1..=300).contains(&self.timeout_seconds) {
            return Err(ScripttoolError::InvalidRequest(
                "timeout_seconds must be between 1 and 300".to_owned(),
            ));
        }
        if !self.allow_unqualified {
            return Err(ScripttoolError::InvalidRequest(
                "scripttool requires explicit allow_unqualified=true; native qualification is deferred".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ScripttoolRequest {
    pub input: PathBuf,
    pub source_format: NativeScreenplayFormat,
    pub target_format: NativeScreenplayFormat,
    pub config: ScripttoolConfig,
    /// Graph callers bind the adapter's observed snapshot to their artifact.
    pub expected_source_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScripttoolEvidence {
    pub schema_version: String,
    pub provider_id: String,
    pub provider_version: String,
    pub provider_sha256: String,
    /// Source inspected while authoring; version output does not prove this.
    pub adapter_reference_commit: String,
    pub executable_source_attested: bool,
    pub source_sha256: String,
    pub output_sha256: String,
    pub source_format: NativeScreenplayFormat,
    pub target_format: NativeScreenplayFormat,
    /// Exact relative argv in the private working directory; never a shell.
    pub argv: Vec<String>,
    /// SHA-256 over the compact JSON serialization of `argv`.
    pub argv_sha256: String,
    pub source_element_count: usize,
    pub output_element_count: usize,
    pub input_structure_validated: bool,
    pub output_structure_validated: bool,
    pub semantic_projection_matched: bool,
    pub qualification: String,
    pub fidelity: String,
    pub losses: Vec<ScreenplayLossEvidence>,
}

#[derive(Debug)]
pub struct ScripttoolRun {
    /// Exact output bytes, independently parsed and compared before return.
    pub output: Vec<u8>,
    pub screenplay: Screenplay,
    pub evidence: ScripttoolEvidence,
}

#[derive(Debug, Error)]
pub enum ScripttoolError {
    #[error("invalid scripttool request: {0}")]
    InvalidRequest(String),
    #[error("unsupported scripttool route: {0}")]
    UnsupportedRoute(String),
    #[error("unsupported screenplay input: {0}")]
    InvalidInput(String),
    #[error("scripttool provider unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("scripttool provider failed: {0}")]
    ProviderFailed(String),
    #[error("scripttool output rejected: {0}")]
    InvalidOutput(String),
    #[error("file changed during scripttool conversion: {0}")]
    SourceChanged(String),
    #[error("scripttool conversion was cancelled")]
    Cancelled,
    #[error("scripttool conversion timed out")]
    TimedOut,
    #[error("scripttool I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("scripttool evidence serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn supported_route(source: NativeScreenplayFormat, target: NativeScreenplayFormat) -> bool {
    target == NativeScreenplayFormat::Fountain
        && matches!(source, NativeScreenplayFormat::Fdx | NativeScreenplayFormat::Osf | NativeScreenplayFormat::FadeIn)
}

pub fn conversion_command(
    source: NativeScreenplayFormat, target: NativeScreenplayFormat,
) -> Result<&'static str, ScripttoolError> {
    if !supported_route(source, target) {
        return Err(ScripttoolError::UnsupportedRoute(
            "only strict FDX, OSF, or Fade In imports to Fountain are authored; reverse native exports and provider JSON are unavailable".to_owned(),
        ));
    }
    match source {
        NativeScreenplayFormat::Fdx => Ok("fdx2fountain"),
        NativeScreenplayFormat::Osf => Ok("osf2fountain"),
        NativeScreenplayFormat::FadeIn => Ok("fadein2fountain"),
        NativeScreenplayFormat::Fountain | NativeScreenplayFormat::Trelby => unreachable!("supported_route rejects Fountain and Trelby input"),
    }
}

/// scripttool prints its invoked basename followed by a single version token.
/// This proves neither source revision nor build provenance.
pub fn supported_version_line(line: &str, expected: &str, executable: &Path) -> bool {
    expected == SCRIPTTOOL_VERSION
        && executable.file_name().and_then(|name| name.to_str())
            .map(|name| line == format!("{name} {expected}"))
            .unwrap_or(false)
        && !line.chars().any(char::is_control)
}

/// A narrow injectable process boundary. Implementations must honor request
/// cancellation and limits; production uses the shared bounded executor below.
pub trait ScripttoolRunner {
    fn run(
        &self, invocation: &ScripttoolInvocation,
        cancellation: &ProcessCancellationToken,
    ) -> Result<ScripttoolProcessOutput, ScripttoolError>;
}

#[derive(Debug)]
pub struct ScripttoolInvocation {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    pub timeout: Duration,
    pub expected_output: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ScripttoolProcessOutput {
    pub termination: ProcessTermination,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub capture_truncated: bool,
    pub output_failures: Vec<String>,
}

pub struct BoundedScripttoolRunner;

impl ScripttoolRunner for BoundedScripttoolRunner {
    fn run(
        &self, invocation: &ScripttoolInvocation,
        cancellation: &ProcessCancellationToken,
    ) -> Result<ScripttoolProcessOutput, ScripttoolError> {
        let mut request = ProcessRequest::direct(path_text(&invocation.executable)?)
            .args(invocation.arguments.clone())
            .working_directory(&invocation.working_directory)
            .stdout(ProcessOutputMode::capture(MAX_CAPTURE_BYTES))
            .stderr(ProcessOutputMode::capture(MAX_CAPTURE_BYTES))
            .timeout(invocation.timeout)
            .cancellation(cancellation.clone())
            .environment(ProcessEnvironment::clear().set("LC_ALL", "C"))
            .network_policy(ProcessNetworkPolicy::Deny);
        if let Some(output) = &invocation.expected_output {
            request = request.expect_output(ProcessExpectedOutput::file(output)
                .require_non_empty().require_change().max_bytes(MAX_SCREENPLAY_BYTES));
        }
        let result = ProcessExecutor::new().execute(request)
            .map_err(|error| ScripttoolError::ProviderFailed(error.to_string()))?;
        Ok(ScripttoolProcessOutput {
            termination: result.termination(),
            stdout: result.stdout().bytes().to_vec(),
            stderr: result.stderr().bytes().to_vec(),
            capture_truncated: result.stdout().truncated() || result.stderr().truncated(),
            output_failures: result.output_failures().to_vec(),
        })
    }
}

pub fn convert(
    request: &ScripttoolRequest, cancellation: ProcessCancellationToken,
) -> Result<ScripttoolRun, ScripttoolError> {
    convert_with_runner(request, &BoundedScripttoolRunner, cancellation)
}

/// Injected runners support contract tests; they confer no native qualification.
pub fn convert_with_runner(
    request: &ScripttoolRequest, runner: &dyn ScripttoolRunner,
    cancellation: ProcessCancellationToken,
) -> Result<ScripttoolRun, ScripttoolError> {
    ensure_not_cancelled(&cancellation)?;
    request.config.validate()?;
    let command = conversion_command(request.source_format, request.target_format)?;
    if let Some(expected) = &request.expected_source_sha256 {
        expected_digest(expected, "expected_source_sha256")?;
    }
    let source = FileSnapshot::read(&request.input, MAX_SCREENPLAY_BYTES, &cancellation)?;
    if request.expected_source_sha256.as_ref().is_some_and(|expected| expected != &source.sha256) {
        return Err(ScripttoolError::InvalidRequest("source SHA-256 does not match the selected artifact".to_owned()));
    }
    let source_bytes = read_bounded(&source.path, MAX_SCREENPLAY_BYTES, &cancellation)?;
    if sha256(&source_bytes) != source.sha256 {
        return Err(ScripttoolError::SourceChanged("source changed before native inspection".to_owned()));
    }
    let input_screenplay = parse_native(&source_bytes, request.source_format)
        .map_err(|error| ScripttoolError::InvalidInput(error.to_string()))?;
    let provider = FileSnapshot::read(&request.config.executable, MAX_PROVIDER_BYTES, &cancellation)?;
    if provider.sha256 != request.config.executable_sha256 {
        return Err(ScripttoolError::ProviderUnavailable("executable SHA-256 does not match its explicit pin".to_owned()));
    }
    let temporary = tempfile::Builder::new().prefix("renderflow-scripttool-").tempdir()?;
    let working_directory = fs::canonicalize(temporary.path())?;
    let source_name = format!("source.{}", request.source_format.extension());
    let staged_path = working_directory.join(&source_name);
    fs::write(&staged_path, &source_bytes)?;
    let staged = FileSnapshot::read(&staged_path, MAX_SCREENPLAY_BYTES, &cancellation)?;
    if staged.sha256 != source.sha256 {
        return Err(ScripttoolError::SourceChanged("staged input does not match source snapshot".to_owned()));
    }
    let output_path = working_directory.join("output.fountain");
    let version = run_checked(runner, &ScripttoolInvocation {
        executable: provider.path.clone(), arguments: vec!["-version".to_owned()],
        working_directory: working_directory.clone(), timeout: VERSION_TIMEOUT,
        expected_output: None,
    }, &cancellation)?;
    let version_line = parse_version(&version.stdout, &request.config.expected_version, &provider.path)?;
    for snapshot in [&source, &provider, &staged] { snapshot.recheck(&cancellation)?; }
    match fs::symlink_metadata(&output_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(ScripttoolError::InvalidOutput("output path must not exist before conversion".to_owned())),
    }
    let arguments = vec![command.to_owned(), "-i".to_owned(), source_name,
        "-o".to_owned(), "output.fountain".to_owned()];
    run_checked(runner, &ScripttoolInvocation {
        executable: provider.path.clone(), arguments: arguments.clone(),
        working_directory, timeout: Duration::from_secs(request.config.timeout_seconds),
        expected_output: Some(output_path.clone()),
    }, &cancellation)?;
    for snapshot in [&source, &provider, &staged] { snapshot.recheck(&cancellation)?; }
    let output = read_bounded(&output_path, MAX_SCREENPLAY_BYTES, &cancellation)?;
    let screenplay = parse_native(&output, request.target_format)
        .map_err(|error| ScripttoolError::InvalidOutput(error.to_string()))?;
    equivalent_semantics(&input_screenplay, &screenplay)
        .map_err(|error| ScripttoolError::InvalidOutput(format!("semantic projection mismatch: {error}")))?;
    let losses = vec![
        loss(request.source_format, ScreenplayLossKind::StableIds,
            "Native source IDs are not preserved; parsed element IDs describe this observed document only."),
        loss(request.source_format, ScreenplayLossKind::Layout,
            "Only the accepted semantic body subset is compared; native page layout, whitespace-only FDX Empty paragraphs, and blank-line spacing are not preserved."),
        loss(request.source_format, ScreenplayLossKind::Pagination,
            "Native pagination and line wrapping are not preserved or qualified."),
        loss(request.source_format, ScreenplayLossKind::SourceProvenance,
            "The source byte digest is retained in this evidence; Fountain output is a distinct derivative."),
    ];
    let evidence = ScripttoolEvidence {
        schema_version: SCRIPTTOOL_EVIDENCE_SCHEMA.to_owned(),
        provider_id: SCRIPTTOOL_PROVIDER_ID.to_owned(), provider_version: version_line,
        provider_sha256: provider.sha256, adapter_reference_commit: SCRIPTTOOL_SOURCE_REF.to_owned(),
        executable_source_attested: false,
        source_sha256: source.sha256, output_sha256: sha256(&output),
        source_format: request.source_format, target_format: request.target_format,
        argv_sha256: sha256(&serde_json::to_vec(&arguments)?), argv: arguments,
        source_element_count: input_screenplay.elements.len(), output_element_count: screenplay.elements.len(),
        input_structure_validated: true, output_structure_validated: true,
        semantic_projection_matched: true, qualification: "authored_unqualified".to_owned(),
        fidelity: "semantic_subset".to_owned(), losses,
    };
    ensure_not_cancelled(&cancellation)?;
    Ok(ScripttoolRun { output, screenplay, evidence })
}

fn loss(format: NativeScreenplayFormat, kind: ScreenplayLossKind, detail: &str) -> ScreenplayLossEvidence {
    ScreenplayLossEvidence {
        direction: ScreenplayLossDirection::Import, format: format.as_str().to_owned(),
        kind, detail: detail.to_owned(),
    }
}

fn run_checked(
    runner: &dyn ScripttoolRunner, invocation: &ScripttoolInvocation,
    cancellation: &ProcessCancellationToken,
) -> Result<ScripttoolProcessOutput, ScripttoolError> {
    ensure_not_cancelled(cancellation)?;
    let result = runner.run(invocation, cancellation)?;
    ensure_not_cancelled(cancellation)?;
    match result.termination {
        ProcessTermination::Cancelled => return Err(ScripttoolError::Cancelled),
        ProcessTermination::TimedOut => return Err(ScripttoolError::TimedOut),
        _ => {}
    }
    if result.capture_truncated || result.stdout.len() > MAX_CAPTURE_BYTES || result.stderr.len() > MAX_CAPTURE_BYTES {
        return Err(ScripttoolError::ProviderFailed("stdout or stderr exceeded its bounded capture limit".to_owned()));
    }
    if !result.termination.is_success() || !result.output_failures.is_empty() {
        return Err(ScripttoolError::ProviderFailed(format!(
            "process terminated as {:?}; expected output failures: {}",
            result.termination, result.output_failures.len(),
        )));
    }
    Ok(result)
}

fn parse_version(bytes: &[u8], expected: &str, executable: &Path) -> Result<String, ScripttoolError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ScripttoolError::ProviderUnavailable("version stdout must be UTF-8".to_owned()))?;
    let line = text.strip_suffix("\n").unwrap_or(text);
    let line = line.strip_suffix("\r").unwrap_or(line);
    if line.len() > 4096 || !supported_version_line(line, expected, executable) {
        return Err(ScripttoolError::ProviderUnavailable(format!(
            "expected one exact executable-basename and {SCRIPTTOOL_VERSION} stdout version line",
        )));
    }
    Ok(line.to_owned())
}

fn path_text(path: &Path) -> Result<&str, ScripttoolError> {
    let text = path.to_str().ok_or_else(|| ScripttoolError::InvalidRequest("native paths must be UTF-8".to_owned()))?;
    if !path.is_absolute() || text.chars().any(char::is_control) {
        return Err(ScripttoolError::InvalidRequest("native executable paths must be absolute and contain no controls".to_owned()));
    }
    Ok(text)
}

fn expected_digest(value: &str, name: &str) -> Result<(), ScripttoolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(ScripttoolError::InvalidRequest(format!("{name} must be a lowercase SHA-256 digest")));
    }
    Ok(())
}

#[derive(Debug)]
struct FileSnapshot { path: PathBuf, maximum: u64, metadata: Metadata, sha256: String }

impl FileSnapshot {
    fn read(path: &Path, maximum: u64, cancellation: &ProcessCancellationToken) -> Result<Self, ScripttoolError> {
        let initial = regular_metadata(path)?;
        let path = fs::canonicalize(path)?;
        path_text(&path)?;
        let (metadata, sha256) = hash_file(&path, maximum, cancellation)?;
        if !same_metadata(&initial, &metadata) {
            return Err(ScripttoolError::SourceChanged("file changed during path resolution".to_owned()));
        }
        Ok(Self { path, maximum, metadata, sha256 })
    }

    fn recheck(&self, cancellation: &ProcessCancellationToken) -> Result<(), ScripttoolError> {
        let (metadata, digest) = hash_file(&self.path, self.maximum, cancellation)?;
        if digest != self.sha256 || !same_metadata(&self.metadata, &metadata) {
            return Err(ScripttoolError::SourceChanged("source, executable, or staged input changed".to_owned()));
        }
        Ok(())
    }
}

fn regular_metadata(path: &Path) -> Result<Metadata, ScripttoolError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ScripttoolError::InvalidRequest("file must be regular with a non-symlink final path component".to_owned()));
    }
    Ok(metadata)
}

fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    if !left.is_file() || !right.is_file() || left.len() != right.len() || left.modified().ok() != right.modified().ok() { return false; }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev() || left.ino() != right.ino() || left.ctime() != right.ctime() || left.ctime_nsec() != right.ctime_nsec() { return false; }
    }
    true
}

fn read_file(
    path: &Path, maximum: u64, retain_bytes: bool, cancellation: &ProcessCancellationToken,
) -> Result<(Metadata, String, Vec<u8>), ScripttoolError> {
    ensure_not_cancelled(cancellation)?;
    let before = regular_metadata(path)?;
    if before.len() == 0 || before.len() > maximum {
        return Err(ScripttoolError::InvalidRequest(format!("file must contain between 1 and {maximum} bytes")));
    }
    let mut file = File::open(path)?;
    if !same_metadata(&before, &file.metadata()?) {
        return Err(ScripttoolError::SourceChanged("file changed while opening".to_owned()));
    }
    let mut digest = Sha256::new();
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    let started = Instant::now();
    loop {
        ensure_not_cancelled(cancellation)?;
        if started.elapsed() >= FILE_TIMEOUT { return Err(ScripttoolError::TimedOut); }
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        total += count as u64;
        if total > maximum { return Err(ScripttoolError::SourceChanged("file grew beyond its byte limit".to_owned())); }
        digest.update(&buffer[..count]);
        if retain_bytes { retained.extend_from_slice(&buffer[..count]); }
    }
    ensure_not_cancelled(cancellation)?;
    if started.elapsed() >= FILE_TIMEOUT { return Err(ScripttoolError::TimedOut); }
    let after = file.metadata()?;
    if total != before.len() || !same_metadata(&before, &after) || !same_metadata(&after, &regular_metadata(path)?) {
        return Err(ScripttoolError::SourceChanged("file changed while reading".to_owned()));
    }
    Ok((before, format!("{:x}", digest.finalize()), retained))
}

fn hash_file(path: &Path, maximum: u64, cancellation: &ProcessCancellationToken) -> Result<(Metadata, String), ScripttoolError> {
    let (metadata, hash, _) = read_file(path, maximum, false, cancellation)?;
    Ok((metadata, hash))
}

fn read_bounded(path: &Path, maximum: u64, cancellation: &ProcessCancellationToken) -> Result<Vec<u8>, ScripttoolError> {
    Ok(read_file(path, maximum, true, cancellation)?.2)
}

fn ensure_not_cancelled(cancellation: &ProcessCancellationToken) -> Result<(), ScripttoolError> {
    if cancellation.is_cancelled() { Err(ScripttoolError::Cancelled) } else { Ok(()) }
}

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
