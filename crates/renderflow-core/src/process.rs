//! Bounded, policy-aware external process execution.
//!
//! All production subprocesses in Renderflow should flow through this module so
//! adapters do not independently reinvent environment handling, output capture,
//! timeout/cancellation behavior, process-tree termination, diagnostics, or
//! expected-output validation.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use tracing::debug;

/// Default wall-clock timeout for ordinary wrapped tools.
pub const DEFAULT_PROCESS_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Default maximum bytes retained independently for stdout and stderr.
pub const DEFAULT_CAPTURE_LIMIT_BYTES: usize = 256 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_CAPTURE_LIMIT_BYTES: usize = 16 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const TERMINATION_GRACE: Duration = Duration::from_millis(300);

/// Whether the caller requested direct argv execution or an explicit shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessInvocationKind {
    Direct,
    Shell,
}

/// Declarative network intent for future sandbox/policy hooks.
///
/// This value is evidence/policy input; it is not itself a network sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessNetworkPolicy {
    Unspecified,
    Allow,
    Deny,
}

/// Process-tree termination support available on the current platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessTreeTermination {
    UnixProcessGroup,
    WindowsTaskkill,
    DirectChild,
}

/// Platform evidence attached to every process outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessPlatform {
    pub os: &'static str,
    pub arch: &'static str,
    pub tree_termination: ProcessTreeTermination,
}

impl ProcessPlatform {
    fn current(tree_mode: ProcessTreeMode) -> Self {
        let tree_termination = if tree_mode == ProcessTreeMode::ChildOnly {
            ProcessTreeTermination::DirectChild
        } else if cfg!(unix) {
            ProcessTreeTermination::UnixProcessGroup
        } else if cfg!(windows) {
            ProcessTreeTermination::WindowsTaskkill
        } else {
            ProcessTreeTermination::DirectChild
        };
        Self {
            os: env::consts::OS,
            arch: env::consts::ARCH,
            tree_termination,
        }
    }
}

/// Whether cancellation/timeout should target a process tree or only the child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessTreeMode {
    ProcessTree,
    ChildOnly,
}

/// Clonable cancellation signal for synchronous process execution.
#[derive(Debug, Clone, Default)]
pub struct ProcessCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl ProcessCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// How stdin is connected to the child.
pub enum ProcessInput {
    Null,
    Inherit,
    Bytes(Vec<u8>),
}

impl fmt::Debug for ProcessInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("Null"),
            Self::Inherit => f.write_str("Inherit"),
            Self::Bytes(bytes) => f
                .debug_struct("Bytes")
                .field("len", &bytes.len())
                .finish(),
        }
    }
}

/// How stdout/stderr are connected to the child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOutputMode {
    Null,
    Inherit,
    Capture { max_bytes: usize },
}

impl ProcessOutputMode {
    pub fn capture(max_bytes: usize) -> Self {
        Self::Capture { max_bytes }
    }
}

/// One process argument with an explicit sensitivity marker.
#[derive(Clone, PartialEq, Eq)]
pub struct ProcessArgument {
    value: String,
    sensitive: bool,
}

impl ProcessArgument {
    pub fn plain(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            sensitive: false,
        }
    }

    pub fn sensitive(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            sensitive: true,
        }
    }

    fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for ProcessArgument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.sensitive {
            f.write_str("[REDACTED]")
        } else {
            f.debug_tuple("arg").field(&self.value).finish()
        }
    }
}

#[derive(Clone)]
struct EnvironmentValue {
    value: String,
    sensitive: bool,
}

impl fmt::Debug for EnvironmentValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.sensitive {
            f.write_str("[REDACTED]")
        } else {
            f.debug_tuple("value").field(&self.value).finish()
        }
    }
}

/// Controlled child-environment policy.
///
/// The default inherits ordinary parent variables but strips names that look
/// credential-bearing. A caller must explicitly allow or set sensitive values.
#[derive(Debug, Clone)]
pub struct ProcessEnvironment {
    inherit_filtered: bool,
    allow_sensitive: BTreeSet<String>,
    deny: BTreeSet<String>,
    overrides: BTreeMap<String, EnvironmentValue>,
}

impl Default for ProcessEnvironment {
    fn default() -> Self {
        Self::filtered_inherit()
    }
}

impl ProcessEnvironment {
    pub fn filtered_inherit() -> Self {
        Self {
            inherit_filtered: true,
            allow_sensitive: BTreeSet::new(),
            deny: BTreeSet::new(),
            overrides: BTreeMap::new(),
        }
    }

    pub fn clear() -> Self {
        Self {
            inherit_filtered: false,
            allow_sensitive: BTreeSet::new(),
            deny: BTreeSet::new(),
            overrides: BTreeMap::new(),
        }
    }

    pub fn allow_sensitive(mut self, name: impl Into<String>) -> Self {
        self.allow_sensitive.insert(normalize_env_name(&name.into()));
        self
    }

    pub fn deny(mut self, name: impl Into<String>) -> Self {
        self.deny.insert(normalize_env_name(&name.into()));
        self
    }

