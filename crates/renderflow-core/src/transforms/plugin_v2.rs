//! Versioned, artifact-native plugin contracts.
//!
//! This module is the stable boundary for new Renderflow extensions. The
//! legacy [`PluginExecutor`](super::plugin::PluginExecutor) API remains
//! available through [`LegacyTextPluginAdapter`].

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::plugin::PluginExecutor;
use crate::artifact::{
    Artifact, ArtifactCollection, ArtifactCollectionTransform, ArtifactDescriptor, ArtifactId,
    ArtifactStorageClass, ArtifactStore, ArtifactTransform, MediaType,
};
use crate::evidence::{redact_sensitive_text, FidelityDeclaration};
use crate::graph::Format;
use crate::process::{
    ProcessCancellationToken, ProcessError, ProcessExecutor, ProcessRequest, ProcessResult,
};

/// Serialized contract identifier for the v2 plugin boundary.
pub const PLUGIN_SDK_CONTRACT: &str = "renderflow.plugin/v2alpha1";

/// Cardinality accepted by a transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginInputKind {
    Single,
    OrderedCollection,
}

/// Reproducibility declaration used by planning and caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDeterminism {
    Deterministic,
    EnvironmentDependent,
    Nondeterministic,
}

/// Cache behavior a plugin author explicitly opts into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCachePolicy {
    ContentAddressed,
    Disabled,
}

/// Expected information-loss behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginLossProfile {
    Lossless,
    Partial,
    Lossy,
    PathDependent,
}

/// Stable metadata used to register, plan, cache, and provenance a transform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginTransformDescriptor {
    pub contract: String,
    pub id: String,
    pub version: String,
    pub capability_id: String,
    pub input_kind: PluginInputKind,
    pub input_formats: Vec<String>,
    pub output_formats: Vec<String>,
    pub determinism: PluginDeterminism,
    pub cache_policy: PluginCachePolicy,
    pub loss_profile: PluginLossProfile,
    #[serde(default)]
    pub required_provider_ids: Vec<String>,
}

impl PluginTransformDescriptor {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        capability_id: impl Into<String>,
        input_kind: PluginInputKind,
    ) -> Self {
        Self {
            contract: PLUGIN_SDK_CONTRACT.to_string(),
            id: id.into(),
            version: version.into(),
            capability_id: capability_id.into(),
            input_kind,
            input_formats: Vec::new(),
            output_formats: Vec::new(),
            determinism: PluginDeterminism::Deterministic,
            cache_policy: PluginCachePolicy::ContentAddressed,
            loss_profile: PluginLossProfile::PathDependent,
            required_provider_ids: Vec::new(),
        }
    }

    pub fn with_formats(
        mut self,
        inputs: impl IntoIterator<Item = impl Into<String>>,
        outputs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.input_formats = inputs.into_iter().map(Into::into).collect();
        self.output_formats = outputs.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_execution_properties(
        mut self,
        determinism: PluginDeterminism,
        cache_policy: PluginCachePolicy,
        loss_profile: PluginLossProfile,
    ) -> Self {
        self.determinism = determinism;
        self.cache_policy = cache_policy;
        self.loss_profile = loss_profile;
        self
    }

    pub fn with_required_providers(
        mut self,
        providers: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.required_provider_ids = providers.into_iter().map(Into::into).collect();
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.contract != PLUGIN_SDK_CONTRACT {
            anyhow::bail!(
                "plugin '{}' declares unsupported contract '{}'",
                self.id,
                self.contract
            );
        }
        for (field, value) in [
            ("id", self.id.as_str()),
            ("version", self.version.as_str()),
            ("capability_id", self.capability_id.as_str()),
        ] {
            if value.trim().is_empty() {
                anyhow::bail!("plugin descriptor field '{}' must not be empty", field);
            }
        }
        if self.input_formats.is_empty() || self.output_formats.is_empty() {
            anyhow::bail!(
                "plugin '{}' must declare at least one input and output format",
                self.id
            );
        }
        for format in self.input_formats.iter().chain(&self.output_formats) {
            format.parse::<Format>().with_context(|| {
                format!(
                    "plugin '{}' declares unknown canonical format '{}'",
                    self.id, format
                )
            })?;
        }
        for provider in &self.required_provider_ids {
            if provider.trim().is_empty() {
                anyhow::bail!("plugin '{}' declares an empty provider ID", self.id);
            }
        }
        let core_version = self
            .version
            .split_once('-')
            .map(|(core, _)| core)
            .unwrap_or(&self.version);
        if core_version.split('.').count() != 3
            || core_version.split('.').any(|part| {
                part.is_empty() || !part.chars().all(|character| character.is_ascii_digit())
            })
        {
            anyhow::bail!(
                "plugin '{}' version '{}' must use semantic major.minor.patch form",
                self.id,
                self.version
            );
        }
        if self.cache_policy == PluginCachePolicy::ContentAddressed
            && self.determinism != PluginDeterminism::Deterministic
        {
            anyhow::bail!(
                "plugin '{}' may use content-addressed caching only when deterministic",
                self.id
            );
        }
        Ok(())
    }

    fn accepts(&self, input: Format, output: Format) -> bool {
        let input = input.to_string();
        let output = output.to_string();
        self.input_formats.iter().any(|value| value == &input)
            && self.output_formats.iter().any(|value| value == &output)
    }
}

