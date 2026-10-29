//! Explicit local command plugins using the existing artifact-native SDK.
//!
//! This adapter is a trusted-code boundary, not an OS sandbox or package loader.
//! Command dependencies are not attested, so cache/checkpoint reuse is disabled.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::plugin_v2::{
    PluginCachePolicy, PluginConfigSchema, PluginExecutionContext, PluginInputKind,
    PluginRuntimeOptions, PluginTransformDescriptor, PluginTransformRequest,
    PluginTransformResult, PluginTransformV2, PluginV2ArtifactAdapter,
};
use crate::artifact::{Artifact, ArtifactCollection, ArtifactStore, ArtifactTransform};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::process::{
    is_explicit_shell_invocation, ProcessCancellationToken, ProcessError, ProcessExpectedOutput,
    ProcessInput, ProcessNetworkPolicy, ProcessOutputMode, ProcessRequest,
};

pub const COMMAND_PLUGIN_PROTOCOL: &str = "renderflow.command-plugin/v1";
const MAX_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CONFIG_BYTES: usize = 64 * 1024;

fn default_timeout() -> u64 { 30 }
fn default_payload_limit() -> u64 { 8 * 1024 * 1024 }

/// A reviewed local program explicitly declared in a transform registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandPluginDefinition {
    pub protocol: String,
    pub descriptor: PluginTransformDescriptor,
    pub program: String,
    pub args: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_payload_limit")]
    pub max_input_bytes: u64,
    #[serde(default = "default_payload_limit")]
    pub max_output_bytes: u64,
}

impl CommandPluginDefinition {
    /// Validate declarations only; never spawn or discover the program.
    pub fn validate(&self) -> Result<()> {
        self.descriptor.validate()?;
        crate::toolchain::ToolId::new(self.descriptor.id.clone())?;
        crate::toolchain::CapabilityId::new(self.descriptor.capability_id.clone())?;
        anyhow::ensure!(self.protocol == COMMAND_PLUGIN_PROTOCOL,
            "unsupported command plugin protocol '{}': expected {COMMAND_PLUGIN_PROTOCOL}", self.protocol);
        anyhow::ensure!(self.descriptor.input_kind == PluginInputKind::Single,
            "command plugin '{}' supports single input only", self.descriptor.id);
        anyhow::ensure!(self.descriptor.cache_policy == PluginCachePolicy::Disabled,
            "command plugin '{}' must disable caching until executable dependencies are attested", self.descriptor.id);
        anyhow::ensure!(self.descriptor.required_provider_ids.is_empty(),
            "command plugins currently declare their one program through the transform provider, not additional providers");
        anyhow::ensure!(!self.program.trim().is_empty() && self.program.len() <= 4096
            && !self.program.chars().any(char::is_control),
            "command plugin program must be nonempty and contain no control characters");
        anyhow::ensure!(self.args.len() <= 128 && self.args.iter().all(|arg| arg.len() <= 4096 && !arg.contains('\0')),
            "command plugin arguments exceed the declared bounds");
        anyhow::ensure!(!is_explicit_shell_invocation(&self.program, &self.args),
            "command plugins use direct argv; explicit shell invocation is unsupported");
        for placeholder in ["{input}", "{output}", "{config}"] {
            anyhow::ensure!(self.args.iter().any(|arg| arg == placeholder),
                "command plugin '{}' requires a standalone {placeholder} argument", self.descriptor.id);
        }
        anyhow::ensure!((1..=1800).contains(&self.timeout_seconds),
            "command plugin timeout_seconds must be between 1 and 1800");
        for limit in [self.max_input_bytes, self.max_output_bytes] {
            anyhow::ensure!((1..=MAX_PAYLOAD_BYTES).contains(&limit),
                "command plugin byte limits must be between 1 and {MAX_PAYLOAD_BYTES}");
        }
        Ok(())
    }

    pub(crate) fn resolved_program(&self, registry_dir: &Path) -> Result<String> {
        let path = Path::new(&self.program);
        if !path.is_absolute() && path.components().count() > 1 {
            return path_string(&registry_dir.join(path));
        }
        Ok(self.program.clone())
    }
}

