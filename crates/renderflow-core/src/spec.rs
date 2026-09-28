use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::Config;
use crate::optimization::OptimizationMode;
use crate::publication::{PageGeometry, PublicationContract};

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
    /// Expected payload digest for an explicitly ordered collection member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Declared page geometry; part of collection identity when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<PageGeometry>,
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
    pub providers: Vec<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub variants: Vec<String>,
}

impl SelectorSet {
    pub fn is_empty(&self) -> bool {
        self.formats.is_empty()
            && self.families.is_empty()
            && self.capabilities.is_empty()
            && self.transforms.is_empty()
            && self.providers.is_empty()
            && self.roles.is_empty()
            && self.profiles.is_empty()
            && self.variants.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
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
    pub variant: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    /// Whether failure to resolve this branch fails the complete request.
    #[serde(default)]
    pub requirement: TargetRequirement,
    /// Provider-neutral options passed to the selected target adapter.
    #[serde(default)]
    pub options: BTreeMap<String, Value>,
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
pub enum TargetRequirement {
    #[default]
    Required,
    Optional,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivativeProfile {
    /// Profile contract version. Only `renderflow.profile/v1` is currently supported.
    #[serde(default = "default_profile_schema")]
    pub schema: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Parent profiles, composed from left to right before this profile.
    #[serde(default)]
    pub extends: Vec<String>,
    #[serde(default)]
    pub targets: Vec<TargetSpec>,
    #[serde(default)]
    pub include: SelectorSet,
    #[serde(default)]
    pub exclude: SelectorSet,
    /// Expand to every policy-allowed branch reachable from the detected source.
    #[serde(default)]
    pub all_reachable: bool,
    /// Override intermediate retention when the profile is selected.
    #[serde(default)]
    pub intermediates: Option<IntermediatePolicy>,
    /// Named publication-hygiene policy applied to artifacts selected by this profile.
    #[serde(default)]
    pub hygiene_policy: Option<String>,
    /// Optional execution gates contributed by this profile.
    #[serde(default)]
    pub policy: ProfilePolicy,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePolicy {
    #[serde(default)]
    pub validation: Option<ValidationPolicy>,
    #[serde(default)]
    pub minimum_fidelity: Option<f32>,
    #[serde(default)]
    pub requirements: Option<ExecutionRequirements>,
    #[serde(default)]
    pub network: Option<NetworkPolicy>,
    #[serde(default)]
    pub ai: Option<AiPolicy>,
    #[serde(default)]
    pub budgets: Option<ResourceBudgets>,
    #[serde(default)]
    pub publication_policy: Option<String>,
    #[serde(default)]
    pub redaction_policy: Option<String>,
}

fn default_profile_schema() -> String {
    "renderflow.profile/v1".to_string()
}

impl Default for DerivativeProfile {
    fn default() -> Self {
        Self {
            schema: default_profile_schema(),
            description: None,
            extends: Vec::new(),
            targets: Vec::new(),
            include: SelectorSet::default(),
            exclude: SelectorSet::default(),
            all_reachable: false,
            intermediates: None,
            hygiene_policy: None,
            policy: ProfilePolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationAudience {
    #[default]
    Candidate,
    Private,
    Public,
    Commercial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionDeterminism {
    #[default]
    Deterministic,
    Probabilistic,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataHygienePolicy {
    /// Metadata keys intentionally retained in publication artifacts.
    #[serde(default)]
    pub allow: Vec<String>,
    /// Metadata keys or field classes removed from publication artifacts.
    #[serde(default)]
    pub deny: Vec<String>,
    /// Remove all non-allowlisted metadata rather than only explicit deny entries.
    #[serde(default)]
    pub allowlist_only: bool,
}

fn default_hygiene_enabled() -> bool {
    true
}

fn default_block() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretHygienePolicy {
    #[serde(default = "default_hygiene_enabled")]
    pub enabled: bool,
    #[serde(default = "default_block")]
    pub block: bool,
    /// Additional literal markers. Values are never included in diagnostics or evidence.
    #[serde(default)]
    pub markers: Vec<String>,
}

impl Default for SecretHygienePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            block: true,
            markers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedReferencePolicy {
    /// Brand, franchise, company, creator, or work names requiring review.
    #[serde(default)]
    pub terms: Vec<String>,
    #[serde(default = "default_block")]
    pub block: bool,
    #[serde(default)]
    pub case_sensitive: bool,
}

impl Default for ProtectedReferencePolicy {
    fn default() -> Self {
        Self {
            terms: Vec::new(),
            block: true,
            case_sensitive: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentRedactionPolicy {
    /// Provider-neutral redaction provider identifier.
    pub provider: String,
    #[serde(default)]
    pub determinism: RedactionDeterminism,
    #[serde(default)]
    pub classes: Vec<String>,
    /// Explicit human approval for this exact policy configuration.
    #[serde(default)]
    pub reviewed: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RightsHygienePolicy {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub rights_holder: Option<String>,
    #[serde(default)]
    pub approval_reference: Option<String>,
    /// Records an explicit rights review; it is not a legal conclusion by Renderflow.
    #[serde(default)]
    pub reviewed: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HygienePolicy {
    #[serde(default)]
    pub audience: PublicationAudience,
    #[serde(default)]
    pub metadata: MetadataHygienePolicy,
    #[serde(default)]
    pub secrets: SecretHygienePolicy,
    #[serde(default)]
    pub protected_references: ProtectedReferencePolicy,
    #[serde(default)]
    pub redaction: Option<ContentRedactionPolicy>,
    #[serde(default)]
    pub rights: RightsHygienePolicy,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationFailureMode {
    Fatal,
    #[default]
    BranchLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectedLossClass {
    Lossless,
    Partial,
    Lossy,
    PathDependent,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationPolicy {
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub failure_mode: ValidationFailureMode,
    #[serde(default)]
    pub allow_unavailable: bool,
}

impl Default for ValidationPolicy {
    fn default() -> Self {
        Self {
            required: true,
            validators: Vec::new(),
            failure_mode: ValidationFailureMode::default(),
            allow_unavailable: false,
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
    pub reject_loss_classes: Vec<RejectedLossClass>,
    #[serde(default)]
    pub publication_policy: Option<String>,
    #[serde(default)]
    pub redaction_policy: Option<String>,
    /// Named hygiene policy applied to the complete selected publication bundle.
    #[serde(default)]
    pub hygiene_policy: Option<String>,
    /// Exact, bounded print-interior route for an ordered PNG/JPEG collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_pdf_interior: Option<PrintPdfInteriorPolicy>,
    /// Exact native fixed-layout EPUB route for an ordered PNG/JPEG collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_layout_epub: Option<FixedLayoutEpubPolicy>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintPdfInteriorPolicy {
    pub executable: String,
    pub provider_version: String,
    pub box_policy: String,
    pub rotation: String,
    pub scaling: String,
    pub color_policy: String,
    pub max_pages: usize,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub timeout_seconds: u64,
}

impl PrintPdfInteriorPolicy {
    pub fn validate(&self) -> Result<()> {
        if self.executable.trim().is_empty() || self.executable.contains('\0') {
            anyhow::bail!("print_pdf.provider: executable must name a local img2pdf command");
        }
        if self.provider_version != "0.6.3" {
            anyhow::bail!("print_pdf.provider: the proven route requires img2pdf version 0.6.3");
        }
        if self.box_policy != "media_bleed_trim_inset"
            || self.rotation != "none"
            || self.scaling != "fit"
            || self.color_policy != "preserve_rgb_gray"
        {
            anyhow::bail!("print_pdf.policy: supported route requires box_policy=media_bleed_trim_inset, rotation=none, scaling=fit, color_policy=preserve_rgb_gray");
        }
        if !(1..=1000).contains(&self.max_pages)
            || !(1..=536_870_912).contains(&self.max_input_bytes)
            || !(1..=536_870_912).contains(&self.max_output_bytes)
            || !(1..=3600).contains(&self.timeout_seconds)
        {
            anyhow::bail!("print_pdf.bounds: max_pages (1..1000), input/output bytes (1..512 MiB), and timeout_seconds (1..3600) are required");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedLayoutEpubPolicy {
    /// Reading order only; RTL never mirrors or alters artwork bytes.
    pub page_progression_direction: String,
    /// The proven route uses single-page spreads only.
    pub spread: String,
    /// Existing first collection member, reused as the EPUB cover image.
    pub cover_member_id: String,
    pub max_pages: usize,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
}

impl FixedLayoutEpubPolicy {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.page_progression_direction.as_str(), "ltr" | "rtl")
            || self.spread != "none"
            || !is_stable_id(&self.cover_member_id)
        {
            anyhow::bail!("fixed_epub.policy: require explicit ltr/rtl page progression, spread=none, and a stable cover member ID");
        }
        if !(1..=1000).contains(&self.max_pages)
            || !(1..=536_870_912).contains(&self.max_input_bytes)
            || !(1..=536_870_912).contains(&self.max_output_bytes)
        {
            anyhow::bail!("fixed_epub.bounds: max_pages (1..1000) and input/output bytes (1..512 MiB) are required");
        }
        Ok(())
    }
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
            reject_loss_classes: Vec::new(),
            publication_policy: None,
            redaction_policy: None,
            hygiene_policy: None,
            print_pdf_interior: None,
            fixed_layout_epub: None,
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
    #[serde(default)]
    pub hygiene: BTreeMap<String, HygienePolicy>,
    #[serde(default)]
    pub publication: Option<PublicationContract>,
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
        if let Some(print) = &self.execution.print_pdf_interior {
            if let Err(error) = print.validate() {
                diagnostics.push(SpecDiagnostic::new(
                    "$.execution.print_pdf_interior",
                    "print_pdf.policy.invalid",
                    error.to_string(),
                ));
            }
        }
        if let Some(epub) = &self.execution.fixed_layout_epub {
            if let Err(error) = epub.validate() {
                diagnostics.push(SpecDiagnostic::new(
                    "$.execution.fixed_layout_epub",
                    "fixed_epub.policy.invalid",
                    error.to_string(),
                ));
            }
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
                let mut members = BTreeSet::new();
                for (member_index, member) in source.members.iter().enumerate() {
                    let path = format!("$.sources[{index}].members[{member_index}]");
                    if !members.insert(member) {
                        diagnostics.push(SpecDiagnostic::new(
                            path.clone(),
                            "collection.member.duplicate",
                            format!("collection member '{member}' appears more than once"),
                        ));
                    }
                    if !source_ids.contains(member) {
                        diagnostics.push(SpecDiagnostic::new(
                            path.clone(),
                            "collection.member.unknown",
                            format!(
                                "collection member '{member}' does not match a declared source id"
                            ),
                        ));
                    }
                    if member == &source.id {
                        diagnostics.push(SpecDiagnostic::new(
                            path.clone(),
                            "collection.member.self_reference",
                            "a collection cannot contain itself",
                        ));
                    }
                    if let Some(member_source) = self.sources.iter().find(|item| &item.id == member)
                    {
                        if member_source.kind != SourceKind::Artifact {
                            diagnostics.push(SpecDiagnostic::new(
                                path.clone(),
                                "collection.member.nested",
                                "collection members must be artifact sources",
                            ));
                        }
                        if member_source.path.is_none() || member_source.uri.is_some() {
                            diagnostics.push(SpecDiagnostic::new(
                                path.clone(),
                                "collection.member.locator",
                                "collection members require a local root-relative path",
                            ));
                        }
                        if let Some(locator) = &member_source.path {
                            if locator.is_empty()
                                || !Path::new(locator)
                                    .components()
                                    .all(|component| matches!(component, Component::Normal(_)))
                            {
                                diagnostics.push(SpecDiagnostic::new(
                                    format!("$.sources[{index}].members[{member_index}]"),
                                    "collection.member.locator",
                                    "collection member paths must be root-relative without traversal",
                                ));
                            }
                        }
                        if member_source.format.is_none() || member_source.media_type.is_none() {
                            diagnostics.push(SpecDiagnostic::new(
                                path.clone(),
                                "collection.member.media",
                                "collection members require explicit format and media_type",
                            ));
                        }
                        if !member_source.sha256.as_ref().is_some_and(|digest| {
                            digest.len() == 64
                                && digest.bytes().all(|byte| {
                                    byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
                                })
                        }) {
                            diagnostics.push(SpecDiagnostic::new(
                                path,
                                "collection.member.digest",
                                "collection members require a lowercase 64-character sha256 digest",
                            ));
                        }
                        if member_source.geometry.as_ref().is_some_and(|geometry| {
                            !geometry.width.is_finite()
                                || geometry.width <= 0.0
                                || !geometry.height.is_finite()
                                || geometry.height <= 0.0
                                || [geometry.margin, geometry.bleed, geometry.safe_area]
                                    .into_iter()
                                    .flatten()
                                    .any(|value| !value.is_finite() || value < 0.0)
                        }) {
                            diagnostics.push(SpecDiagnostic::new(
                                format!("$.sources[{index}].members[{member_index}]"),
                                "collection.member.geometry",
                                "collection member geometry requires finite positive dimensions and nonnegative bounds",
                            ));
                        }
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
        validate_selector_variants(&self.targets.include, "$.targets.include", &mut diagnostics);
        validate_selector_variants(&self.targets.exclude, "$.targets.exclude", &mut diagnostics);

        for (index, profile_name) in self.targets.profiles.iter().enumerate() {
            if !self.profiles.contains_key(profile_name) {
                diagnostics.push(SpecDiagnostic::new(
                    format!("$.targets.profiles[{index}]"),
                    "profile.unknown",
                    format!("target profile '{profile_name}' is not declared in $.profiles"),
                ));
            }
        }
        if self.execution.hygiene_policy.is_none() {
            let selected_hygiene = self
                .targets
                .profiles
                .iter()
                .filter_map(|profile_name| self.profiles.get(profile_name))
                .filter_map(|profile| profile.hygiene_policy.as_deref())
                .collect::<BTreeSet<_>>();
            if selected_hygiene.len() > 1 {
                diagnostics.push(SpecDiagnostic::new(
                    "$.targets.profiles",
                    "hygiene.policy.conflict",
                    "selected profiles use different hygiene policies; choose one with execution.hygiene_policy",
                ));
            }
        }

        if let Some(publication) = &self.publication {
            let hygiene = self
                .execution
                .hygiene_policy
                .as_deref()
                .and_then(|policy| self.hygiene.get(policy));
            diagnostics.extend(
                publication
                    .diagnostics(hygiene)
                    .into_iter()
                    .map(|item| SpecDiagnostic::new(item.path, item.code, item.message)),
            );
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
            if profile.schema != "renderflow.profile/v1" {
                diagnostics.push(SpecDiagnostic::new(
                    format!("{base}.schema"),
                    "profile.schema.unsupported",
                    "supported profile schema is renderflow.profile/v1",
                ));
            }
            for (index, parent) in profile.extends.iter().enumerate() {
                if !self.profiles.contains_key(parent) {
                    diagnostics.push(SpecDiagnostic::new(
                        format!("{base}.extends[{index}]"),
                        "profile.parent.unknown",
                        format!("parent profile '{parent}' is not declared in $.profiles"),
                    ));
                }
            }
            if profile.targets.is_empty()
                && profile.include.is_empty()
                && profile.extends.is_empty()
                && !profile.all_reachable
            {
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
            validate_selector_variants(
                &profile.include,
                &format!("{base}.include"),
                &mut diagnostics,
            );
            validate_selector_variants(
                &profile.exclude,
                &format!("{base}.exclude"),
                &mut diagnostics,
            );
            if let Some(policy) = &profile.hygiene_policy {
                if !self.hygiene.contains_key(policy) {
                    diagnostics.push(SpecDiagnostic::new(
                        format!("{base}.hygiene_policy"),
                        "hygiene.policy.unknown",
                        format!("hygiene policy '{policy}' is not declared in $.hygiene"),
                    ));
                }
            }
        }

        if let Some(policy) = &self.execution.hygiene_policy {
            if !self.hygiene.contains_key(policy) {
                diagnostics.push(SpecDiagnostic::new(
                    "$.execution.hygiene_policy",
                    "hygiene.policy.unknown",
                    format!("hygiene policy '{policy}' is not declared in $.hygiene"),
                ));
            }
        }

        for (policy_name, policy) in &self.hygiene {
            let base = format!("$.hygiene.{policy_name}");
            if !is_stable_id(policy_name) {
                diagnostics.push(SpecDiagnostic::new(
                    base.clone(),
                    "hygiene.id.invalid",
                    "hygiene policy names must use only ASCII letters, digits, '.', '_', or '-'",
                ));
            }
            validate_non_empty_values(
                &policy.metadata.allow,
                &format!("{base}.metadata.allow"),
                &mut diagnostics,
            );
            validate_non_empty_values(
                &policy.metadata.deny,
                &format!("{base}.metadata.deny"),
                &mut diagnostics,
            );
            validate_non_empty_values(
                &policy.secrets.markers,
                &format!("{base}.secrets.markers"),
                &mut diagnostics,
            );
            validate_non_empty_values(
                &policy.protected_references.terms,
                &format!("{base}.protected_references.terms"),
                &mut diagnostics,
            );
            if let Some(redaction) = &policy.redaction {
                if redaction.provider.trim().is_empty() {
                    diagnostics.push(SpecDiagnostic::new(
                        format!("{base}.redaction.provider"),
                        "hygiene.redaction.provider_empty",
                        "a configured redaction policy requires a provider id",
                    ));
                }
                validate_non_empty_values(
                    &redaction.classes,
                    &format!("{base}.redaction.classes"),
                    &mut diagnostics,
                );
            }
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
        if let Some(variant) = &target.variant {
            if !is_stable_id(variant) {
                diagnostics.push(SpecDiagnostic::new(
                    format!("{path}.variant"),
                    "target.variant.invalid",
                    "variant id must use only ASCII letters, digits, '.', '_', or '-'",
                ));
            }
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

fn validate_selector_variants(
    selector: &SelectorSet,
    base: &str,
    diagnostics: &mut Vec<SpecDiagnostic>,
) {
    for (index, variant) in selector.variants.iter().enumerate() {
        if !is_stable_id(variant) {
            diagnostics.push(SpecDiagnostic::new(
                format!("{base}.variants[{index}]"),
                "selector.variant.invalid",
                "variant id must use only ASCII letters, digits, '.', '_', or '-'",
            ));
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

fn validate_non_empty_values(values: &[String], path: &str, diagnostics: &mut Vec<SpecDiagnostic>) {
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            diagnostics.push(SpecDiagnostic::new(
                format!("{path}[{index}]"),
                "hygiene.value.empty",
                "hygiene policy values must not be empty",
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
        Ok(config) => match config.validate_structure() {
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
    config.validate_structure()?;
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

fn legacy_source_format(config: &Config) -> String {
    std::path::Path::new(&config.input)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .unwrap_or_else(|| config.input_format().to_string())
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
            variant: None,
            preset: output.profile.clone(),
            template: output.template.clone(),
            requirement: TargetRequirement::Required,
            options: BTreeMap::new(),
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
            format: Some(legacy_source_format(config)),
            sha256: None,
            geometry: None,
            detect: config.input_format.is_none(),
            immutable: true,
        }],
        profiles: BTreeMap::new(),
        hygiene: BTreeMap::new(),
        publication: None,
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
            "hygiene": {
                "type": "object",
                "additionalProperties": {"$ref": "#/$defs/hygienePolicy"},
                "default": {}
            },
            "publication": {
                "anyOf": [
                    {"$ref": "#/$defs/publicationContract"},
                    {"type": "null"}
                ]
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
            "publicationContributor": {
                "type": "object", "additionalProperties": false,
                "required": ["name", "role"],
                "properties": {
                    "name": {"type": "string", "minLength": 1},
                    "role": {"type": "string", "minLength": 1},
                    "identifier": {"type": ["string", "null"]}
                }
            },
            "publicationAsset": {
                "type": "object", "additionalProperties": false,
                "required": ["role", "path"],
                "properties": {
                    "role": {"type": "string", "minLength": 1},
                    "path": {"type": "string", "minLength": 1},
                    "artifact_dna": {"type": ["string", "null"]},
                    "alt_text": {"type": ["string", "null"]},
                    "approval_reference": {"type": ["string", "null"]}
                }
            },
            "pageGeometry": {
                "type": "object", "additionalProperties": false,
                "required": ["width", "height"],
                "properties": {
                    "width": {"type": "number", "exclusiveMinimum": 0},
                    "height": {"type": "number", "exclusiveMinimum": 0},
                    "unit": {"type": "string", "default": "mm"},
                    "margin": {"type": ["number", "null"], "minimum": 0},
                    "bleed": {"type": ["number", "null"], "minimum": 0},
                    "safe_area": {"type": ["number", "null"], "minimum": 0}
                }
            },
            "publicationRights": {
                "type": "object", "additionalProperties": false,
                "properties": {
                    "license": {"type": ["string", "null"]},
                    "rights_holder": {"type": ["string", "null"]},
                    "approval_reference": {"type": ["string", "null"]},
                    "reviewed": {"type": "boolean", "default": false}
                }
            },
            "accessibilityMetadata": {
                "type": "object", "additionalProperties": false,
                "properties": {
                    "summary": {"type": ["string", "null"]},
                    "access_modes": {"type": "array", "items": {"type": "string"}, "default": []},
                    "hazards": {"type": "array", "items": {"type": "string"}, "default": []}
                }
            },
            "publicationRoleConstraints": {
                "type": "object", "additionalProperties": false,
                "required": ["format"],
                "properties": {
                    "format": {"type": "string", "minLength": 1},
                    "stage": {"type": "string", "default": ""},
                    "geometry": {"anyOf": [{"$ref": "#/$defs/pageGeometry"}, {"type": "null"}]},
                    "color_policy": {"type": ["string", "null"]},
                    "minimum_image_dpi": {"type": ["integer", "null"], "minimum": 1},
                    "require_embedded_fonts": {"type": "boolean", "default": false},
                    "validators": {"type": "array", "items": {"type": "string"}, "default": []}
                }
            },
            "publicationContract": {
                "type": "object", "additionalProperties": false,
                "required": ["publication", "issue_id", "title", "publication_date", "language", "geometry"],
                "properties": {
                    "schema": {"const": "renderflow.publication/v1", "default": "renderflow.publication/v1"},
                    "publication": {"type": "string", "minLength": 1},
                    "series": {"type": ["string", "null"]},
                    "issue_id": {"type": "string", "minLength": 1},
                    "issue_number": {"type": ["string", "null"]},
                    "title": {"type": "string", "minLength": 1},
                    "subtitle": {"type": ["string", "null"]},
                    "contributors": {"type": "array", "items": {"$ref": "#/$defs/publicationContributor"}, "default": []},
                    "publication_date": {"type": "string", "minLength": 1},
                    "status": {"enum": ["draft", "reviewed", "approved", "released"], "default": "draft"},
                    "language": {"type": "string", "minLength": 1},
                    "artwork": {"type": "array", "items": {"$ref": "#/$defs/publicationAsset"}, "default": []},
                    "geometry": {"$ref": "#/$defs/pageGeometry"},
                    "color_policy": {"type": ["string", "null"]},
                    "font_policy": {"type": ["string", "null"]},
                    "asset_policy": {"type": ["string", "null"]},
                    "rights": {"$ref": "#/$defs/publicationRights"},
                    "accessibility": {"$ref": "#/$defs/accessibilityMetadata"},
                    "canonical_url": {"type": ["string", "null"]},
                    "identifiers": {"type": "object", "additionalProperties": {"type": "string"}, "default": {}},
                    "output_roles": {"type": "object", "additionalProperties": {"$ref": "#/$defs/publicationRoleConstraints"}, "default": {}},
                    "extensions": {"type": "object", "default": {}}
                }
            },
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
                    "sha256": {"type": ["string", "null"], "pattern": "^[0-9a-f]{64}$"},
                    "geometry": {"anyOf": [{"$ref": "#/$defs/pageGeometry"}, {"type": "null"}]},
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
                    "providers": {"type": "array", "items": {"$ref": "#/$defs/stableId"}, "default": []},
                    "roles": {"type": "array", "items": {"type": "string"}, "default": []},
                    "profiles": {"type": "array", "items": {"$ref": "#/$defs/stableId"}, "default": []},
                    "variants": {"type": "array", "items": {"$ref": "#/$defs/stableId"}, "default": []}
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
                    "variant": {"anyOf": [{"$ref": "#/$defs/stableId"}, {"type": "null"}]},
                    "preset": {"type": ["string", "null"]},
                    "template": {"type": ["string", "null"]},
                    "requirement": {"enum": ["required", "optional"], "default": "required"},
                    "options": {"type": "object", "default": {}}
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
                    "schema": {"const": "renderflow.profile/v1", "default": "renderflow.profile/v1"},
                    "description": {"type": ["string", "null"]},
                    "extends": {"type": "array", "items": {"$ref": "#/$defs/stableId"}, "default": []},
                    "targets": {"type": "array", "items": {"$ref": "#/$defs/target"}, "default": []},
                    "include": {"$ref": "#/$defs/selectorSet"},
                    "exclude": {"$ref": "#/$defs/selectorSet"},
                    "all_reachable": {"type": "boolean", "default": false},
                    "intermediates": {"enum": ["cache_only", "retain"]},
                    "hygiene_policy": {"anyOf": [{"$ref": "#/$defs/stableId"}, {"type": "null"}]},
                    "policy": {"$ref": "#/$defs/profilePolicy"}
                }
            },
            "profilePolicy": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "validation": {"$ref": "#/$defs/validation"},
                    "minimum_fidelity": {"type": ["number", "null"], "minimum": 0.0, "maximum": 1.0},
                    "requirements": {"$ref": "#/$defs/requirements"},
                    "network": {"enum": ["deny", "allow"]},
                    "ai": {"enum": ["deny", "local_only", "allow"]},
                    "budgets": {"$ref": "#/$defs/budgets"},
                    "publication_policy": {"type": ["string", "null"]},
                    "redaction_policy": {"type": ["string", "null"]}
                }
            },
            "hygienePolicy": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "audience": {"enum": ["candidate", "private", "public", "commercial"], "default": "candidate"},
                    "metadata": {"$ref": "#/$defs/metadataHygiene"},
                    "secrets": {"$ref": "#/$defs/secretHygiene"},
                    "protected_references": {"$ref": "#/$defs/protectedReferences"},
                    "redaction": {"anyOf": [{"$ref": "#/$defs/contentRedaction"}, {"type": "null"}]},
                    "rights": {"$ref": "#/$defs/rightsHygiene"}
                }
            },
            "metadataHygiene": {
                "type": "object", "additionalProperties": false,
                "properties": {
                    "allow": {"type": "array", "items": {"type": "string", "minLength": 1}, "default": []},
                    "deny": {"type": "array", "items": {"type": "string", "minLength": 1}, "default": []},
                    "allowlist_only": {"type": "boolean", "default": false}
                }
            },
            "secretHygiene": {
                "type": "object", "additionalProperties": false,
                "properties": {
                    "enabled": {"type": "boolean", "default": true},
                    "block": {"type": "boolean", "default": true},
                    "markers": {"type": "array", "items": {"type": "string", "minLength": 1}, "default": []}
                }
            },
            "protectedReferences": {
                "type": "object", "additionalProperties": false,
                "properties": {
                    "terms": {"type": "array", "items": {"type": "string", "minLength": 1}, "default": []},
                    "block": {"type": "boolean", "default": true},
                    "case_sensitive": {"type": "boolean", "default": false}
                }
            },
            "contentRedaction": {
                "type": "object", "additionalProperties": false, "required": ["provider"],
                "properties": {
                    "provider": {"type": "string", "minLength": 1},
                    "determinism": {"enum": ["deterministic", "probabilistic"], "default": "deterministic"},
                    "classes": {"type": "array", "items": {"type": "string", "minLength": 1}, "default": []},
                    "reviewed": {"type": "boolean", "default": false}
                }
            },
            "rightsHygiene": {
                "type": "object", "additionalProperties": false,
                "properties": {
                    "required": {"type": "boolean", "default": false},
                    "license": {"type": ["string", "null"]},
                    "rights_holder": {"type": ["string", "null"]},
                    "approval_reference": {"type": ["string", "null"]},
                    "reviewed": {"type": "boolean", "default": false}
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
                    "validators": {"type": "array", "items": {"type": "string"}, "default": []},
                    "failure_mode": {"enum": ["fatal", "branch_local"], "default": "branch_local"},
                    "allow_unavailable": {"type": "boolean", "default": false}
                }
            },
            "printPdfInterior": {
                "type": "object", "additionalProperties": false,
                "required": ["executable", "provider_version", "box_policy", "rotation", "scaling", "color_policy", "max_pages", "max_input_bytes", "max_output_bytes", "timeout_seconds"],
                "properties": {
                    "executable": {"type": "string", "minLength": 1},
                    "provider_version": {"const": "0.6.3"},
                    "box_policy": {"const": "media_bleed_trim_inset"},
                    "rotation": {"const": "none"},
                    "scaling": {"const": "fit"},
                    "color_policy": {"const": "preserve_rgb_gray"},
                    "max_pages": {"type": "integer", "minimum": 1, "maximum": 1000},
                    "max_input_bytes": {"type": "integer", "minimum": 1, "maximum": 536870912},
                    "max_output_bytes": {"type": "integer", "minimum": 1, "maximum": 536870912},
                    "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 3600}
                }
            },
            "fixedLayoutEpub": {
                "type": "object", "additionalProperties": false,
                "required": ["page_progression_direction", "spread", "cover_member_id", "max_pages", "max_input_bytes", "max_output_bytes"],
                "properties": {
                    "page_progression_direction": {"enum": ["ltr", "rtl"]},
                    "spread": {"const": "none"},
                    "cover_member_id": {"$ref": "#/$defs/stableId"},
                    "max_pages": {"type": "integer", "minimum": 1, "maximum": 1000},
                    "max_input_bytes": {"type": "integer", "minimum": 1, "maximum": 536870912},
                    "max_output_bytes": {"type": "integer", "minimum": 1, "maximum": 536870912}
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
                    "reject_loss_classes": {
                        "type": "array",
                        "items": {"enum": ["lossless", "partial", "lossy", "path_dependent", "unknown"]},
                        "uniqueItems": true,
                        "default": []
                    },
                    "publication_policy": {"type": ["string", "null"]},
                    "redaction_policy": {"type": ["string", "null"]},
                    "hygiene_policy": {"anyOf": [{"$ref": "#/$defs/stableId"}, {"type": "null"}]}
                    ,"print_pdf_interior": {"anyOf": [{"$ref": "#/$defs/printPdfInterior"}, {"type": "null"}]}
                    ,"fixed_layout_epub": {"anyOf": [{"$ref": "#/$defs/fixedLayoutEpub"}, {"type": "null"}]}
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
