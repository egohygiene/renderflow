//! Runtime external-tool capability registry and reproducible toolchain evidence.
//!
//! The registry separates tool identity/capability declarations from live host
//! probing. Built-in descriptors are loaded from `data/tool-registry.yaml`, while
//! plugins and YAML command transforms can register additional descriptors at
//! runtime through the same public API.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::graph::{Format, MultiTargetDag, TransformGraph};
use crate::process::{ProcessExecutor, ToolProbeEvidence, ToolProbeStatus as ProcessProbeStatus};

pub const TOOL_REGISTRY_SCHEMA: &str = "renderflow.tool-registry/v1";
pub const TOOLCHAIN_SCHEMA: &str = "renderflow.toolchain/v1";

/// Stable machine-readable tool/provider identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolId(String);

impl ToolId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate_identifier("tool id", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ToolId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// Stable machine-readable capability identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate_identifier("capability id", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for CapabilityId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

fn validate_identifier(kind: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        anyhow::bail!("{kind} must not be empty");
    }
    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
    {
        anyhow::bail!("{kind} '{value}' may contain only ASCII letters, digits, '.', '_' and '-'");
    }
    Ok(())
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    let mut previous_separator = false;
    for character in value.chars() {
        let normalized = character.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() || matches!(normalized, '_' | '-') {
            output.push(normalized);
            previous_separator = false;
        } else if !previous_separator {
            output.push('-');
            previous_separator = true;
        }
    }
    output.trim_matches('-').to_string()
}

/// Expected reproducibility behavior for a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolDeterminism {
    Deterministic,
    ConfigurationDependent,
    Nondeterministic,
}

/// Whether a provider runs locally or depends on network/service behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLocality {
    Local,
    LocalService,
    NetworkOptional,
    NetworkRequired,
}

/// Expected fidelity/loss behavior for the provider as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolFidelity {
    Lossless,
    PartialLoss,
    Lossy,
    PathDependent,
}

/// Fleet/support importance shown by `doctor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolSupportTier {
    Required,
    #[default]
    Optional,
    Experimental,
}

impl ToolSupportTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Experimental => "experimental",
        }
    }
}

/// How a tool/provider is discovered at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolDiscovery {
    Executable {
        candidates: Vec<String>,
        #[serde(default = "default_version_args")]
        version_args: Vec<String>,
    },
    RuntimeService {
        service: String,
    },
    Virtual,
}

fn default_version_args() -> Vec<String> {
    vec!["--version".to_string()]
}

/// Minimum/maximum accepted semantic-ish numeric version.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolVersionRequirement {
    #[serde(default)]
    pub min_inclusive: Option<String>,
    #[serde(default)]
    pub max_exclusive: Option<String>,
}

impl ToolVersionRequirement {
    pub fn is_unconstrained(&self) -> bool {
        self.min_inclusive.is_none() && self.max_exclusive.is_none()
    }