    pub fn set(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        let sensitive = is_sensitive_name(&name);
        self.overrides.insert(
            normalize_env_name(&name),
            EnvironmentValue {
                value: value.into(),
                sensitive,
            },
        );
        self
    }

    pub fn set_sensitive(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let name = name.into();
        self.overrides.insert(
            normalize_env_name(&name),
            EnvironmentValue {
                value: value.into(),
                sensitive: true,
            },
        );
        self
    }

    fn apply(&self, command: &mut Command) -> Vec<String> {
        command.env_clear();
        let mut redactions = Vec::new();

        if self.inherit_filtered {
            for (name, value) in env::vars_os() {
                let normalized = normalize_env_name(&name.to_string_lossy());
                if self.deny.contains(&normalized) {
                    continue;
                }
                let sensitive = is_sensitive_name(&normalized);
                if sensitive && !self.allow_sensitive.contains(&normalized) {
                    continue;
                }
                if sensitive {
                    redactions.push(value.to_string_lossy().into_owned());
                }
                command.env(&name, &value);
            }
        }

        for (name, value) in &self.overrides {
            command.env(name, &value.value);
            if value.sensitive || is_sensitive_name(name) {
                redactions.push(value.value.clone());
            }
        }

        redactions
    }
}

/// Expected filesystem output produced by a subprocess.
#[derive(Debug, Clone)]
pub struct ProcessExpectedOutput {
    path: PathBuf,
    kind: ExpectedOutputKind,
    require_non_empty: bool,
    require_change: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedOutputKind {
    Any,
    File,
    Directory,
}

impl ProcessExpectedOutput {
    pub fn any(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: ExpectedOutputKind::Any,
            require_non_empty: false,
            require_change: false,
        }
    }

    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: ExpectedOutputKind::File,
            require_non_empty: false,
            require_change: false,
        }
    }

    pub fn directory(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: ExpectedOutputKind::Directory,
            require_non_empty: false,
            require_change: false,
        }
    }

    pub fn require_non_empty(mut self) -> Self {
        self.require_non_empty = true;
        self
    }

    pub fn require_change(mut self) -> Self {
        self.require_change = true;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Request supplied to the canonical process executor.
pub struct ProcessRequest {
    executable: String,
    args: Vec<ProcessArgument>,
    invocation_kind: ProcessInvocationKind,
    working_directory: Option<PathBuf>,
    stdin: ProcessInput,
    stdout: ProcessOutputMode,
    stderr: ProcessOutputMode,
    environment: ProcessEnvironment,
    timeout: Option<Duration>,
    cancellation: Option<ProcessCancellationToken>,
    tree_mode: ProcessTreeMode,
    expected_outputs: Vec<ProcessExpectedOutput>,
    redacted_values: Vec<String>,
    network_policy: ProcessNetworkPolicy,
    sandbox_profile: Option<String>,
}

impl fmt::Debug for ProcessRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessRequest")
            .field("executable", &self.executable)
            .field("args", &safe_argument_display(&self.args, &Redactor::default()))
            .field("invocation_kind", &self.invocation_kind)
            .field("working_directory", &self.working_directory)
            .field("stdin", &self.stdin)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .field("environment", &"<controlled>")
            .field("timeout", &self.timeout)
            .field("tree_mode", &self.tree_mode)
            .field("expected_outputs", &self.expected_outputs)
            .field("network_policy", &self.network_policy)
            .field("sandbox_profile", &self.sandbox_profile)
            .finish()
    }
}

impl ProcessRequest {
    pub fn direct(executable: impl Into<String>) -> Self {
        Self::new(executable.into(), ProcessInvocationKind::Direct)
    }

    pub fn shell(executable: impl Into<String>) -> Self {
        Self::new(executable.into(), ProcessInvocationKind::Shell)
    }

    fn new(executable: String, invocation_kind: ProcessInvocationKind) -> Self {
        Self {
            executable,
            args: Vec::new(),
            invocation_kind,
            working_directory: None,
            stdin: ProcessInput::Null,
            stdout: ProcessOutputMode::capture(DEFAULT_CAPTURE_LIMIT_BYTES),
            stderr: ProcessOutputMode::capture(DEFAULT_CAPTURE_LIMIT_BYTES),
            environment: ProcessEnvironment::default(),
            timeout: Some(DEFAULT_PROCESS_TIMEOUT),
            cancellation: None,
            tree_mode: ProcessTreeMode::ProcessTree,
            expected_outputs: Vec::new(),
            redacted_values: Vec::new(),
            network_policy: ProcessNetworkPolicy::Unspecified,
            sandbox_profile: None,
        }
    }

    pub fn arg(mut self, value: impl Into<String>) -> Self {
        self.args.push(ProcessArgument::plain(value));
        self
    }

    pub fn sensitive_arg(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.redacted_values.push(value.clone());
        self.args.push(ProcessArgument::sensitive(value));
        self
    }

