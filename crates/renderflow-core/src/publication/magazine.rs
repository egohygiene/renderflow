//! Deterministic, candidate-only magazine guidance derived from Artifact DNA.
//!
//! The compiler consumes only validated visual/layout observations. Optional
//! model assistance is prepared as a request for the provider-neutral AI skill
//! runtime and remains a distinct, review-required candidate.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::ai::{
    AiCandidateState, AiExecutionEvidence, AiExecutionPreferenceV1, AiInputArtifactEvidence,
    AiProtectedReferenceRule, AiSkillExecutionOutcome, AiSkillExecutionRequest,
};
use crate::artifact::ArtifactStore;
use crate::dna::{
    ArtifactDna, DnaExtractorEvidence, DnaHygieneStatus, DnaModality, DnaValidationStatus,
};
use crate::evidence::DigestEvidence;
use crate::publication::{PublicationAsset, PublicationContract};

pub const MAGAZINE_CANDIDATE_SCHEMA_V1: &str = "renderflow.magazine-candidates/v1";
pub const MAGAZINE_CANDIDATE_SKILL_ID_V1: &str = "skill.magazine.candidates";
const MAGAZINE_CANDIDATE_POLICY_V1: &str = "policy.magazine-candidates.public-safe/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineCandidatePolicy {
    #[serde(default)]
    pub protected_references: Vec<String>,
    #[serde(default = "default_true")]
    pub reject_pii: bool,
    #[serde(default = "default_true")]
    pub reject_secrets: bool,
    #[serde(default)]
    pub secret_markers: Vec<String>,
}

