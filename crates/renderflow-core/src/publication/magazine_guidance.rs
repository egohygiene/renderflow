//! Deterministic magazine guidance derived from validated visual/layout Artifact DNA.
//!
//! This module is deliberately publication-content agnostic. It turns sanitized
//! Artifact DNA into reviewable asset-brief and metadata candidates; it never
//! writes or rewrites comic or magazine prose.

use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dna::{
    ArtifactDna, DnaHygieneStatus, DnaModality, DnaReviewState, DnaValidationStatus,
    ARTIFACT_DNA_SCHEMA_V1,
};
use crate::evidence::{sha256_serialized, DigestEvidence};

pub const MAGAZINE_GUIDANCE_SCHEMA_V1: &str = "renderflow.magazine-guidance/v1";
pub const MAGAZINE_GUIDANCE_POLICY_V1: &str = "policy.magazine-guidance.public-safe/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagazineCandidateState {
    Candidate,
    Approved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineGuidanceApproval {
    pub state: MagazineCandidateState,
    pub human_review_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineGuidanceValidation {
    pub schema_valid: bool,
    pub dna_valid: bool,
    pub hygiene_passed: bool,
    pub publication_content_generated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineAssetBriefCandidate {
    pub role: String,
    pub purpose: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layout_guidance: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette_guidance: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub typography_guidance: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accessibility_guidance: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub negative_guidance: Vec<String>,
    pub review_required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineMetadataCandidate {
    pub source_artifact_id: String,
    pub source_media_type: String,
    pub source_format: String,
    pub evidence_summary: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    pub review_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineAiEnrichmentPolicy {
    pub enabled: bool,
    pub allow_remote: bool,
    pub source_approved_for_ai: bool,
    pub privacy_approved_for_remote: bool,
    pub protected_reference_policy: String,
    pub copyright_policy: String,
    pub prompt_hygiene_policy: String,
}

impl Default for MagazineAiEnrichmentPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_remote: false,
            source_approved_for_ai: false,
            privacy_approved_for_remote: false,
            protected_reference_policy: "policy.ai.protected-reference-safe/v1".to_string(),
            copyright_policy: "policy.publication.rights-review/v1".to_string(),
            prompt_hygiene_policy: "policy.ai.prompt-hygiene/v1".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineAiSkillCandidate {
    pub skill_id: String,
    pub skill_version: String,
    pub purpose: String,
    pub state: MagazineCandidateState,
    pub local_first: bool,
    pub remote_opt_in_required: bool,
    pub input_digest: DigestEvidence,
    pub settings_digest: DigestEvidence,
    pub policy_digest: DigestEvidence,
    pub validation_status: String,
    pub approval_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineGuidanceProvenance {
    pub source_digest: DigestEvidence,
    pub dna_digest: DigestEvidence,
    pub dna_policy_id: String,
    pub dna_policy_digest: DigestEvidence,
    pub dna_configuration_digest: DigestEvidence,
    pub guidance_policy_id: String,
    pub guidance_policy_digest: DigestEvidence,
    pub generator: String,
    pub generator_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineGuidanceBundle {
    pub schema_version: String,
    pub asset_brief: MagazineAssetBriefCandidate,
    pub metadata: MagazineMetadataCandidate,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ai_candidates: Vec<MagazineAiSkillCandidate>,
    pub provenance: MagazineGuidanceProvenance,
    pub validation: MagazineGuidanceValidation,
    pub approval: MagazineGuidanceApproval,
}

impl MagazineGuidanceBundle {
    pub fn from_dna(dna: &ArtifactDna) -> Result<Self> {
        dna.validate()?;
        if dna.schema_version != ARTIFACT_DNA_SCHEMA_V1 {
            anyhow::bail!("unsupported Artifact DNA schema for magazine guidance");
        }
        if !dna
            .modalities
            .iter()
            .any(|modality| matches!(modality, DnaModality::Visual | DnaModality::Layout))
        {
            anyhow::bail!("magazine guidance requires visual or layout Artifact DNA");
        }
        if matches!(dna.validation.status, DnaValidationStatus::Blocked)
            || matches!(dna.hygiene.status, DnaHygieneStatus::Blocked)
        {
            anyhow::bail!("blocked Artifact DNA cannot be consumed by the magazine profile");
        }

        let mut layout_guidance = Vec::new();
        let mut palette_guidance = Vec::new();
        let mut typography_guidance = Vec::new();
        let mut accessibility_guidance = Vec::new();
        let mut evidence_summary = BTreeMap::new();
        let mut keywords = Vec::new();

        for observation in &dna.observations {
            if !matches!(observation.modality, DnaModality::Visual | DnaModality::Layout) {
                continue;
            }
            evidence_summary.insert(observation.dimension.clone(), observation.value.clone());
            let rendered = render_guidance_value(&observation.dimension, &observation.value);
            if observation.dimension.starts_with("visual.palette") {
                palette_guidance.push(rendered);
                keywords.push("palette".to_string());
            } else if observation.dimension.contains("typography") {
                typography_guidance.push(rendered);
                accessibility_guidance.push(
                    "Preserve readable hierarchy, sufficient contrast, and meaningful alternatives for text carried by artwork."
                        .to_string(),
                );
                keywords.push("typography".to_string());
            } else if observation.dimension.starts_with("layout.") {
                layout_guidance.push(rendered);
                keywords.push("layout".to_string());
            } else if observation.dimension.starts_with("visual.") {
                layout_guidance.push(rendered);
                keywords.push("visual".to_string());
            }
        }
        keywords.sort();
        keywords.dedup();
        layout_guidance.sort();
        palette_guidance.sort();
        typography_guidance.sort();
        accessibility_guidance.sort();
        accessibility_guidance.dedup();

        let asset_brief = MagazineAssetBriefCandidate {
            role: "publication.assets.reference-guidance".to_string(),
            purpose: "Reusable visual/layout guidance for original magazine assets; not publication copy and not a final artwork request."
                .to_string(),
            layout_guidance,
            palette_guidance,
            typography_guidance,
            accessibility_guidance,
            negative_guidance: vec![
                "Do not imitate a named creator, artist, brand, franchise, or protected work."
                    .to_string(),
                "Do not reproduce verbatim source text or source artwork content.".to_string(),
                "Do not treat descriptive similarity evidence as rights clearance.".to_string(),
            ],
            review_required: true,
        };
        let metadata = MagazineMetadataCandidate {
            source_artifact_id: dna.source.artifact_id.clone(),
            source_media_type: dna.source.media_type.clone(),
            source_format: dna.source.format.clone(),
            evidence_summary,
            keywords,
            review_required: true,
        };

        let dna_digest = sha256_serialized(dna)?;
        let policy = MagazineAiEnrichmentPolicy::default();
        let guidance_policy_digest = sha256_serialized(&policy)?;
        let requires_review = dna.approval.human_review_required
            || dna.approval.state != DnaReviewState::Approved
            || true;
        let mut bundle = Self {
            schema_version: MAGAZINE_GUIDANCE_SCHEMA_V1.to_string(),
            asset_brief,
            metadata,
            ai_candidates: Vec::new(),
            provenance: MagazineGuidanceProvenance {
                source_digest: dna.source.digest.clone(),
                dna_digest,
                dna_policy_id: dna.provenance.policy_id.clone(),
                dna_policy_digest: dna.provenance.policy_digest.clone(),
                dna_configuration_digest: dna.provenance.configuration_digest.clone(),
                guidance_policy_id: MAGAZINE_GUIDANCE_POLICY_V1.to_string(),
                guidance_policy_digest,
                generator: "renderflow.magazine-guidance".to_string(),
                generator_version: env!("CARGO_PKG_VERSION").to_string(),
            },
            validation: MagazineGuidanceValidation {
                schema_valid: true,
                dna_valid: true,
                hygiene_passed: true,
                publication_content_generated: false,
                diagnostics: Vec::new(),
            },
            approval: MagazineGuidanceApproval {
                state: MagazineCandidateState::Candidate,
                human_review_required: requires_review,
                approval_reference: None,
            },
        };
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn plan_ai_candidates(
        &mut self,
        registry: &crate::ai::AiSkillRegistry,
        policy: &MagazineAiEnrichmentPolicy,
    ) -> Result<()> {
        if !policy.enabled {
            self.ai_candidates.clear();
            return Ok(());
        }
        if policy.allow_remote && !policy.privacy_approved_for_remote {
            anyhow::bail!("remote AI requires explicit privacy approval");
        }
        if !policy.source_approved_for_ai {
            anyhow::bail!("AI enrichment requires explicit source approval");
        }

        let input = serde_json::json!({
            "dna_digest": self.provenance.dna_digest,
            "asset_brief": self.asset_brief,
            "metadata": self.metadata,
        });
        let input_digest = sha256_serialized(&input)?;
        let policy_digest = sha256_serialized(policy)?;
        let mut candidates = Vec::new();
        for skill_id in [
            "skill.prompt.from-sanitized-dna",
            "skill.accessibility.describe-candidate",
            "skill.metadata.extract",
        ] {
            let Some(skill) = registry.get(skill_id, None) else {
                continue;
            };
            candidates.push(MagazineAiSkillCandidate {
                skill_id: skill.id.clone(),
                skill_version: skill.version.clone(),
                purpose: skill.purpose.clone(),
                state: MagazineCandidateState::Candidate,
                local_first: true,
                remote_opt_in_required: skill.budgets.remote_execution,
                input_digest: input_digest.clone(),
                settings_digest: sha256_serialized(&skill.generation)?,
                policy_digest: policy_digest.clone(),
                validation_status: "planned_schema_bound_candidate".to_string(),
                approval_required: true,
                provider_id: None,
                model_id: None,
            });
        }
        self.ai_candidates = candidates;
        self.provenance.guidance_policy_digest = policy_digest;
        self.validate()?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != MAGAZINE_GUIDANCE_SCHEMA_V1 {
            anyhow::bail!("unsupported magazine guidance schema");
        }
        if self.asset_brief.role.trim().is_empty() || self.asset_brief.purpose.trim().is_empty() {
            anyhow::bail!("magazine asset brief requires role and purpose");
        }
        if !self.asset_brief.review_required || !self.metadata.review_required {
            anyhow::bail!("magazine guidance outputs must remain review-required candidates");
        }
        if self.validation.publication_content_generated {
            anyhow::bail!("magazine guidance must not generate publication content");
        }
        if self.approval.state == MagazineCandidateState::Approved
            && self.approval.approval_reference.as_deref().is_none_or(str::is_empty)
        {
            anyhow::bail!("approved magazine guidance requires an approval reference");
        }
        if self.ai_candidates.iter().any(|candidate| {
            candidate.state != MagazineCandidateState::Candidate || !candidate.approval_required
        }) {
            anyhow::bail!("AI-derived magazine guidance must remain candidate-only");
        }
        Ok(())
    }
}

fn render_guidance_value(dimension: &str, value: &Value) -> String {
    match value {
        Value::String(value) => format!("{dimension}: {value}"),
        Value::Number(value) => format!("{dimension}: {value}"),
        _ => format!("{dimension}: {}", serde_json::to_string(value).unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dna::{
        DnaApproval, DnaDeterminism, DnaEvidenceOrigin, DnaExtractorEvidence, DnaHygieneEvidence,
        DnaObservation, DnaObservationEvidence, DnaProvenance, DnaProviderLocality,
        DnaSimilarityGuidance, DnaSourceReference, DnaValidation,
    };

    fn fixture_dna() -> ArtifactDna {
        let digest = DigestEvidence {
            algorithm: "sha256".to_string(),
            value: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        };
        ArtifactDna {
            schema_version: ARTIFACT_DNA_SCHEMA_V1.to_string(),
            source: DnaSourceReference {
                artifact_id: format!("sha256:{}", digest.value),
                digest: digest.clone(),
                format: "svg".to_string(),
                media_type: "image/svg+xml".to_string(),
                size_bytes: 123,
            },
            modalities: vec![DnaModality::Visual, DnaModality::Layout],
            observations: vec![
                DnaObservation {
                    id: "layout.aspect".to_string(),
                    modality: DnaModality::Layout,
                    dimension: "layout.canvas.aspect_ratio".to_string(),
                    value: serde_json::json!(0.75),
                    description: None,
                    evidence: DnaObservationEvidence {
                        origin: DnaEvidenceOrigin::ProviderObserved,
                        provider_id: "renderflow.builtin.artifact-dna".to_string(),
                        provider_version: "1".to_string(),
                        determinism: DnaDeterminism::Deterministic,
                        confidence: 1.0,
                        scope: BTreeMap::new(),
                    },
                    similarity_eligible: true,
                },
                DnaObservation {
                    id: "visual.palette".to_string(),
                    modality: DnaModality::Visual,
                    dimension: "visual.palette.hex".to_string(),
                    value: serde_json::json!(["#111111", "#eeeeee"]),
                    description: None,
                    evidence: DnaObservationEvidence {
                        origin: DnaEvidenceOrigin::ProviderObserved,
                        provider_id: "renderflow.builtin.artifact-dna".to_string(),
                        provider_version: "1".to_string(),
                        determinism: DnaDeterminism::Deterministic,
                        confidence: 1.0,
                        scope: BTreeMap::new(),
                    },
                    similarity_eligible: true,
                },
            ],
            provider_extensions: BTreeMap::new(),
            omissions: Vec::new(),
            similarity_guidance: DnaSimilarityGuidance {
                purpose: "original related assets".to_string(),
                direct_imitation_allowed: false,
                legal_clearance_claimed: false,
                dimension_weights: BTreeMap::new(),
                excluded_dimensions: Vec::new(),
            },
            hygiene: DnaHygieneEvidence {
                policy_id: "policy.artifact-dna.public-safe/v1".to_string(),
                status: DnaHygieneStatus::Passed,
                raw_payload_retained: false,
                identifying_metadata_retained: false,
                findings: Vec::new(),
            },
            provenance: DnaProvenance {
                source_digest: digest.clone(),
                extractors: vec![DnaExtractorEvidence {
                    provider_id: "renderflow.builtin.artifact-dna".to_string(),
                    provider_version: "1".to_string(),
                    locality: DnaProviderLocality::Local,
                    determinism: DnaDeterminism::Deterministic,
                    ai_assisted: false,
                }],
                policy_id: "policy.artifact-dna.public-safe/v1".to_string(),
                policy_digest: digest.clone(),
                configuration_digest: digest,
            },
            approval: DnaApproval {
                state: DnaReviewState::Candidate,
                human_review_required: false,
                approval_reference: None,
            },
            validation: DnaValidation {
                status: DnaValidationStatus::Valid,
                schema_valid: true,
                diagnostics: Vec::new(),
            },
        }
    }

    #[test]
    fn deterministic_guidance_is_useful_without_ai() {
        let bundle = MagazineGuidanceBundle::from_dna(&fixture_dna()).unwrap();
        assert!(!bundle.asset_brief.layout_guidance.is_empty());
        assert!(!bundle.asset_brief.palette_guidance.is_empty());
        assert!(bundle.ai_candidates.is_empty());
        assert!(!bundle.validation.publication_content_generated);
        assert_eq!(bundle.approval.state, MagazineCandidateState::Candidate);
    }

    #[test]
    fn ai_planning_is_explicit_and_candidate_only() {
        let registry = crate::ai::AiSkillRegistry::bundled().unwrap();
        let mut bundle = MagazineGuidanceBundle::from_dna(&fixture_dna()).unwrap();
        bundle
            .plan_ai_candidates(
                &registry,
                &MagazineAiEnrichmentPolicy {
                    enabled: true,
                    source_approved_for_ai: true,
                    ..MagazineAiEnrichmentPolicy::default()
                },
            )
            .unwrap();
        assert!(!bundle.ai_candidates.is_empty());
        assert!(bundle.ai_candidates.iter().all(|candidate| {
            candidate.state == MagazineCandidateState::Candidate && candidate.approval_required
        }));
    }

    #[test]
    fn remote_ai_requires_explicit_privacy_approval() {
        let registry = crate::ai::AiSkillRegistry::bundled().unwrap();
        let mut bundle = MagazineGuidanceBundle::from_dna(&fixture_dna()).unwrap();
        let error = bundle
            .plan_ai_candidates(
                &registry,
                &MagazineAiEnrichmentPolicy {
                    enabled: true,
                    allow_remote: true,
                    source_approved_for_ai: true,
                    ..MagazineAiEnrichmentPolicy::default()
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("privacy approval"));
    }
}