    pub fn args<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args
            .extend(values.into_iter().map(|value| ProcessArgument::plain(value)));
        self
    }

    pub fn working_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(path.into());
        self
    }

    pub fn stdin(mut self, input: ProcessInput) -> Self {
        self.stdin = input;
        self
    }

    pub fn stdout(mut self, mode: ProcessOutputMode) -> Self {
        self.stdout = mode;
        self
    }

    pub fn stderr(mut self, mode: ProcessOutputMode) -> Self {
        self.stderr = mode;
        self
    }

    pub fn capture_limit(mut self, max_bytes: usize) -> Self {
        self.stdout = ProcessOutputMode::capture(max_bytes);
        self.stderr = ProcessOutputMode::capture(max_bytes);
        self
    }

    pub fn environment(mut self, environment: ProcessEnvironment) -> Self {
        self.environment = environment;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn without_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    pub fn cancellation(mut self, cancellation: ProcessCancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    pub fn child_only(mut self) -> Self {
        self.tree_mode = ProcessTreeMode::ChildOnly;
        self
    }

    pub fn expect_output(mut self, expected: ProcessExpectedOutput) -> Self {
        self.expected_outputs.push(expected);
        self
    }

    pub fn redact_value(mut self, value: impl Into<String>) -> Self {
        self.redacted_values.push(value.into());
        self
    }

    pub fn network_policy(mut self, policy: ProcessNetworkPolicy) -> Self {
        self.network_policy = policy;
        self
    }

    pub fn sandbox_profile(mut self, profile: impl Into<String>) -> Self {
        self.sandbox_profile = Some(profile.into());
        self
    }

    pub fn executable(&self) -> &str {
        &self.executable
    }

    pub fn invocation_kind(&self) -> ProcessInvocationKind {
        self.invocation_kind
    }
}

/// Hook for environment-specific policy enforcement such as network or sandbox
/// restrictions. The core request fields are declarative until a configured
/// hook enforces them.
pub trait ProcessPolicyHook: Send + Sync {
    fn validate(&self, request: &ProcessRequest) -> Result<(), String>;
}

/// Canonical subprocess execution service.
#[derive(Clone, Default)]
pub struct ProcessExecutor {
    hooks: Vec<Arc<dyn ProcessPolicyHook>>,
}

impl ProcessExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy_hook(mut self, hook: Arc<dyn ProcessPolicyHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    pub fn execute(&self, request: ProcessRequest) -> Result<ProcessResult, ProcessError> {
        if request.executable.trim().is_empty() {
            return Err(ProcessError::PolicyRejected(
                "process executable must not be empty".to_string(),
            ));
        }
        if request.invocation_kind == ProcessInvocationKind::Direct
            && is_explicit_shell_invocation(
                &request.executable,
                &request
                    .args
                    .iter()
                    .map(|arg| arg.value.clone())
                    .collect::<Vec<_>>(),
            )
        {
            return Err(ProcessError::ShellRequiresOptIn {
                executable: request.executable.clone(),
            });
        }

        let mut redactions = request.redacted_values.clone();
        for argument in &request.args {
            if argument.sensitive {
                redactions.push(argument.value.clone());
            }
        }
        let mut redactor = Redactor::new(redactions);

        for hook in &self.hooks {
            if let Err(message) = hook.validate(&request) {
                return Err(ProcessError::PolicyRejected(redactor.redact(&message)));
            }
        }

        let before_outputs: Vec<OutputSnapshot> = request
            .expected_outputs
            .iter()
            .map(|expected| OutputSnapshot::capture(expected.path()))
            .collect();

        let mut command = Command::new(&request.executable);
        command.args(request.args.iter().map(ProcessArgument::value));
        if let Some(path) = &request.working_directory {
            command.current_dir(path);
        }
        redactor.extend(request.environment.apply(&mut command));

        command.stdin(match &request.stdin {
            ProcessInput::Null => Stdio::null(),
            ProcessInput::Inherit => Stdio::inherit(),
            ProcessInput::Bytes(_) => Stdio::piped(),
        });
        command.stdout(stdio_for_output(request.stdout));
        command.stderr(stdio_for_output(request.stderr));

        #[cfg(unix)]
        if request.tree_mode == ProcessTreeMode::ProcessTree {
            command.process_group(0);
        }

        let safe_args = safe_argument_display(&request.args, &redactor);
        let command_display = safe_command_display(&request.executable, &safe_args);
        debug!(
            executable = %request.executable,
            args = ?safe_args,
            shell = request.invocation_kind == ProcessInvocationKind::Shell,
            timeout_ms = request.timeout.map(|value| value.as_millis() as u64),
            network_policy = ?request.network_policy,
            sandbox_profile = ?request.sandbox_profile,
            "Starting external process"
        );

        let started_at_epoch_ms = epoch_millis();
        let started = Instant::now();
        let mut child = command.spawn().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                ProcessError::MissingExecutable {
                    executable: request.executable.clone(),
                }
            } else {
                ProcessError::Launch {
                    executable: request.executable.clone(),
                    message: redactor.redact(&error.to_string()),
                }
            }
        })?;

        let stdout_reader = match request.stdout {
            ProcessOutputMode::Capture { max_bytes } => child
                .stdout
                .take()
                .map(|reader| spawn_bounded_reader(reader, max_bytes)),
            _ => None,
        };
        let stderr_reader = match request.stderr {
            ProcessOutputMode::Capture { max_bytes } => child
                .stderr
                .take()
                .map(|reader| spawn_bounded_reader(reader, max_bytes)),
            _ => None,
        };

        let stdin_writer = match request.stdin {
            ProcessInput::Bytes(bytes) => child.stdin.take().map(|mut writer| {
                thread::spawn(move || -> io::Result<()> {
                    match writer.write_all(&bytes) {
                        Ok(()) => Ok(()),
                        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
                        Err(error) => Err(error),
                    }
                })
            }),
            _ => None,
        };

        let termination = loop {
            if request
                .cancellation
                .as_ref()
                .is_some_and(ProcessCancellationToken::is_cancelled)
            {
                terminate_process_tree(&mut child, request.tree_mode).map_err(|error| {
                    ProcessError::Io(redactor.redact(&format!(
                        "failed to terminate cancelled process: {error}"
                    )))
                })?;
                break ProcessTermination::Cancelled;
            }

            if request.timeout.is_some_and(|timeout| started.elapsed() >= timeout) {
                terminate_process_tree(&mut child, request.tree_mode).map_err(|error| {
                    ProcessError::Io(redactor.redact(&format!(
                        "failed to terminate timed-out process: {error}"
                    )))
                })?;
                break ProcessTermination::TimedOut;
            }

            match child.try_wait().map_err(|error| {
                ProcessError::Io(redactor.redact(&format!(
                    "failed while waiting for process: {error}"
                )))
            })? {
                Some(status) => break termination_from_status(status),
                None => thread::sleep(POLL_INTERVAL),
            }
        };

        if let Some(writer) = stdin_writer {
            match writer.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) if !termination.is_success() => {
                    debug!(error = %redactor.redact(&error.to_string()), "stdin writer ended after process termination");
                }
                Ok(Err(error)) => {
                    return Err(ProcessError::Io(redactor.redact(&format!(
                        "failed to write process stdin: {error}"
                    ))));
                }
                Err(_) if !termination.is_success() => {}
                Err(_) => {
                    return Err(ProcessError::Io(
                        "process stdin writer thread panicked".to_string(),
                    ));
                }
            }
        }

        let stdout_bytes = join_bounded_reader(stdout_reader, "stdout")?;
        let stderr_bytes = join_bounded_reader(stderr_reader, "stderr")?;
        let stdout = CapturedOutput::new(stdout_bytes, &redactor);
        let stderr = CapturedOutput::new(stderr_bytes, &redactor);

        let output_failures = if termination.is_success() {
            request
                .expected_outputs
                .iter()
                .zip(before_outputs.iter())
                .filter_map(|(expected, before)| expected.validate(before))
                .collect()
        } else {
            Vec::new()
        };

        let result = ProcessResult {
            command_display,
            termination,
            stdout,
            stderr,
            started_at_epoch_ms,
            duration_ms: started.elapsed().as_millis() as u64,
            platform: ProcessPlatform::current(request.tree_mode),
            output_failures,
        };

        debug!(
            termination = ?result.termination,
            duration_ms = result.duration_ms,
            stdout_bytes = result.stdout.total_bytes,
            stderr_bytes = result.stderr.total_bytes,
            stdout_truncated = result.stdout.truncated,
            stderr_truncated = result.stderr.truncated,
            output_failures = result.output_failures.len(),
            "External process completed"
        );

        Ok(result)
    }

    pub fn execute_checked(&self, request: ProcessRequest) -> Result<ProcessResult, ProcessError> {
        let result = self.execute(request)?;
        result.ensure_success()?;
        Ok(result)
    }

    /// Probe `<tool> --version` using the same bounded process policy.
    pub fn probe_version(&self, executable: &str) -> ToolProbeEvidence {
        let request = ProcessRequest::direct(executable)
            .arg("--version")
            .timeout(PROBE_TIMEOUT)
            .capture_limit(PROBE_CAPTURE_LIMIT_BYTES);

        match self.execute(request) {
            Ok(result) => {
                let version_line = first_non_empty_line(result.stdout.redacted_text())
                    .or_else(|| first_non_empty_line(result.stderr.redacted_text()))
                    .map(str::to_string);
                let status = match result.termination {
                    ProcessTermination::Exited { code: 0 } => ToolProbeStatus::Available,
                    ProcessTermination::TimedOut => ToolProbeStatus::TimedOut,
                    _ => ToolProbeStatus::Failed,
                };
                ToolProbeEvidence {
                    executable: executable.to_string(),
                    status,
                    version_line,
                    duration_ms: result.duration_ms,
                    platform: result.platform,
                    diagnostic: if status == ToolProbeStatus::Available {
                        None
                    } else {
                        Some(result.failure_message())
                    },
                }
            }
            Err(ProcessError::MissingExecutable { .. }) => ToolProbeEvidence {
                executable: executable.to_string(),
                status: ToolProbeStatus::Missing,
                version_line: None,
                duration_ms: 0,
                platform: ProcessPlatform::current(ProcessTreeMode::ProcessTree),
                diagnostic: Some(format!("{executable} not found in PATH")),
            },
            Err(error) => ToolProbeEvidence {
                executable: executable.to_string(),
                status: ToolProbeStatus::Failed,
                version_line: None,
                duration_ms: 0,
                platform: ProcessPlatform::current(ProcessTreeMode::ProcessTree),
                diagnostic: Some(error.to_string()),
            },
        }
    }
}