/// Versioned JSON Schema supplied by a plugin for its configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginConfigSchema {
    pub id: String,
    pub schema: Value,
}

impl PluginConfigSchema {
    pub fn no_config() -> Self {
        Self {
            id: "renderflow.plugin.config/none".to_string(),
            schema: serde_json::json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "maxProperties": 0
            }),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            anyhow::bail!("plugin configuration schema ID must not be empty");
        }
        if !self.schema.is_object() {
            anyhow::bail!("plugin configuration schema must be a JSON object");
        }
        Ok(())
    }
}

/// Structured plugin diagnostic safe for run evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub help: Option<String>,
}

/// Progress signal emitted without exposing mutable execution internals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginProgressEvent {
    pub execution_id: String,
    pub step_id: String,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub message: String,
}

/// Host-owned progress receiver.
pub trait PluginProgressReporter: Send + Sync {
    fn report(&self, event: PluginProgressEvent);
}

#[derive(Default)]
struct NoopProgressReporter;

impl PluginProgressReporter for NoopProgressReporter {
    fn report(&self, _event: PluginProgressEvent) {}
}

/// Lifecycle phases observable by read-only extension hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginLifecyclePhase {
    Inspect,
    Plan,
    ExecuteStarted,
    ExecuteCompleted,
    ExecuteFailed,
    Validate,
    ArtifactCommitted,
}

/// Immutable lifecycle observation delivered to observers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginLifecycleEvent {
    pub contract: String,
    pub execution_id: String,
    pub step_id: Option<String>,
    pub transform_id: Option<String>,
    pub phase: PluginLifecyclePhase,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
    #[serde(default)]
    pub detail: BTreeMap<String, Value>,
}

/// Read-only hook. Observers receive snapshots and have no mutation handle.
pub trait PluginLifecycleObserver: Send + Sync {
    fn observe(&self, event: &PluginLifecycleEvent);
}

/// Approved process service that always carries the host cancellation token.
#[derive(Clone)]
pub struct PluginProcessService {
    executor: ProcessExecutor,
    cancellation: ProcessCancellationToken,
}

impl PluginProcessService {
    fn new(executor: ProcessExecutor, cancellation: ProcessCancellationToken) -> Self {
        Self {
            executor,
            cancellation,
        }
    }

    pub fn execute(&self, request: ProcessRequest) -> Result<ProcessResult, ProcessError> {
        self.executor
            .execute(request.cancellation(self.cancellation.clone()))
    }

    pub fn execute_checked(&self, request: ProcessRequest) -> Result<ProcessResult, ProcessError> {
        self.executor
            .execute_checked(request.cancellation(self.cancellation.clone()))
    }
}

/// Capability-limited artifact-store view for one plugin invocation.
///
/// Inputs can be opened or resolved for approved subprocesses. Outputs can only
/// be committed as new immutable objects in the expected output format and
/// automatically receive the complete ordered source lineage.
#[derive(Clone)]
pub struct PluginArtifactStore {
    store: ArtifactStore,
    scratch: Arc<tempfile::TempDir>,
    inputs: HashMap<ArtifactId, Artifact>,
    ordered_sources: Vec<ArtifactId>,
    output_format: Format,
}

impl PluginArtifactStore {
    fn new(
        store: ArtifactStore,
        inputs: &ArtifactCollection,
        output_format: Format,
    ) -> Result<Self> {
        let artifacts = inputs.iter().cloned().collect::<Vec<_>>();
        let scratch = tempfile::tempdir_in(store.temporary_directory())
            .context("failed to create scoped plugin scratch directory")?;
        Ok(Self {
            store,
            scratch: Arc::new(scratch),
            inputs: artifacts
                .iter()
                .map(|artifact| (artifact.id().clone(), artifact.clone()))
                .collect(),
            ordered_sources: artifacts
                .iter()
                .map(|artifact| artifact.id().clone())
                .collect(),
            output_format,
        })
    }

    fn authorized(&self, artifact: &Artifact) -> Result<&Artifact> {
        self.inputs.get(artifact.id()).ok_or_else(|| {
            anyhow::anyhow!(
                "artifact '{}' is not an authorized input for this plugin invocation",
                artifact.id()
            )
        })
    }