    fn supports(&self, version_line: Option<&str>) -> Result<bool> {
        if self.is_unconstrained() {
            return Ok(true);
        }
        let installed = version_line
            .and_then(parse_numeric_version)
            .ok_or_else(|| anyhow::anyhow!("version probe did not contain a numeric version"))?;

        if let Some(minimum) = &self.min_inclusive {
            let minimum = parse_numeric_version(minimum)
                .ok_or_else(|| anyhow::anyhow!("invalid registry minimum version '{minimum}'"))?;
            if installed < minimum {
                return Ok(false);
            }
        }
        if let Some(maximum) = &self.max_exclusive {
            let maximum = parse_numeric_version(maximum)
                .ok_or_else(|| anyhow::anyhow!("invalid registry maximum version '{maximum}'"))?;
            if installed >= maximum {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NumericVersion([u64; 3]);

fn parse_numeric_version(text: &str) -> Option<NumericVersion> {
    for token in text.split(|character: char| !(character.is_ascii_digit() || character == '.')) {
        let token = token.trim_matches('.');
        if token.is_empty() || !token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        let mut parts = [0_u64; 3];
        let mut parsed_any = false;
        for (index, part) in token.split('.').take(3).enumerate() {
            if part.is_empty() {
                break;
            }
            let Ok(value) = part.parse::<u64>() else {
                break;
            };
            parts[index] = value;
            parsed_any = true;
        }
        if parsed_any {
            return Some(NumericVersion(parts));
        }
    }
    None
}

fn normalized_numeric_version(text: &str) -> Option<String> {
    parse_numeric_version(text)
        .map(|version| format!("{}.{}.{}", version.0[0], version.0[1], version.0[2]))
}

/// Environment requirement without storing or exposing the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolEnvironmentRequirement {
    pub name: String,
    #[serde(default)]
    pub credential: bool,
}

/// Canonical provider/tool descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub id: ToolId,
    pub name: String,
    pub discovery: ToolDiscovery,
    #[serde(default)]
    pub version: ToolVersionRequirement,
    #[serde(default)]
    pub operating_systems: Vec<String>,
    #[serde(default)]
    pub architectures: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<CapabilityId>,
    #[serde(default)]
    pub input_media_types: Vec<String>,
    #[serde(default)]
    pub output_media_types: Vec<String>,
    pub determinism: ToolDeterminism,
    pub locality: ToolLocality,
    pub fidelity: ToolFidelity,
    #[serde(default)]
    pub required_environment: Vec<ToolEnvironmentRequirement>,
    #[serde(default)]
    pub required_configuration: Vec<String>,
    #[serde(default)]
    pub required_services: Vec<String>,
    #[serde(default)]
    pub fallbacks: Vec<ToolId>,
    #[serde(default)]
    pub support_tier: ToolSupportTier,
    #[serde(default)]
    pub license_notes: Option<String>,
    #[serde(default)]
    pub distribution_notes: Option<String>,
}

impl ToolDescriptor {
    pub fn command(id: ToolId, executable: impl Into<String>) -> Self {
        let executable = executable.into();
        Self {
            name: executable.clone(),
            id,
            discovery: ToolDiscovery::Executable {
                candidates: vec![executable],
                version_args: default_version_args(),
            },
            version: ToolVersionRequirement::default(),
            operating_systems: Vec::new(),
            architectures: Vec::new(),
            capabilities: Vec::new(),
            input_media_types: Vec::new(),
            output_media_types: Vec::new(),
            determinism: ToolDeterminism::ConfigurationDependent,
            locality: ToolLocality::Local,
            fidelity: ToolFidelity::PathDependent,
            required_environment: Vec::new(),
            required_configuration: Vec::new(),
            required_services: Vec::new(),
            fallbacks: Vec::new(),
            support_tier: ToolSupportTier::Experimental,
            license_notes: None,
            distribution_notes: Some(
                "Dynamically registered command provider; distribution is owned by the host environment"
                    .to_string(),
            ),
        }
    }

    pub fn virtual_provider(id: ToolId, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id,
            discovery: ToolDiscovery::Virtual,
            version: ToolVersionRequirement::default(),
            operating_systems: Vec::new(),
            architectures: Vec::new(),
            capabilities: Vec::new(),
            input_media_types: Vec::new(),
            output_media_types: Vec::new(),
            determinism: ToolDeterminism::ConfigurationDependent,
            locality: ToolLocality::NetworkOptional,
            fidelity: ToolFidelity::PathDependent,
            required_environment: Vec::new(),
            required_configuration: Vec::new(),
            required_services: Vec::new(),
            fallbacks: Vec::new(),
            support_tier: ToolSupportTier::Experimental,
            license_notes: None,
            distribution_notes: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ToolRegistryDocument {
    schema: String,
    tools: Vec<ToolDescriptor>,
}

/// Runtime facts used to evaluate non-executable requirements.
#[derive(Debug, Clone)]
pub struct ToolRuntimeContext {
    pub operating_system: String,
    pub architecture: String,
    environment_names: BTreeSet<String>,
    configuration_keys: BTreeSet<String>,
    runtime_services: BTreeSet<String>,
}

impl ToolRuntimeContext {
    pub fn current() -> Self {
        Self {
            operating_system: env::consts::OS.to_string(),
            architecture: env::consts::ARCH.to_string(),
            environment_names: env::vars_os()
                .map(|(name, _)| name.to_string_lossy().to_ascii_uppercase())
                .collect(),
            configuration_keys: BTreeSet::new(),
            runtime_services: BTreeSet::new(),
        }
    }

    pub fn for_platform(os: impl Into<String>, architecture: impl Into<String>) -> Self {
        Self {
            operating_system: os.into(),
            architecture: architecture.into(),
            environment_names: BTreeSet::new(),
            configuration_keys: BTreeSet::new(),
            runtime_services: BTreeSet::new(),
        }
    }

    pub fn with_environment(mut self, name: impl Into<String>) -> Self {
        self.environment_names
            .insert(name.into().to_ascii_uppercase());
        self
    }

    pub fn with_configuration(mut self, key: impl Into<String>) -> Self {
        self.configuration_keys.insert(key.into());
        self
    }

    pub fn with_runtime_service(mut self, service: impl Into<String>) -> Self {
        self.runtime_services.insert(service.into());
        self
    }

    pub fn has_runtime_service(&self, service: &str) -> bool {
        self.runtime_services.contains(service)
    }
}

/// Probe seam used by tests and embedders to avoid arbitrary host-state dependencies.
pub trait ToolProbe: Send + Sync {
    fn probe(&self, executable: &str, version_args: &[String]) -> ToolProbeEvidence;
}

#[derive(Clone, Default)]
pub struct ProcessToolProbe {
    executor: ProcessExecutor,
}

impl ProcessToolProbe {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ToolProbe for ProcessToolProbe {
    fn probe(&self, executable: &str, version_args: &[String]) -> ToolProbeEvidence {
        self.executor
            .probe_version_with_args(executable, version_args)
    }
}

/// Canonical availability state used by planner, doctor, SDK, and CLI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolAvailabilityStatus {
    Available,
    MissingExecutable,
    UnsupportedVersion,
    MissingRuntimeService,
    MissingCredential,
    MissingConfiguration,
    UnsupportedPlatform,
    ProbeFailed,
    UnknownProvider,
}

impl ToolAvailabilityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::MissingExecutable => "missing_executable",
            Self::UnsupportedVersion => "unsupported_version",
            Self::MissingRuntimeService => "missing_runtime_service",
            Self::MissingCredential => "missing_credential",
            Self::MissingConfiguration => "missing_configuration",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::ProbeFailed => "probe_failed",
            Self::UnknownProvider => "unknown_provider",
        }
    }
}

/// One evaluated tool/provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolAvailability {
    pub id: ToolId,
    pub name: String,
    pub status: ToolAvailabilityStatus,
    pub support_tier: ToolSupportTier,
    pub selected_executable: Option<String>,
    pub version_line: Option<String>,
    pub normalized_version: Option<String>,
    pub diagnostic: Option<String>,
}