/// Completed process termination classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessTermination {
    Exited { code: i32 },
    Signaled,
    TimedOut,
    Cancelled,
}

impl ProcessTermination {
    pub fn is_success(self) -> bool {
        matches!(self, Self::Exited { code: 0 })
    }
}

/// Bounded captured stream. Raw bytes remain private from `Debug`; callers can
/// explicitly consume them for binary-safe pipelines while diagnostics should
/// use [`redacted_text`](Self::redacted_text).
pub struct CapturedOutput {
    bytes: Vec<u8>,
    redacted_text: String,
    total_bytes: u64,
    truncated: bool,
}

impl CapturedOutput {
    fn new(bytes: BoundedBytes, redactor: &Redactor) -> Self {
        let redacted_text = redactor.redact(&String::from_utf8_lossy(&bytes.bytes));
        Self {
            bytes: bytes.bytes,
            redacted_text,
            total_bytes: bytes.total_bytes,
            truncated: bytes.truncated,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn redacted_text(&self) -> &str {
        &self.redacted_text
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    fn diagnostic_text(&self) -> String {
        let text = self.redacted_text.trim_end();
        if self.truncated {
            format!(
                "{text}\n[capture truncated: retained {} of {} bytes]",
                self.bytes.len(), self.total_bytes
            )
        } else {
            text.to_string()
        }
    }
}

impl fmt::Debug for CapturedOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapturedOutput")
            .field("retained_bytes", &self.bytes.len())
            .field("total_bytes", &self.total_bytes)
            .field("truncated", &self.truncated)
            .field("redacted_text", &self.redacted_text)
            .finish()
    }
}

/// Structured process outcome suitable for later execution-evidence projection.
pub struct ProcessResult {
    command_display: String,
    termination: ProcessTermination,
    stdout: CapturedOutput,
    stderr: CapturedOutput,
    started_at_epoch_ms: u64,
    duration_ms: u64,
    platform: ProcessPlatform,
    output_failures: Vec<String>,
}

impl ProcessResult {
    pub fn termination(&self) -> ProcessTermination {
        self.termination
    }