/// One local command implemented as an ordinary v2 plugin.
pub struct CommandPlugin {
    definition: CommandPluginDefinition,
    registry_dir: PathBuf,
}

impl CommandPlugin {
    pub fn new(definition: CommandPluginDefinition, registry_dir: impl AsRef<Path>) -> Result<Self> {
        definition.validate()?;
        let registry_dir = fs::canonicalize(registry_dir.as_ref())
            .context("cannot resolve command plugin registry directory")?;
        Ok(Self { definition, registry_dir })
    }

    /// Register a configured plugin through the existing artifact adapter.
    pub fn configured(self: Arc<Self>, config: Value) -> Result<ConfiguredCommandPlugin> {
        self.validate_config(&config)?;
        let plugin: Arc<dyn PluginTransformV2> = self;
        let options = PluginRuntimeOptions::new().with_config(config);
        let adapter = PluginV2ArtifactAdapter::new(plugin.clone(), options.clone())?;
        Ok(ConfiguredCommandPlugin { plugin, options, adapter })
    }
}

impl PluginTransformV2 for CommandPlugin {
    fn descriptor(&self) -> PluginTransformDescriptor {
        self.definition.descriptor.clone()
    }

    fn config_schema(&self) -> PluginConfigSchema {
        PluginConfigSchema {
            id: "renderflow.command-plugin.config/v1".to_string(),
            schema: serde_json::json!({"type": "object"}),
        }
    }

    fn validate_config(&self, config: &Value) -> Result<()> {
        validate_command_config(config)
    }

    fn execute(&self, request: PluginTransformRequest, context: &PluginExecutionContext) -> Result<PluginTransformResult> {
        let input = request.inputs.into_one()?;
        anyhow::ensure!(!context.cancellation.is_cancelled(), "command plugin cancelled before staging");
        anyhow::ensure!(input.size_bytes() <= self.definition.max_input_bytes,
            "command plugin input exceeds max_input_bytes");
        // Stage an independent read-only input using the SDK, never the source path.
        let input_path = fs::canonicalize(context.artifacts.input_path(&input)?)?;
        let output_path = context.artifacts.scratch_path("output")?;
        let scratch = fs::canonicalize(output_path.parent().context("missing plugin scratch directory")?)?;
        let output_path = scratch.join("output");
        let config_path = scratch.join("config.json");
        let config_bytes = serde_json::to_vec(&request.config)?;
        fs::write(&config_path, &config_bytes).context("cannot stage command plugin configuration")?;
        let mut permissions = fs::metadata(&config_path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&config_path, permissions)?;
        let args = self.definition.args.iter().map(|arg| {
            match arg.as_str() {
                "{input}" => path_string(&input_path),
                "{output}" => path_string(&output_path),
                "{config}" => path_string(&config_path),
                "{format}" => Ok(request.output_format.to_string()),
                _ => Ok(arg.replace("{registry_dir}", &path_string(&self.registry_dir)?)),
            }
        }).collect::<Result<Vec<_>>>()?;
        let program = self.definition.resolved_program(&self.registry_dir)?;
        anyhow::ensure!(!is_explicit_shell_invocation(&program, &args),
            "command plugins do not support shell invocation");
        let mut process = ProcessRequest::direct(program)
            .working_directory(&scratch)
            .stdin(ProcessInput::Null)
            .network_policy(ProcessNetworkPolicy::Deny)
            .stdout(ProcessOutputMode::capture(64 * 1024))
            .stderr(ProcessOutputMode::capture(64 * 1024))
            .timeout(Duration::from_secs(self.definition.timeout_seconds))
            .expect_output(ProcessExpectedOutput::file(&output_path)
                .require_non_empty().max_bytes(self.definition.max_output_bytes));
        // Argument/config content is user data; do not copy it into process diagnostics.
        for arg in args { process = process.sensitive_arg(arg); }
        process = process.redact_value(String::from_utf8(config_bytes)?);
        let result = context.process.execute(process).map_err(|error| {
            // Child diagnostics may contain input text, not just credential names.
            // Keep a useful failure class without putting stdout/stderr in receipts.
            let kind = match error {
                ProcessError::MissingExecutable { .. } => "missing_executable",
                ProcessError::Launch { .. } => "launch_failed",
                ProcessError::ShellRequiresOptIn { .. } => "shell_refused",
                ProcessError::PolicyRejected(_) => "policy_rejected",
                ProcessError::Io(_) => "process_io",
                ProcessError::Unsuccessful(_) => "unsuccessful",
            };
            anyhow::anyhow!("command_plugin.process.{kind}")
        })?;
        anyhow::ensure!(result.termination().is_success(),
            "command_plugin.process.termination: {:?}", result.termination());
        anyhow::ensure!(result.output_failures().is_empty(), "command_plugin.output.invalid");
        anyhow::ensure!(!context.cancellation.is_cancelled(), "command plugin cancelled before publication");
        let bytes = read_output(&output_path, self.definition.max_output_bytes)?;
        // Shape validation is separate from semantic accuracy of extracted content.
        if request.output_format == Format::Json {
            serde_json::from_slice::<Value>(&bytes).context("command plugin produced invalid JSON")?;
        }
        let artifact = context.artifacts.commit_bytes(&bytes, None, BTreeMap::new())?;
        Ok(PluginTransformResult {
            outputs: ArtifactCollection::one(artifact),
            provenance: BTreeMap::from([
                ("command_protocol".to_string(), Value::String(COMMAND_PLUGIN_PROTOCOL.to_string())),
                ("executable_identity_verified".to_string(), Value::Bool(false)),
            ]),
            ..PluginTransformResult::default()
        })
    }
}