    pub fn open_input(&self, artifact: &Artifact) -> Result<File> {
        self.store.open(self.authorized(artifact)?)
    }

    pub fn read_input_bytes(&self, artifact: &Artifact) -> Result<Vec<u8>> {
        self.store.read_bytes(self.authorized(artifact)?)
    }

    pub fn input_path(&self, artifact: &Artifact) -> Result<PathBuf> {
        let artifact = self.authorized(artifact)?;
        let mut source = self.store.open(artifact)?;
        let mut staged = tempfile::NamedTempFile::new_in(self.scratch.path())
            .context("failed to stage a read-only plugin input")?;
        std::io::copy(&mut source, &mut staged)
            .context("failed to copy a plugin input into scoped scratch space")?;
        let (_file, path) = staged
            .keep()
            .map_err(|error| error.error)
            .context("failed to retain a staged plugin input")?;
        let mut permissions = std::fs::metadata(&path)?.permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions)?;
        Ok(path)
    }

    /// Return a scoped path for an external tool output or other scratch data.
    pub fn scratch_path(&self, file_name: &str) -> Result<PathBuf> {
        let candidate = Path::new(file_name);
        if file_name.trim().is_empty()
            || candidate.is_absolute()
            || candidate.components().count() != 1
        {
            anyhow::bail!("plugin scratch file name must be one non-empty path component");
        }
        Ok(self.scratch.path().join(candidate))
    }

    pub fn commit_bytes(
        &self,
        bytes: &[u8],
        media_type: Option<MediaType>,
        metadata: BTreeMap<String, Value>,
    ) -> Result<Artifact> {
        let descriptor = self.output_descriptor(media_type, metadata);
        self.store.put_bytes(bytes, descriptor)
    }

    pub fn commit_path(
        &self,
        path: impl AsRef<Path>,
        media_type: Option<MediaType>,
        metadata: BTreeMap<String, Value>,
    ) -> Result<Artifact> {
        let descriptor = self.output_descriptor(media_type, metadata);
        self.store.import_path(path, descriptor)
    }

    fn output_descriptor(
        &self,
        media_type: Option<MediaType>,
        metadata: BTreeMap<String, Value>,
    ) -> ArtifactDescriptor {
        let mut descriptor =
            ArtifactDescriptor::for_format(self.output_format, ArtifactStorageClass::Intermediate)
                .with_sources(self.ordered_sources.clone());
        if let Some(media_type) = media_type {
            descriptor = descriptor.with_media_type(media_type);
        }
        for (key, value) in metadata {
            descriptor = descriptor
                .with_metadata(format!("renderflow.plugin.{key}"), redact_json_value(value));
        }
        descriptor
    }

    fn seal_output(
        &self,
        artifact: &Artifact,
        descriptor: &PluginTransformDescriptor,
        diagnostics: &[PluginDiagnostic],
        metrics: &BTreeMap<String, f64>,
        provenance: &BTreeMap<String, Value>,
    ) -> Result<Artifact> {
        let mut output_descriptor =
            ArtifactDescriptor::for_format(self.output_format, ArtifactStorageClass::Intermediate)
                .with_media_type(artifact.media_type().clone())
                .with_sources(self.ordered_sources.clone())
                .with_metadata("renderflow.plugin.contract", PLUGIN_SDK_CONTRACT)
                .with_metadata("renderflow.plugin.transform_id", descriptor.id.clone())
                .with_metadata(
                    "renderflow.plugin.transform_version",
                    descriptor.version.clone(),
                )
                .with_metadata(
                    "renderflow.plugin.capability_id",
                    descriptor.capability_id.clone(),
                )
                .with_metadata(
                    "renderflow.plugin.determinism",
                    serde_json::to_value(descriptor.determinism)?,
                )
                .with_metadata(
                    "renderflow.plugin.loss_profile",
                    serde_json::to_value(descriptor.loss_profile)?,
                )
                .with_metadata(
                    "renderflow.plugin.diagnostics",
                    redact_json_value(serde_json::to_value(diagnostics)?),
                )
                .with_metadata("renderflow.plugin.metrics", serde_json::to_value(metrics)?);
        for (key, value) in provenance {
            output_descriptor = output_descriptor.with_metadata(
                format!("renderflow.plugin.provenance.{key}"),
                redact_json_value(value.clone()),
            );
        }
        self.store
            .put_reader(self.store.open(artifact)?, output_descriptor)
    }

    fn verify_inputs(&self) -> Result<()> {
        for artifact in self.inputs.values() {
            self.store.verify(artifact)?;
        }
        Ok(())
    }

    fn verify_outputs(&self, outputs: &ArtifactCollection) -> Result<()> {
        for artifact in outputs.iter() {
            if artifact.format().as_str() != self.output_format.to_string() {
                anyhow::bail!(
                    "plugin returned '{}' while '{}' was requested",
                    artifact.format(),
                    self.output_format
                );
            }
            if artifact.sources() != self.ordered_sources {
                anyhow::bail!(
                    "plugin output '{}' does not preserve the authorized ordered source lineage",
                    artifact.id()
                );
            }
            self.store.verify(artifact)?;
        }
        Ok(())
    }
}