    pub fn stdout(&self) -> &CapturedOutput {
        &self.stdout
    }

    pub fn stderr(&self) -> &CapturedOutput {
        &self.stderr
    }

    pub fn started_at_epoch_ms(&self) -> u64 {
        self.started_at_epoch_ms
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    pub fn platform(&self) -> &ProcessPlatform {
        &self.platform
    }

    pub fn output_failures(&self) -> &[String] {
        &self.output_failures
    }

    pub fn is_success(&self) -> bool {
        self.termination.is_success() && self.output_failures.is_empty()
    }

    pub fn ensure_success(&self) -> Result<(), ProcessError> {
        if self.is_success() {
            Ok(())
        } else {
            Err(ProcessError::Unsuccessful(self.failure_message()))
        }
    }

    pub fn failure_message(&self) -> String {
        let mut message = match self.termination {
            ProcessTermination::Exited { code } => {
                format!("Command `{}` exited with code {code}", self.command_display)
            }
            ProcessTermination::Signaled => {
                format!("Command `{}` was terminated by a signal", self.command_display)
            }
            ProcessTermination::TimedOut => {
                format!("Command `{}` timed out", self.command_display)
            }
            ProcessTermination::Cancelled => {
                format!("Command `{}` was cancelled", self.command_display)
            }
        };

        let stderr = self.stderr.diagnostic_text();
        if !stderr.is_empty() {
            message.push_str("\nStderr: ");
            message.push_str(&stderr);
        }
        if !self.output_failures.is_empty() {
            message.push_str("\nOutput validation: ");
            message.push_str(&self.output_failures.join("; "));
        }
        message
    }
}

impl fmt::Debug for ProcessResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessResult")
            .field("command", &self.command_display)
            .field("termination", &self.termination)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .field("started_at_epoch_ms", &self.started_at_epoch_ms)
            .field("duration_ms", &self.duration_ms)
            .field("platform", &self.platform)
            .field("output_failures", &self.output_failures)
            .finish()
    }
}

/// Stable tool-probe classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolProbeStatus {
    Available,
    Missing,
    Failed,
    TimedOut,
}