impl Default for MagazineCandidatePolicy {
    fn default() -> Self {
        Self {
            protected_references: Vec::new(),
            reject_pii: true,
            reject_secrets: true,
            secret_markers: Vec::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineGuidanceItem {
    pub observation_id: String,
    pub dimension: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineAssetBriefCandidate {
    pub state: AiCandidateState,
    pub asset_role: String,
    pub purpose: String,
    pub layout_guidance: Vec<MagazineGuidanceItem>,
    pub palette_guidance: Vec<MagazineGuidanceItem>,
    pub typography_guidance: Vec<MagazineGuidanceItem>,
    pub accessibility_guidance: Vec<MagazineGuidanceItem>,
    pub negative_guidance: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineMetadataCandidate {
    pub state: AiCandidateState,
    pub asset_role: String,
    pub source_format: String,
    pub source_media_type: String,
    pub descriptive_properties: BTreeMap<String, Value>,
    pub accessibility_review_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagazineAiStatus {
    NotRequested,
    Unavailable,
    Produced,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineAiCandidate {
    pub status: MagazineAiStatus,
    pub skill_id: String,
    pub skill_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<AiExecutionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineCandidateSourceEvidence {
    pub publication_issue_id: String,
    pub asset_role: String,
    pub source_artifact_id: String,
    pub source_digest: DigestEvidence,
    pub dna_digest: DigestEvidence,
    pub dna_schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineCandidateProvenance {
    pub generator: String,
    pub generator_version: String,
    pub policy_id: String,
    pub policy_digest: DigestEvidence,
    pub settings_digest: DigestEvidence,
    pub source_digest: DigestEvidence,
    pub dna_digest: DigestEvidence,
    pub dna_policy_id: String,
    pub dna_policy_digest: DigestEvidence,
    pub dna_configuration_digest: DigestEvidence,
    pub dna_extractors: Vec<DnaExtractorEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineCandidateValidation {
    pub schema_valid: bool,
    pub status: String,
    pub validators: Vec<String>,
    pub publication_content_generated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineCandidateApproval {
    pub state: AiCandidateState,
    pub human_review_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MagazineCandidateEnvelope {
    pub schema_version: String,
    pub candidate_id: String,
    pub source: MagazineCandidateSourceEvidence,
    pub asset_brief: MagazineAssetBriefCandidate,
    pub metadata: MagazineMetadataCandidate,
    pub ai: MagazineAiCandidate,
    pub provenance: MagazineCandidateProvenance,
    pub validation: MagazineCandidateValidation,
    pub approval: MagazineCandidateApproval,
}

impl MagazineCandidateEnvelope {
    /// Validate the stable envelope invariants before serialization or use by
    /// downstream publication tooling.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != MAGAZINE_CANDIDATE_SCHEMA_V1 {
            anyhow::bail!("unsupported magazine candidate schema");
        }
        if !self.candidate_id.starts_with("magazine:sha256:")
            || self.candidate_id.len() != "magazine:sha256:".len() + 64
        {
            anyhow::bail!("magazine candidate id is not a SHA-256 identity");
        }
        if self.asset_brief.state != AiCandidateState::Candidate
            || self.metadata.state != AiCandidateState::Candidate
            || self.approval.state != AiCandidateState::Candidate
            || !self.approval.human_review_required
        {
            anyhow::bail!(
                "magazine asset briefs and metadata must remain review-required candidates"
            );
        }
        if !self.validation.schema_valid || self.validation.status != "valid_review_required" {
            anyhow::bail!("magazine candidate validation evidence is incomplete");
        }
        if self.validation.publication_content_generated {
            anyhow::bail!("magazine candidate infrastructure cannot generate publication content");
        }
        for digest in [
            &self.source.source_digest,
            &self.source.dna_digest,
            &self.provenance.policy_digest,
            &self.provenance.settings_digest,
            &self.provenance.source_digest,
            &self.provenance.dna_digest,
            &self.provenance.dna_policy_digest,
            &self.provenance.dna_configuration_digest,
        ] {
            if digest.algorithm != "sha256"
                || digest.value.len() != 64
                || !digest.value.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                anyhow::bail!("magazine candidate contains invalid digest evidence");
            }
        }
        match self.ai.status {
            MagazineAiStatus::Produced => {
                let execution = self
                    .ai
                    .execution
                    .as_ref()
                    .context("produced AI candidate is missing execution evidence")?;
                if self.ai.candidate.is_none()
                    || self.ai.provider_id.as_deref() != Some(&execution.identity.provider_id)
                    || self.ai.model_id.as_deref() != Some(&execution.identity.model_id)
                    || execution.output_state != AiCandidateState::Candidate
                    || !execution.approval_required
                {
                    anyhow::bail!("produced AI guidance must remain an evidenced candidate");
                }
            }
            MagazineAiStatus::NotRequested | MagazineAiStatus::Unavailable => {
                if self.ai.candidate.is_some() || self.ai.execution.is_some() {
                    anyhow::bail!("non-produced AI state cannot contain model output");
                }
            }
        }
        Ok(())
    }

    pub fn attach_ai_outcome(
        &mut self,
        outcome: AiSkillExecutionOutcome,
        store: &ArtifactStore,
    ) -> Result<()> {
        let bytes = store.read_bytes(&outcome.artifact)?;
        let candidate: Value = serde_json::from_slice(&bytes)
            .context("AI magazine candidate artifact is not valid JSON")?;
        self.ai = MagazineAiCandidate {
            status: MagazineAiStatus::Produced,
            skill_id: outcome.evidence.skill_id.clone(),
            skill_version: outcome.evidence.skill_version.clone(),
            provider_id: Some(outcome.evidence.identity.provider_id.clone()),
            model_id: Some(outcome.evidence.identity.model_id.clone()),
            candidate: Some(candidate),
            execution: Some(outcome.evidence),
            diagnostics: Vec::new(),
        };
        Ok(())
    }

    pub fn mark_ai_unavailable(&mut self) {
        self.ai.status = MagazineAiStatus::Unavailable;
        self.ai
            .diagnostics
            .push("magazine.ai.compatible_execution_unavailable".to_string());
    }
}

pub fn build_magazine_candidates(
    publication: &PublicationContract,
    asset: &PublicationAsset,
    dna: &ArtifactDna,
    policy: &MagazineCandidatePolicy,
) -> Result<MagazineCandidateEnvelope> {
    validate_dna_for_magazine(dna, policy)?;
    let dna_bytes = serde_json::to_vec(dna)?;
    let dna_digest = digest_bytes(&dna_bytes);
    let policy_digest = digest_json(policy)?;
    let settings_digest = digest_json(&serde_json::json!({
        "schema_version": MAGAZINE_CANDIDATE_SCHEMA_V1,
        "asset_role": asset.role,
        "compiler": "renderflow.builtin.magazine-candidates",
        "compiler_version": env!("CARGO_PKG_VERSION"),
    }))?;

    let excluded = dna
        .similarity_guidance
        .excluded_dimensions
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut layout_guidance = Vec::new();
    let mut palette_guidance = Vec::new();
    let mut typography_guidance = Vec::new();
    let mut accessibility_guidance = Vec::new();
    let mut descriptive_properties = BTreeMap::new();

    for observation in &dna.observations {
        if !observation.similarity_eligible
            || excluded.contains(observation.dimension.as_str())
            || is_identity_or_content_dimension(&observation.dimension)
        {
            continue;
        }
        let item = MagazineGuidanceItem {
            observation_id: observation.id.clone(),
            dimension: observation.dimension.clone(),
            value: observation.value.clone(),
            description: observation.description.clone(),
            confidence: observation.evidence.confidence,
        };
        if observation.dimension.starts_with("typography.")
            || observation.dimension.starts_with("layout.typography")
        {
            typography_guidance.push(item);
        } else if observation.dimension.starts_with("layout.")
            || observation.dimension.starts_with("visual.canvas")
        {
            layout_guidance.push(item);
        } else if observation.dimension.starts_with("visual.palette")
            || observation.dimension.starts_with("visual.color")
        {
            palette_guidance.push(item);
        } else if observation.dimension.starts_with("accessibility.") {
            accessibility_guidance.push(item);
        }
        descriptive_properties.insert(observation.dimension.clone(), observation.value.clone());
    }

    let source = MagazineCandidateSourceEvidence {
        publication_issue_id: publication.issue_id.clone(),
        asset_role: asset.role.clone(),
        source_artifact_id: dna.source.artifact_id.clone(),
        source_digest: dna.source.digest.clone(),
        dna_digest: dna_digest.clone(),
        dna_schema_version: dna.schema_version.clone(),
    };
    let mut envelope = MagazineCandidateEnvelope {
        schema_version: MAGAZINE_CANDIDATE_SCHEMA_V1.to_string(),
        candidate_id: String::new(),
        source,
        asset_brief: MagazineAssetBriefCandidate {
            state: AiCandidateState::Candidate,
            asset_role: asset.role.clone(),
            purpose: format!(
                "Original {} asset for publication issue {}; guidance only, no content generation",
                asset.role, publication.issue_id
            ),
            layout_guidance,
            palette_guidance,
            typography_guidance,
            accessibility_guidance,
            negative_guidance: vec![
                "Do not imitate or name a creator, brand, franchise, or protected work".to_string(),
                "Do not generate or rewrite editorial or comic content".to_string(),
                "Do not treat this candidate as approved publication metadata".to_string(),
            ],
        },
        metadata: MagazineMetadataCandidate {
            state: AiCandidateState::Candidate,
            asset_role: asset.role.clone(),
            source_format: dna.source.format.clone(),
            source_media_type: dna.source.media_type.clone(),
            descriptive_properties,
            accessibility_review_required: true,
        },
        ai: MagazineAiCandidate {
            status: MagazineAiStatus::NotRequested,
            skill_id: MAGAZINE_CANDIDATE_SKILL_ID_V1.to_string(),
            skill_version: "1.0.0".to_string(),
            provider_id: None,
            model_id: None,
            candidate: None,
            execution: None,
            diagnostics: Vec::new(),
        },
        provenance: MagazineCandidateProvenance {
            generator: "renderflow.builtin.magazine-candidates".to_string(),
            generator_version: env!("CARGO_PKG_VERSION").to_string(),
            policy_id: MAGAZINE_CANDIDATE_POLICY_V1.to_string(),
            policy_digest,
            settings_digest,
            source_digest: dna.source.digest.clone(),
            dna_digest,
            dna_policy_id: dna.provenance.policy_id.clone(),
            dna_policy_digest: dna.provenance.policy_digest.clone(),
            dna_configuration_digest: dna.provenance.configuration_digest.clone(),
            dna_extractors: dna.provenance.extractors.clone(),
        },
        validation: MagazineCandidateValidation {
            schema_valid: true,
            status: "valid_review_required".to_string(),
            validators: vec![
                "validator.magazine-candidates/v1".to_string(),
                "validator.artifact-dna/v1".to_string(),
                "validator.prompt-hygiene/v1".to_string(),
            ],
            publication_content_generated: false,
        },
        approval: MagazineCandidateApproval {
            state: AiCandidateState::Candidate,
            human_review_required: true,
            approval_reference: None,
        },
    };
    envelope.candidate_id = format!(
        "magazine:sha256:{}",
        digest_json(&serde_json::json!({
            "source": envelope.source,
            "asset_brief": envelope.asset_brief,
            "metadata": envelope.metadata,
            "provenance": envelope.provenance,
        }))?
        .value
    );
    envelope.validate()?;
    Ok(envelope)
}

#[allow(clippy::too_many_arguments)]
pub fn create_magazine_ai_request(
    candidate: &MagazineCandidateEnvelope,
    dna: &ArtifactDna,
    policy: &MagazineCandidatePolicy,
    preference: AiExecutionPreferenceV1,
    allow_remote: bool,
    allow_unverified: bool,
    source_approved: bool,
    privacy_approved_for_remote: bool,
) -> Result<AiSkillExecutionRequest> {
    validate_dna_for_magazine(dna, policy)?;
    let dna_json = serde_json::to_string(dna)?;
    let deterministic_candidate = serde_json::to_string(&serde_json::json!({
        "asset_brief": candidate.asset_brief,
        "metadata": candidate.metadata,
    }))?;
    Ok(AiSkillExecutionRequest {
        skill_id: MAGAZINE_CANDIDATE_SKILL_ID_V1.to_string(),
        skill_version: Some("1.0.0".to_string()),
        input: serde_json::json!({
            "dna": dna_json,
            "deterministic_candidate": deterministic_candidate,
            "intent": candidate.asset_brief.purpose,
        }),
        input_artifacts: vec![AiInputArtifactEvidence {
            artifact_id: dna.source.artifact_id.clone(),
            digest: dna.source.digest.value.clone(),
            media_type: dna.source.media_type.clone(),
            approved_for_ai: source_approved,
        }],
        preference,
        allow_remote,
        allow_unverified,
        source_approved,
        privacy_approved_for_remote,
        additional_protected_references: policy
            .protected_references
            .iter()
            .map(|term| AiProtectedReferenceRule {
                term: term.clone(),
                descriptive_replacement: Some(
                    "source-independent visual characteristics".to_string(),
                ),
            })
            .collect(),
    })
}

fn validate_dna_for_magazine(dna: &ArtifactDna, policy: &MagazineCandidatePolicy) -> Result<()> {
    dna.validate()?;
    if !dna.validation.schema_valid
        || dna.validation.status == DnaValidationStatus::Blocked
        || dna.hygiene.status == DnaHygieneStatus::Blocked
    {
        anyhow::bail!("magazine candidates require validated, non-blocked Artifact DNA");
    }
    if dna.hygiene.raw_payload_retained || dna.hygiene.identifying_metadata_retained {
        anyhow::bail!(
            "magazine candidates require sanitized Artifact DNA without raw or identifying data"
        );
    }
    if !dna
        .modalities
        .iter()
        .any(|modality| matches!(modality, DnaModality::Visual | DnaModality::Layout))
    {
        anyhow::bail!("magazine candidates require visual or layout Artifact DNA");
    }
    if dna.similarity_guidance.direct_imitation_allowed {
        anyhow::bail!("magazine candidates reject Artifact DNA that permits direct imitation");
    }
    let eligible_text = dna
        .observations
        .iter()
        .filter(|observation| observation.similarity_eligible)
        .map(|observation| {
            format!(
                "{} {} {}",
                observation.dimension,
                observation.description.as_deref().unwrap_or_default(),
                observation.value
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let lower = eligible_text.to_ascii_lowercase();
    if policy.reject_secrets && contains_secret(&eligible_text, &policy.secret_markers) {
        anyhow::bail!("Artifact DNA failed the magazine secret-hygiene gate");
    }
    if policy.reject_pii && contains_email(&eligible_text) {
        anyhow::bail!("Artifact DNA failed the magazine privacy gate");
    }
    if policy
        .protected_references
        .iter()
        .any(|term| !term.trim().is_empty() && lower.contains(&term.to_ascii_lowercase()))
    {
        anyhow::bail!("Artifact DNA failed the magazine protected-reference gate");
    }
    Ok(())
}

fn is_identity_or_content_dimension(dimension: &str) -> bool {
    dimension.starts_with("identity.")
        || dimension.starts_with("textual.content")
        || dimension.starts_with("copyright.")
}

fn contains_email(value: &str) -> bool {
    value.split_ascii_whitespace().any(|word| {
        let word = word.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '@' | '.' | '_' | '-' | '+')
        });
        let Some((local, domain)) = word.split_once('@') else {
            return false;
        };
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    })
}

fn contains_secret(value: &str, additional_markers: &[String]) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "-----begin private key-----",
        "api_key=",
        "api-key:",
        "bearer eyj",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
        || additional_markers
            .iter()
            .any(|marker| !marker.is_empty() && lower.contains(&marker.to_ascii_lowercase()))
}

fn digest_json(value: &impl Serialize) -> Result<DigestEvidence> {
    Ok(digest_bytes(&serde_json::to_vec(value)?))
}

fn digest_bytes(bytes: &[u8]) -> DigestEvidence {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    DigestEvidence {
        algorithm: "sha256".to_string(),
        value: format!("{:x}", hasher.finalize()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dna() -> ArtifactDna {
        ArtifactDna::from_json(include_str!(
            "../../../../tests/fixtures/artifact-dna/visual.json"
        ))
        .unwrap()
    }

    fn publication() -> PublicationContract {
        serde_yaml_ng::from_str(include_str!(
            "../../../../examples/magazine/renderflow.yaml"
        ))
        .map(|spec: crate::spec::SpecV2| spec.publication.unwrap())
        .unwrap()
    }

    #[test]
    fn deterministic_candidate_uses_only_safe_visual_layout_evidence() {
        let publication = publication();
        let asset = publication
            .artwork
            .iter()
            .find(|asset| asset.role == "cover")
            .unwrap();
        let candidate = build_magazine_candidates(
            &publication,
            asset,
            &fixture_dna(),
            &MagazineCandidatePolicy::default(),
        )
        .unwrap();
        assert_eq!(candidate.ai.status, MagazineAiStatus::NotRequested);
        assert_eq!(candidate.approval.state, AiCandidateState::Candidate);
        assert_eq!(candidate.asset_brief.layout_guidance.len(), 1);
        assert_eq!(candidate.asset_brief.palette_guidance.len(), 1);
        assert!(candidate
            .metadata
            .descriptive_properties
            .contains_key("visual.palette.hex"));
        let encoded = serde_json::to_string(&candidate).unwrap();
        serde_json::from_str::<MagazineCandidateEnvelope>(&encoded)
            .unwrap()
            .validate()
            .unwrap();
    }

    #[test]
    fn protected_reference_gate_runs_before_ai_request_creation() {
        let publication = publication();
        let asset = &publication.artwork[0];
        let mut dna = fixture_dna();
        dna.observations[0].description = Some("Example Protected Franchise layout".to_string());
        let policy = MagazineCandidatePolicy {
            protected_references: vec!["Example Protected Franchise".to_string()],
            ..MagazineCandidatePolicy::default()
        };
        assert!(build_magazine_candidates(&publication, asset, &dna, &policy).is_err());
    }
}