fn redact_json_value(value: Value) -> Value {
    let serialized = serde_json::to_string(&value).unwrap_or_default();
    serde_json::from_str(&redact_sensitive_text(&serialized))
        .unwrap_or_else(|_| Value::String("[REDACTED]".to_string()))
}

/// Host services and immutable execution identity for one plugin call.
#[derive(Clone)]
pub struct PluginExecutionContext {
    pub execution_id: String,
    pub step_id: String,
    pub cancellation: ProcessCancellationToken,
    pub artifacts: PluginArtifactStore,
    pub process: PluginProcessService,
    progress: Arc<dyn PluginProgressReporter>,
    observers: Arc<Vec<Arc<dyn PluginLifecycleObserver>>>,
}

impl PluginExecutionContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub fn report_progress(
        &self,
        completed_units: u64,
        total_units: Option<u64>,
        message: impl Into<String>,
    ) {
        self.progress.report(PluginProgressEvent {
            execution_id: self.execution_id.clone(),
            step_id: self.step_id.clone(),
            completed_units,
            total_units,
            message: message.into(),
        });
    }

    fn observe(&self, transform_id: &str, phase: PluginLifecyclePhase, artifacts: Vec<String>) {
        let event = PluginLifecycleEvent {
            contract: PLUGIN_SDK_CONTRACT.to_string(),
            execution_id: self.execution_id.clone(),
            step_id: Some(self.step_id.clone()),
            transform_id: Some(transform_id.to_string()),
            phase,
            artifact_ids: artifacts,
            detail: BTreeMap::new(),
        };
        for observer in self.observers.iter() {
            observer.observe(&event);
        }
    }
}

/// Typed invocation passed to a v2 transform.
#[derive(Debug, Clone)]
pub struct PluginTransformRequest {
    pub inputs: ArtifactCollection,
    pub output_format: Format,
    pub config: Value,
}

/// Typed result returned by a v2 transform.
#[derive(Debug, Clone, Default)]
pub struct PluginTransformResult {
    pub outputs: ArtifactCollection,
    pub diagnostics: Vec<PluginDiagnostic>,
    pub metrics: BTreeMap<String, f64>,
    pub provenance: BTreeMap<String, Value>,
}

/// Artifact-native transform contract for new plugins.
pub trait PluginTransformV2: Send + Sync {
    fn descriptor(&self) -> PluginTransformDescriptor;

    fn config_schema(&self) -> PluginConfigSchema {
        PluginConfigSchema::no_config()
    }

    /// Validate configuration before any artifact or process side effect.
    fn validate_config(&self, config: &Value) -> Result<()> {
        if self.config_schema() == PluginConfigSchema::no_config()
            && config.as_object().is_some_and(|object| !object.is_empty())
        {
            anyhow::bail!("plugin does not accept configuration values");
        }
        Ok(())
    }

    fn execute(
        &self,
        request: PluginTransformRequest,
        context: &PluginExecutionContext,
    ) -> Result<PluginTransformResult>;
}

/// Explicit behavior when a transform identifier is already registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginReplacementPolicy {
    Reject,
    ReplaceSameCapability,
}

/// Registry error returned instead of silently replacing plugins.
#[derive(Debug, thiserror::Error)]
pub enum PluginRegistrationError {
    #[error("invalid plugin descriptor: {0}")]
    InvalidDescriptor(String),
    #[error("plugin transform '{0}' is already registered")]
    Duplicate(String),
    #[error("plugin transform '{id}' cannot replace capability '{existing}' with '{replacement}'")]
    CapabilityMismatch {
        id: String,
        existing: String,
        replacement: String,
    },
}

struct PluginV2Entry {
    transform: Arc<dyn PluginTransformV2>,
    descriptor: PluginTransformDescriptor,
}

/// Versioned registry with explicit duplicate/replacement semantics.
#[derive(Default)]
pub struct PluginRegistryV2 {
    entries: HashMap<String, PluginV2Entry>,
}