/// Version-probe evidence for tool discovery/provenance consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProbeEvidence {
    pub executable: String,
    pub status: ToolProbeStatus,
    pub version_line: Option<String>,
    pub duration_ms: u64,
    pub platform: ProcessPlatform,
    pub diagnostic: Option<String>,
}

impl ToolProbeEvidence {
    pub fn is_available(&self) -> bool {
        self.status == ToolProbeStatus::Available
    }
}

/// Errors raised by process policy, launch, I/O, or checked execution.
#[derive(Debug)]
pub enum ProcessError {
    MissingExecutable { executable: String },
    Launch { executable: String, message: String },
    ShellRequiresOptIn { executable: String },
    PolicyRejected(String),
    Io(String),
    Unsuccessful(String),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingExecutable { executable } => write!(
                f,
                "`{executable}` was not found. Make sure it is installed and available in PATH."
            ),
            Self::Launch {
                executable,
                message,
            } => write!(f, "Failed to launch `{executable}`: {message}"),
            Self::ShellRequiresOptIn { executable } => write!(
                f,
                "Shell executable `{executable}` requires explicit ProcessRequest::shell(...) opt-in"
            ),
            Self::PolicyRejected(message) => write!(f, "Process policy rejected request: {message}"),
            Self::Io(message) => f.write_str(message),
            Self::Unsuccessful(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ProcessError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputSnapshot {
    exists: bool,
    is_file: bool,
    is_directory: bool,
    len: Option<u64>,
    modified: Option<SystemTime>,
}

impl OutputSnapshot {
    fn capture(path: &Path) -> Self {
        match fs::metadata(path) {
            Ok(metadata) => Self {
                exists: true,
                is_file: metadata.is_file(),
                is_directory: metadata.is_dir(),
                len: metadata.is_file().then_some(metadata.len()),
                modified: metadata.modified().ok(),
            },
            Err(_) => Self {
                exists: false,
                is_file: false,
                is_directory: false,
                len: None,
                modified: None,
            },
        }
    }
}

impl ProcessExpectedOutput {
    fn validate(&self, before: &OutputSnapshot) -> Option<String> {
        let after = OutputSnapshot::capture(&self.path);
        if !after.exists {
            return Some(format!("expected output '{}' was not produced", self.path.display()));
        }
        match self.kind {
            ExpectedOutputKind::Any => {}
            ExpectedOutputKind::File if !after.is_file => {
                return Some(format!("expected output '{}' is not a file", self.path.display()));
            }
            ExpectedOutputKind::Directory if !after.is_directory => {
                return Some(format!(
                    "expected output '{}' is not a directory",
                    self.path.display()
                ));
            }
            _ => {}
        }
        if self.require_non_empty && after.is_file && after.len == Some(0) {
            return Some(format!("expected output '{}' is empty", self.path.display()));
        }
        if self.require_change && &after == before {
            return Some(format!(
                "expected output '{}' was not changed by the process",
                self.path.display()
            ));
        }
        None
    }
}

#[derive(Default)]
struct Redactor {
    secrets: Vec<String>,
}

impl Redactor {
    fn new(values: Vec<String>) -> Self {
        let mut redactor = Self::default();
        redactor.extend(values);
        redactor
    }

    fn extend<I>(&mut self, values: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.secrets
            .extend(values.into_iter().filter(|value| !value.is_empty()));
        self.secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
        self.secrets.dedup();
    }

    fn redact(&self, text: &str) -> String {
        let mut redacted = redact_url_credentials(text);
        for secret in &self.secrets {
            redacted = redacted.replace(secret, "[REDACTED]");
        }
        redact_bearer_tokens(&redacted)
    }
}

#[derive(Debug)]
struct BoundedBytes {
    bytes: Vec<u8>,
    total_bytes: u64,
    truncated: bool,
}

fn spawn_bounded_reader<R>(reader: R, max_bytes: usize) -> thread::JoinHandle<io::Result<BoundedBytes>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || drain_bounded(reader, max_bytes))
}

fn drain_bounded<R: Read>(mut reader: R, max_bytes: usize) -> io::Result<BoundedBytes> {
    let mut bytes = Vec::with_capacity(max_bytes.min(8 * 1024));
    let mut total_bytes = 0_u64;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(read as u64);
        if bytes.len() < max_bytes {
            let remaining = max_bytes - bytes.len();
            bytes.extend_from_slice(&buffer[..read.min(remaining)]);
        }
    }
    Ok(BoundedBytes {
        truncated: total_bytes > bytes.len() as u64,
        bytes,
        total_bytes,
    })
}

fn join_bounded_reader(
    handle: Option<thread::JoinHandle<io::Result<BoundedBytes>>>,
    stream: &str,
) -> Result<BoundedBytes, ProcessError> {
    match handle {
        Some(handle) => match handle.join() {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(error)) => Err(ProcessError::Io(format!(
                "failed to capture process {stream}: {error}"
            ))),
            Err(_) => Err(ProcessError::Io(format!(
                "process {stream} reader thread panicked"
            ))),
        },
        None => Ok(BoundedBytes {
            bytes: Vec::new(),
            total_bytes: 0,
            truncated: false,
        }),
    }
}