impl ToolAvailability {
    pub fn is_available(&self) -> bool {
        self.status == ToolAvailabilityStatus::Available
    }

    pub fn summary(&self) -> String {
        let detail = self
            .diagnostic
            .as_deref()
            .or(self.version_line.as_deref())
            .unwrap_or("no additional detail");
        format!("{}: {} ({detail})", self.id, self.status.as_str())
    }
}

/// Deterministically sorted set of evaluated tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInventory {
    pub tools: Vec<ToolAvailability>,
}

impl ToolInventory {
    pub fn get(&self, id: &str) -> Option<&ToolAvailability> {
        self.tools.iter().find(|tool| tool.id.as_str() == id)
    }

    pub fn available_ids(&self) -> HashSet<String> {
        self.tools
            .iter()
            .filter(|tool| tool.is_available())
            .map(|tool| tool.id.to_string())
            .collect()
    }

    pub fn blocked_summaries(&self) -> Vec<String> {
        self.tools
            .iter()
            .filter(|tool| !tool.is_available())
            .map(ToolAvailability::summary)
            .collect()
    }
}

/// Evidence for one selected provider variant/model included in a toolchain fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedToolVariantEvidence {
    pub id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

/// Evidence for a selected provider included in a toolchain fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedToolEvidence {
    pub id: ToolId,
    pub executable: Option<String>,
    pub version: Option<String>,
    pub capabilities: Vec<CapabilityId>,
    pub determinism: ToolDeterminism,
    pub locality: ToolLocality,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<SelectedToolVariantEvidence>,
}

/// Reproducibility evidence derived only from providers selected by a plan/run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainSnapshot {
    pub schema: String,
    pub fingerprint: String,
    pub operating_system: String,
    pub architecture: String,
    pub selected_tools: Vec<SelectedToolEvidence>,
}

#[derive(Serialize)]
struct FingerprintMaterial<'a> {
    schema: &'a str,
    operating_system: &'a str,
    architecture: &'a str,
    selected_tools: &'a [SelectedToolEvidence],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResolutionDecision {
    pub id: ToolId,
    pub status: ToolAvailabilityStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResolution {
    pub requested: ToolId,
    pub selected: Option<ToolId>,
    pub decisions: Vec<ToolResolutionDecision>,
}