impl PluginRegistryV2 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        transform: Arc<dyn PluginTransformV2>,
    ) -> Result<&mut Self, PluginRegistrationError> {
        self.register_with_policy(transform, PluginReplacementPolicy::Reject)
    }

    pub fn register_with_policy(
        &mut self,
        transform: Arc<dyn PluginTransformV2>,
        policy: PluginReplacementPolicy,
    ) -> Result<&mut Self, PluginRegistrationError> {
        let descriptor = transform.descriptor();
        descriptor
            .validate()
            .map_err(|error| PluginRegistrationError::InvalidDescriptor(error.to_string()))?;
        transform
            .config_schema()
            .validate()
            .map_err(|error| PluginRegistrationError::InvalidDescriptor(error.to_string()))?;
        if let Some(existing) = self.entries.get(&descriptor.id) {
            match policy {
                PluginReplacementPolicy::Reject => {
                    return Err(PluginRegistrationError::Duplicate(descriptor.id));
                }
                PluginReplacementPolicy::ReplaceSameCapability
                    if existing.descriptor.capability_id != descriptor.capability_id =>
                {
                    return Err(PluginRegistrationError::CapabilityMismatch {
                        id: descriptor.id,
                        existing: existing.descriptor.capability_id.clone(),
                        replacement: descriptor.capability_id,
                    });
                }
                PluginReplacementPolicy::ReplaceSameCapability => {}
            }
        }
        self.entries.insert(
            descriptor.id.clone(),
            PluginV2Entry {
                transform,
                descriptor,
            },
        );
        Ok(self)
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn PluginTransformV2>> {
        self.entries
            .get(id)
            .map(|entry| Arc::clone(&entry.transform))
    }

    pub fn descriptor(&self, id: &str) -> Option<&PluginTransformDescriptor> {
        self.entries.get(id).map(|entry| &entry.descriptor)
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &PluginTransformDescriptor> {
        self.entries.values().map(|entry| &entry.descriptor)
    }
}

#[derive(Clone)]
struct PluginAdapterRuntime {
    execution_id: Option<String>,
    config: Value,
    process: ProcessExecutor,
    cancellation: ProcessCancellationToken,
    progress: Arc<dyn PluginProgressReporter>,
    observers: Arc<Vec<Arc<dyn PluginLifecycleObserver>>>,
}

impl Default for PluginAdapterRuntime {
    fn default() -> Self {
        Self {
            execution_id: None,
            config: Value::Object(Default::default()),
            process: ProcessExecutor::new(),
            cancellation: ProcessCancellationToken::new(),
            progress: Arc::new(NoopProgressReporter),
            observers: Arc::new(Vec::new()),
        }
    }
}

/// Adapter settings supplied by an embedding host at registration time.
#[derive(Clone, Default)]
pub struct PluginRuntimeOptions {
    runtime: PluginAdapterRuntime,
}

impl PluginRuntimeOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(mut self, config: Value) -> Self {
        self.runtime.config = config;
        self
    }

    pub fn with_execution_id(mut self, execution_id: impl Into<String>) -> Self {
        self.runtime.execution_id = Some(execution_id.into());
        self
    }

    pub fn with_process_executor(mut self, executor: ProcessExecutor) -> Self {
        self.runtime.process = executor;
        self
    }

    pub fn with_cancellation(mut self, cancellation: ProcessCancellationToken) -> Self {
        self.runtime.cancellation = cancellation;
        self
    }

    pub fn with_progress_reporter(mut self, reporter: Arc<dyn PluginProgressReporter>) -> Self {
        self.runtime.progress = reporter;
        self
    }

    pub fn with_observer(mut self, observer: Arc<dyn PluginLifecycleObserver>) -> Self {
        Arc::make_mut(&mut self.runtime.observers).push(observer);
        self
    }
}

/// Single-input adapter used by the canonical artifact DAG executor.
pub struct PluginV2ArtifactAdapter {
    transform: Arc<dyn PluginTransformV2>,
    descriptor: PluginTransformDescriptor,
    runtime: PluginAdapterRuntime,
}

impl PluginV2ArtifactAdapter {
    pub fn new(
        transform: Arc<dyn PluginTransformV2>,
        options: PluginRuntimeOptions,
    ) -> Result<Self> {
        let descriptor = transform.descriptor();
        descriptor.validate()?;
        transform.config_schema().validate()?;
        if descriptor.input_kind != PluginInputKind::Single {
            anyhow::bail!("plugin '{}' is not a single-input transform", descriptor.id);
        }
        transform.validate_config(&options.runtime.config)?;
        Ok(Self {
            transform,
            descriptor,
            runtime: options.runtime,
        })
    }
}

impl ArtifactTransform for PluginV2ArtifactAdapter {
    fn name(&self) -> &str {
        &self.descriptor.id
    }

    fn version(&self) -> &str {
        &self.descriptor.version
    }

    fn cache_identity(&self) -> String {
        adapter_cache_identity(&self.descriptor, &self.runtime.config)
    }

