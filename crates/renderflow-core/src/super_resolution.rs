//! Local AI image super-resolution providers and reproducibility evidence.
//!
//! The Upscayl integration deliberately separates four concerns:
//!
//! - canonical model identity and publication-relevant metadata,
//! - runtime model material discovery/checksums,
//! - spec-v2 variant expansion intent,
//! - bounded execution through [`crate::process::ProcessExecutor`].
//!
//! The canonical planner introduced by Renderflow #354 can consume the pure
//! variant-selection APIs here without creating a second image-only planner.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::process::{
    ProcessExecutor, ProcessExpectedOutput, ProcessNetworkPolicy, ProcessRequest, ToolProbeStatus,
};
use crate::spec::{AiPolicy, SelectorSet, SpecV2, TargetSpec};
use crate::toolchain::{
    SelectedToolVariantEvidence, ToolAvailabilityStatus, ToolInventory, ToolProbe, ToolRegistry,
    ToolRuntimeContext,
};

pub const UPSCAYL_MODEL_CATALOG_SCHEMA: &str = "renderflow.upscayl-models/v1";
pub const UPSCAYL_TOOL_ID: &str = "tool.upscayl-ncnn";
pub const SUPER_RESOLUTION_CAPABILITY_ID: &str = "image.super_resolution";
pub const DEFAULT_UPSCAYL_MODEL: &str = "upscayl-standard-4x";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpscaylModelOrigin {
    BuiltIn,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommercialUsePolicy {
    Permitted,
    Prohibited,
    Unknown,
}

impl CommercialUsePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Permitted => "permitted",
            Self::Prohibited => "prohibited",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpscaylModelDescriptor {
    pub variant_id: String,
    pub model_name: String,
    pub display_name: String,
    pub native_scale: Option<u32>,
    pub origin: UpscaylModelOrigin,
    pub commercial_use: CommercialUsePolicy,
    pub description: String,
    pub license_notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_digest: Option<String>,
}

impl UpscaylModelDescriptor {
    pub fn is_materialized(&self) -> bool {
        self.param_path.is_some()
            && self.bin_path.is_some()
            && self.param_sha256.is_some()
            && self.bin_sha256.is_some()
            && self.model_digest.is_some()
    }

    pub fn toolchain_variant_evidence(&self) -> SelectedToolVariantEvidence {
        let mut attributes = BTreeMap::new();
        attributes.insert("model_name".to_string(), self.model_name.clone());
        attributes.insert(
            "origin".to_string(),
            match self.origin {
                UpscaylModelOrigin::BuiltIn => "built_in",
                UpscaylModelOrigin::Custom => "custom",
            }
            .to_string(),
        );
        attributes.insert(
            "commercial_use".to_string(),
            self.commercial_use.as_str().to_string(),
        );
        if let Some(scale) = self.native_scale {
            attributes.insert("native_scale".to_string(), scale.to_string());
        }
        if let Some(digest) = &self.model_digest {
            attributes.insert("model_digest".to_string(), digest.clone());
        }
        SelectedToolVariantEvidence {
            id: self.variant_id.clone(),
            attributes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpscaylDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

impl UpscaylDiagnostic {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            variant_id: None,
            path: None,
        }
    }

    fn for_variant(mut self, variant_id: impl Into<String>) -> Self {
        self.variant_id = Some(variant_id.into());
        self
    }

    fn at_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpscaylModelCatalog {
    pub schema: String,
    pub models: Vec<UpscaylModelDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<UpscaylDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct UpscaylModelCatalogDocument {
    schema: String,
    models: Vec<UpscaylModelDefinition>,
}

#[derive(Debug, Deserialize)]
struct UpscaylModelDefinition {
    variant_id: String,
    model_name: String,
    display_name: String,
    native_scale: u32,
    commercial_use: CommercialUsePolicy,
    description: String,
    license_notes: String,
}

impl UpscaylModelCatalog {
    pub fn builtins() -> Self {
        Self::from_yaml(include_str!("../data/upscayl-models.yaml"))
            .expect("embedded Upscayl model catalog must be valid")
    }

    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let document: UpscaylModelCatalogDocument =
            serde_yaml_ng::from_str(yaml).context("failed to parse Upscayl model catalog YAML")?;
        if document.schema != UPSCAYL_MODEL_CATALOG_SCHEMA {
            anyhow::bail!(
                "unsupported Upscayl model catalog schema '{}'; expected '{}'",
                document.schema,
                UPSCAYL_MODEL_CATALOG_SCHEMA
            );
        }

        let mut seen_variants = BTreeSet::new();
        let mut seen_names = BTreeSet::new();
        let mut models = Vec::with_capacity(document.models.len());
        for definition in document.models {
            validate_variant_id(&definition.variant_id)?;
            if !seen_variants.insert(definition.variant_id.clone()) {
                anyhow::bail!("duplicate Upscayl variant id '{}'", definition.variant_id);
            }
            if !seen_names.insert(definition.model_name.clone()) {
                anyhow::bail!("duplicate Upscayl model name '{}'", definition.model_name);
            }
            if definition.native_scale == 0 {
                anyhow::bail!(
                    "Upscayl model '{}' has invalid native scale 0",
                    definition.model_name
                );
            }
            models.push(UpscaylModelDescriptor {
                variant_id: definition.variant_id,
                model_name: definition.model_name,
                display_name: definition.display_name,
                native_scale: Some(definition.native_scale),
                origin: UpscaylModelOrigin::BuiltIn,
                commercial_use: definition.commercial_use,
                description: definition.description,
                license_notes: definition.license_notes,
                param_path: None,
                bin_path: None,
                param_sha256: None,
                bin_sha256: None,
                model_digest: None,
            });
        }
        models.sort_by(|left, right| left.variant_id.cmp(&right.variant_id));
        Ok(Self {
            schema: document.schema,
            models,
            diagnostics: Vec::new(),
        })
    }

    /// Discover model material from an Upscayl-compatible model directory.
    ///
    /// Every `.param` file must have a sibling `.bin`. Known built-in model
    /// names materialize their canonical descriptors. Unknown pairs are exposed
    /// as custom variants without mutating the built-in catalog.
    pub fn discover(models_dir: impl AsRef<Path>) -> Result<Self> {
        let models_dir = models_dir.as_ref();
        let mut catalog = Self::builtins();
        if !models_dir.is_dir() {
            catalog.diagnostics.push(
                UpscaylDiagnostic::new(
                    "model_directory.missing",
                    format!(
                        "Upscayl model directory '{}' does not exist or is not a directory",
                        models_dir.display()
                    ),
                )
                .at_path(models_dir),
            );
            return Ok(catalog);
        }

        let mut params = Vec::new();
        for entry in fs::read_dir(models_dir)
            .with_context(|| format!("failed to read model directory '{}'", models_dir.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) == Some("param") {
                params.push(path);
            }
        }
        params.sort();

        for param_path in params {
            let Some(model_name) = param_path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let model_name = model_name.to_string();
            let bin_path = param_path.with_extension("bin");
            if !bin_path.is_file() {
                let variant_id = canonical_variant_id(&model_name);
                catalog.diagnostics.push(
                    UpscaylDiagnostic::new(
                        "model_files.incomplete",
                        format!(
                            "model '{}' has '{}' but is missing sibling '{}'",
                            model_name,
                            param_path.display(),
                            bin_path.display()
                        ),
                    )
                    .for_variant(variant_id)
                    .at_path(&param_path),
                );
                continue;
            }

            let param_sha256 = sha256_file(&param_path)?;
            let bin_sha256 = sha256_file(&bin_path)?;
            let model_digest = combined_model_digest(&param_path, &bin_path)?;

            if let Some(existing) = catalog
                .models
                .iter_mut()
                .find(|model| model.model_name == model_name)
            {
                existing.param_path = Some(param_path);
                existing.bin_path = Some(bin_path);
                existing.param_sha256 = Some(param_sha256);
                existing.bin_sha256 = Some(bin_sha256);
                existing.model_digest = Some(model_digest);
                continue;
            }

            let variant_id = canonical_variant_id(&model_name);
            let native_scale = infer_native_scale(&model_name);
            if native_scale.is_none() {
                catalog.diagnostics.push(
                    UpscaylDiagnostic::new(
                        "model_scale.unknown",
                        format!(
                            "custom model '{}' does not advertise a native x2/x3/x4 scale in its name",
                            model_name
                        ),
                    )
                    .for_variant(variant_id.clone())
                    .at_path(&param_path),
                );
            }
            catalog.models.push(UpscaylModelDescriptor {
                variant_id,
                model_name: model_name.clone(),
                display_name: model_name,
                native_scale,
                origin: UpscaylModelOrigin::Custom,
                commercial_use: CommercialUsePolicy::Unknown,
                description: "Custom Upscayl/NCNN model discovered at runtime.".to_string(),
                license_notes: "Custom model licensing is supplied by the user; Renderflow does not infer commercial-use permission.".to_string(),
                param_path: Some(param_path),
                bin_path: Some(bin_path),
                param_sha256: Some(param_sha256),
                bin_sha256: Some(bin_sha256),
                model_digest: Some(model_digest),
            });
        }

        catalog
            .models
            .sort_by(|left, right| left.variant_id.cmp(&right.variant_id));
        Ok(catalog)
    }

    pub fn get(&self, id_or_name: &str) -> Option<&UpscaylModelDescriptor> {
        self.models
            .iter()
            .find(|model| model.variant_id == id_or_name || model.model_name == id_or_name)
    }

    pub fn default_model(&self) -> Option<&UpscaylModelDescriptor> {
        self.get(DEFAULT_UPSCAYL_MODEL)
    }

    pub fn materialized_models(&self) -> impl Iterator<Item = &UpscaylModelDescriptor> {
        self.models.iter().filter(|model| model.is_materialized())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpscaylVariantSelectionReport {
    pub provider_id: String,
    pub capability_id: String,
    pub variants: Vec<UpscaylModelDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<UpscaylDiagnostic>,
}

impl UpscaylVariantSelectionReport {
    pub fn is_allowed(&self) -> bool {
        !self.variants.is_empty()
            && !self
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.starts_with("policy."))
    }
}

/// Resolve the Upscayl variant intent expressed by a v2 specification.
///
/// This is intentionally pure planning input. It does not mutate the graph or
/// execute a transform; #354's canonical planner can project the returned
/// variants into the unified execution plan.
pub fn select_upscayl_variants(
    spec: &SpecV2,
    catalog: &UpscaylModelCatalog,
) -> UpscaylVariantSelectionReport {
    let mut requested = BTreeSet::new();
    let mut diagnostics = Vec::new();
    let mut relevant_request = false;

    for target in &spec.targets.exact {
        relevant_request |=
            collect_target_request(target, catalog, &mut requested, &mut diagnostics);
    }

    for profile_name in &spec.targets.profiles {
        if let Some(profile) = spec.profiles.get(profile_name) {
            for target in &profile.targets {
                relevant_request |=
                    collect_target_request(target, catalog, &mut requested, &mut diagnostics);
            }
            relevant_request |= collect_selector_request(
                &profile.include,
                catalog,
                &mut requested,
                &mut diagnostics,
                true,
            );
            apply_variant_exclusions(&profile.exclude, &mut requested);
        }
    }

    if spec.targets.all_reachable
        && !spec
            .targets
            .exclude
            .capabilities
            .iter()
            .any(|capability| capability == SUPER_RESOLUTION_CAPABILITY_ID)
    {
        let include = &spec.targets.include;
        let included = include.capabilities.is_empty()
            || include
                .capabilities
                .iter()
                .any(|capability| capability == SUPER_RESOLUTION_CAPABILITY_ID)
            || !include.variants.is_empty();
        if included {
            relevant_request = true;
            if include.variants.is_empty() {
                for model in &catalog.models {
                    requested.insert(model.variant_id.clone());
                }
            } else {
                for variant in &include.variants {
                    add_variant_request(variant, catalog, &mut requested, &mut diagnostics);
                }
            }
        }
    }
    apply_variant_exclusions(&spec.targets.exclude, &mut requested);

    if relevant_request {
        apply_execution_policy(spec, &mut requested, &mut diagnostics);
    }

    let variants = requested
        .into_iter()
        .filter_map(|variant| catalog.get(&variant).cloned())
        .collect();

    UpscaylVariantSelectionReport {
        provider_id: UPSCAYL_TOOL_ID.to_string(),
        capability_id: SUPER_RESOLUTION_CAPABILITY_ID.to_string(),
        variants,
        diagnostics,
    }
}

fn collect_target_request(
    target: &TargetSpec,
    catalog: &UpscaylModelCatalog,
    requested: &mut BTreeSet<String>,
    diagnostics: &mut Vec<UpscaylDiagnostic>,
) -> bool {
    if target.capability.as_deref() != Some(SUPER_RESOLUTION_CAPABILITY_ID) {
        return false;
    }
    if let Some(variant) = &target.variant {
        add_variant_request(variant, catalog, requested, diagnostics);
    } else if let Some(default_model) = catalog.default_model() {
        requested.insert(default_model.variant_id.clone());
    }
    true
}

fn collect_selector_request(
    selector: &SelectorSet,
    catalog: &UpscaylModelCatalog,
    requested: &mut BTreeSet<String>,
    diagnostics: &mut Vec<UpscaylDiagnostic>,
    allow_capability_expansion: bool,
) -> bool {
    let capability_selected = selector
        .capabilities
        .iter()
        .any(|capability| capability == SUPER_RESOLUTION_CAPABILITY_ID);
    if !capability_selected && selector.variants.is_empty() {
        return false;
    }
    if selector.variants.is_empty() && allow_capability_expansion {
        for model in &catalog.models {
            requested.insert(model.variant_id.clone());
        }
    } else {
        for variant in &selector.variants {
            add_variant_request(variant, catalog, requested, diagnostics);
        }
    }
    true
}

fn add_variant_request(
    variant: &str,
    catalog: &UpscaylModelCatalog,
    requested: &mut BTreeSet<String>,
    diagnostics: &mut Vec<UpscaylDiagnostic>,
) {
    if let Some(model) = catalog.get(variant) {
        requested.insert(model.variant_id.clone());
    } else {
        diagnostics.push(
            UpscaylDiagnostic::new(
                "variant.unknown",
                format!("Upscayl variant/model '{variant}' is not registered"),
            )
            .for_variant(variant),
        );
    }
}

fn apply_variant_exclusions(selector: &SelectorSet, requested: &mut BTreeSet<String>) {
    for variant in &selector.variants {
        requested.remove(variant);
        if !variant.starts_with("variant.") {
            requested.remove(&canonical_variant_id(variant));
        }
    }
}

fn apply_execution_policy(
    spec: &SpecV2,
    requested: &mut BTreeSet<String>,
    diagnostics: &mut Vec<UpscaylDiagnostic>,
) {
    if spec.execution.ai == AiPolicy::Deny {
        requested.clear();
        diagnostics.push(UpscaylDiagnostic::new(
            "policy.ai.denied",
            "Upscayl is an AI super-resolution transform; set execution.ai to local_only or allow to opt in",
        ));
        return;
    }
    if spec.execution.requirements.deterministic {
        requested.clear();
        diagnostics.push(UpscaylDiagnostic::new(
            "policy.determinism.unsatisfied",
            "Upscayl output may vary across Vulkan/GPU implementations and cannot satisfy deterministic: true",
        ));
        return;
    }
    if spec
        .execution
        .tools
        .deny
        .iter()
        .any(|tool| tool == UPSCAYL_TOOL_ID)
    {
        requested.clear();
        diagnostics.push(UpscaylDiagnostic::new(
            "policy.tool.denied",
            format!("provider '{UPSCAYL_TOOL_ID}' is denied by execution.tools"),
        ));
        return;
    }
    if !spec.execution.tools.allow.is_empty()
        && !spec
            .execution
            .tools
            .allow
            .iter()
            .any(|tool| tool == UPSCAYL_TOOL_ID)
    {
        requested.clear();
        diagnostics.push(UpscaylDiagnostic::new(
            "policy.tool.not_allowed",
            format!("provider '{UPSCAYL_TOOL_ID}' is not present in execution.tools.allow"),
        ));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpscaylReadinessStatus {
    Ready,
    MissingExecutable,
    MissingModelFiles,
    MissingVulkanRuntime,
    RuntimeUnverified,
    ProviderUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpscaylReadiness {
    pub status: UpscaylReadinessStatus,
    pub provider_executable: Option<String>,
    pub materialized_models: usize,
    pub diagnostics: Vec<UpscaylDiagnostic>,
}

pub fn assess_upscayl_readiness_with(
    catalog: &UpscaylModelCatalog,
    probe: &dyn ToolProbe,
    context: &ToolRuntimeContext,
) -> UpscaylReadiness {
    let registry = ToolRegistry::builtins();
    let inventory = registry.assess_ids_with([UPSCAYL_TOOL_ID], probe, context);
    let availability = inventory
        .get(UPSCAYL_TOOL_ID)
        .expect("built-in Upscayl provider must be represented in inventory");
    let materialized_models = catalog.materialized_models().count();
    let mut diagnostics = catalog.diagnostics.clone();

    if !availability.is_available() {
        let status = if availability.status == ToolAvailabilityStatus::MissingExecutable {
            UpscaylReadinessStatus::MissingExecutable
        } else {
            UpscaylReadinessStatus::ProviderUnavailable
        };
        diagnostics.push(UpscaylDiagnostic::new(
            "provider.unavailable",
            availability.summary(),
        ));
        return UpscaylReadiness {
            status,
            provider_executable: availability.selected_executable.clone(),
            materialized_models,
            diagnostics,
        };
    }

    if materialized_models == 0 {
        diagnostics.push(UpscaylDiagnostic::new(
            "model_files.missing",
            "no complete Upscayl .param/.bin model pairs were discovered",
        ));
        return UpscaylReadiness {
            status: UpscaylReadinessStatus::MissingModelFiles,
            provider_executable: availability.selected_executable.clone(),
            materialized_models,
            diagnostics,
        };
    }

    if context.has_runtime_service("vulkan") {
        return UpscaylReadiness {
            status: UpscaylReadinessStatus::Ready,
            provider_executable: availability.selected_executable.clone(),
            materialized_models,
            diagnostics,
        };
    }

    let vulkan_probe = probe.probe("vulkaninfo", &["--summary".to_string()]);
    let status = match vulkan_probe.status {
        ToolProbeStatus::Available => UpscaylReadinessStatus::Ready,
        ToolProbeStatus::Missing => {
            diagnostics.push(UpscaylDiagnostic::new(
                "vulkan.unverified",
                "vulkaninfo is not available, so Vulkan/GPU readiness cannot be verified before execution",
            ));
            UpscaylReadinessStatus::RuntimeUnverified
        }
        ToolProbeStatus::Failed | ToolProbeStatus::TimedOut => {
            diagnostics.push(UpscaylDiagnostic::new(
                "vulkan.unavailable",
                vulkan_probe
                    .diagnostic
                    .unwrap_or_else(|| "Vulkan probe failed".to_string()),
            ));
            UpscaylReadinessStatus::MissingVulkanRuntime
        }
    };

    UpscaylReadiness {
        status,
        provider_executable: availability.selected_executable.clone(),
        materialized_models,
        diagnostics,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpscaylOutputFormat {
    #[default]
    Png,
    Jpg,
    Webp,
}

impl UpscaylOutputFormat {
    pub fn as_cli_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Webp => "webp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpscaylOptions {
    #[serde(default)]
    pub requested_scale: Option<u32>,
    #[serde(default)]
    pub gpu_id: Option<String>,
    #[serde(default)]
    pub tile_size: Option<u32>,
    #[serde(default)]
    pub tta: bool,
    #[serde(default)]
    pub output_format: UpscaylOutputFormat,
    #[serde(default)]
    pub compression: u8,
    #[serde(default = "default_upscayl_timeout_seconds")]
    pub timeout_seconds: u64,
}

fn default_upscayl_timeout_seconds() -> u64 {
    30 * 60
}

impl Default for UpscaylOptions {
    fn default() -> Self {
        Self {
            requested_scale: None,
            gpu_id: None,
            tile_size: None,
            tta: false,
            output_format: UpscaylOutputFormat::Png,
            compression: 0,
            timeout_seconds: default_upscayl_timeout_seconds(),
        }
    }
}

impl UpscaylOptions {
    pub fn validate(&self) -> Result<()> {
        if matches!(self.requested_scale, Some(0 | 5..)) {
            anyhow::bail!("requested Upscayl scale must be between 1 and 4");
        }
        if self.compression > 100 {
            anyhow::bail!("Upscayl compression must be between 0 and 100");
        }
        if self.timeout_seconds == 0 {
            anyhow::bail!("Upscayl timeout_seconds must be greater than zero");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpscaylExecutionRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub models_dir: PathBuf,
    pub model: UpscaylModelDescriptor,
    pub options: UpscaylOptions,
}

impl UpscaylExecutionRequest {
    pub fn validate(&self) -> Result<()> {
        self.options.validate()?;
        if !self.input.is_file() {
            anyhow::bail!("Upscayl input '{}' is not a file", self.input.display());
        }
        if !self.models_dir.is_dir() {
            anyhow::bail!(
                "Upscayl models directory '{}' is not a directory",
                self.models_dir.display()
            );
        }
        if !self.model.is_materialized() {
            anyhow::bail!(
                "Upscayl model '{}' is not materialized with a complete .param/.bin pair",
                self.model.variant_id
            );
        }
        Ok(())
    }

    pub fn command_arguments(&self) -> Result<Vec<String>> {
        self.options.validate()?;
        let mut arguments = vec![
            "-i".to_string(),
            self.input.to_string_lossy().into_owned(),
            "-o".to_string(),
            self.output.to_string_lossy().into_owned(),
            "-m".to_string(),
            self.models_dir.to_string_lossy().into_owned(),
            "-n".to_string(),
            self.model.model_name.clone(),
        ];
        if let Some(requested_scale) = self.options.requested_scale {
            if self.model.native_scale != Some(requested_scale) {
                arguments.push("-s".to_string());
                arguments.push(requested_scale.to_string());
            }
        }
        if let Some(gpu_id) = &self.options.gpu_id {
            arguments.push("-g".to_string());
            arguments.push(gpu_id.clone());
        }
        arguments.push("-f".to_string());
        arguments.push(self.options.output_format.as_cli_str().to_string());
        arguments.push("-c".to_string());
        arguments.push(self.options.compression.to_string());
        if let Some(tile_size) = self.options.tile_size {
            arguments.push("-t".to_string());
            arguments.push(tile_size.to_string());
        }
        if self.options.tta {
            arguments.push("-x".to_string());
        }
        Ok(arguments)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpscaylExecutionEvidence {
    pub provider_id: String,
    pub capability_id: String,
    pub variant_id: String,
    pub model_name: String,
    pub model_digest: String,
    pub executable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable_sha256: Option<String>,
    pub native_scale: Option<u32>,
    pub requested_scale: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_scale: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_backend: Option<String>,
    pub output_format: UpscaylOutputFormat,
    pub command_configuration: BTreeMap<String, String>,
}

impl UpscaylExecutionEvidence {
    pub fn toolchain_variant_evidence(&self) -> SelectedToolVariantEvidence {
        let mut attributes = self.command_configuration.clone();
        attributes.insert("model_name".to_string(), self.model_name.clone());
        attributes.insert("model_digest".to_string(), self.model_digest.clone());
        if let Some(scale) = self.native_scale {
            attributes.insert("native_scale".to_string(), scale.to_string());
        }
        if let Some(scale) = self.requested_scale {
            attributes.insert("requested_scale".to_string(), scale.to_string());
        }
        if let Some(scale) = self.post_scale {
            attributes.insert("post_scale".to_string(), scale.to_string());
        }
        if let Some(gpu) = &self.gpu_id {
            attributes.insert("gpu_id".to_string(), gpu.clone());
        }
        if let Some(backend) = &self.runtime_backend {
            attributes.insert("runtime_backend".to_string(), backend.clone());
        }
        if let Some(binary_digest) = &self.executable_sha256 {
            attributes.insert("executable_sha256".to_string(), binary_digest.clone());
        }
        SelectedToolVariantEvidence {
            id: self.variant_id.clone(),
            attributes,
        }
    }
}

#[derive(Clone)]
pub struct UpscaylAdapter {
    executable: String,
    executor: ProcessExecutor,
}

impl UpscaylAdapter {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
            executor: ProcessExecutor::new(),
        }
    }

    pub fn from_inventory(inventory: &ToolInventory) -> Result<Self> {
        let availability = inventory
            .get(UPSCAYL_TOOL_ID)
            .ok_or_else(|| anyhow::anyhow!("Upscayl provider was not assessed"))?;
        if !availability.is_available() {
            anyhow::bail!(
                "Upscayl provider is unavailable: {}",
                availability.summary()
            );
        }
        let executable = availability.selected_executable.clone().ok_or_else(|| {
            anyhow::anyhow!("Upscayl provider did not resolve an executable candidate")
        })?;
        Ok(Self::new(executable))
    }

    pub fn execute(&self, request: &UpscaylExecutionRequest) -> Result<UpscaylExecutionEvidence> {
        request.validate()?;
        let arguments = request.command_arguments()?;
        let process_request = ProcessRequest::direct(&self.executable)
            .args(arguments)
            .network_policy(ProcessNetworkPolicy::Deny)
            .timeout(Duration::from_secs(request.options.timeout_seconds))
            .expect_output(ProcessExpectedOutput::file(&request.output).require_non_empty());
        let result = self
            .executor
            .execute_checked(process_request)
            .context("Upscayl execution failed")?;

        let runtime_backend = parse_runtime_backend(result.stdout().redacted_text())
            .or_else(|| parse_runtime_backend(result.stderr().redacted_text()));
        let executable_sha256 =
            resolve_executable_path(&self.executable).and_then(|path| sha256_file(&path).ok());
        let model_digest = request
            .model
            .model_digest
            .clone()
            .ok_or_else(|| anyhow::anyhow!("materialized model is missing model digest"))?;

        let mut command_configuration = BTreeMap::new();
        command_configuration.insert(
            "output_format".to_string(),
            request.options.output_format.as_cli_str().to_string(),
        );
        command_configuration.insert(
            "compression".to_string(),
            request.options.compression.to_string(),
        );
        command_configuration.insert("tta".to_string(), request.options.tta.to_string());
        if let Some(tile_size) = request.options.tile_size {
            command_configuration.insert("tile_size".to_string(), tile_size.to_string());
        }

        let requested_scale = request.options.requested_scale;
        let post_scale = requested_scale.filter(|scale| Some(*scale) != request.model.native_scale);
        Ok(UpscaylExecutionEvidence {
            provider_id: UPSCAYL_TOOL_ID.to_string(),
            capability_id: SUPER_RESOLUTION_CAPABILITY_ID.to_string(),
            variant_id: request.model.variant_id.clone(),
            model_name: request.model.model_name.clone(),
            model_digest,
            executable: self.executable.clone(),
            executable_sha256,
            native_scale: request.model.native_scale,
            requested_scale,
            post_scale,
            gpu_id: request.options.gpu_id.clone(),
            runtime_backend,
            output_format: request.options.output_format,
            command_configuration,
        })
    }
}

fn validate_variant_id(value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        anyhow::bail!(
            "variant id '{}' may contain only ASCII letters, digits, '.', '_' and '-'",
            value
        );
    }
    Ok(())
}

fn canonical_variant_id(model_name: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in model_name.chars() {
        let character = character.to_ascii_lowercase();
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            slug.push(character);
            separator = false;
        } else if !separator {
            slug.push('-');
            separator = true;
        }
    }
    format!("variant.upscayl-ncnn.{}", slug.trim_matches('-'))
}

fn infer_native_scale(model_name: &str) -> Option<u32> {
    let lower = model_name.to_ascii_lowercase();
    for scale in [2_u32, 3, 4] {
        if lower.contains(&format!("x{scale}")) || lower.contains(&format!("{scale}x")) {
            return Some(scale);
        }
    }
    None
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read '{}'", path.display()))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{digest:x}"))
}

fn combined_model_digest(param_path: &Path, bin_path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"upscayl-model-v1\0param\0");
    hasher.update(
        fs::read(param_path)
            .with_context(|| format!("failed to read '{}'", param_path.display()))?,
    );
    hasher.update(b"\0bin\0");
    hasher.update(
        fs::read(bin_path).with_context(|| format!("failed to read '{}'", bin_path.display()))?,
    );
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn resolve_executable_path(executable: &str) -> Option<PathBuf> {
    let candidate = Path::new(executable);
    if candidate.components().count() > 1 && candidate.is_file() {
        return Some(candidate.to_path_buf());
    }
    let path = env::var_os("PATH")?;
    for directory in env::split_paths(&path) {
        let candidate = directory.join(executable);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = directory.join(format!("{executable}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn parse_runtime_backend(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.contains(']') {
            Some(trimmed.to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{ProcessPlatform, ProcessTreeTermination, ToolProbeEvidence};
    use crate::spec::{ExecutionPolicy, SourceKind, SourceSpec, TargetSelection, SPEC_V2_ID};

    #[derive(Default)]
    struct FakeProbe {
        responses: BTreeMap<String, ToolProbeEvidence>,
    }

    impl FakeProbe {
        fn with(mut self, executable: &str, status: ToolProbeStatus) -> Self {
            self.responses.insert(
                executable.to_string(),
                ToolProbeEvidence {
                    executable: executable.to_string(),
                    status,
                    version_line: Some("upscayl-ncnn help".to_string()),
                    duration_ms: 1,
                    platform: ProcessPlatform {
                        os: "linux",
                        arch: "x86_64",
                        tree_termination: ProcessTreeTermination::UnixProcessGroup,
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
                    status: ToolProbeStatus::Missing,
                    version_line: None,
                    duration_ms: 1,
                    platform: ProcessPlatform {
                        os: "linux",
                        arch: "x86_64",
                        tree_termination: ProcessTreeTermination::UnixProcessGroup,
                    },
                    diagnostic: Some("missing".to_string()),
                })
        }
    }

    fn minimal_spec() -> SpecV2 {
        SpecV2 {
            schema: SPEC_V2_ID.to_string(),
            sources: vec![SourceSpec {
                id: "source.image".to_string(),
                role: None,
                kind: SourceKind::Artifact,
                path: Some("image.png".to_string()),
                uri: None,
                members: Vec::new(),
                media_type: Some("image/png".to_string()),
                format: Some("png".to_string()),
                detect: false,
                immutable: true,
            }],
            profiles: BTreeMap::new(),
            targets: TargetSelection::default(),
            execution: ExecutionPolicy {
                ai: AiPolicy::LocalOnly,
                ..ExecutionPolicy::default()
            },
            output: Default::default(),
            variables: BTreeMap::new(),
            transforms: None,
        }
    }

    #[test]
    fn built_in_catalog_has_stable_seven_model_set() {
        let catalog = UpscaylModelCatalog::builtins();
        assert_eq!(catalog.models.len(), 7);
        let ids: Vec<&str> = catalog
            .models
            .iter()
            .map(|model| model.variant_id.as_str())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
        assert!(catalog.get(DEFAULT_UPSCAYL_MODEL).is_some());
    }

    #[test]
    fn non_commercial_models_are_explicit_metadata() {
        let catalog = UpscaylModelCatalog::builtins();
        for name in ["remacri-4x", "ultramix-balanced-4x", "ultrasharp-4x"] {
            assert_eq!(
                catalog.get(name).unwrap().commercial_use,
                CommercialUsePolicy::Prohibited
            );
        }
    }

    #[test]
    fn custom_model_discovery_is_deterministic_and_checksum_sensitive() {
        let directory = tempfile::tempdir().unwrap();
        let param = directory.path().join("comic-x2.param");
        let bin = directory.path().join("comic-x2.bin");
        fs::write(&param, b"param-v1").unwrap();
        fs::write(&bin, b"weights-v1").unwrap();
        let first = UpscaylModelCatalog::discover(directory.path()).unwrap();
        let first_model = first.get("comic-x2").unwrap();
        assert_eq!(first_model.origin, UpscaylModelOrigin::Custom);
        assert_eq!(first_model.native_scale, Some(2));
        let first_digest = first_model.model_digest.clone().unwrap();

        fs::write(&bin, b"weights-v2").unwrap();
        let second = UpscaylModelCatalog::discover(directory.path()).unwrap();
        let second_digest = second
            .get("comic-x2")
            .unwrap()
            .model_digest
            .clone()
            .unwrap();
        assert_ne!(first_digest, second_digest);
    }

    #[test]
    fn all_reachable_expands_every_model_when_ai_is_opted_in() {
        let mut spec = minimal_spec();
        spec.targets.all_reachable = true;
        let report = select_upscayl_variants(&spec, &UpscaylModelCatalog::builtins());
        assert_eq!(report.variants.len(), 7);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn exact_variant_selection_resolves_one_model() {
        let mut spec = minimal_spec();
        spec.targets.exact.push(TargetSpec {
            id: Some("target.upscale".to_string()),
            role: None,
            format: None,
            family: None,
            capability: Some(SUPER_RESOLUTION_CAPABILITY_ID.to_string()),
            transform: None,
            variant: Some("variant.upscayl-ncnn.digital-art-4x".to_string()),
            preset: None,
            template: None,
        });
        let report = select_upscayl_variants(&spec, &UpscaylModelCatalog::builtins());
        assert_eq!(report.variants.len(), 1);
        assert_eq!(report.variants[0].model_name, "digital-art-4x");
    }

    #[test]
    fn ai_deny_blocks_super_resolution_expansion() {
        let mut spec = minimal_spec();
        spec.execution.ai = AiPolicy::Deny;
        spec.targets.all_reachable = true;
        let report = select_upscayl_variants(&spec, &UpscaylModelCatalog::builtins());
        assert!(report.variants.is_empty());
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "policy.ai.denied"));
    }

    #[test]
    fn command_arguments_distinguish_native_and_post_scale() {
        let mut model = UpscaylModelCatalog::builtins()
            .get(DEFAULT_UPSCAYL_MODEL)
            .unwrap()
            .clone();
        model.param_path = Some(PathBuf::from("models/upscayl-standard-4x.param"));
        model.bin_path = Some(PathBuf::from("models/upscayl-standard-4x.bin"));
        model.param_sha256 = Some("sha256:param".to_string());
        model.bin_sha256 = Some("sha256:bin".to_string());
        model.model_digest = Some("sha256:model".to_string());
        let request = UpscaylExecutionRequest {
            input: PathBuf::from("input.png"),
            output: PathBuf::from("output.png"),
            models_dir: PathBuf::from("models"),
            model,
            options: UpscaylOptions {
                requested_scale: Some(2),
                ..UpscaylOptions::default()
            },
        };
        let args = request.command_arguments().unwrap();
        let scale_index = args.iter().position(|arg| arg == "-s").unwrap();
        assert_eq!(args[scale_index + 1], "2");
    }

    #[test]
    fn all_reachable_can_select_variant_subset() {
        let mut spec = minimal_spec();
        spec.targets.all_reachable = true;
        spec.targets
            .include
            .capabilities
            .push(SUPER_RESOLUTION_CAPABILITY_ID.to_string());
        spec.targets.include.variants = vec![
            "variant.upscayl-ncnn.digital-art-4x".to_string(),
            "variant.upscayl-ncnn.high-fidelity-4x".to_string(),
        ];

        let report = select_upscayl_variants(&spec, &UpscaylModelCatalog::builtins());
        let names: Vec<&str> = report
            .variants
            .iter()
            .map(|model| model.model_name.as_str())
            .collect();
        assert_eq!(names, vec!["digital-art-4x", "high-fidelity-4x"]);
    }

    #[test]
    fn all_reachable_can_exclude_named_variant() {
        let mut spec = minimal_spec();
        spec.targets.all_reachable = true;
        spec.targets.exclude.variants = vec!["variant.upscayl-ncnn.ultrasharp-4x".to_string()];

        let report = select_upscayl_variants(&spec, &UpscaylModelCatalog::builtins());
        assert_eq!(report.variants.len(), 6);
        assert!(!report
            .variants
            .iter()
            .any(|model| model.model_name == "ultrasharp-4x"));
    }

    #[test]
    fn readiness_reports_missing_executable_without_gpu() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("comic-x2.param"), b"param").unwrap();
        fs::write(directory.path().join("comic-x2.bin"), b"bin").unwrap();
        let catalog = UpscaylModelCatalog::discover(directory.path()).unwrap();
        let context = ToolRuntimeContext::for_platform("linux", "x86_64");

        let readiness = assess_upscayl_readiness_with(&catalog, &FakeProbe::default(), &context);
        assert_eq!(readiness.status, UpscaylReadinessStatus::MissingExecutable);
    }

    #[test]
    fn readiness_reports_missing_model_files() {
        let catalog = UpscaylModelCatalog::builtins();
        let probe = FakeProbe::default().with("upscayl-ncnn", ToolProbeStatus::Available);
        let context = ToolRuntimeContext::for_platform("linux", "x86_64");

        let readiness = assess_upscayl_readiness_with(&catalog, &probe, &context);
        assert_eq!(readiness.status, UpscaylReadinessStatus::MissingModelFiles);
    }

    #[test]
    fn compression_out_of_range_is_rejected() {
        let options = UpscaylOptions {
            compression: 101,
            ..UpscaylOptions::default()
        };
        assert!(options.validate().is_err());
    }

    #[test]
    #[ignore = "requires an Upscayl executable, Vulkan GPU, model files, and explicit input path"]
    fn upscayl_real_provider_smoke() {
        let input = env::var("RENDERFLOW_UPSCAYL_SMOKE_INPUT")
            .expect("set RENDERFLOW_UPSCAYL_SMOKE_INPUT for the ignored smoke test");
        let models_dir = env::var("RENDERFLOW_UPSCAYL_MODELS_DIR")
            .expect("set RENDERFLOW_UPSCAYL_MODELS_DIR for the ignored smoke test");
        let executable =
            env::var("RENDERFLOW_UPSCAYL_EXECUTABLE").unwrap_or_else(|_| "upscayl-bin".to_string());
        let catalog = UpscaylModelCatalog::discover(&models_dir).unwrap();
        let model = catalog
            .get(DEFAULT_UPSCAYL_MODEL)
            .expect("default Upscayl model must be present")
            .clone();
        assert!(model.is_materialized(), "default model files are missing");
        let output_dir = tempfile::tempdir().unwrap();
        let output = output_dir.path().join("upscayl-smoke.png");
        let evidence = UpscaylAdapter::new(executable)
            .execute(&UpscaylExecutionRequest {
                input: PathBuf::from(input),
                output: output.clone(),
                models_dir: PathBuf::from(models_dir),
                model,
                options: UpscaylOptions::default(),
            })
            .unwrap();
        assert!(output.is_file());
        assert_eq!(evidence.provider_id, UPSCAYL_TOOL_ID);
    }

    #[test]
    fn readiness_can_be_tested_without_gpu_hardware() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("comic-x2.param"), b"param").unwrap();
        fs::write(directory.path().join("comic-x2.bin"), b"bin").unwrap();
        let catalog = UpscaylModelCatalog::discover(directory.path()).unwrap();
        let probe = FakeProbe::default()
            .with("upscayl-ncnn", ToolProbeStatus::Available)
            .with("vulkaninfo", ToolProbeStatus::Available);
        let context = ToolRuntimeContext::for_platform("linux", "x86_64");
        let readiness = assess_upscayl_readiness_with(&catalog, &probe, &context);
        assert_eq!(readiness.status, UpscaylReadinessStatus::Ready);
    }
}
