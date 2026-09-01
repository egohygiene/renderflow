use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::Config;
use crate::optimization::OptimizationMode;

pub const SPEC_V2_ID: &str = "renderflow/v2";
pub const SPEC_V2_SCHEMA_PATH: &str = "schemas/renderflow-v2.schema.json";

fn default_true() -> bool {
    true
}

fn default_bundle_root() -> String {
    "dist".to_string()
}

fn default_naming_template() -> String {
    "{source.id}/{target.role}.{ext}".to_string()
}

fn default_max_parallel() -> usize {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    #[default]
    Artifact,
    Collection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpec {
    pub id: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub kind: SourceKind,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default = "default_true")]
    pub detect: bool,
    #[serde(default = "default_true")]
    pub immutable: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectorSet {
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub families: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub transforms: Vec<String>,
    #[serde(default)]
    pub profiles: Vec<String>,
}

impl SelectorSet {
    pub fn is_empty(&self) -> bool {
        self.formats.is_empty()
            && self.families.is_empty()
            && self.capabilities.is_empty()
            && self.transforms.is_empty()
            && self.profiles.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
}

impl TargetSpec {
    fn has_selector(&self) -> bool {
        self.format.is_some()
            || self.family.is_some()
            || self.capability.is_some()
            || self.transform.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntermediatePolicy {
    #[default]
    CacheOnly,
    Retain,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSelection {
    #[serde(default)]
    pub exact: Vec<TargetSpec>,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub all_reachable: bool,
    #[serde(default)]
    pub include: SelectorSet,
    #[serde(default)]
    pub exclude: SelectorSet,
    #[serde(default)]
    pub intermediates: IntermediatePolicy,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivativeProfile {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub targets: Vec<TargetSpec>,
    #[serde(default)]
    pub include: SelectorSet,
    #[serde(default)]
    pub exclude: SelectorSet,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowDenyPolicy {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudgets {
    #[serde(default)]
    pub max_output_bytes: Option<u64>,
    #[serde(default)]
    pub max_storage_bytes: Option<u64>,
    #[serde(default)]
    pub max_artifacts: Option<u64>,
    #[serde(default)]
    pub max_depth: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRequirements {
    #[serde(default)]
    pub deterministic: bool,
    #[serde(default)]
    pub local_only: bool,
    #[serde(default)]
    pub offline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    #[default]
    Deny,
    Allow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPolicy {
    #[default]
    Deny,
    LocalOnly,
    Allow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationPolicy {
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub validators: Vec<String>,
}

impl Default for ValidationPolicy {
    fn default() -> Self {
        Self {
            required: true,
            validators: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPolicy {
    #[serde(default)]
    pub optimization: OptimizationMode,
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    #[serde(default)]
    pub budgets: ResourceBudgets,
    #[serde(default)]
    pub tools: AllowDenyPolicy,
    #[serde(default)]
    pub transforms: AllowDenyPolicy,
    #[serde(default)]
    pub requirements: ExecutionRequirements,
    #[serde(default)]
    pub network: NetworkPolicy,
    #[serde(default)]
    pub ai: AiPolicy,
    #[serde(default)]
    pub retry_policy: Option<String>,
    #[serde(default)]
    pub timeout_policy: Option<String>,
    #[serde(default)]
    pub validation: ValidationPolicy,
    #[serde(default)]
    pub minimum_fidelity: Option<f32>,
    #[serde(default)]
    pub publication_policy: Option<String>,
    #[serde(default)]
    pub redaction_policy: Option<String>,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            optimization: OptimizationMode::default(),
            max_parallel: default_max_parallel(),
            budgets: ResourceBudgets::default(),
            tools: AllowDenyPolicy::default(),
            transforms: AllowDenyPolicy::default(),
            requirements: ExecutionRequirements::default(),
            network: NetworkPolicy::Deny,
            ai: AiPolicy::Deny,
            retry_policy: None,
            timeout_policy: None,
            validation: ValidationPolicy::default(),
            minimum_fidelity: None,
            publication_policy: None,
            redaction_policy: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    #[default]
    Error,
    Replace,
    Dedupe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputLayout {
    #[serde(default = "default_bundle_root")]
    pub bundle_root: String,
    #[serde(default = "default_naming_template")]
    pub naming_template: String,
    #[serde(default)]
    pub collision: CollisionPolicy,
}

impl Default for OutputLayout {
    fn default() -> Self {
        Self {
            bundle_root: default_bundle_root(),
            naming_template: default_naming_template(),
            collision: CollisionPolicy::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecV2 {
    pub schema: String,
    pub sources: Vec<SourceSpec>,
    #[serde(default)]
    pub profiles: BTreeMap<String, DerivativeProfile>,
    pub targets: TargetSelection,
    #[serde(default)]
    pub execution: ExecutionPolicy,
    #[serde(default)]
    pub output: OutputLayout,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub transforms: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceSpecVersion {
    V1,
    V2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadedSpec {
    pub source_version: SourceSpecVersion,
    pub spec: SpecV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecDiagnostic {
    pub path: String,
    pub code: String,
    pub message: String,
}

impl SpecDiagnostic {
    fn new(path: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecValidationReport {
    pub valid: bool,
    pub source_version: Option<SourceSpecVersion>,
    pub schema: Option<String>,
    pub diagnostics: Vec<SpecDiagnostic>,
}

impl fmt::Display for SpecValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.valid {
            write!(f, "spec is valid")
        } else {
            for diagnostic in &self.diagnostics {
                writeln!(
                    f,
                    "{} [{}]: {}",
                    diagnostic.path, diagnostic.code, diagnostic.message
                )?;
            }
            Ok(())
        }
    }
}

impl SpecV2 {
    pub fn validate(&self) -> Vec<SpecDiagnostic> {
        let mut diagnostics = Vec::new();

        if self.schema != SPEC_V2_ID {
            diagnostics.push(SpecDiagnostic::new(
                "$.schema",
                "schema.unsupported",
                format!("expected schema '{SPEC_V2_ID}', got '{}'", self.schema),
            ));
        }

        if self.sources.is_empty() {
            diagnostics.push(SpecDiagnostic::new(
                "$.sources",
                "sources.empty",
                "at least one source artifact is required",
            ));
        }

        let mut source_ids = BTreeSet::new();
        for (index, source) in self.sources.iter().enumerate() {
            let base = format!("$.sources[{index}]");
            if !is_stable_id(&source.id) {
                diagnostics.push(SpecDiagnostic::new(
                    format!("{base}.id"),
                    "source.id.invalid",
                    "source id must use only ASCII letters, digits, '.', '_', or '-'",
                ));
            }
            if !source_ids.insert(source.id.clone()) {
                diagnostics.push(SpecDiagnostic::new(
                    format!("{base}.id"),
                    "source.id.duplicate",
                    format!("source id '{}' is declared more than once", source.id),
                ));
            }
            if !source.immutable {
                diagnostics.push(SpecDiagnostic::new(
                    format!("{base}.immutable"),
                    "source.mutable",
                    "v2 sources are immutable inputs; copy or derive a new artifact instead",
                ));
            }

            match source.kind {
                SourceKind::Artifact => {
                    let locator_count =
                        usize::from(source.path.is_some()) + usize::from(source.uri.is_some());
                    if locator_count != 1 {
                        diagnostics.push(SpecDiagnostic::new(
                            base.clone(),
                            "source.locator.invalid",
                            "artifact sources require exactly one of 'path' or 'uri'",
                        ));
                    }
                    if !source.members.is_empty() {
                        diagnostics.push(SpecDiagnostic::new(
                            format!("{base}.members"),
                            "source.members.unexpected",
                            "artifact sources cannot declare collection members",
                        ));
                    }
                }
                SourceKind::Collection => {
                    if source.path.is_some() || source.uri.is_some() {
                        diagnostics.push(SpecDiagnostic::new(
                            base.clone(),
                            "collection.locator.unexpected",
                            "collection sources reference member source ids instead of a path or uri",
                        ));
                    }
                    if source.members.is_empty() {
                        diagnostics.push(SpecDiagnostic::new(
                            format!("{base}.members"),
                            "collection.members.empty",
                            "ordered collections require at least one member source id",
                        ));
                    }
                }
            }
        }

        for (index, source) in self.sources.iter().enumerate() {
            if source.kind == SourceKind::Collection {
                for (member_index, member) in source.members.iter().enumerate() {
                    if !source_ids.contains(member) {
                        diagnostics.push(SpecDiagnostic::new(
                            format!("$.sources[{index}].members[{member_index}]"),
                            "collection.member.unknown",
                            format!(
                                "collection member '{member}' does not match a declared source id"
                            ),
                        ));
                    }
                    if member == &source.id {
                        diagnostics.push(SpecDiagnostic::new(
                            format!("$.sources[{index}].members[{member_index}]"),
                            "collection.member.self_reference",
                            "a collection cannot contain itself",
                        ));
                    }
                }
            }
        }

        if self.targets.exact.is_empty()
            && self.targets.profiles.is_empty()
            && !self.targets.all_reachable
        {
            diagnostics.push(SpecDiagnostic::new(
                "$.targets",
                "targets.empty",
                "declare at least one exact target, named profile, or all_reachable: true",
            ));
        }

        validate_targets(&self.targets.exact, "$.targets.exact", &mut diagnostics);

        for (index, profile_name) in self.targets.profiles.iter().enumerate() {
            if !self.profiles.contains_key(profile_name) {
                diagnostics.push(SpecDiagnostic::new(
                    format!("$.targets.profiles[{index}]"),
                    "profile.unknown",
                    format!("target profile '{profile_name}' is not declared in $.profiles"),
                ));
            }
        }

        for (profile_name, profile) in &self.profiles {
            let base = format!("$.profiles.{profile_name}");
            if !is_stable_id(profile_name) {
                diagnostics.push(SpecDiagnostic::new(
                    base.clone(),
                    "profile.id.invalid",
                    "profile names must use only ASCII letters, digits, '.', '_', or '-'",
                ));
            }
            if profile.targets.is_empty() && profile.include.is_empty() {
                diagnostics.push(SpecDiagnostic::new(
                    base.clone(),
                    "profile.empty",
                    "a derivative profile must declare targets or include selectors",
                ));
            }
            validate_targets(
                &profile.targets,
                &format!("{base}.targets"),
                &mut diagnostics,
            );
        }

        if self.execution.max_parallel == 0 {
            diagnostics.push(SpecDiagnostic::new(
                "$.execution.max_parallel",
                "execution.concurrency.invalid",
                "max_parallel must be at least 1",
            ));
        }

        validate_optional_positive(
            self.execution.budgets.max_output_bytes,
            "$.execution.budgets.max_output_bytes",
            &mut diagnostics,
        );
        validate_optional_positive(
            self.execution.budgets.max_storage_bytes,
            "$.execution.budgets.max_storage_bytes",
            &mut diagnostics,
        );
        validate_optional_positive(
            self.execution.budgets.max_artifacts,
            "$.execution.budgets.max_artifacts",
            &mut diagnostics,
        );
        if self.execution.budgets.max_depth == Some(0) {
            diagnostics.push(SpecDiagnostic::new(
                "$.execution.budgets.max_depth",
                "execution.budget.invalid",
                "max_depth must be greater than zero when provided",
            ));
        }

        validate_allow_deny(&self.execution.tools, "$.execution.tools", &mut diagnostics);
        validate_allow_deny(
            &self.execution.transforms,
            "$.execution.transforms",
            &mut diagnostics,
        );

        if let Some(fidelity) = self.execution.minimum_fidelity {
            if !(0.0..=1.0).contains(&fidelity) {
                diagnostics.push(SpecDiagnostic::new(
                    "$.execution.minimum_fidelity",
                    "execution.fidelity.invalid",
                    "minimum_fidelity must be between 0.0 and 1.0 inclusive",
                ));
            }
        }

        if self.output.bundle_root.trim().is_empty() {
            diagnostics.push(SpecDiagnostic::new(
                "$.output.bundle_root",
                "output.bundle_root.empty",
                "bundle_root must not be empty",
            ));
        }
        if self.output.naming_template.trim().is_empty() {
            diagnostics.push(SpecDiagnostic::new(
                "$.output.naming_template",
                "output.naming_template.empty",
                "naming_template must not be empty",
            ));
        }

        diagnostics
    }
}

fn validate_targets(targets: &[TargetSpec], base: &str, diagnostics: &mut Vec<SpecDiagnostic>) {
    let mut ids = BTreeSet::new();
    for (index, target) in targets.iter().enumerate() {
        let path = format!("{base}[{index}]");
        if !target.has_selector() {
            diagnostics.push(SpecDiagnostic::new(
                path.clone(),
                "target.selector.empty",
                "target must select by format, family, capability, transform, or profile",
            ));
        }
        if let Some(id) = &target.id {
            if !is_stable_id(id) {
                diagnostics.push(SpecDiagnostic::new(
                    format!("{path}.id"),
                    "target.id.invalid",
                    "target id must use only ASCII letters, digits, '.', '_', or '-'",
                ));
            }
            if !ids.insert(id.clone()) {
                diagnostics.push(SpecDiagnostic::new(
                    format!("{path}.id"),
                    "target.id.duplicate",
                    format!("target id '{id}' is declared more than once in this target list"),
                ));
            }
        }
    }
}

fn validate_optional_positive(
    value: Option<u64>,
    path: &str,
    diagnostics: &mut Vec<SpecDiagnostic>,
) {
    if value == Some(0) {
        diagnostics.push(SpecDiagnostic::new(
            path,
            "execution.budget.invalid",
            "budget must be greater than zero when provided",
        ));
    }
}

fn validate_allow_deny(
    policy: &AllowDenyPolicy,
    base: &str,
    diagnostics: &mut Vec<SpecDiagnostic>,
) {
    let allowed: BTreeSet<&str> = policy.allow.iter().map(String::as_str).collect();
    for (index, denied) in policy.deny.iter().enumerate() {
        if allowed.contains(denied.as_str()) {
            diagnostics.push(SpecDiagnostic::new(
                format!("{base}.deny[{index}]"),
                "policy.allow_deny.conflict",
                format!("'{denied}' appears in both allow and deny lists"),
            ));
        }
    }
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub fn validate_spec_file(path: &str) -> SpecValidationReport {
    match fs::read_to_string(path) {
        Ok(content) => validate_spec_str(&content),
        Err(error) => SpecValidationReport {
            valid: false,
            source_version: None,
            schema: None,
            diagnostics: vec![SpecDiagnostic::new(
                "$",
                "io.read",
                format!("failed to read spec '{path}': {error}"),
            )],
        },
    }
}

pub fn validate_spec_str(content: &str) -> SpecValidationReport {
    let root: serde_yaml_ng::Value = match serde_yaml_ng::from_str(content) {
        Ok(value) => value,
        Err(error) => {
            return SpecValidationReport {
                valid: false,
                source_version: None,
                schema: None,
                diagnostics: vec![SpecDiagnostic::new("$", "yaml.parse", error.to_string())],
            };
        }
    };

    let schema = root
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yaml_ng::Value::String("schema".to_string())))
        .and_then(serde_yaml_ng::Value::as_str)
        .map(str::to_string);

    match schema.as_deref() {
        None => validate_v1_compat(content),
        Some(SPEC_V2_ID) => validate_v2(content),
        Some(other) => SpecValidationReport {
            valid: false,
            source_version: None,
            schema: Some(other.to_string()),
            diagnostics: vec![SpecDiagnostic::new(
                "$.schema",
                "schema.unsupported",
                format!(
                    "unsupported renderflow schema '{other}'; supported: unversioned v1 compatibility or '{SPEC_V2_ID}'"
                ),
            )],
        },
    }
}

fn validate_v1_compat(content: &str) -> SpecValidationReport {
    match serde_yaml_ng::from_str::<Config>(content) {
        Ok(config) => match config.validate() {
            Ok(()) => {
                let migrated = migrate_v1_config(&config);
                let diagnostics = migrated.validate();
                SpecValidationReport {
                    valid: diagnostics.is_empty(),
                    source_version: Some(SourceSpecVersion::V1),
                    schema: None,
                    diagnostics,
                }
            }
            Err(error) => SpecValidationReport {
                valid: false,
                source_version: Some(SourceSpecVersion::V1),
                schema: None,
                diagnostics: vec![SpecDiagnostic::new(
                    "$",
                    "v1.compat.invalid",
                    error.to_string(),
                )],
            },
        },
        Err(error) => SpecValidationReport {
            valid: false,
            source_version: Some(SourceSpecVersion::V1),
            schema: None,
            diagnostics: vec![SpecDiagnostic::new(
                "$",
                "v1.compat.parse",
                error.to_string(),
            )],
        },
    }
}

fn validate_v2(content: &str) -> SpecValidationReport {
    match serde_yaml_ng::from_str::<SpecV2>(content) {
        Ok(spec) => {
            let diagnostics = spec.validate();
            SpecValidationReport {
                valid: diagnostics.is_empty(),
                source_version: Some(SourceSpecVersion::V2),
                schema: Some(spec.schema.clone()),
                diagnostics,
            }
        }
        Err(error) => SpecValidationReport {
            valid: false,
            source_version: Some(SourceSpecVersion::V2),
            schema: Some(SPEC_V2_ID.to_string()),
            diagnostics: vec![SpecDiagnostic::new("$", "v2.parse", error.to_string())],
        },
    }
}

pub fn load_spec(path: &str) -> Result<LoadedSpec> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read Renderflow spec: {path}"))?;
    load_spec_str(&content)
}

pub fn load_spec_str(content: &str) -> Result<LoadedSpec> {
    let report = validate_spec_str(content);
    if !report.valid {
        anyhow::bail!("Renderflow spec validation failed:\n{report}");
    }

    match report.source_version {
        Some(SourceSpecVersion::V2) => Ok(LoadedSpec {
            source_version: SourceSpecVersion::V2,
            spec: serde_yaml_ng::from_str(content).context("failed to parse validated v2 spec")?,
        }),
        Some(SourceSpecVersion::V1) => {
            let config: Config =
                serde_yaml_ng::from_str(content).context("failed to parse validated v1 config")?;
            Ok(LoadedSpec {
                source_version: SourceSpecVersion::V1,
                spec: migrate_v1_config(&config),
            })
        }
        None => anyhow::bail!("spec version could not be determined"),
    }
}

pub fn migrate_v1_file(path: &str) -> Result<SpecV2> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read v1 Renderflow config: {path}"))?;
    migrate_v1_str(&content)
}

pub fn migrate_v1_str(content: &str) -> Result<SpecV2> {
    let root: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(content).context("failed to parse Renderflow YAML")?;
    if root
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yaml_ng::Value::String("schema".to_string())))
        .is_some()
    {
        anyhow::bail!(
            "migration expects an unversioned v1 config; input already declares a schema"
        );
    }

    let config: Config = serde_yaml_ng::from_str(content).context("failed to parse v1 config")?;
    config.validate()?;
    let migrated = migrate_v1_config(&config);
    let diagnostics = migrated.validate();
    if !diagnostics.is_empty() {
        let report = SpecValidationReport {
            valid: false,
            source_version: Some(SourceSpecVersion::V2),
            schema: Some(SPEC_V2_ID.to_string()),
            diagnostics,
        };
        anyhow::bail!("migrated v2 spec is invalid:\n{report}");
    }
    Ok(migrated)
}

pub(crate) fn migrate_v1_config(config: &Config) -> SpecV2 {
    let exact = config
        .outputs
        .iter()
        .enumerate()
        .map(|(index, output)| TargetSpec {
            id: Some(format!("target.{}", index + 1)),
            role: Some(output.output_type.to_string()),
            format: Some(output.output_type.to_string()),
            family: None,
            capability: None,
            transform: None,
            preset: output.profile.clone(),
            template: output.template.clone(),
        })
        .collect();

    let mut variables = BTreeMap::new();
    variables.extend(config.variables.clone());

    SpecV2 {
        schema: SPEC_V2_ID.to_string(),
        sources: vec![SourceSpec {
            id: "source.main".to_string(),
            role: Some("primary".to_string()),
            kind: SourceKind::Artifact,
            path: Some(config.input.clone()),
            uri: None,
            members: Vec::new(),
            media_type: None,
            format: Some(config.input_format().to_string()),
            detect: config.input_format.is_none(),
            immutable: true,
        }],
        profiles: BTreeMap::new(),
        targets: TargetSelection {
            exact,
            profiles: Vec::new(),
            all_reachable: false,
            include: SelectorSet::default(),
            exclude: SelectorSet::default(),
            intermediates: IntermediatePolicy::CacheOnly,
        },
        execution: ExecutionPolicy {
            optimization: config.optimization,
            ..ExecutionPolicy::default()
        },
        output: OutputLayout {
            bundle_root: config.output_dir.clone(),
            ..OutputLayout::default()
        },
        variables,
        transforms: config.transforms.clone(),
    }
}

pub fn json_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://egohygiene.github.io/renderflow/schemas/renderflow-v2.schema.json",
        "title": "Renderflow execution specification v2",
        "description": "Declarative source, derivative target, execution policy, and output-layout intent consumed by the Renderflow planner.",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema", "sources", "targets"],
        "properties": {
            "schema": {"const": SPEC_V2_ID},
            "sources": {"type": "array", "minItems": 1, "items": {"$ref": "#/$defs/source"}},
            "profiles": {
                "type": "object",
                "additionalProperties": {"$ref": "#/$defs/profile"},
                "default": {}
            },
            "targets": {"$ref": "#/$defs/targetSelection"},
            "execution": {"$ref": "#/$defs/executionPolicy"},
            "output": {"$ref": "#/$defs/outputLayout"},
            "variables": {
                "type": "object",
                "additionalProperties": {"type": "string"},
                "default": {}
            },
            "transforms": {"type": ["string", "null"]}
        },
        "$defs": {
            "stableId": {"type": "string", "minLength": 1, "pattern": "^[A-Za-z0-9._-]+$"},
            "source": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id"],
                "properties": {
                    "id": {"$ref": "#/$defs/stableId"},
                    "role": {"type": ["string", "null"]},
                    "kind": {"enum": ["artifact", "collection"], "default": "artifact"},
                    "path": {"type": ["string", "null"]},
                    "uri": {"type": ["string", "null"]},
                    "members": {"type": "array", "items": {"$ref": "#/$defs/stableId"}, "default": []},
                    "media_type": {"type": ["string", "null"]},
                    "format": {"type": ["string", "null"]},
                    "detect": {"type": "boolean", "default": true},
                    "immutable": {"const": true, "default": true}
                }
            },
            "selectorSet": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "formats": {"type": "array", "items": {"type": "string"}, "default": []},
                    "families": {"type": "array", "items": {"type": "string"}, "default": []},
                    "capabilities": {"type": "array", "items": {"type": "string"}, "default": []},
                    "transforms": {"type": "array", "items": {"type": "string"}, "default": []},
                    "profiles": {"type": "array", "items": {"$ref": "#/$defs/stableId"}, "default": []}
                }
            },
            "target": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": {"anyOf": [{"$ref": "#/$defs/stableId"}, {"type": "null"}]},
                    "role": {"type": ["string", "null"]},
                    "format": {"type": ["string", "null"]},
                    "family": {"type": ["string", "null"]},
                    "capability": {"type": ["string", "null"]},
                    "transform": {"type": ["string", "null"]},
                    "preset": {"type": ["string", "null"]},
                    "template": {"type": ["string", "null"]}
                },
                "anyOf": [
                    {"required": ["format"]},
                    {"required": ["family"]},
                    {"required": ["capability"]},
                    {"required": ["transform"]}
                ]
            },
            "targetSelection": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "exact": {"type": "array", "items": {"$ref": "#/$defs/target"}, "default": []},
                    "profiles": {"type": "array", "items": {"$ref": "#/$defs/stableId"}, "default": []},
                    "all_reachable": {"type": "boolean", "default": false},
                    "include": {"$ref": "#/$defs/selectorSet"},
                    "exclude": {"$ref": "#/$defs/selectorSet"},
                    "intermediates": {"enum": ["cache_only", "retain"], "default": "cache_only"}
                }
            },
            "profile": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "description": {"type": ["string", "null"]},
                    "targets": {"type": "array", "items": {"$ref": "#/$defs/target"}, "default": []},
                    "include": {"$ref": "#/$defs/selectorSet"},
                    "exclude": {"$ref": "#/$defs/selectorSet"}
                }
            },
            "allowDeny": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "allow": {"type": "array", "items": {"type": "string"}, "default": []},
                    "deny": {"type": "array", "items": {"type": "string"}, "default": []}
                }
            },
            "budgets": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "max_output_bytes": {"type": ["integer", "null"], "minimum": 1},
                    "max_storage_bytes": {"type": ["integer", "null"], "minimum": 1},
                    "max_artifacts": {"type": ["integer", "null"], "minimum": 1},
                    "max_depth": {"type": ["integer", "null"], "minimum": 1}
                }
            },
            "requirements": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "deterministic": {"type": "boolean", "default": false},
                    "local_only": {"type": "boolean", "default": false},
                    "offline": {"type": "boolean", "default": false}
                }
            },
            "validation": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "required": {"type": "boolean", "default": true},
                    "validators": {"type": "array", "items": {"type": "string"}, "default": []}
                }
            },
            "executionPolicy": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "optimization": {"enum": ["speed", "quality", "balanced", "pareto"], "default": "balanced"},
                    "max_parallel": {"type": "integer", "minimum": 1, "default": 1},
                    "budgets": {"$ref": "#/$defs/budgets"},
                    "tools": {"$ref": "#/$defs/allowDeny"},
                    "transforms": {"$ref": "#/$defs/allowDeny"},
                    "requirements": {"$ref": "#/$defs/requirements"},
                    "network": {"enum": ["deny", "allow"], "default": "deny"},
                    "ai": {"enum": ["deny", "local_only", "allow"], "default": "deny"},
                    "retry_policy": {"type": ["string", "null"]},
                    "timeout_policy": {"type": ["string", "null"]},
                    "validation": {"$ref": "#/$defs/validation"},
                    "minimum_fidelity": {"type": ["number", "null"], "minimum": 0.0, "maximum": 1.0},
                    "publication_policy": {"type": ["string", "null"]},
                    "redaction_policy": {"type": ["string", "null"]}
                }
            },
            "outputLayout": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "bundle_root": {"type": "string", "minLength": 1, "default": "dist"},
                    "naming_template": {"type": "string", "minLength": 1, "default": "{source.id}/{target.role}.{ext}"},
                    "collision": {"enum": ["error", "replace", "dedupe"], "default": "error"}
                }
            }
        }
    })
}