    fn cacheable(&self) -> bool {
        self.descriptor.cache_policy == PluginCachePolicy::ContentAddressed
            && self.descriptor.determinism == PluginDeterminism::Deterministic
    }

    fn fidelity(&self) -> Option<FidelityDeclaration> {
        Some(fidelity_declaration(self.descriptor.loss_profile))
    }

    fn apply(
        &self,
        input: &Artifact,
        output_format: Format,
        store: &ArtifactStore,
    ) -> Result<Artifact> {
        execute_plugin(
            self.transform.as_ref(),
            &self.descriptor,
            &self.runtime,
            ArtifactCollection::one(input.clone()),
            output_format,
            store,
        )?
        .outputs
        .into_one()
    }
}

/// Ordered-collection adapter used by the canonical artifact DAG executor.
pub struct PluginV2CollectionAdapter {
    transform: Arc<dyn PluginTransformV2>,
    descriptor: PluginTransformDescriptor,
    runtime: PluginAdapterRuntime,
}

impl PluginV2CollectionAdapter {
    pub fn new(
        transform: Arc<dyn PluginTransformV2>,
        options: PluginRuntimeOptions,
    ) -> Result<Self> {
        let descriptor = transform.descriptor();
        descriptor.validate()?;
        transform.config_schema().validate()?;
        if descriptor.input_kind != PluginInputKind::OrderedCollection {
            anyhow::bail!("plugin '{}' is not a collection transform", descriptor.id);
        }
        transform.validate_config(&options.runtime.config)?;
        Ok(Self {
            transform,
            descriptor,
            runtime: options.runtime,
        })
    }
}

impl ArtifactCollectionTransform for PluginV2CollectionAdapter {
    fn name(&self) -> &str {
        &self.descriptor.id
    }

    fn version(&self) -> &str {
        &self.descriptor.version
    }

    fn cache_identity(&self) -> String {
        adapter_cache_identity(&self.descriptor, &self.runtime.config)
    }

    fn cacheable(&self) -> bool {
        self.descriptor.cache_policy == PluginCachePolicy::ContentAddressed
            && self.descriptor.determinism == PluginDeterminism::Deterministic
    }

    fn fidelity(&self) -> Option<FidelityDeclaration> {
        Some(fidelity_declaration(self.descriptor.loss_profile))
    }

    fn apply(
        &self,
        inputs: &ArtifactCollection,
        output_format: Format,
        store: &ArtifactStore,
    ) -> Result<Artifact> {
        execute_plugin(
            self.transform.as_ref(),
            &self.descriptor,
            &self.runtime,
            inputs.clone(),
            output_format,
            store,
        )?
        .outputs
        .into_one()
    }
}

fn adapter_cache_identity(descriptor: &PluginTransformDescriptor, config: &Value) -> String {
    serde_json::to_string(&(descriptor, config)).unwrap_or_else(|_| {
        format!(
            "{}:{}:{}",
            descriptor.contract, descriptor.id, descriptor.version
        )
    })
}

fn fidelity_declaration(profile: PluginLossProfile) -> FidelityDeclaration {
    match profile {
        PluginLossProfile::Lossless => FidelityDeclaration::Lossless,
        PluginLossProfile::Partial => FidelityDeclaration::Partial,
        PluginLossProfile::Lossy => FidelityDeclaration::Lossy,
        PluginLossProfile::PathDependent => FidelityDeclaration::PathDependent,
    }
}

