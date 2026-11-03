//! Explicit installed-provider adapter. No npm/npx installation or theme lookup.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::process::{ProcessCancellationToken, ProcessEnvironment, ProcessExecutor,
    ProcessNetworkPolicy, ProcessRequest};
// Reuse the existing public installed-package observation API. It identifies
// regular-file bytes under a bounded tree; it does not attest npm closure.
use crate::screenplay::afterwriting::{package_fingerprint, PACKAGE_TREE_ALGORITHM};
use super::{digest, read_file, sha256, PresentationTarget};

pub const SLIDEV_VERSION: &str = "53.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedProgram {
    pub path: PathBuf,
    pub sha256: String,
    /// Exact stdout first line of --version, supplied by the operator.
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlidevProvider {
    pub schema_version: String,
    pub node: PinnedProgram,
    /// npm-style prepared project containing node_modules/@slidev/cli.
    pub installation_root: PathBuf,
    pub installation_sha256: String,
    /// Required for PDF; no browser download fallback is used by this adapter.
    #[serde(default)]
    pub browser: Option<PinnedProgram>,
    pub timeout_seconds: u64,
}

pub fn parse_provider(bytes: &[u8]) -> Result<SlidevProvider> {
    ensure!(bytes.len() <= 64 * 1024, "presentation.provider.config.limit");
    let provider: SlidevProvider = serde_json::from_slice(bytes).context("presentation.provider.config")?;
    validate(&provider)?;
    Ok(provider)
}

fn validate(provider: &SlidevProvider) -> Result<()> {
    ensure!(provider.schema_version == "renderflow.slidev-provider/v1", "presentation.provider.schema");
    ensure!(provider.installation_root.is_absolute(), "presentation.provider.root: use an absolute prepared installation path");
    digest(&provider.installation_sha256)?;
    ensure!((1..=1800).contains(&provider.timeout_seconds), "presentation.provider.timeout: expected 1..1800 seconds");
    for program in std::iter::once(&provider.node).chain(provider.browser.iter()) {
        ensure!(program.path.is_absolute(), "presentation.provider.program: use an absolute path");
        digest(&program.sha256)?;
        ensure!(!program.version.is_empty() && program.version.len() <= 512 && !program.version.chars().any(char::is_control),
            "presentation.provider.version: expected one exact version line");
    }
    let version = provider.node.version.strip_prefix('v').context("presentation.node.version: expected vMAJOR.MINOR.PATCH")?;
    let parts = version.split('.').map(str::parse::<u32>).collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(parts.len() == 3 && (parts[0] > 22 || (parts[0] == 22 && parts[1] >= 12)),
        "presentation.node.version: Slidev 53 requires Node >=22.12.0");
    Ok(())
}

fn observe_program(program: &PinnedProgram, token: &ProcessCancellationToken) -> Result<()> {
    super::render::check_cancelled(token)?;
    ensure!(sha256(&read_file(&program.path, 256 * 1024 * 1024)?) == program.sha256,
        "presentation.provider.program.changed");
    let result = ProcessExecutor::new().execute_checked(ProcessRequest::direct(
        program.path.to_str().context("presentation.provider.path.utf8")?)
        .arg("--version").timeout(Duration::from_secs(5)).capture_limit(16 * 1024)
        .environment(environment()).network_policy(ProcessNetworkPolicy::Deny).cancellation(token.clone()))?;
    ensure!(!result.stdout().truncated(), "presentation.provider.version.truncated");
    let bytes = std::str::from_utf8(result.stdout().bytes())?;
    ensure!(bytes.lines().next().map(str::trim) == Some(program.version.as_str()), "presentation.provider.version.changed");
    Ok(())
}

fn environment() -> ProcessEnvironment {
    // Do not expose arbitrary VITE_* / NODE_OPTIONS values to a publication.
    // Preserve host process lookup/temp paths and the managed proxy/CA route.
    let mut environment = ProcessEnvironment::clear();
    for name in ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP", "TMPDIR",
        "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY", "http_proxy", "https_proxy", "no_proxy",
        "SSL_CERT_FILE", "SSL_CERT_DIR", "NODE_EXTRA_CA_CERTS"] {
        if let Some(value) = std::env::var_os(name) {
            environment = environment.set(name, value.to_string_lossy());
        }
    }
    environment.set("CI", "true").set("TZ", "UTC").set("SOURCE_DATE_EPOCH", "315532800")
        .set("PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD", "1").set("SLIDEV_CLI_NO_OPEN", "1")
}

fn observe_installation(provider: &SlidevProvider, token: &ProcessCancellationToken) -> Result<PathBuf> {
    let observed = package_fingerprint(&provider.installation_root, token.clone())?;
    ensure!(observed.tree_sha256 == provider.installation_sha256, "presentation.provider.installation.changed");
    let package = super::asset_path(&provider.installation_root, "node_modules/@slidev/cli/package.json")?;
    let metadata: Value = serde_json::from_slice(&read_file(&package, 1024 * 1024)?)?;
    ensure!(metadata.get("name").and_then(Value::as_str) == Some("@slidev/cli")
        && metadata.get("version").and_then(Value::as_str) == Some(SLIDEV_VERSION), "presentation.slidev.version: expected @slidev/cli 53.0.0");
    super::asset_path(&provider.installation_root, "node_modules/@slidev/cli/bin/slidev.mjs")
}

