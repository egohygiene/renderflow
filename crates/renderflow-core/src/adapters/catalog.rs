//! Data-defined provider packs and adopt/adapt/reject evidence.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::toolchain::{
    CapabilityId, ToolAvailabilityStatus, ToolDeterminism, ToolFidelity, ToolId, ToolInventory,
    ToolLocality, ToolRegistry, ToolSupportTier,
};

pub const ADAPTER_CATALOG_SCHEMA_V1: &str = "renderflow.adapter-catalog/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterFamily {
    Documents,
    Pdf,
    Images,
    AudioVideo,
    Ebooks,
    Office,
    Archives,
    Data,
    Ocr,
    Subtitles,
    Publication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterMaturity {
    Integrated,
    Experimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptionDecision {
    Adopt,
    Adapt,
    Reject,
    Defer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterExecutionContract {
    pub service: String,
    #[serde(default)]
    pub direct_argv: bool,
    #[serde(default)]
    pub bounded_output: bool,
    #[serde(default)]
    pub timeout: bool,
    #[serde(default)]
    pub cancellation: bool,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub side_effects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterProviderContract {
    pub id: String,
    pub runtime_tool: ToolId,
    pub name: String,
    pub families: Vec<AdapterFamily>,
    pub maturity: AdapterMaturity,
    pub selection_priority: u16,
    pub capabilities: Vec<CapabilityId>,
    #[serde(default)]
    pub input_media_types: Vec<String>,
    #[serde(default)]
    pub output_media_types: Vec<String>,
    pub determinism: ToolDeterminism,
    pub locality: ToolLocality,
    pub fidelity: ToolFidelity,
    pub execution: AdapterExecutionContract,
    #[serde(default)]
    pub configuration_schema: BTreeMap<String, String>,
    #[serde(default)]
    pub validation: Vec<String>,
    #[serde(default)]
    pub provenance: Vec<String>,
    pub upstream: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterEvaluation {
    pub candidate: String,
    pub families: Vec<AdapterFamily>,
    pub decision: AdoptionDecision,
    pub upstream: String,
    pub rationale: String,
    #[serde(default)]
    pub follow_up: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterCatalogDocument {
    schema: String,
    providers: Vec<AdapterProviderContract>,
    evaluations: Vec<AdapterEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCatalog {
    pub schema: String,
    pub providers: Vec<AdapterProviderContract>,
    pub evaluations: Vec<AdapterEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterProviderStatus {
    pub contract: AdapterProviderContract,
    pub availability: ToolAvailabilityStatus,
    pub selected_executable: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCatalogReport {
    pub schema: String,
    pub providers: Vec<AdapterProviderStatus>,
    pub evaluations: Vec<AdapterEvaluation>,
    pub capabilities: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<AdapterSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterSelectionDecision {
    pub adapter_id: String,
    pub runtime_tool: ToolId,
    pub availability: ToolAvailabilityStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterSelection {
    pub capability: String,
    pub preferred: Vec<String>,
    pub selected: Option<String>,
    pub decisions: Vec<AdapterSelectionDecision>,
}

impl AdapterCatalog {
    pub fn builtins(registry: &ToolRegistry) -> Result<Self> {
        Self::from_yaml(include_str!("../../data/adapter-packs.yaml"), registry)
    }

    pub fn from_yaml(yaml: &str, registry: &ToolRegistry) -> Result<Self> {
        let mut document: AdapterCatalogDocument =
            serde_yaml_ng::from_str(yaml).context("failed to parse adapter catalog YAML")?;
        if document.schema != ADAPTER_CATALOG_SCHEMA_V1 {
            anyhow::bail!(
                "unsupported adapter catalog schema '{}'; expected '{}'",
                document.schema,
                ADAPTER_CATALOG_SCHEMA_V1
            );
        }
        validate_catalog(&document, registry)?;
        for provider in &mut document.providers {
            provider.families.sort();
            provider.families.dedup();
            provider.capabilities.sort();
            provider.capabilities.dedup();
            provider.validation.sort();
            provider.validation.dedup();
            provider.provenance.sort();
            provider.provenance.dedup();
        }
        document
            .providers
            .sort_by(|left, right| left.id.cmp(&right.id));
        document
            .evaluations
            .sort_by(|left, right| left.candidate.cmp(&right.candidate));
        Ok(Self {
            schema: document.schema,
            providers: document.providers,
            evaluations: document.evaluations,
        })
    }

    pub fn providers_for(&self, capability: &str) -> Vec<&AdapterProviderContract> {
        let mut providers = self
            .providers
            .iter()
            .filter(|provider| {
                provider
                    .capabilities
                    .iter()
                    .any(|candidate| candidate.as_str() == capability)
            })
            .collect::<Vec<_>>();
        providers.sort_by(|left, right| {
            maturity_rank(left.maturity)
                .cmp(&maturity_rank(right.maturity))
                .then_with(|| left.selection_priority.cmp(&right.selection_priority))
                .then_with(|| left.id.cmp(&right.id))
        });
        providers
    }

    /// Select an available provider deterministically without hiding fallbacks.
    ///
    /// Explicit preferences win, followed by maturity, declared priority, and
    /// stable adapter id. Every rejected candidate remains visible in evidence.
    pub fn select_for(
        &self,
        capability: &str,
        preferred: &[String],
        inventory: &ToolInventory,
    ) -> AdapterSelection {
        let preference = preferred
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect::<BTreeMap<_, _>>();
        let mut providers = self.providers_for(capability);
        providers.sort_by(|left, right| {
            preference
                .get(left.id.as_str())
                .copied()
                .unwrap_or(usize::MAX)
                .cmp(
                    &preference
                        .get(right.id.as_str())
                        .copied()
                        .unwrap_or(usize::MAX),
                )
                .then_with(|| maturity_rank(left.maturity).cmp(&maturity_rank(right.maturity)))
                .then_with(|| left.selection_priority.cmp(&right.selection_priority))
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut selected = None;
        let mut decisions = Vec::new();
        for provider in providers {
            let availability = inventory
                .get(provider.runtime_tool.as_str())
                .map(|tool| tool.status)
                .unwrap_or(ToolAvailabilityStatus::UnknownProvider);
            let eligible = selected.is_none() && availability == ToolAvailabilityStatus::Available;
            if eligible {
                selected = Some(provider.id.clone());
            }
            decisions.push(AdapterSelectionDecision {
                adapter_id: provider.id.clone(),
                runtime_tool: provider.runtime_tool.clone(),
                availability,
                reason: if eligible {
                    "selected by explicit preference/maturity/priority order".to_string()
                } else if availability != ToolAvailabilityStatus::Available {
                    format!("provider unavailable: {}", availability.as_str())
                } else {
                    "available fallback ranked after selected provider".to_string()
                },
            });
        }
        AdapterSelection {
            capability: capability.to_string(),
            preferred: preferred.to_vec(),
            selected,
            decisions,
        }
    }

    pub fn report(
        &self,
        inventory: &ToolInventory,
        capability: Option<&str>,
        preferred: &[String],
        available_only: bool,
    ) -> AdapterCatalogReport {
        let mut providers = self
            .providers
            .iter()
            .filter(|provider| {
                capability.is_none_or(|capability| {
                    provider
                        .capabilities
                        .iter()
                        .any(|candidate| candidate.as_str() == capability)
                })
            })
            .filter_map(|contract| {
                let availability = inventory.get(contract.runtime_tool.as_str())?;
                if available_only && !availability.is_available() {
                    return None;
                }
                Some(AdapterProviderStatus {
                    contract: contract.clone(),
                    availability: availability.status,
                    selected_executable: availability.selected_executable.clone(),
                    version: availability
                        .normalized_version
                        .clone()
                        .or_else(|| availability.version_line.clone()),
                })
            })
            .collect::<Vec<_>>();
        providers.sort_by(|left, right| {
            maturity_rank(left.contract.maturity)
                .cmp(&maturity_rank(right.contract.maturity))
                .then_with(|| {
                    left.contract
                        .selection_priority
                        .cmp(&right.contract.selection_priority)
                })
                .then_with(|| left.contract.id.cmp(&right.contract.id))
        });
        let mut capabilities = BTreeMap::<String, Vec<String>>::new();
        for provider in &providers {
            for capability in &provider.contract.capabilities {
                capabilities
                    .entry(capability.to_string())
                    .or_default()
                    .push(provider.contract.id.clone());
            }
        }
        for ids in capabilities.values_mut() {
            ids.sort();
        }
        AdapterCatalogReport {
            schema: self.schema.clone(),
            providers,
            evaluations: self.evaluations.clone(),
            capabilities,
            selection: capability
                .map(|capability| self.select_for(capability, preferred, inventory)),
        }
    }
}

fn maturity_rank(maturity: AdapterMaturity) -> u8 {
    match maturity {
        AdapterMaturity::Integrated => 0,
        AdapterMaturity::Experimental => 1,
    }
}

fn validate_catalog(document: &AdapterCatalogDocument, registry: &ToolRegistry) -> Result<()> {
    let mut provider_ids = HashSet::new();
    for provider in &document.providers {
        if provider.id.trim().is_empty() || !provider.id.starts_with("adapter.") {
            anyhow::bail!("adapter id '{}' must begin with 'adapter.'", provider.id);
        }
        if !provider_ids.insert(provider.id.clone()) {
            anyhow::bail!("duplicate adapter id '{}'", provider.id);
        }
        let tool = registry
            .get(provider.runtime_tool.as_str())
            .with_context(|| {
                format!(
                    "adapter '{}' references unknown runtime tool '{}'",
                    provider.id, provider.runtime_tool
                )
            })?;
        if provider.families.is_empty() || provider.capabilities.is_empty() {
            anyhow::bail!(
                "adapter '{}' must declare families and capabilities",
                provider.id
            );
        }
        if provider.execution.service != "renderflow.process/v1"
            || !provider.execution.direct_argv
            || !provider.execution.bounded_output
            || !provider.execution.timeout
            || !provider.execution.cancellation
        {
            anyhow::bail!(
                "adapter '{}' must use the complete renderflow.process/v1 bounded direct-argv contract",
                provider.id
            );
        }
        if provider.validation.is_empty() && provider.maturity == AdapterMaturity::Integrated {
            anyhow::bail!(
                "integrated adapter '{}' requires validation evidence",
                provider.id
            );
        }
        if provider.maturity == AdapterMaturity::Integrated
            && tool.support_tier == ToolSupportTier::Experimental
        {
            anyhow::bail!(
                "integrated adapter '{}' cannot use experimental tool '{}'",
                provider.id,
                tool.id
            );
        }
        let tool_capabilities = tool
            .capabilities
            .iter()
            .map(|capability| capability.as_str())
            .collect::<BTreeSet<_>>();
        for capability in &provider.capabilities {
            if !tool_capabilities.contains(capability.as_str()) {
                anyhow::bail!(
                    "adapter '{}' declares capability '{}' absent from tool '{}'",
                    provider.id,
                    capability,
                    tool.id
                );
            }
        }
    }
    let mut evaluated = HashSet::new();
    for evaluation in &document.evaluations {
        if evaluation.candidate.trim().is_empty() || !evaluated.insert(&evaluation.candidate) {
            anyhow::bail!("adapter evaluations require unique non-empty candidate names");
        }
        if evaluation.families.is_empty() || evaluation.rationale.trim().is_empty() {
            anyhow::bail!(
                "adapter evaluation '{}' requires a family and rationale",
                evaluation.candidate
            );
        }
    }
    Ok(())
}