/// Public registry that can be extended by plugins without exposing planner internals.
#[derive(Debug, Clone)]
pub struct ToolRegistry {
    schema: String,
    tools: BTreeMap<ToolId, ToolDescriptor>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            schema: TOOL_REGISTRY_SCHEMA.to_string(),
            tools: BTreeMap::new(),
        }
    }

    pub fn builtins() -> Self {
        Self::from_yaml(include_str!("../data/tool-registry.yaml"))
            .expect("embedded Renderflow tool registry must be valid")
    }

    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let document: ToolRegistryDocument =
            serde_yaml_ng::from_str(yaml).context("failed to parse tool registry YAML")?;
        if document.schema != TOOL_REGISTRY_SCHEMA {
            anyhow::bail!(
                "unsupported tool registry schema '{}'; expected '{}'",
                document.schema,
                TOOL_REGISTRY_SCHEMA
            );
        }
        let mut registry = Self {
            schema: document.schema,
            tools: BTreeMap::new(),
        };
        for descriptor in document.tools {
            registry.register(descriptor)?;
        }
        Ok(registry)
    }

    pub fn schema(&self) -> &str {
        &self.schema
    }

    pub fn register(&mut self, mut descriptor: ToolDescriptor) -> Result<&mut Self> {
        validate_identifier("tool id", descriptor.id.as_str())?;
        for capability in &descriptor.capabilities {
            validate_identifier("capability id", capability.as_str())?;
        }
        descriptor.capabilities.sort();
        descriptor.capabilities.dedup();
        descriptor.fallbacks.sort();
        descriptor.fallbacks.dedup();
        self.tools.insert(descriptor.id.clone(), descriptor);
        Ok(self)
    }

    pub fn contains(&self, id: &ToolId) -> bool {
        self.tools.contains_key(id)
    }

    pub fn get(&self, id: &str) -> Option<&ToolDescriptor> {
        self.tools.values().find(|tool| tool.id.as_str() == id)
    }

    pub fn all(&self) -> impl Iterator<Item = &ToolDescriptor> {
        self.tools.values()
    }

    pub fn capabilities(&self) -> BTreeMap<String, Vec<String>> {
        let mut capabilities: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for descriptor in self.tools.values() {
            for capability in &descriptor.capabilities {
                capabilities
                    .entry(capability.to_string())
                    .or_default()
                    .push(descriptor.id.to_string());
            }
        }
        for providers in capabilities.values_mut() {
            providers.sort();
            providers.dedup();
        }
        capabilities
    }

    pub fn canonical_id_for_executable(&self, executable: &str) -> ToolId {
        for descriptor in self.tools.values() {
            if let ToolDiscovery::Executable { candidates, .. } = &descriptor.discovery {
                if candidates.iter().any(|candidate| candidate == executable) {
                    return descriptor.id.clone();
                }
            }
        }
        canonical_tool_id_for_hint(executable)
    }

    pub fn ensure_command_provider(
        &mut self,
        id: ToolId,
        executable: impl Into<String>,
        capability: CapabilityId,
    ) -> Result<&mut Self> {
        if !self.tools.contains_key(&id) {
            self.register(ToolDescriptor::command(id.clone(), executable))?;
        }
        self.add_capability(&id, capability)?;
        Ok(self)
    }

    pub fn ensure_virtual_provider(
        &mut self,
        id: ToolId,
        name: impl Into<String>,
        capability: CapabilityId,
    ) -> Result<&mut Self> {
        if !self.tools.contains_key(&id) {
            self.register(ToolDescriptor::virtual_provider(id.clone(), name))?;
        }
        self.add_capability(&id, capability)?;
        Ok(self)
    }

    pub fn add_capability(&mut self, id: &ToolId, capability: CapabilityId) -> Result<&mut Self> {
        let descriptor = self
            .tools
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("tool provider '{}' is not registered", id))?;
        if !descriptor.capabilities.contains(&capability) {
            descriptor.capabilities.push(capability);
            descriptor.capabilities.sort();
        }
        Ok(self)
    }

    pub fn assess_all_with(
        &self,
        probe: &dyn ToolProbe,
        context: &ToolRuntimeContext,
    ) -> ToolInventory {
        let ids: Vec<String> = self.tools.keys().map(ToString::to_string).collect();
        self.assess_ids_with(ids, probe, context)
    }

    pub fn assess_all_current(&self) -> ToolInventory {
        self.assess_all_with(&ProcessToolProbe::new(), &ToolRuntimeContext::current())
    }

    pub fn assess_ids_with<I, S>(
        &self,
        ids: I,
        probe: &dyn ToolProbe,
        context: &ToolRuntimeContext,
    ) -> ToolInventory
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let unique: BTreeSet<String> = ids.into_iter().map(|id| id.as_ref().to_string()).collect();
        let mut tools = Vec::with_capacity(unique.len());
        for id in unique {
            if let Some(descriptor) = self.get(&id) {
                tools.push(evaluate_descriptor(descriptor, probe, context));
            } else {
                let tool_id = ToolId::new(id.clone())
                    .unwrap_or_else(|_| ToolId(format!("tool.unknown.{}", slug(&id))));
                tools.push(ToolAvailability {
                    id: tool_id,
                    name: id.clone(),
                    status: ToolAvailabilityStatus::UnknownProvider,
                    support_tier: ToolSupportTier::Experimental,
                    selected_executable: None,
                    version_line: None,
                    normalized_version: None,
                    diagnostic: Some(format!("provider '{id}' is not registered")),
                });
            }
        }
        ToolInventory { tools }
    }

    pub fn assess_ids_current<I, S>(&self, ids: I) -> ToolInventory
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.assess_ids_with(
            ids,
            &ProcessToolProbe::new(),
            &ToolRuntimeContext::current(),
        )
    }

    pub fn fingerprint_selected<I, S>(
        &self,
        inventory: &ToolInventory,
        ids: I,
        context: &ToolRuntimeContext,
    ) -> Result<ToolchainSnapshot>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.fingerprint_selected_with_variants(inventory, ids, &BTreeMap::new(), context)
    }

    pub fn fingerprint_selected_with_variants<I, S>(
        &self,
        inventory: &ToolInventory,
        ids: I,
        variants: &BTreeMap<String, Vec<SelectedToolVariantEvidence>>,
        context: &ToolRuntimeContext,
    ) -> Result<ToolchainSnapshot>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let ids: BTreeSet<String> = ids.into_iter().map(|id| id.as_ref().to_string()).collect();
        let mut selected_tools = Vec::with_capacity(ids.len());
        for id in ids {
            let availability = inventory
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("tool '{}' was not assessed", id))?;
            if !availability.is_available() {
                anyhow::bail!(
                    "cannot fingerprint unavailable provider '{}': {}",
                    id,
                    availability.summary()
                );
            }
            let descriptor = self
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("tool '{}' is not registered", id))?;
            let mut selected_variants = variants.get(&id).cloned().unwrap_or_default();
            selected_variants.sort_by(|left, right| {
                left.id
                    .cmp(&right.id)
                    .then_with(|| left.attributes.cmp(&right.attributes))
            });
            selected_variants.dedup();
            selected_tools.push(SelectedToolEvidence {
                id: descriptor.id.clone(),
                executable: availability.selected_executable.clone(),
                version: availability
                    .normalized_version
                    .clone()
                    .or_else(|| availability.version_line.clone()),
                capabilities: descriptor.capabilities.clone(),
                determinism: descriptor.determinism,
                locality: descriptor.locality,
                variants: selected_variants,
            });
        }
        selected_tools.sort_by(|left, right| left.id.cmp(&right.id));

        let material = FingerprintMaterial {
            schema: TOOLCHAIN_SCHEMA,
            operating_system: &context.operating_system,
            architecture: &context.architecture,
            selected_tools: &selected_tools,
        };
        let encoded = serde_json::to_vec(&material)
            .context("failed to serialize toolchain fingerprint material")?;
        let digest = Sha256::digest(encoded);
        Ok(ToolchainSnapshot {
            schema: TOOLCHAIN_SCHEMA.to_string(),
            fingerprint: format!("sha256:{digest:x}"),
            operating_system: context.operating_system.clone(),
            architecture: context.architecture.clone(),
            selected_tools,
        })
    }

    pub fn fingerprint_for_dag(
        &self,
        inventory: &ToolInventory,
        dag: &MultiTargetDag,
        context: &ToolRuntimeContext,
    ) -> Result<ToolchainSnapshot> {
        let mut variants: BTreeMap<String, Vec<SelectedToolVariantEvidence>> = BTreeMap::new();
        for edge in dag.all_edges() {
            if let (Some(provider_id), Some(variant_id)) =
                (edge.provider_id.as_deref(), edge.variant_id.as_deref())
            {
                variants.entry(provider_id.to_string()).or_default().push(
                    SelectedToolVariantEvidence {
                        id: variant_id.to_string(),
                        attributes: edge.evidence.clone(),
                    },
                );
            }
        }
        self.fingerprint_selected_with_variants(
            inventory,
            dag.all_edges().iter().flat_map(|edge| {
                edge.provider_id
                    .iter()
                    .map(String::as_str)
                    .chain(edge.required_provider_ids.iter().map(String::as_str))
            }),
            &variants,
            context,
        )
    }

    pub fn resolve_with(
        &self,
        requested: &ToolId,
        probe: &dyn ToolProbe,
        context: &ToolRuntimeContext,
    ) -> ToolResolution {
        let mut queue = vec![requested.clone()];
        let mut visited = BTreeSet::new();
        let mut decisions = Vec::new();

        while let Some(id) = queue.first().cloned() {
            queue.remove(0);
            if !visited.insert(id.clone()) {
                continue;
            }
            let Some(descriptor) = self.tools.get(&id) else {
                decisions.push(ToolResolutionDecision {
                    id,
                    status: ToolAvailabilityStatus::UnknownProvider,
                    reason: "provider is not registered".to_string(),
                });
                continue;
            };
            let availability = evaluate_descriptor(descriptor, probe, context);
            decisions.push(ToolResolutionDecision {
                id: id.clone(),
                status: availability.status,
                reason: availability.summary(),
            });
            if availability.is_available() {
                return ToolResolution {
                    requested: requested.clone(),
                    selected: Some(id),
                    decisions,
                };
            }
            queue.extend(descriptor.fallbacks.iter().cloned());
        }

        ToolResolution {
            requested: requested.clone(),
            selected: None,
            decisions,
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn evaluate_descriptor(
    descriptor: &ToolDescriptor,
    probe: &dyn ToolProbe,
    context: &ToolRuntimeContext,
) -> ToolAvailability {
    let unavailable = |status: ToolAvailabilityStatus, diagnostic: String| ToolAvailability {
        id: descriptor.id.clone(),
        name: descriptor.name.clone(),
        status,
        support_tier: descriptor.support_tier,
        selected_executable: None,
        version_line: None,
        normalized_version: None,
        diagnostic: Some(diagnostic),
    };

    if !descriptor.operating_systems.is_empty()
        && !descriptor
            .operating_systems
            .iter()
            .any(|os| os == &context.operating_system)
    {
        return unavailable(
            ToolAvailabilityStatus::UnsupportedPlatform,
            format!(
                "platform '{}' is unsupported; expected one of {}",
                context.operating_system,
                descriptor.operating_systems.join(", ")
            ),
        );
    }
    if !descriptor.architectures.is_empty()
        && !descriptor
            .architectures
            .iter()
            .any(|architecture| architecture == &context.architecture)
    {
        return unavailable(
            ToolAvailabilityStatus::UnsupportedPlatform,
            format!(
                "architecture '{}' is unsupported; expected one of {}",
                context.architecture,
                descriptor.architectures.join(", ")
            ),
        );
    }

    for requirement in &descriptor.required_environment {
        if !context
            .environment_names
            .contains(&requirement.name.to_ascii_uppercase())
        {
            return unavailable(
                if requirement.credential {
                    ToolAvailabilityStatus::MissingCredential
                } else {
                    ToolAvailabilityStatus::MissingConfiguration
                },
                format!(
                    "required environment variable '{}' is not set",
                    requirement.name
                ),
            );
        }
    }
    for key in &descriptor.required_configuration {
        if !context.configuration_keys.contains(key) {
            return unavailable(
                ToolAvailabilityStatus::MissingConfiguration,
                format!("required configuration key '{key}' is not available"),
            );
        }
    }
    for service in &descriptor.required_services {
        if !context.runtime_services.contains(service) {
            return unavailable(
                ToolAvailabilityStatus::MissingRuntimeService,
                format!("required runtime service '{service}' is unavailable"),
            );
        }
    }

    match &descriptor.discovery {
        ToolDiscovery::Virtual => ToolAvailability {
            id: descriptor.id.clone(),
            name: descriptor.name.clone(),
            status: ToolAvailabilityStatus::Available,
            support_tier: descriptor.support_tier,
            selected_executable: None,
            version_line: None,
            normalized_version: None,
            diagnostic: None,
        },
        ToolDiscovery::RuntimeService { service } => {
            if context.runtime_services.contains(service) {
                ToolAvailability {
                    id: descriptor.id.clone(),
                    name: descriptor.name.clone(),
                    status: ToolAvailabilityStatus::Available,
                    support_tier: descriptor.support_tier,
                    selected_executable: None,
                    version_line: None,
                    normalized_version: None,
                    diagnostic: Some(format!("runtime service '{service}' is available")),
                }
            } else {
                unavailable(
                    ToolAvailabilityStatus::MissingRuntimeService,
                    format!("runtime service '{service}' is unavailable"),
                )
            }
        }
        ToolDiscovery::Executable {
            candidates,
            version_args,
        } => {
            let mut saw_probe_failure = None;
            for executable in candidates {
                let evidence = probe.probe(executable, version_args);
                match evidence.status {
                    ProcessProbeStatus::Available => {
                        let version_line = evidence.version_line.clone();
                        match descriptor.version.supports(version_line.as_deref()) {
                            Ok(true) => {
                                return ToolAvailability {
                                    id: descriptor.id.clone(),
                                    name: descriptor.name.clone(),
                                    status: ToolAvailabilityStatus::Available,
                                    support_tier: descriptor.support_tier,
                                    selected_executable: Some(executable.clone()),
                                    normalized_version: version_line
                                        .as_deref()
                                        .and_then(normalized_numeric_version),
                                    version_line,
                                    diagnostic: None,
                                };
                            }
                            Ok(false) => {
                                return ToolAvailability {
                                    id: descriptor.id.clone(),
                                    name: descriptor.name.clone(),
                                    status: ToolAvailabilityStatus::UnsupportedVersion,
                                    support_tier: descriptor.support_tier,
                                    selected_executable: Some(executable.clone()),
                                    normalized_version: version_line
                                        .as_deref()
                                        .and_then(normalized_numeric_version),
                                    diagnostic: Some(format!(
                                        "installed version does not satisfy registry requirement {:?}",
                                        descriptor.version
                                    )),
                                    version_line,
                                };
                            }
                            Err(error) => {
                                return unavailable(
                                    ToolAvailabilityStatus::UnsupportedVersion,
                                    error.to_string(),
                                );
                            }
                        }
                    }
                    ProcessProbeStatus::Missing => {}
                    ProcessProbeStatus::TimedOut | ProcessProbeStatus::Failed => {
                        saw_probe_failure = Some(
                            evidence
                                .diagnostic
                                .unwrap_or_else(|| "version probe failed".to_string()),
                        );
                    }
                }
            }
            if let Some(diagnostic) = saw_probe_failure {
                unavailable(ToolAvailabilityStatus::ProbeFailed, diagnostic)
            } else {
                unavailable(
                    ToolAvailabilityStatus::MissingExecutable,
                    format!("none of [{}] were found", candidates.join(", ")),
                )
            }
        }
    }
}

/// Map known executable/tool hints to stable built-in IDs; arbitrary hints get
/// a stable dynamic-command namespace.
pub fn canonical_tool_id_for_hint(hint: &str) -> ToolId {
    let canonical = match hint {
        "pandoc" => "tool.pandoc".to_string(),
        "tectonic" => "tool.tectonic".to_string(),
        "ffmpeg" => "tool.ffmpeg".to_string(),
        "wkhtmltopdf" => "tool.wkhtmltopdf".to_string(),
        "zip" => "tool.zip".to_string(),
        "img2pdf" => "tool.img2pdf".to_string(),
        "gs" | "ghostscript" => "tool.ghostscript".to_string(),
        "upscayl-ncnn" | "upscayl-bin" => "tool.upscayl-ncnn".to_string(),
        value => format!("tool.command.{}", slug(value)),
    };
    ToolId::new(canonical).expect("canonicalized tool id is valid")
}

/// Stable capability ID for a concrete format transformation.
pub fn transform_capability_id(from: Format, to: Format) -> CapabilityId {
    CapabilityId::new(format!("transform.{}.{}", from, to))
        .expect("Format display values produce valid capability ids")
}

/// Assess all provider IDs referenced by a graph and return an availability-filtered graph.
pub fn filter_graph_for_current_toolchain(
    graph: &TransformGraph,
    registry: &ToolRegistry,
) -> (TransformGraph, ToolInventory, ToolRuntimeContext) {
    let context = ToolRuntimeContext::current();
    let inventory =
        registry.assess_ids_with(graph.provider_ids(), &ProcessToolProbe::new(), &context);
    let available = inventory.available_ids();
    let filtered = graph.filtered_by_available_providers(&available);
    (filtered, inventory, context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{ProcessPlatform, ProcessTreeTermination};

    #[derive(Default)]
    struct FakeProbe {
        responses: BTreeMap<String, ToolProbeEvidence>,
    }

    impl FakeProbe {
        fn with(
            mut self,
            executable: &str,
            status: ProcessProbeStatus,
            version: Option<&str>,
        ) -> Self {
            self.responses.insert(
                executable.to_string(),
                ToolProbeEvidence {
                    executable: executable.to_string(),
                    status,
                    version_line: version.map(str::to_string),
                    duration_ms: 1,
                    platform: ProcessPlatform {
                        os: "test",
                        arch: "test",
                        tree_termination: ProcessTreeTermination::DirectChild,
                    },
                    diagnostic: None,
                },
            );
            self
        }
    }

    impl ToolProbe for FakeProbe {
        fn probe(&self, executable: &str, _version_args: &[String]) -> ToolProbeEvidence {
            self.responses
                .get(executable)
                .cloned()
                .unwrap_or(ToolProbeEvidence {
                    executable: executable.to_string(),
                    status: ProcessProbeStatus::Missing,
                    version_line: None,
                    duration_ms: 0,
                    platform: ProcessPlatform {
                        os: "test",
                        arch: "test",
                        tree_termination: ProcessTreeTermination::DirectChild,
                    },
                    diagnostic: Some("missing".to_string()),
                })
        }
    }

    fn test_descriptor(id: &str, executable: &str) -> ToolDescriptor {
        ToolDescriptor {
            id: ToolId::new(id).unwrap(),
            name: id.to_string(),
            discovery: ToolDiscovery::Executable {
                candidates: vec![executable.to_string()],
                version_args: vec!["--version".to_string()],
            },
            version: ToolVersionRequirement::default(),
            operating_systems: Vec::new(),
            architectures: Vec::new(),
            capabilities: vec![CapabilityId::new("transform.test").unwrap()],
            input_media_types: Vec::new(),
            output_media_types: Vec::new(),
            determinism: ToolDeterminism::Deterministic,
            locality: ToolLocality::Local,
            fidelity: ToolFidelity::Lossless,
            required_environment: Vec::new(),
            required_configuration: Vec::new(),
            required_services: Vec::new(),
            fallbacks: Vec::new(),
            support_tier: ToolSupportTier::Optional,
            license_notes: None,
            distribution_notes: None,
        }
    }

    #[test]
    fn embedded_registry_contains_core_wrapped_tools() {
        let registry = ToolRegistry::builtins();
        assert!(registry.get("tool.pandoc").is_some());
        assert!(registry.get("tool.tectonic").is_some());
        assert!(registry.get("tool.ffmpeg").is_some());
        assert!(registry.get("tool.ghostscript").is_some());
    }

    #[test]
    fn stable_ids_reject_whitespace() {
        assert!(ToolId::new("tool.bad id").is_err());
        assert!(CapabilityId::new("bad capability").is_err());
    }

    #[test]
    fn missing_executable_is_distinct() {
        let mut registry = ToolRegistry::new();
        registry
            .register(test_descriptor("tool.fake", "fake"))
            .unwrap();
        let inventory = registry.assess_all_with(
            &FakeProbe::default(),
            &ToolRuntimeContext::for_platform("linux", "x86_64"),
        );
        assert_eq!(
            inventory.tools[0].status,
            ToolAvailabilityStatus::MissingExecutable
        );
    }

    #[test]
    fn unsupported_version_is_distinct() {
        let mut descriptor = test_descriptor("tool.fake", "fake");
        descriptor.version.min_inclusive = Some("2.0.0".to_string());
        let mut registry = ToolRegistry::new();
        registry.register(descriptor).unwrap();
        let probe =
            FakeProbe::default().with("fake", ProcessProbeStatus::Available, Some("fake 1.9.0"));
        let inventory =
            registry.assess_all_with(&probe, &ToolRuntimeContext::for_platform("linux", "x86_64"));
        assert_eq!(
            inventory.tools[0].status,
            ToolAvailabilityStatus::UnsupportedVersion
        );
    }

    #[test]
    fn doctor_states_distinguish_service_credential_config_and_platform() {
        let mut registry = ToolRegistry::new();

        let mut service =
            ToolDescriptor::virtual_provider(ToolId::new("tool.service").unwrap(), "service");
        service.discovery = ToolDiscovery::RuntimeService {
            service: "daemon".to_string(),
        };
        registry.register(service).unwrap();

        let mut credential =
            ToolDescriptor::virtual_provider(ToolId::new("tool.credential").unwrap(), "credential");
        credential
            .required_environment
            .push(ToolEnvironmentRequirement {
                name: "FAKE_TOKEN".to_string(),
                credential: true,
            });
        registry.register(credential).unwrap();

        let mut configuration = ToolDescriptor::virtual_provider(
            ToolId::new("tool.configuration").unwrap(),
            "configuration",
        );
        configuration
            .required_configuration
            .push("profile".to_string());
        registry.register(configuration).unwrap();

        let mut platform =
            ToolDescriptor::virtual_provider(ToolId::new("tool.platform").unwrap(), "platform");
        platform.operating_systems = vec!["plan9".to_string()];
        registry.register(platform).unwrap();

        let inventory = registry.assess_all_with(
            &FakeProbe::default(),
            &ToolRuntimeContext::for_platform("linux", "x86_64"),
        );
        assert_eq!(
            inventory.get("tool.service").unwrap().status,
            ToolAvailabilityStatus::MissingRuntimeService
        );
        assert_eq!(
            inventory.get("tool.credential").unwrap().status,
            ToolAvailabilityStatus::MissingCredential
        );
        assert_eq!(
            inventory.get("tool.configuration").unwrap().status,
            ToolAvailabilityStatus::MissingConfiguration
        );
        assert_eq!(
            inventory.get("tool.platform").unwrap().status,
            ToolAvailabilityStatus::UnsupportedPlatform
        );
    }

    #[test]
    fn fallback_resolution_explains_why_substitute_won() {
        let mut primary = test_descriptor("tool.primary", "primary");
        primary
            .fallbacks
            .push(ToolId::new("tool.fallback").unwrap());
        let fallback = test_descriptor("tool.fallback", "fallback");
        let mut registry = ToolRegistry::new();
        registry.register(primary).unwrap();
        registry.register(fallback).unwrap();
        let probe = FakeProbe::default().with(
            "fallback",
            ProcessProbeStatus::Available,
            Some("fallback 3.0.0"),
        );
        let resolution = registry.resolve_with(
            &ToolId::new("tool.primary").unwrap(),
            &probe,
            &ToolRuntimeContext::for_platform("linux", "x86_64"),
        );
        assert_eq!(
            resolution.selected.as_ref().map(ToolId::as_str),
            Some("tool.fallback")
        );
        assert_eq!(resolution.decisions.len(), 2);
        assert_eq!(
            resolution.decisions[0].status,
            ToolAvailabilityStatus::MissingExecutable
        );
    }

    #[test]
    fn fingerprint_is_order_independent_and_selected_only() {
        let mut registry = ToolRegistry::new();
        registry.register(test_descriptor("tool.a", "a")).unwrap();
        registry.register(test_descriptor("tool.b", "b")).unwrap();
        let probe = FakeProbe::default()
            .with("a", ProcessProbeStatus::Available, Some("a 1.2.3"))
            .with("b", ProcessProbeStatus::Available, Some("b 4.5.6"));
        let context = ToolRuntimeContext::for_platform("linux", "x86_64");
        let inventory = registry.assess_all_with(&probe, &context);
        let first = registry
            .fingerprint_selected(&inventory, ["tool.b", "tool.a"], &context)
            .unwrap();
        let second = registry
            .fingerprint_selected(&inventory, ["tool.a", "tool.b"], &context)
            .unwrap();
        let only_a = registry
            .fingerprint_selected(&inventory, ["tool.a"], &context)
            .unwrap();
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_ne!(first.fingerprint, only_a.fingerprint);
    }

    #[test]
    fn selected_variant_material_changes_toolchain_fingerprint() {
        let mut registry = ToolRegistry::new();
        registry.register(test_descriptor("tool.a", "a")).unwrap();
        let probe = FakeProbe::default().with("a", ProcessProbeStatus::Available, Some("a 1.2.3"));
        let context = ToolRuntimeContext::for_platform("linux", "x86_64");
        let inventory = registry.assess_all_with(&probe, &context);

        let mut first_variants = BTreeMap::new();
        first_variants.insert(
            "tool.a".to_string(),
            vec![SelectedToolVariantEvidence {
                id: "variant.a".to_string(),
                attributes: BTreeMap::from([(
                    "model_digest".to_string(),
                    "sha256:first".to_string(),
                )]),
            }],
        );
        let mut second_variants = first_variants.clone();
        second_variants.get_mut("tool.a").unwrap()[0]
            .attributes
            .insert("model_digest".to_string(), "sha256:second".to_string());

        let first = registry
            .fingerprint_selected_with_variants(&inventory, ["tool.a"], &first_variants, &context)
            .unwrap();
        let second = registry
            .fingerprint_selected_with_variants(&inventory, ["tool.a"], &second_variants, &context)
            .unwrap();
        assert_ne!(first.fingerprint, second.fingerprint);
    }

    #[test]
    fn dynamic_command_provider_can_be_augmented_with_transform_capability() {
        let mut registry = ToolRegistry::builtins();
        let id = registry.canonical_id_for_executable("made-up-renderer");
        registry
            .ensure_command_provider(
                id.clone(),
                "made-up-renderer",
                CapabilityId::new("transform.markdown.pdf").unwrap(),
            )
            .unwrap();
        let descriptor = registry.get(id.as_str()).unwrap();
        assert!(descriptor
            .capabilities
            .iter()
            .any(|capability| capability.as_str() == "transform.markdown.pdf"));
    }
}