fn execute_plugin(
    transform: &dyn PluginTransformV2,
    descriptor: &PluginTransformDescriptor,
    runtime: &PluginAdapterRuntime,
    inputs: ArtifactCollection,
    output_format: Format,
    store: &ArtifactStore,
) -> Result<PluginTransformResult> {
    if inputs.is_empty() {
        anyhow::bail!("plugin '{}' requires at least one input", descriptor.id);
    }
    if descriptor.input_kind == PluginInputKind::Single && inputs.len() != 1 {
        anyhow::bail!("plugin '{}' requires exactly one input", descriptor.id);
    }
    let input_format = inputs
        .iter()
        .next()
        .expect("non-empty inputs checked")
        .format()
        .as_str()
        .parse::<Format>()
        .with_context(|| {
            format!(
                "plugin '{}' received an unknown input format",
                descriptor.id
            )
        })?;
    if inputs
        .iter()
        .any(|artifact| artifact.format().as_str() != input_format.to_string())
    {
        anyhow::bail!(
            "plugin '{}' received a mixed-format collection",
            descriptor.id
        );
    }
    if !descriptor.accepts(input_format, output_format) {
        anyhow::bail!(
            "plugin '{}' does not declare support for '{}' to '{}'",
            descriptor.id,
            input_format,
            output_format
        );
    }
    transform.validate_config(&runtime.config)?;
    if runtime.cancellation.is_cancelled() {
        anyhow::bail!(
            "plugin '{}' execution was cancelled before start",
            descriptor.id
        );
    }

    let step_id = format!(
        "plugin:{}:{}-to-{}",
        descriptor.id, input_format, output_format
    );
    let artifact_store = PluginArtifactStore::new(store.clone(), &inputs, output_format)?;
    let context = PluginExecutionContext {
        execution_id: runtime.execution_id.clone().unwrap_or_else(|| {
            format!(
                "execution:{}:{}",
                std::process::id(),
                crate::evidence::unix_time_ms()
            )
        }),
        step_id,
        cancellation: runtime.cancellation.clone(),
        artifacts: artifact_store.clone(),
        process: PluginProcessService::new(runtime.process.clone(), runtime.cancellation.clone()),
        progress: Arc::clone(&runtime.progress),
        observers: Arc::clone(&runtime.observers),
    };
    context.observe(
        &descriptor.id,
        PluginLifecyclePhase::ExecuteStarted,
        inputs
            .iter()
            .map(|artifact| artifact.id().to_string())
            .collect(),
    );
    let request = PluginTransformRequest {
        inputs,
        output_format,
        config: runtime.config.clone(),
    };
    let result = transform.execute(request, &context);
    artifact_store.verify_inputs()?;
    match result {
        Ok(mut result) => {
            if result.outputs.is_empty() {
                anyhow::bail!("plugin '{}' produced no artifacts", descriptor.id);
            }
            artifact_store.verify_outputs(&result.outputs)?;
            result.outputs = ArtifactCollection::new(
                result
                    .outputs
                    .iter()
                    .map(|artifact| {
                        artifact_store.seal_output(
                            artifact,
                            descriptor,
                            &result.diagnostics,
                            &result.metrics,
                            &result.provenance,
                        )
                    })
                    .collect::<Result<Vec<_>>>()?,
            );
            context.observe(
                &descriptor.id,
                PluginLifecyclePhase::ArtifactCommitted,
                result
                    .outputs
                    .iter()
                    .map(|artifact| artifact.id().to_string())
                    .collect(),
            );
            context.observe(
                &descriptor.id,
                PluginLifecyclePhase::ExecuteCompleted,
                result
                    .outputs
                    .iter()
                    .map(|artifact| artifact.id().to_string())
                    .collect(),
            );
            Ok(result)
        }
        Err(error) => {
            context.observe(
                &descriptor.id,
                PluginLifecyclePhase::ExecuteFailed,
                Vec::new(),
            );
            Err(error)
        }
    }
}

/// Compatibility adapter that lets an existing text plugin run through v2.
pub struct LegacyTextPluginAdapter {
    executor: Arc<dyn PluginExecutor>,
    descriptor: PluginTransformDescriptor,
}

impl LegacyTextPluginAdapter {
    pub fn new(
        executor: Arc<dyn PluginExecutor>,
        version: impl Into<String>,
        capability_id: impl Into<String>,
        input: Format,
        output: Format,
    ) -> Self {
        let id = executor.name().to_string();
        Self {
            executor,
            descriptor: PluginTransformDescriptor::new(
                id,
                version,
                capability_id,
                PluginInputKind::Single,
            )
            .with_formats([input.to_string()], [output.to_string()]),
        }
    }
}

impl PluginTransformV2 for LegacyTextPluginAdapter {
    fn descriptor(&self) -> PluginTransformDescriptor {
        self.descriptor.clone()
    }

    fn execute(
        &self,
        request: PluginTransformRequest,
        context: &PluginExecutionContext,
    ) -> Result<PluginTransformResult> {
        let input = request.inputs.into_one()?;
        let text = String::from_utf8(context.artifacts.read_input_bytes(&input)?)
            .context("legacy text plugin received a non-UTF-8 artifact")?;
        let output = self.executor.execute(text)?;
        let artifact = context
            .artifacts
            .commit_bytes(output.as_bytes(), None, BTreeMap::new())?;
        Ok(PluginTransformResult {
            outputs: ArtifactCollection::one(artifact),
            ..PluginTransformResult::default()
        })
    }
}

/// Thread-safe observer useful to embedders that need an in-memory event log.
#[derive(Default)]
pub struct RecordingPluginObserver {
    events: Mutex<Vec<PluginLifecycleEvent>>,
}