pub fn json_schema_pretty() -> Result<String> {
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&json_schema())?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OutputType;

    const VALID_MULTI_SOURCE: &str =
        include_str!("../../../tests/fixtures/spec-v2/valid-multi-source.yaml");
    const VALID_EXACT: &str = include_str!("../../../tests/fixtures/spec-v2/valid-exact.yaml");
    const INVALID_DUPLICATE_SOURCE: &str =
        include_str!("../../../tests/fixtures/spec-v2/invalid-duplicate-source.yaml");
    const INVALID_POLICY: &str =
        include_str!("../../../tests/fixtures/spec-v2/invalid-policy.yaml");

    #[test]
    fn v2_multi_source_and_ordered_collection_are_representable() {
        let loaded = load_spec_str(VALID_MULTI_SOURCE).expect("valid v2 spec should load");
        assert_eq!(loaded.source_version, SourceSpecVersion::V2);
        assert_eq!(loaded.spec.sources.len(), 3);
        let collection = loaded
            .spec
            .sources
            .iter()
            .find(|source| source.kind == SourceKind::Collection)
            .expect("collection source should exist");
        assert_eq!(collection.members, vec!["source.cover", "source.body"]);
        assert!(loaded.spec.targets.all_reachable);
    }

    #[test]
    fn exact_targets_and_profiles_are_representable() {
        let loaded = load_spec_str(VALID_EXACT).expect("valid exact-target v2 spec should load");
        assert_eq!(loaded.spec.targets.exact.len(), 2);
        assert_eq!(loaded.spec.targets.profiles, vec!["publication.web"]);
    }

    #[test]
    fn network_and_ai_default_to_deny() {
        let yaml = r#"
schema: renderflow/v2
sources:
  - id: source.main
    path: input.md
targets:
  exact:
    - format: html
"#;
        let loaded = load_spec_str(yaml).expect("minimal v2 spec should load");
        assert_eq!(loaded.spec.execution.network, NetworkPolicy::Deny);
        assert_eq!(loaded.spec.execution.ai, AiPolicy::Deny);
    }

    #[test]
    fn duplicate_source_ids_report_a_field_path() {
        let report = validate_spec_str(INVALID_DUPLICATE_SOURCE);
        assert!(!report.valid);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.path == "$.sources[1].id" && diagnostic.code == "source.id.duplicate"
        }));
    }

    #[test]
    fn invalid_policy_reports_precise_paths() {
        let report = validate_spec_str(INVALID_POLICY);
        assert!(!report.valid);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == "$.execution.max_parallel"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == "$.execution.minimum_fidelity"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == "$.execution.tools.deny[0]"));
    }

    #[test]
    fn unversioned_v1_is_loaded_through_explicit_compatibility_path() {
        let yaml = r#"
outputs:
  - type: pdf
  - type: html
input: input.md
output_dir: public
"#;
        let loaded = load_spec_str(yaml).expect("v1 config should migrate in memory");
        assert_eq!(loaded.source_version, SourceSpecVersion::V1);
        assert_eq!(loaded.spec.schema, SPEC_V2_ID);
        assert_eq!(loaded.spec.output.bundle_root, "public");
        assert_eq!(loaded.spec.targets.exact.len(), 2);
    }

    #[test]
    fn v1_migration_preserves_transform_and_optimization_intent() {
        let yaml = r#"
outputs:
  - type: html
input: input.md
output_dir: dist
optimization: quality
transforms: transforms.yaml
variables:
  project: renderflow
"#;
        let migrated = migrate_v1_str(yaml).expect("v1 migration should succeed");
        assert_eq!(migrated.execution.optimization, OptimizationMode::Quality);
        assert_eq!(migrated.transforms.as_deref(), Some("transforms.yaml"));
        assert_eq!(
            migrated.variables.get("project").map(String::as_str),
            Some("renderflow")
        );
    }

    #[test]
    fn unsupported_schema_is_actionable() {
        let report = validate_spec_str("schema: renderflow/v99\nsources: []\ntargets: {}\n");
        assert!(!report.valid);
        assert_eq!(report.diagnostics[0].path, "$.schema");
        assert_eq!(report.diagnostics[0].code, "schema.unsupported");
    }

    #[test]
    fn runtime_json_schema_declares_v2_identifier() {
        let schema = json_schema();
        assert_eq!(schema["properties"]["schema"]["const"], SPEC_V2_ID);
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn migration_rejects_already_versioned_input() {
        let error = migrate_v1_str(VALID_EXACT).expect_err("v2 input must not be migrated as v1");
        assert!(error.to_string().contains("already declares a schema"));
    }

    #[test]
    fn unsupported_v1_output_still_fails_compatibility_validation() {
        let yaml = "outputs:\n  - type: definitely-not-real\ninput: input.md\n";
        let report = validate_spec_str(yaml);
        assert!(!report.valid);
        assert_eq!(report.source_version, Some(SourceSpecVersion::V1));
    }

    #[test]
    fn output_type_conversion_remains_lossless_for_v1_document_targets() {
        let config: Config =
            serde_yaml_ng::from_str("outputs:\n  - type: pdf\ninput: input.md\noutput_dir: dist\n")
                .expect("v1 config parses");
        let migrated = migrate_v1_config(&config);
        assert!(matches!(config.outputs[0].output_type, OutputType::Pdf));
        assert_eq!(migrated.targets.exact[0].format.as_deref(), Some("pdf"));
    }
}