fn stdio_for_output(mode: ProcessOutputMode) -> Stdio {
    match mode {
        ProcessOutputMode::Null => Stdio::null(),
        ProcessOutputMode::Inherit => Stdio::inherit(),
        ProcessOutputMode::Capture { .. } => Stdio::piped(),
    }
}

fn termination_from_status(status: ExitStatus) -> ProcessTermination {
    match status.code() {
        Some(code) => ProcessTermination::Exited { code },
        None => ProcessTermination::Signaled,
    }
}

fn terminate_process_tree(child: &mut Child, mode: ProcessTreeMode) -> io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }

    if mode == ProcessTreeMode::ChildOnly {
        let _ = child.kill();
        let _ = child.wait();
        return Ok(());
    }

    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        const SIGTERM: i32 = 15;
        const SIGKILL: i32 = 9;

        let term_result = unsafe { kill(-pid, SIGTERM) };
        if term_result != 0 {
            let _ = child.kill();
        }
        let deadline = Instant::now() + TERMINATION_GRACE;
        while Instant::now() < deadline {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(POLL_INTERVAL);
        }
        let _ = unsafe { kill(-pid, SIGKILL) };
        let _ = child.kill();
        let _ = child.wait();
        Ok(())
    }

    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let taskkill_succeeded = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !taskkill_succeeded {
            let _ = child.kill();
        }
        let _ = child.wait();
        return Ok(());
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
        let _ = child.wait();
        Ok(())
    }
}

fn safe_argument_display(args: &[ProcessArgument], redactor: &Redactor) -> Vec<String> {
    let mut safe = Vec::with_capacity(args.len());
    let mut redact_next = false;
    for argument in args {
        let raw = argument.value();
        let rendered = if argument.sensitive || redact_next {
            "[REDACTED]".to_string()
        } else if let Some((name, _value)) = raw.split_once('=') {
            if is_sensitive_name(name) {
                format!("{name}=[REDACTED]")
            } else {
                redactor.redact(raw)
            }
        } else if raw.to_ascii_lowercase().starts_with("authorization:") {
            "Authorization: [REDACTED]".to_string()
        } else {
            redactor.redact(raw)
        };
        redact_next = !argument.sensitive && !raw.contains('=') && is_sensitive_name(raw);
        safe.push(rendered);
    }
    safe
}

fn safe_command_display(executable: &str, args: &[String]) -> String {
    if args.is_empty() {
        executable.to_string()
    } else {
        format!("{} {}", executable, args.join(" "))
    }
}

fn normalize_env_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

fn is_sensitive_name(name: &str) -> bool {
    let normalized = name
        .trim_start_matches('-')
        .replace('-', "_")
        .to_ascii_uppercase();
    normalized.contains("TOKEN")
        || normalized.contains("SECRET")
        || normalized.contains("PASSWORD")
        || normalized.contains("PASSWD")
        || normalized.contains("API_KEY")
        || normalized.contains("APIKEY")
        || normalized.contains("CREDENTIAL")
        || normalized.contains("PRIVATE_KEY")
        || normalized == "AUTHORIZATION"
        || normalized.ends_with("_AUTH")
}

/// Return `true` when an explicitly named shell is being asked to interpret a
/// command string. Wrappers use this to opt in visibly instead of smuggling a
/// shell through the direct-argv path.
pub(crate) fn is_explicit_shell_invocation(executable: &str, args: &[String]) -> bool {
    let basename = Path::new(executable)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(executable)
        .to_ascii_lowercase();
    let is_shell = matches!(
        basename.as_str(),
        "sh"
            | "bash"
            | "zsh"
            | "dash"
            | "ksh"
            | "fish"
            | "cmd"
            | "cmd.exe"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
    );
    if !is_shell {
        return false;
    }
    args.iter().any(|argument| {
        argument == "-c"
            || argument == "-Command"
            || argument == "-command"
            || argument == "/C"
            || argument == "/c"
    })
}

fn redact_url_credentials(text: &str) -> String {
    let Some(scheme_index) = text.find("://") else {
        return text.to_string();
    };
    let authority_start = scheme_index + 3;
    let authority_end = text[authority_start..]
        .find(['/', ' ', '\n', '\r', '\t'])
        .map(|offset| authority_start + offset)
        .unwrap_or(text.len());
    let authority = &text[authority_start..authority_end];
    let Some(at_index) = authority.rfind('@') else {
        return text.to_string();
    };
    if !authority[..at_index].contains(':') {
        return text.to_string();
    }
    format!(
        "{}[REDACTED]@{}{}",
        &text[..authority_start],
        &authority[at_index + 1..],
        &text[authority_end..]
    )
}