impl RecordingPluginObserver {
    pub fn events(&self) -> Vec<PluginLifecycleEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

impl PluginLifecycleObserver for RecordingPluginObserver {
    fn observe(&self, event: &PluginLifecycleEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactDescriptor;

    struct CopyPlugin {
        input_kind: PluginInputKind,
    }

    impl PluginTransformV2 for CopyPlugin {
        fn descriptor(&self) -> PluginTransformDescriptor {
            PluginTransformDescriptor::new(
                "test.copy",
                "1.0.0",
                "transform.binary.copy",
                self.input_kind,
            )
            .with_formats(["png"], ["png"])
        }

        fn execute(
            &self,
            request: PluginTransformRequest,
            context: &PluginExecutionContext,
        ) -> Result<PluginTransformResult> {
            let mut bytes = Vec::new();
            for input in request.inputs.iter() {
                bytes.extend(context.artifacts.read_input_bytes(input)?);
            }
            let output = context
                .artifacts
                .commit_bytes(&bytes, None, BTreeMap::new())?;
            Ok(PluginTransformResult {
                outputs: ArtifactCollection::one(output),
                diagnostics: vec![PluginDiagnostic {
                    code: "test.ok".to_string(),
                    message: "copied".to_string(),
                    help: None,
                }],
                ..PluginTransformResult::default()
            })
        }
    }

    fn fixture_store() -> (tempfile::TempDir, ArtifactStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        (directory, store)
    }

    fn input(store: &ArtifactStore, bytes: &[u8]) -> Artifact {
        store
            .put_bytes(
                bytes,
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap()
    }

    #[test]
    fn binary_plugin_preserves_bytes_and_seals_provenance() {
        let (_directory, store) = fixture_store();
        let source = input(&store, &[0, 159, 146, 150]);
        let adapter = PluginV2ArtifactAdapter::new(
            Arc::new(CopyPlugin {
                input_kind: PluginInputKind::Single,
            }),
            PluginRuntimeOptions::new(),
        )
        .unwrap();
        let output = adapter.apply(&source, Format::Png, &store).unwrap();
        assert_eq!(store.read_bytes(&output).unwrap(), vec![0, 159, 146, 150]);
        assert_eq!(output.sources(), &[source.id().clone()]);
        assert_eq!(
            output.metadata().get("renderflow.plugin.transform_version"),
            Some(&Value::String("1.0.0".to_string()))
        );
    }

    #[test]
    fn collection_plugin_receives_ordered_binary_inputs() {
        let (_directory, store) = fixture_store();
        let first = input(&store, &[1, 2]);
        let second = input(&store, &[3, 4]);
        let adapter = PluginV2CollectionAdapter::new(
            Arc::new(CopyPlugin {
                input_kind: PluginInputKind::OrderedCollection,
            }),
            PluginRuntimeOptions::new(),
        )
        .unwrap();
        let output = adapter
            .apply(
                &ArtifactCollection::new(vec![first.clone(), second.clone()]),
                Format::Png,
                &store,
            )
            .unwrap();
        assert_eq!(store.read_bytes(&output).unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(output.sources(), &[first.id().clone(), second.id().clone()]);
    }

    #[test]
    fn registration_never_silently_replaces_a_plugin() {
        let mut registry = PluginRegistryV2::new();
        let first: Arc<dyn PluginTransformV2> = Arc::new(CopyPlugin {
            input_kind: PluginInputKind::Single,
        });
        registry.register(Arc::clone(&first)).unwrap();
        assert!(matches!(
            registry.register(first),
            Err(PluginRegistrationError::Duplicate(_))
        ));
    }

    #[test]
    fn nondeterministic_plugins_cannot_claim_content_addressed_cache() {
        let descriptor = PluginTransformDescriptor::new(
            "test.random",
            "1.0.0",
            "transform.random",
            PluginInputKind::Single,
        )
        .with_formats(["png"], ["png"])
        .with_execution_properties(
            PluginDeterminism::Nondeterministic,
            PluginCachePolicy::ContentAddressed,
            PluginLossProfile::PathDependent,
        );
        assert!(descriptor.validate().is_err());
    }

    #[test]
    fn lifecycle_observers_receive_read_only_snapshots() {
        let (_directory, store) = fixture_store();
        let source = input(&store, &[1, 2, 3]);
        let observer = Arc::new(RecordingPluginObserver::default());
        let adapter = PluginV2ArtifactAdapter::new(
            Arc::new(CopyPlugin {
                input_kind: PluginInputKind::Single,
            }),
            PluginRuntimeOptions::new().with_observer(observer.clone()),
        )
        .unwrap();
        adapter.apply(&source, Format::Png, &store).unwrap();
        let phases = observer
            .events()
            .into_iter()
            .map(|event| event.phase)
            .collect::<Vec<_>>();
        assert_eq!(
            phases,
            vec![
                PluginLifecyclePhase::ExecuteStarted,
                PluginLifecyclePhase::ArtifactCommitted,
                PluginLifecyclePhase::ExecuteCompleted
            ]
        );
    }
}
