//! Versioned provider/model compatibility catalog and deterministic resolver.

use std::cmp::Ordering;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const AI_MODEL_CATALOG_SCHEMA_V1: &str = "renderflow.ai-model-catalog/v1";
pub const AI_RESOLUTION_SCHEMA_V1: &str = "renderflow.ai-resolution/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiLocality {
    Local,
    Remote,
    Hybrid,
}

impl fmt::Display for AiLocality {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::Hybrid => "hybrid",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiModality {
    Text,
    Image,
    Audio,
    Video,
    Document,
    StructuredJson,
    ArtifactDna,
    Embeddings,
    Mask,
    Metadata,
}

impl fmt::Display for AiModality {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Document => "document",
            Self::StructuredJson => "structured_json",
            Self::ArtifactDna => "artifact_dna",
            Self::Embeddings => "embeddings",
            Self::Mask => "mask",
            Self::Metadata => "metadata",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiOperation {
    Generation,
    Editing,
    Extraction,
    Classification,
    Transcription,
    Embeddings,
    MultimodalReasoning,
    SchemaConstrainedOutput,
}

impl fmt::Display for AiOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Generation => "generation",
            Self::Editing => "editing",
            Self::Extraction => "extraction",
            Self::Classification => "classification",
            Self::Transcription => "transcription",
            Self::Embeddings => "embeddings",
            Self::MultimodalReasoning => "multimodal_reasoning",
            Self::SchemaConstrainedOutput => "schema_constrained_output",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiDeterminism {
    ByteDeterministic,
    ConfigurationRepeatable,
    Probabilistic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAvailability {
    Available,
    Unavailable,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiRuntimeDescriptor {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hardware_requirements: Vec<String>,
    pub availability_probe: AiAvailabilityProbe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiAvailabilityProbe {
    pub kind: String,
    pub bounded_timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiModelLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_images: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_audio_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_video_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiModelFeatures {
    pub native_json: bool,
    pub json_schema: bool,
    pub seed: bool,
    pub sampler_controls: bool,
    pub streaming: bool,
    pub batch: bool,
    pub tool_use: bool,
    pub multi_input: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiLicenseEvidence {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_terms: Option<String>,
    pub commercial_use: String,
    pub human_review_required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiAdvisoryHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_tier: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_tier: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_tier: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiModelCatalogEntry {
    pub id: String,
    pub family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_digest: Option<String>,
    pub input_modalities: Vec<AiModality>,
    pub output_modalities: Vec<AiModality>,
    pub operations: Vec<AiOperation>,
    #[serde(default)]
    pub limits: AiModelLimits,
    #[serde(default)]
    pub features: AiModelFeatures,
    pub determinism: AiDeterminism,
    pub license: AiLicenseEvidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commercial_constraints: Vec<String>,
    #[serde(default)]
    pub advisory: AiAdvisoryHints,
    pub maturity: String,
    pub conformance: String,
    pub availability: AiAvailability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_reason: Option<String>,
    pub required_provenance_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiProviderCatalogEntry {
    pub id: String,
    pub adapter: String,
    pub display_name: String,
    pub locality: AiLocality,
    pub requires_network_permission: bool,
    pub runtime: AiRuntimeDescriptor,
    pub models: Vec<AiModelCatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiModelCatalog {
    pub schema_version: String,
    pub revision: String,
    pub providers: Vec<AiProviderCatalogEntry>,
}

impl AiModelCatalog {
    pub fn bundled() -> Result<Self> {
        Self::from_json(include_str!("../../data/ai/model-catalog-v1.json"))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read AI model catalog '{}'", path.display()))?;
        Self::from_json(&contents)
            .with_context(|| format!("invalid AI model catalog '{}'", path.display()))
    }

    pub fn from_json(contents: &str) -> Result<Self> {
        let catalog: Self = serde_json::from_str(contents).context("catalog is not valid JSON")?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != AI_MODEL_CATALOG_SCHEMA_V1 {
            anyhow::bail!(
                "unsupported AI model catalog schema '{}'; expected '{}'",
                self.schema_version,
                AI_MODEL_CATALOG_SCHEMA_V1
            );
        }
        if self.revision.trim().is_empty() {
            anyhow::bail!("AI model catalog revision must not be empty");
        }
        let mut provider_ids = std::collections::BTreeSet::new();
        for provider in &self.providers {
            validate_stable_id("provider", &provider.id)?;
            if !provider_ids.insert(provider.id.as_str()) {
                anyhow::bail!("duplicate AI provider id '{}'", provider.id);
            }
            if provider.models.is_empty() {
                anyhow::bail!(
                    "AI provider '{}' must declare at least one model",
                    provider.id
                );
            }
            let mut model_ids = std::collections::BTreeSet::new();
            for model in &provider.models {
                validate_stable_id("model", &model.id)?;
                if !model_ids.insert(model.id.as_str()) {
                    anyhow::bail!(
                        "duplicate AI model id '{}' for provider '{}'",
                        model.id,
                        provider.id
                    );
                }
                if model.input_modalities.is_empty()
                    || model.output_modalities.is_empty()
                    || model.operations.is_empty()
                {
                    anyhow::bail!(
                        "AI model '{}:{}' must declare model-specific modalities and operations",
                        provider.id,
                        model.id
                    );
                }
                if model.required_provenance_fields.is_empty() {
                    anyhow::bail!(
                        "AI model '{}:{}' must declare required provenance fields",
                        provider.id,
                        model.id
                    );
                }
            }
        }
        Ok(())
    }

    pub fn resolve(&self, request: &AiResolutionRequest) -> AiResolutionReport {
        let mut assessments = Vec::new();
        for provider in &self.providers {
            for model in &provider.models {
                assessments.push(assess(provider, model, request));
            }
        }

        let mut compatible: Vec<usize> = assessments
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| candidate.reasons.is_empty().then_some(index))
            .collect();
        compatible.sort_by(|left, right| {
            rank_candidate(
                &assessments[*left],
                &assessments[*right],
                request.preference,
            )
        });

        let selected = compatible.first().map(|index| {
            let candidate = &mut assessments[*index];
            candidate.status = AiCandidateStatus::Selected;
            candidate.reasons.push(AiResolutionReason {
                code: "resolver.selected".to_string(),
                message: format!("Selected by deterministic '{}' ranking", request.preference),
            });
            AiModelSelection {
                provider_id: candidate.provider_id.clone(),
                adapter: candidate.adapter.clone(),
                runtime_id: candidate.runtime_id.clone(),
                runtime_revision: candidate.runtime_revision.clone(),
                runtime_digest: candidate.runtime_digest.clone(),
                model_id: candidate.model_id.clone(),
                model_revision: candidate.model_revision.clone(),
                weights_digest: candidate.weights_digest.clone(),
                locality: candidate.locality,
                availability: candidate.availability,
                determinism: candidate.determinism,
                execution_ready: candidate.availability == AiAvailability::Available,
            }
        });

        for index in compatible.into_iter().skip(1) {
            assessments[index].status = AiCandidateStatus::Compatible;
            assessments[index].reasons.push(AiResolutionReason {
                code: "resolver.compatible_not_selected".to_string(),
                message: "Compatible candidate ranked below the selected model".to_string(),
            });
        }

        AiResolutionReport {
            schema_version: AI_RESOLUTION_SCHEMA_V1.to_string(),
            catalog_revision: self.revision.clone(),
            request: request.clone(),
            selected,
            candidates: assessments,
        }
    }
}

fn validate_stable_id(kind: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        anyhow::bail!("{kind} id '{value}' must use only ASCII letters, digits, '.', '_' or '-'");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiExecutionPreferenceV1 {
    LocalOnly,
    RemoteOnly,
    #[default]
    LocalPreferred,
    LowestCost,
    HighestQuality,
}

impl fmt::Display for AiExecutionPreferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LocalOnly => "local-only",
            Self::RemoteOnly => "remote-only",
            Self::LocalPreferred => "local-preferred",
            Self::LowestCost => "lowest-cost",
            Self::HighestQuality => "highest-quality",
        })
    }
}

impl FromStr for AiExecutionPreferenceV1 {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "local-only" => Ok(Self::LocalOnly),
            "remote-only" => Ok(Self::RemoteOnly),
            "local-preferred" => Ok(Self::LocalPreferred),
            "lowest-cost" => Ok(Self::LowestCost),
            "highest-quality" => Ok(Self::HighestQuality),
            _ => anyhow::bail!(
                "unknown AI execution preference '{value}'; expected local-only, remote-only, local-preferred, lowest-cost, or highest-quality"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiResolutionRequest {
    pub operations: Vec<AiOperation>,
    pub input_modalities: Vec<AiModality>,
    pub output_modalities: Vec<AiModality>,
    pub requires_json_schema: bool,
    #[serde(default)]
    pub preference: AiExecutionPreferenceV1,
    #[serde(default)]
    pub allow_remote: bool,
    #[serde(default)]
    pub remote_policy_allows: bool,
    #[serde(default)]
    pub allow_unverified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiCandidateStatus {
    Selected,
    Compatible,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiResolutionReason {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiCandidateAssessment {
    pub provider_id: String,
    pub adapter: String,
    pub runtime_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_digest: Option<String>,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights_digest: Option<String>,
    pub locality: AiLocality,
    pub availability: AiAvailability,
    pub determinism: AiDeterminism,
    pub status: AiCandidateStatus,
    pub advisory: AiAdvisoryHints,
    pub reasons: Vec<AiResolutionReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiModelSelection {
    pub provider_id: String,
    pub adapter: String,
    pub runtime_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_digest: Option<String>,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights_digest: Option<String>,
    pub locality: AiLocality,
    pub availability: AiAvailability,
    pub determinism: AiDeterminism,
    pub execution_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiResolutionReport {
    pub schema_version: String,
    pub catalog_revision: String,
    pub request: AiResolutionRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<AiModelSelection>,
    pub candidates: Vec<AiCandidateAssessment>,
}

fn assess(
    provider: &AiProviderCatalogEntry,
    model: &AiModelCatalogEntry,
    request: &AiResolutionRequest,
) -> AiCandidateAssessment {
    let mut reasons = Vec::new();
    let remote = provider.locality == AiLocality::Remote;
    if remote && !request.allow_remote {
        reasons.push(reason(
            "policy.remote_not_approved",
            "Remote execution requires explicit --allow-remote approval",
        ));
    }
    if (remote || provider.requires_network_permission) && !request.remote_policy_allows {
        reasons.push(reason(
            "policy.skill_remote_forbidden",
            "The selected AI skill forbids network or remote execution",
        ));
    }
    match request.preference {
        AiExecutionPreferenceV1::LocalOnly if remote => reasons.push(reason(
            "policy.local_only",
            "Local-only policy forbids this remote model",
        )),
        AiExecutionPreferenceV1::RemoteOnly if provider.locality == AiLocality::Local => reasons
            .push(reason(
                "policy.remote_only",
                "Remote-only policy excludes this local model",
            )),
        _ => {}
    }
    for operation in &request.operations {
        if !model.operations.contains(operation) {
            reasons.push(reason(
                "capability.operation_missing",
                &format!("Model does not support operation '{operation}'"),
            ));
        }
    }
    for modality in &request.input_modalities {
        if !model.input_modalities.contains(modality) {
            reasons.push(reason(
                "capability.input_modality_missing",
                &format!("Model does not accept '{modality}' input"),
            ));
        }
    }
    for modality in &request.output_modalities {
        if !model.output_modalities.contains(modality) {
            reasons.push(reason(
                "capability.output_modality_missing",
                &format!("Model does not produce '{modality}' output"),
            ));
        }
    }
    if request.requires_json_schema && !model.features.json_schema {
        reasons.push(reason(
            "capability.json_schema_missing",
            "Model adapter does not support schema-constrained output",
        ));
    }
    match model.availability {
        AiAvailability::Unavailable => reasons.push(reason(
            "availability.unavailable",
            model
                .availability_reason
                .as_deref()
                .unwrap_or("Model is unavailable"),
        )),
        AiAvailability::Unverified if !request.allow_unverified => reasons.push(reason(
            "availability.unverified",
            model
                .availability_reason
                .as_deref()
                .unwrap_or("Model availability has not been verified"),
        )),
        _ => {}
    }
    AiCandidateAssessment {
        provider_id: provider.id.clone(),
        adapter: provider.adapter.clone(),
        runtime_id: provider.runtime.id.clone(),
        runtime_revision: provider.runtime.revision.clone(),
        runtime_digest: provider.runtime.digest.clone(),
        model_id: model.id.clone(),
        model_revision: model.revision.clone(),
        weights_digest: model.weights_digest.clone(),
        locality: provider.locality,
        availability: model.availability,
        determinism: model.determinism,
        status: AiCandidateStatus::Rejected,
        advisory: model.advisory.clone(),
        reasons,
    }
}

fn reason(code: &str, message: &str) -> AiResolutionReason {
    AiResolutionReason {
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn rank_candidate(
    left: &AiCandidateAssessment,
    right: &AiCandidateAssessment,
    preference: AiExecutionPreferenceV1,
) -> Ordering {
    let locality_rank = |locality| match locality {
        AiLocality::Local => 0_u8,
        AiLocality::Hybrid => 1,
        AiLocality::Remote => 2,
    };
    let ordering = match preference {
        AiExecutionPreferenceV1::LowestCost => left
            .advisory
            .cost_tier
            .unwrap_or(u8::MAX)
            .cmp(&right.advisory.cost_tier.unwrap_or(u8::MAX)),
        AiExecutionPreferenceV1::HighestQuality => right
            .advisory
            .quality_tier
            .unwrap_or(0)
            .cmp(&left.advisory.quality_tier.unwrap_or(0)),
        AiExecutionPreferenceV1::LocalOnly | AiExecutionPreferenceV1::LocalPreferred => {
            locality_rank(left.locality).cmp(&locality_rank(right.locality))
        }
        AiExecutionPreferenceV1::RemoteOnly => Ordering::Equal,
    };
    ordering
        .then(left.provider_id.cmp(&right.provider_id))
        .then(left.model_id.cmp(&right.model_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_valid_and_model_specific() {
        let catalog = AiModelCatalog::bundled().unwrap();
        assert_eq!(catalog.schema_version, AI_MODEL_CATALOG_SCHEMA_V1);
        assert!(catalog.providers.iter().all(|provider| provider
            .models
            .iter()
            .all(|model| !model.operations.is_empty())));
    }

    #[test]
    fn local_only_never_falls_back_to_remote() {
        let catalog = AiModelCatalog::bundled().unwrap();
        let report = catalog.resolve(&AiResolutionRequest {
            operations: vec![AiOperation::SchemaConstrainedOutput],
            input_modalities: vec![AiModality::Text],
            output_modalities: vec![AiModality::StructuredJson],
            requires_json_schema: true,
            preference: AiExecutionPreferenceV1::LocalOnly,
            allow_remote: true,
            remote_policy_allows: true,
            allow_unverified: true,
        });
        assert!(report
            .candidates
            .iter()
            .filter(|candidate| candidate.locality == AiLocality::Remote)
            .all(|candidate| candidate.status == AiCandidateStatus::Rejected));
    }

    #[test]
    fn remote_models_require_explicit_permission() {
        let catalog = AiModelCatalog::bundled().unwrap();
        let report = catalog.resolve(&AiResolutionRequest {
            operations: vec![AiOperation::Generation],
            input_modalities: vec![AiModality::Text],
            output_modalities: vec![AiModality::Text],
            requires_json_schema: false,
            preference: AiExecutionPreferenceV1::RemoteOnly,
            allow_remote: false,
            remote_policy_allows: true,
            allow_unverified: true,
        });
        assert!(report.selected.is_none());
        assert!(report.candidates.iter().any(|candidate| candidate
            .reasons
            .iter()
            .any(|reason| reason.code == "policy.remote_not_approved")));
    }
}