fn redact_bearer_tokens(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remainder = text;
    loop {
        let lower = remainder.to_ascii_lowercase();
        let Some(index) = lower.find("bearer ") else {
            result.push_str(remainder);
            break;
        };
        result.push_str(&remainder[..index]);
        let token_start = index + "bearer ".len();
        result.push_str(&remainder[index..token_start]);
        result.push_str("[REDACTED]");
        let token_end = remainder[token_start..]
            .find(char::is_whitespace)
            .map(|offset| token_start + offset)
            .unwrap_or(remainder.len());
        remainder = &remainder[token_end..];
    }
    result
}

fn first_non_empty_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_executable_is_classified() {
        let error = ProcessExecutor::new()
            .execute(ProcessRequest::direct(
                "__renderflow_process_executor_missing_tool__",
            ))
            .unwrap_err();
        assert!(matches!(error, ProcessError::MissingExecutable { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn non_zero_exit_is_structured() {
        let result = ProcessExecutor::new()
            .execute(ProcessRequest::direct("false"))
            .unwrap();
        assert_eq!(result.termination(), ProcessTermination::Exited { code: 1 });
        assert!(!result.is_success());
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_process() {
        let started = Instant::now();
        let result = ProcessExecutor::new()
            .execute(
                ProcessRequest::shell("sh")
                    .args(["-c", "sleep 5"])
                    .timeout(Duration::from_millis(75)),
            )
            .unwrap();
        assert_eq!(result.termination(), ProcessTermination::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_terminates_process_tree() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("descendant-finished");
        let script = format!("(sleep 1; touch '{}') & wait", marker.display());
        let token = ProcessCancellationToken::new();
        let cancel = token.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(75));
            cancel.cancel();
        });

        let result = ProcessExecutor::new()
            .execute(
                ProcessRequest::shell("sh")
                    .args(["-c", script.as_str()])
                    .cancellation(token)
                    .timeout(Duration::from_secs(3)),
            )
            .unwrap();
        assert_eq!(result.termination(), ProcessTermination::Cancelled);
        thread::sleep(Duration::from_millis(1100));
        assert!(
            !marker.exists(),
            "a descendant survived process-tree cancellation"
        );
    }

    #[cfg(unix)]
    #[test]
    fn capture_is_bounded_and_reports_truncation() {
        let result = ProcessExecutor::new()
            .execute_checked(
                ProcessRequest::shell("sh")
                    .args(["-c", "printf '0123456789abcdef'"])
                    .capture_limit(8),
            )
            .unwrap();
        assert_eq!(result.stdout().bytes(), b"01234567");
        assert_eq!(result.stdout().total_bytes(), 16);
        assert!(result.stdout().truncated());
    }

    #[cfg(unix)]
    #[test]
    fn secret_values_are_redacted_from_debug_and_errors() {
        let secret = "super-secret-value-123";
        let result = ProcessExecutor::new()
            .execute(
                ProcessRequest::shell("sh")
                    .args(["-c", "printf '%s' \"$RF_SECRET\" >&2; exit 7"])
                    .environment(ProcessEnvironment::clear().set_sensitive("RF_SECRET", secret))
                    .redact_value(secret),
            )
            .unwrap();
        let error = result.ensure_success().unwrap_err().to_string();
        let debug = format!("{result:?}");
        assert!(!error.contains(secret));
        assert!(!debug.contains(secret));
        assert!(error.contains("[REDACTED]"));
    }

    #[cfg(unix)]
    #[test]
    fn invalid_expected_output_fails_checked_execution() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("missing.out");
        let error = ProcessExecutor::new()
            .execute_checked(
                ProcessRequest::direct("true")
                    .expect_output(ProcessExpectedOutput::file(&output)),
            )
            .unwrap_err();
        assert!(error.to_string().contains("was not produced"));
    }

    #[cfg(unix)]
    #[test]
    fn direct_shell_requires_explicit_opt_in() {
        let error = ProcessExecutor::new()
            .execute(ProcessRequest::direct("sh").args(["-c", "true"]))
            .unwrap_err();
        assert!(matches!(error, ProcessError::ShellRequiresOptIn { .. }));
    }

    #[test]
    fn filtered_environment_detects_secret_names() {
        assert!(is_sensitive_name("GITHUB_TOKEN"));
        assert!(is_sensitive_name("OPENAI_API_KEY"));
        assert!(!is_sensitive_name("PATH"));
        assert!(!is_sensitive_name("HOME"));
    }

    #[test]
    fn bearer_tokens_and_url_credentials_are_redacted() {
        let redactor = Redactor::default();
        assert_eq!(
            redactor.redact("Authorization: Bearer abc123"),
            "Authorization: Bearer [REDACTED]"
        );
        assert_eq!(
            redactor.redact("https://user:password@example.com/path"),
            "https://[REDACTED]@example.com/path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn version_probe_captures_tool_evidence() {
        let probe = ProcessExecutor::new().probe_version("rustc");
        assert!(probe.is_available());
        assert!(probe
            .version_line
            .as_deref()
            .is_some_and(|line| line.contains("rustc")));
    }
}