pub(crate) fn validate_command_config(config: &Value) -> Result<()> {
    anyhow::ensure!(config.is_object(), "command plugin config must be a JSON object");
    anyhow::ensure!(serde_json::to_vec(config)?.len() <= MAX_CONFIG_BYTES,
        "command plugin config exceeds {MAX_CONFIG_BYTES} bytes");
    Ok(())
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str().map(str::to_string).context("command plugin paths must be UTF-8")
}

fn read_output(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).context("command plugin output is missing")?;
    anyhow::ensure!(metadata.file_type().is_file(), "command plugin output must be a regular file, not a symlink");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        anyhow::ensure!(metadata.nlink() == 1, "command plugin output must not be a hard link");
    }
    anyhow::ensure!(metadata.len() > 0 && metadata.len() <= limit, "command plugin output violates byte limits");
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(!bytes.is_empty() && bytes.len() as u64 <= limit,
        "command plugin output violates byte limits");
    Ok(bytes)
}

/// Carries per-execution DAG cancellation into the unchanged v2 plugin API.
pub struct ConfiguredCommandPlugin {
    plugin: Arc<dyn PluginTransformV2>,
    options: PluginRuntimeOptions,
    adapter: PluginV2ArtifactAdapter,
}

impl ArtifactTransform for ConfiguredCommandPlugin {
    fn name(&self) -> &str { self.adapter.name() }
    fn version(&self) -> &str { self.adapter.version() }
    fn cache_identity(&self) -> String { self.adapter.cache_identity() }
    fn cacheable(&self) -> bool { false }
    fn fidelity(&self) -> Option<FidelityDeclaration> { self.adapter.fidelity() }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        self.adapter.apply(input, output_format, store)
    }

    fn apply_with_cancellation(&self, input: &Artifact, output_format: Format, store: &ArtifactStore,
        cancellation: Option<Arc<AtomicBool>>) -> Result<Artifact> {
        let options = match cancellation {
            Some(flag) => self.options.clone().with_cancellation(ProcessCancellationToken::from_shared(flag)),
            None => self.options.clone(),
        };
        PluginV2ArtifactAdapter::new(self.plugin.clone(), options)?.apply(input, output_format, store)
    }
}