pub(super) fn render(provider: &SlidevProvider, project: &Path, targets: &[PresentationTarget],
    slide_count: usize, token: &ProcessCancellationToken) -> Result<Value> {
    validate(provider)?;
    let pdf = targets.contains(&PresentationTarget::Pdf);
    if pdf { ensure!(provider.browser.is_some(), "presentation.browser: PDF needs an explicitly pinned installed browser"); }
    observe_program(&provider.node, token)?;
    if pdf { observe_program(provider.browser.as_ref().context("presentation.browser")?, token)?; }
    let entry = observe_installation(provider, token)?;
    let modules = provider.installation_root.join("node_modules");
    link_directory(&modules, &project.join("node_modules"))?;
    let project = fs::canonicalize(project)?;
    let mut invocations = Vec::new();
    for target in targets {
        super::render::check_cancelled(token)?;
        let mut arguments = vec![entry.to_str().context("presentation.slidev.path.utf8")?.to_string()];
        match target {
            PresentationTarget::Source => continue,
            PresentationTarget::Web => arguments.extend(["build", "slides.md", "--out", "web", "--base", "./", "--without-notes", "--router-mode", "hash"].map(str::to_string)),
            PresentationTarget::Pdf => arguments.extend([
                "export".to_string(), "slides.md".to_string(), "--output".to_string(), "slides.pdf".to_string(),
                "--format".to_string(), "pdf".to_string(), "--with-toc".to_string(),
                "--executable-path".to_string(), provider.browser.as_ref().context("presentation.browser")?.path.to_str().context("presentation.browser.path.utf8")?.to_string(),
                "--timeout".to_string(), "30000".to_string(),
            ]),
        }
        let result = ProcessExecutor::new().execute_checked(ProcessRequest::direct(
            provider.node.path.to_str().context("presentation.node.path.utf8")?)
            .args(arguments).working_directory(&project).environment(environment())
            .network_policy(ProcessNetworkPolicy::Deny)
            .timeout(Duration::from_secs(provider.timeout_seconds)).capture_limit(64 * 1024)
            .cancellation(token.clone()))?;
        invocations.push(json!({"target":target,"exit_success":result.is_success(),
            "stdout_truncated":result.stdout().truncated(),"stderr_truncated":result.stderr().truncated()}));
    }
    // Persistent program/dependency drift invalidates all staged candidates.
    observe_installation(provider, token)?;
    ensure!(sha256(&read_file(&provider.node.path, 256 * 1024 * 1024)?) == provider.node.sha256,
        "presentation.node.changed_after_render");
    if pdf {
        let browser = provider.browser.as_ref().context("presentation.browser")?;
        ensure!(sha256(&read_file(&browser.path, 256 * 1024 * 1024)?) == browser.sha256,
            "presentation.browser.changed_after_render");
    }
    Ok(json!({"adapter":"renderflow.slidev/v1","slidev_version":SLIDEV_VERSION,
        "node_sha256":provider.node.sha256,"node_version":provider.node.version,
        "installation_algorithm":PACKAGE_TREE_ALGORITHM,"installation_sha256":provider.installation_sha256,
        "browser":if pdf { provider.browser.as_ref().map(|p| json!({"sha256":p.sha256,"version":p.version})) } else { None },
        "invocations":invocations,"expected_slides":slide_count,"runtime_qualified":false,
        "network":"deny requested; shared process flag is not an OS sandbox; PDF uses local loopback",
        "limits":"Package bytes and executable digests do not attest dependency closure, browser shared libraries, runtime behavior or platform fidelity. Output inspection is separate from provider exit status."}))
}

#[cfg(unix)]
fn link_directory(source: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}
#[cfg(windows)]
fn link_directory(source: &Path, target: &Path) -> Result<()> {
    std::os::windows::fs::symlink_dir(source, target)?;
    Ok(())
}
#[cfg(not(any(unix, windows)))]
fn link_directory(_source: &Path, _target: &Path) -> Result<()> {
    anyhow::bail!("presentation.platform: prepared installation linking is unsupported")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_refuses_unpinned_programs_and_unsupported_node() {
        let mut provider = SlidevProvider {
            schema_version:"renderflow.slidev-provider/v1".into(),
            node:PinnedProgram {path:std::env::temp_dir().join("node"),sha256:"a".repeat(64),version:"v20.0.0".into()},
            installation_root:std::env::temp_dir().join("prepared"),installation_sha256:"b".repeat(64),browser:None,timeout_seconds:120,
        };
        assert!(validate(&provider).is_err());
        provider.node.version = "v22.12.0".into();
        assert!(validate(&provider).is_ok());
        provider.node.sha256.clear();
        assert!(validate(&provider).is_err());
    }
}
