//! Versioned, provider-neutral Artifact DNA extraction and similarity guidance.
//!
//! Artifact DNA is an optional, immutable JSON artifact derived from a source
//! artifact. The core contract carries typed observations and evidence while
//! extractor implementations remain replaceable. Built-in extraction is local
//! and deterministic; AI-assisted extractors must enter through the same trait
//! and are filtered by explicit policy before execution.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::artifact::{
    Artifact, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore, CanonicalFormat, MediaType,
};
use crate::evidence::DigestEvidence;

pub const ARTIFACT_DNA_SCHEMA_V1: &str = "renderflow.artifact-dna/v1";
pub const ARTIFACT_DNA_COMPARISON_SCHEMA_V1: &str = "renderflow.artifact-dna-comparison/v1";
pub const ARTIFACT_DNA_FORMAT_V1: &str = "artifact-dna-json";
pub const ARTIFACT_DNA_MEDIA_TYPE_V1: &str = "application/vnd.renderflow.artifact-dna+json";

/// Stable top-level modality families. Individual dimensions remain open and
/// are identified by namespaced strings so providers can extend each family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaModality {
    Visual,
    Sonic,
    Textual,
    Layout,
    CrossModal,
    Compound,
}

impl fmt::Display for DnaModality {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Visual => "visual",
            Self::Sonic => "sonic",
            Self::Textual => "textual",
            Self::Layout => "layout",
            Self::CrossModal => "cross_modal",
            Self::Compound => "compound",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaEvidenceOrigin {
    SourceReported,
    ProviderObserved,
    Inferred,
    HumanProvided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaDeterminism {
    Deterministic,
    Heuristic,
    Probabilistic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaProviderLocality {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaReviewState {
    Candidate,
    Approved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaValidationStatus {
    Valid,
    ValidWithWarnings,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaHygieneStatus {
    Passed,
    Sanitized,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaHygieneAction {
    Omit,
    Rewrite,
    Block,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaObservationEvidence {
    pub origin: DnaEvidenceOrigin,
    pub provider_id: String,
    pub provider_version: String,
    pub determinism: DnaDeterminism,
    pub confidence: f64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scope: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaObservation {
    pub id: String,
    pub modality: DnaModality,
    /// Namespaced stable dimension such as `visual.canvas.aspect_ratio`.
    pub dimension: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub evidence: DnaObservationEvidence,
    #[serde(default = "default_true")]
    pub similarity_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaSourceReference {
    pub artifact_id: String,
    pub digest: DigestEvidence,
    pub format: String,
    pub media_type: String,
    pub size_bytes: u64,
}

impl DnaSourceReference {
    fn from_artifact(artifact: &Artifact) -> Self {
        Self {
            artifact_id: artifact.id().to_string(),
            digest: DigestEvidence {
                algorithm: artifact.digest().algorithm().to_string(),
                value: artifact.digest().value().to_string(),
            },
            format: artifact.format().to_string(),
            media_type: artifact.media_type().to_string(),
            size_bytes: artifact.size_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaExtractorEvidence {
    pub provider_id: String,
    pub provider_version: String,
    pub locality: DnaProviderLocality,
    pub determinism: DnaDeterminism,
    pub ai_assisted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaProvenance {
    pub source_digest: DigestEvidence,
    pub extractors: Vec<DnaExtractorEvidence>,
    pub policy_id: String,
    pub policy_digest: DigestEvidence,
    pub configuration_digest: DigestEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaOmission {
    pub code: String,
    pub class: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaHygieneFinding {
    pub code: String,
    pub class: String,
    pub action: DnaHygieneAction,
    /// Safe diagnostic text. The matched value is intentionally never stored.
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaHygieneEvidence {
    pub policy_id: String,
    pub status: DnaHygieneStatus,
    pub raw_payload_retained: bool,
    pub identifying_metadata_retained: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<DnaHygieneFinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaSimilarityGuidance {
    pub purpose: String,
    pub direct_imitation_allowed: bool,
    pub legal_clearance_claimed: bool,
    #[serde(default)]
    pub dimension_weights: BTreeMap<String, f64>,
    #[serde(default)]
    pub excluded_dimensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaApproval {
    pub state: DnaReviewState,
    pub human_review_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaValidation {
    pub status: DnaValidationStatus,
    pub schema_valid: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

/// Stable core Artifact DNA contract. Provider-specific information is kept in
/// a namespaced extension map and never changes the meaning of core fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDna {
    pub schema_version: String,
    pub source: DnaSourceReference,
    pub modalities: Vec<DnaModality>,
    pub observations: Vec<DnaObservation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub provider_extensions: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omissions: Vec<DnaOmission>,
    pub similarity_guidance: DnaSimilarityGuidance,
    pub hygiene: DnaHygieneEvidence,
    pub provenance: DnaProvenance,
    pub approval: DnaApproval,
    pub validation: DnaValidation,
}

impl ArtifactDna {
    pub fn from_json(contents: &str) -> Result<Self> {
        let dna: Self = serde_json::from_str(contents).context("Artifact DNA is not valid JSON")?;
        dna.validate()?;
        Ok(dna)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read Artifact DNA '{}'", path.display()))?;
        Self::from_json(&contents)
            .with_context(|| format!("invalid Artifact DNA '{}'", path.display()))
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != ARTIFACT_DNA_SCHEMA_V1 {
            anyhow::bail!(
                "unsupported Artifact DNA schema '{}'; expected '{}'",
                self.schema_version,
                ARTIFACT_DNA_SCHEMA_V1
            );
        }
        validate_digest(&self.source.digest)?;
        validate_digest(&self.provenance.source_digest)?;
        validate_digest(&self.provenance.policy_digest)?;
        validate_digest(&self.provenance.configuration_digest)?;
        if self.source.digest != self.provenance.source_digest {
            anyhow::bail!("Artifact DNA source and provenance digests must match");
        }
        if self.source.artifact_id.trim().is_empty() || self.modalities.is_empty() {
            anyhow::bail!("Artifact DNA requires a source identity and at least one modality");
        }
        if self.modalities.iter().collect::<BTreeSet<_>>().len() != self.modalities.len() {
            anyhow::bail!("Artifact DNA modalities must be unique");
        }
        if self.provenance.extractors.is_empty()
            || self.provenance.policy_id.trim().is_empty()
            || self.hygiene.policy_id.trim().is_empty()
        {
            anyhow::bail!("Artifact DNA requires extractor and hygiene policy provenance");
        }
        if self.provenance.policy_id != self.hygiene.policy_id {
            anyhow::bail!("Artifact DNA hygiene and provenance policy IDs must match");
        }
        if !self.validation.schema_valid {
            anyhow::bail!("Artifact DNA cannot validate with schema_valid set to false");
        }
        let mut ids = BTreeSet::new();
        for observation in &self.observations {
            validate_observation(observation)?;
            if !ids.insert(&observation.id) {
                anyhow::bail!("duplicate Artifact DNA observation id '{}'", observation.id);
            }
            if !self.modalities.contains(&observation.modality) {
                anyhow::bail!(
                    "observation '{}' uses undeclared modality '{}'",
                    observation.id,
                    observation.modality
                );
            }
        }
        for namespace in self.provider_extensions.keys() {
            if !is_namespaced(namespace) {
                anyhow::bail!(
                    "provider extension '{}' must be a namespaced identifier",
                    namespace
                );
            }
        }
        for (dimension, weight) in &self.similarity_guidance.dimension_weights {
            if !is_namespaced(dimension) || !weight.is_finite() || *weight < 0.0 {
                anyhow::bail!("invalid similarity weight for dimension '{dimension}'");
            }
        }
        if self.hygiene.raw_payload_retained || self.hygiene.identifying_metadata_retained {
            anyhow::bail!(
                "public Artifact DNA must not retain raw payloads or identifying metadata"
            );
        }
        if self.similarity_guidance.direct_imitation_allowed
            || self.similarity_guidance.legal_clearance_claimed
        {
            anyhow::bail!("Artifact DNA cannot authorize imitation or claim legal clearance");
        }
        if self.approval.state == DnaReviewState::Approved {
            let has_approval = self
                .approval
                .approval_reference
                .as_deref()
                .is_some_and(|reference| !reference.trim().is_empty());
            if !has_approval {
                anyhow::bail!("approved Artifact DNA requires an approval reference");
            }
        }
        if self
            .observations
            .iter()
            .any(|observation| observation.evidence.determinism != DnaDeterminism::Deterministic)
            && !self.approval.human_review_required
        {
            anyhow::bail!("heuristic and probabilistic Artifact DNA requires human review");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaProtectedReferenceRule {
    pub term: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptive_replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DnaExtractionPolicy {
    /// Extraction is off unless a caller explicitly enables it.
    pub enabled: bool,
    pub policy_id: String,
    pub allow_ai: bool,
    pub allow_network: bool,
    pub allow_remote: bool,
    pub max_source_bytes: u64,
    pub max_observations: usize,
    pub secret_action: DnaHygieneAction,
    pub pii_action: DnaHygieneAction,
    pub protected_reference_action: DnaHygieneAction,
    pub protected_references: Vec<DnaProtectedReferenceRule>,
}

impl Default for DnaExtractionPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            policy_id: "policy.artifact-dna.public-safe/v1".to_string(),
            allow_ai: false,
            allow_network: false,
            allow_remote: false,
            max_source_bytes: 64 * 1024 * 1024,
            max_observations: 512,
            secret_action: DnaHygieneAction::Omit,
            pii_action: DnaHygieneAction::Omit,
            protected_reference_action: DnaHygieneAction::Rewrite,
            protected_references: Vec::new(),
        }
    }
}

impl DnaExtractionPolicy {
    pub fn explicit_local() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaExtractionStatus {
    Skipped,
    Complete,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DnaExtractionOutcome {
    pub status: DnaExtractionStatus,
    pub dna: Option<ArtifactDna>,
    pub artifact: Option<Artifact>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DnaExtractionBatch {
    pub modalities: Vec<DnaModality>,
    pub observations: Vec<DnaObservation>,
    pub provider_extensions: BTreeMap<String, Value>,
    pub omissions: Vec<DnaOmission>,
}

/// Provider-neutral extraction boundary. Local analyzers and model-backed
/// enrichers return the same stable observations. Policy filters run before an
/// extractor is called and hygiene gates run before its output is exposed.
pub trait ArtifactDnaExtractor: Send + Sync {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn locality(&self) -> DnaProviderLocality;
    fn determinism(&self) -> DnaDeterminism;
    fn ai_assisted(&self) -> bool;
    fn requires_network(&self) -> bool;
    fn supports(&self, artifact: &Artifact) -> bool;
    fn extract(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<DnaExtractionBatch>;
}

#[derive(Default)]
pub struct ArtifactDnaEngine {
    extractors: Vec<Arc<dyn ArtifactDnaExtractor>>,
}

impl ArtifactDnaEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtins() -> Self {
        Self::new().with_extractor(Arc::new(BuiltinArtifactDnaExtractor))
    }

    pub fn with_extractor(mut self, extractor: Arc<dyn ArtifactDnaExtractor>) -> Self {
        self.extractors.push(extractor);
        self
    }

    pub fn extract(
        &self,
        artifact: &Artifact,
        store: &ArtifactStore,
        policy: &DnaExtractionPolicy,
    ) -> Result<DnaExtractionOutcome> {
        if !policy.enabled {
            return Ok(DnaExtractionOutcome {
                status: DnaExtractionStatus::Skipped,
                dna: None,
                artifact: None,
                diagnostics: vec!["Artifact DNA extraction is disabled by policy".to_string()],
            });
        }
        if artifact.size_bytes() > policy.max_source_bytes {
            return Ok(DnaExtractionOutcome {
                status: DnaExtractionStatus::Skipped,
                dna: None,
                artifact: None,
                diagnostics: vec![format!(
                    "source size {} exceeds Artifact DNA budget {}",
                    artifact.size_bytes(),
                    policy.max_source_bytes
                )],
            });
        }

        store.verify(artifact)?;
        let configuration_digest = digest_serialized(policy)?;
        let mut observations = Vec::new();
        let mut modalities = BTreeSet::new();
        let mut extensions = BTreeMap::new();
        let mut omissions = Vec::new();
        let mut extractor_evidence = Vec::new();
        let mut diagnostics = Vec::new();

        for extractor in self
            .extractors
            .iter()
            .filter(|extractor| extractor.supports(artifact))
        {
            if let Some(reason) = blocked_extractor_reason(extractor.as_ref(), policy) {
                diagnostics.push(format!("{} skipped: {reason}", extractor.id()));
                continue;
            }
            let batch = extractor
                .extract(artifact, store)
                .with_context(|| format!("Artifact DNA extractor '{}' failed", extractor.id()))?;
            modalities.extend(batch.modalities);
            observations.extend(batch.observations);
            for (namespace, value) in batch.provider_extensions {
                if extensions.insert(namespace.clone(), value).is_some() {
                    anyhow::bail!("duplicate Artifact DNA provider extension '{namespace}'");
                }
            }
            omissions.extend(batch.omissions);
            extractor_evidence.push(DnaExtractorEvidence {
                provider_id: extractor.id().to_string(),
                provider_version: extractor.version().to_string(),
                locality: extractor.locality(),
                determinism: extractor.determinism(),
                ai_assisted: extractor.ai_assisted(),
            });
        }

        if extractor_evidence.is_empty() {
            return Ok(DnaExtractionOutcome {
                status: DnaExtractionStatus::Skipped,
                dna: None,
                artifact: None,
                diagnostics: if diagnostics.is_empty() {
                    vec!["no compatible Artifact DNA extractor is registered".to_string()]
                } else {
                    diagnostics
                },
            });
        }
        if observations.len() > policy.max_observations {
            observations.truncate(policy.max_observations);
            omissions.push(DnaOmission {
                code: "dna.budget.observation_limit".to_string(),
                class: "budget".to_string(),
                message: "Additional observations were omitted by the configured budget"
                    .to_string(),
            });
            diagnostics.push("observation budget reached; output is partial".to_string());
        }

        let hygiene = sanitize_output(&mut observations, &mut extensions, &mut omissions, policy);
        if hygiene.status == DnaHygieneStatus::Blocked {
            return Ok(DnaExtractionOutcome {
                status: DnaExtractionStatus::Blocked,
                dna: None,
                artifact: None,
                diagnostics: vec![
                    "Artifact DNA was blocked by privacy or protected-reference policy".to_string(),
                ],
            });
        }

        observations.sort_by(|left, right| {
            (&left.modality, &left.dimension, &left.id).cmp(&(
                &right.modality,
                &right.dimension,
                &right.id,
            ))
        });
        let mut modalities = modalities.into_iter().collect::<Vec<_>>();
        modalities.sort();
        let dimension_weights = observations
            .iter()
            .filter(|observation| observation.similarity_eligible)
            .map(|observation| (observation.dimension.clone(), 1.0))
            .collect();
        let requires_human_review = observations
            .iter()
            .any(|observation| observation.evidence.determinism != DnaDeterminism::Deterministic);
        let source = DnaSourceReference::from_artifact(artifact);
        let policy_digest = digest_serialized(policy)?;
        let validation_status = if diagnostics.is_empty() && omissions.is_empty() {
            DnaValidationStatus::Valid
        } else {
            DnaValidationStatus::ValidWithWarnings
        };
        let status = if diagnostics.is_empty() && omissions.is_empty() {
            DnaExtractionStatus::Complete
        } else {
            DnaExtractionStatus::Partial
        };
        let dna = ArtifactDna {
            schema_version: ARTIFACT_DNA_SCHEMA_V1.to_string(),
            provenance: DnaProvenance {
                source_digest: source.digest.clone(),
                extractors: extractor_evidence,
                policy_id: policy.policy_id.clone(),
                policy_digest,
                configuration_digest,
            },
            source,
            modalities,
            observations,
            provider_extensions: extensions,
            omissions,
            similarity_guidance: DnaSimilarityGuidance {
                purpose:
                    "Compare reusable descriptive characteristics for related, original assets"
                        .to_string(),
                direct_imitation_allowed: false,
                legal_clearance_claimed: false,
                dimension_weights,
                excluded_dimensions: vec![
                    "identity.creator".to_string(),
                    "identity.brand".to_string(),
                    "identity.protected_work".to_string(),
                ],
            },
            hygiene,
            approval: DnaApproval {
                state: DnaReviewState::Candidate,
                human_review_required: requires_human_review,
                approval_reference: None,
            },
            validation: DnaValidation {
                status: validation_status,
                schema_valid: true,
                diagnostics: diagnostics.clone(),
            },
        };
        dna.validate()?;
        let bytes = serde_json::to_vec_pretty(&dna)?;
        let descriptor = ArtifactDescriptor::new(
            CanonicalFormat::new(ARTIFACT_DNA_FORMAT_V1)?,
            MediaType::new(ARTIFACT_DNA_MEDIA_TYPE_V1)?,
            ArtifactStorageClass::Intermediate,
        )
        .with_source(artifact.id().clone())
        .with_metadata("renderflow.artifact-dna.schema", ARTIFACT_DNA_SCHEMA_V1)
        .with_metadata("renderflow.artifact-dna.policy", policy.policy_id.clone())
        .with_metadata(
            "renderflow.artifact-dna.validation",
            serde_json::to_value(validation_status)?,
        );
        let dna_artifact = store.put_bytes(&bytes, descriptor)?;
        store.verify(artifact)?;

        Ok(DnaExtractionOutcome {
            status,
            dna: Some(dna),
            artifact: Some(dna_artifact),
            diagnostics,
        })
    }
}

fn blocked_extractor_reason(
    extractor: &dyn ArtifactDnaExtractor,
    policy: &DnaExtractionPolicy,
) -> Option<&'static str> {
    if extractor.ai_assisted() && !policy.allow_ai {
        Some("AI-assisted extraction was not explicitly allowed")
    } else if extractor.requires_network() && !policy.allow_network {
        Some("network access was not explicitly allowed")
    } else if extractor.locality() == DnaProviderLocality::Remote && !policy.allow_remote {
        Some("remote execution was not explicitly allowed")
    } else {
        None
    }
}

#[derive(Debug, Default)]
pub struct BuiltinArtifactDnaExtractor;

impl ArtifactDnaExtractor for BuiltinArtifactDnaExtractor {
    fn id(&self) -> &str {
        "renderflow.builtin.artifact-dna"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn locality(&self) -> DnaProviderLocality {
        DnaProviderLocality::Local
    }

    fn determinism(&self) -> DnaDeterminism {
        DnaDeterminism::Deterministic
    }

    fn ai_assisted(&self) -> bool {
        false
    }

    fn requires_network(&self) -> bool {
        false
    }

    fn supports(&self, _artifact: &Artifact) -> bool {
        true
    }

    fn extract(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<DnaExtractionBatch> {
        let bytes = store.read_bytes(artifact)?;
        let modality = primary_modality(artifact.media_type().as_str(), artifact.format().as_str());
        let mut batch = DnaExtractionBatch {
            modalities: vec![modality],
            ..DnaExtractionBatch::default()
        };
        batch.observations.extend([
            observation(
                "technical.format",
                modality,
                "technical.format",
                json!(artifact.format().as_str()),
                false,
            ),
            observation(
                "technical.media-type",
                modality,
                "technical.media_type",
                json!(artifact.media_type().as_str()),
                false,
            ),
            observation(
                "technical.size-bytes",
                modality,
                "technical.size_bytes",
                json!(artifact.size_bytes()),
                false,
            ),
        ]);

        if artifact.media_type().as_str().starts_with("image/") {
            batch.modalities.push(DnaModality::Visual);
            batch.modalities.push(DnaModality::Layout);
            extract_image_geometry(&bytes, artifact.media_type().as_str(), &mut batch);
            if artifact.media_type().as_str() == "image/svg+xml" {
                extract_svg_characteristics(&bytes, &mut batch);
            }
        } else if is_textual_media_type(artifact.media_type().as_str()) {
            batch.modalities.push(DnaModality::Textual);
            batch.modalities.push(DnaModality::Layout);
            extract_text_statistics(&bytes, &mut batch);
        }
        batch.modalities.sort();
        batch.modalities.dedup();
        Ok(batch)
    }
}

fn primary_modality(media_type: &str, format: &str) -> DnaModality {
    if media_type.starts_with("image/") {
        DnaModality::Visual
    } else if media_type.starts_with("audio/") || matches!(format, "midi" | "bwf") {
        DnaModality::Sonic
    } else if is_textual_media_type(media_type) {
        DnaModality::Textual
    } else {
        DnaModality::Compound
    }
}

fn is_textual_media_type(media_type: &str) -> bool {
    media_type.starts_with("text/")
        || matches!(
            media_type,
            "application/json"
                | "application/yaml"
                | "application/toml"
                | "application/xml"
                | "application/x-latex"
        )
}

fn observation(
    id: &str,
    modality: DnaModality,
    dimension: &str,
    value: Value,
    similarity_eligible: bool,
) -> DnaObservation {
    DnaObservation {
        id: id.to_string(),
        modality,
        dimension: dimension.to_string(),
        value,
        description: None,
        evidence: DnaObservationEvidence {
            origin: DnaEvidenceOrigin::ProviderObserved,
            provider_id: "renderflow.builtin.artifact-dna".to_string(),
            provider_version: env!("CARGO_PKG_VERSION").to_string(),
            determinism: DnaDeterminism::Deterministic,
            confidence: 1.0,
            scope: BTreeMap::new(),
        },
        similarity_eligible,
    }
}

fn extract_image_geometry(bytes: &[u8], media_type: &str, batch: &mut DnaExtractionBatch) {
    let dimensions = if media_type == "image/png" {
        png_dimensions(bytes)
    } else if media_type == "image/jpeg" {
        jpeg_dimensions(bytes)
    } else if media_type == "image/svg+xml" {
        svg_dimensions(bytes)
    } else {
        None
    };
    let Some((width, height)) = dimensions else {
        batch.omissions.push(DnaOmission {
            code: "dna.visual.geometry.unavailable".to_string(),
            class: "unsupported_image_geometry".to_string(),
            message: "Canvas geometry could not be determined by the built-in local extractor"
                .to_string(),
        });
        return;
    };
    let ratio = if height == 0 {
        0.0
    } else {
        f64::from(width) / f64::from(height)
    };
    let orientation = if width > height {
        "landscape"
    } else if height > width {
        "portrait"
    } else {
        "square"
    };
    batch.observations.extend([
        observation(
            "visual.canvas.width",
            DnaModality::Visual,
            "visual.canvas.width_pixels",
            json!(width),
            true,
        ),
        observation(
            "visual.canvas.height",
            DnaModality::Visual,
            "visual.canvas.height_pixels",
            json!(height),
            true,
        ),
        observation(
            "visual.canvas.aspect-ratio",
            DnaModality::Layout,
            "layout.canvas.aspect_ratio",
            json!(round_six(ratio)),
            true,
        ),
        observation(
            "visual.canvas.orientation",
            DnaModality::Layout,
            "layout.canvas.orientation",
            json!(orientation),
            true,
        ),
    ]);
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    Some((
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0..2] != [0xff, 0xd8] {
        return None;
    }
    let mut offset = 2;
    while offset + 4 <= bytes.len() {
        while offset < bytes.len() && bytes[offset] != 0xff {
            offset += 1;
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        if offset >= bytes.len() {
            return None;
        }
        let marker = bytes[offset];
        offset += 1;
        if matches!(marker, 0xd8 | 0xd9) {
            continue;
        }
        if offset + 2 > bytes.len() {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]));
        if length < 2 || offset + length > bytes.len() {
            return None;
        }
        if is_jpeg_start_of_frame(marker) && length >= 7 {
            let height = u32::from(u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]));
            let width = u32::from(u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]));
            return Some((width, height));
        }
        offset += length;
    }
    None
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

fn svg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let svg = opening_tag(text, "svg")?;
    let width = numeric_attribute(svg, "width");
    let height = numeric_attribute(svg, "height");
    match (width, height) {
        (Some(width), Some(height)) => Some((width.round() as u32, height.round() as u32)),
        _ => {
            let view_box =
                string_attribute(svg, "viewBox").or_else(|| string_attribute(svg, "viewbox"))?;
            let values = view_box
                .split(|character: char| character.is_ascii_whitespace() || character == ',')
                .filter_map(|value| value.parse::<f64>().ok())
                .collect::<Vec<_>>();
            (values.len() == 4).then(|| (values[2].round() as u32, values[3].round() as u32))
        }
    }
}

fn extract_svg_characteristics(bytes: &[u8], batch: &mut DnaExtractionBatch) {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    let element_counts = [
        "rect", "circle", "ellipse", "line", "path", "polygon", "text", "g",
    ]
    .into_iter()
    .map(|element| (element.to_string(), count_opening_tags(text, element)))
    .filter(|(_, count)| *count > 0)
    .collect::<BTreeMap<_, _>>();
    batch.observations.push(observation(
        "layout.svg.element-counts",
        DnaModality::Layout,
        "layout.svg.element_counts",
        json!(element_counts),
        true,
    ));
    let palette = svg_hex_colors(text);
    if !palette.is_empty() {
        batch.observations.push(observation(
            "visual.svg.palette",
            DnaModality::Visual,
            "visual.palette.hex",
            json!(palette),
            true,
        ));
    }
    let text_count = count_opening_tags(text, "text");
    batch.observations.push(observation(
        "layout.svg.text-elements",
        DnaModality::Layout,
        "layout.typography.text_element_count",
        json!(text_count),
        true,
    ));
}

fn opening_tag<'a>(text: &'a str, element: &str) -> Option<&'a str> {
    let start = text.find(&format!("<{element}"))?;
    let tail = &text[start..];
    let end = tail.find('>')?;
    Some(&tail[..=end])
}

fn string_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let marker = format!("{name}={quote}");
        if let Some(start) = tag.find(&marker) {
            let value = &tag[start + marker.len()..];
            return value.split(quote).next();
        }
    }
    None
}

fn numeric_attribute(tag: &str, name: &str) -> Option<f64> {
    let value = string_attribute(tag, name)?;
    let numeric = value
        .chars()
        .take_while(|character| character.is_ascii_digit() || matches!(character, '.' | '-'))
        .collect::<String>();
    numeric.parse().ok()
}

fn count_opening_tags(text: &str, element: &str) -> usize {
    let needle = format!("<{element}");
    text.match_indices(&needle)
        .filter(|(offset, _)| {
            text[*offset + needle.len()..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_whitespace() || character == '>')
        })
        .count()
}

fn svg_hex_colors(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut colors = BTreeSet::new();
    for index in 0..bytes.len() {
        if bytes[index] != b'#' {
            continue;
        }
        for digits in [8_usize, 6, 3] {
            if index + 1 + digits <= bytes.len()
                && bytes[index + 1..index + 1 + digits]
                    .iter()
                    .all(u8::is_ascii_hexdigit)
                && bytes
                    .get(index + 1 + digits)
                    .is_none_or(|next| !next.is_ascii_hexdigit())
            {
                colors.insert(text[index..index + 1 + digits].to_ascii_lowercase());
                break;
            }
        }
    }
    colors.into_iter().take(32).collect()
}

fn extract_text_statistics(bytes: &[u8], batch: &mut DnaExtractionBatch) {
    let Ok(text) = std::str::from_utf8(bytes) else {
        batch.omissions.push(DnaOmission {
            code: "dna.text.invalid-utf8".to_string(),
            class: "text_encoding".to_string(),
            message: "Text statistics were omitted because the payload is not UTF-8".to_string(),
        });
        return;
    };
    let lines = text.lines().collect::<Vec<_>>();
    let line_count = lines.len();
    let non_empty_lines = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let word_count = text.split_whitespace().count();
    let paragraph_count = text
        .split("\n\n")
        .filter(|paragraph| !paragraph.trim().is_empty())
        .count();
    let heading_count = lines
        .iter()
        .filter(|line| line.trim_start().starts_with('#'))
        .count();
    let average_line_length = if non_empty_lines == 0 {
        0.0
    } else {
        let characters = lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.chars().count())
            .sum::<usize>();
        characters as f64 / non_empty_lines as f64
    };
    batch.observations.extend([
        observation(
            "text.word-count",
            DnaModality::Textual,
            "textual.structure.word_count",
            json!(word_count),
            false,
        ),
        observation(
            "text.paragraph-count",
            DnaModality::Textual,
            "textual.structure.paragraph_count",
            json!(paragraph_count),
            true,
        ),
        observation(
            "layout.line-count",
            DnaModality::Layout,
            "layout.flow.line_count",
            json!(line_count),
            false,
        ),
        observation(
            "layout.heading-count",
            DnaModality::Layout,
            "layout.hierarchy.heading_count",
            json!(heading_count),
            true,
        ),
        observation(
            "layout.average-line-length",
            DnaModality::Layout,
            "layout.flow.average_line_length",
            json!(round_six(average_line_length)),
            true,
        ),
    ]);
}

fn sanitize_output(
    observations: &mut Vec<DnaObservation>,
    extensions: &mut BTreeMap<String, Value>,
    omissions: &mut Vec<DnaOmission>,
    policy: &DnaExtractionPolicy,
) -> DnaHygieneEvidence {
    let mut findings = Vec::new();
    let mut blocked = false;
    observations.retain_mut(|observation| {
        let mut value = observation.value.clone();
        let mut description = observation.description.clone().map(Value::String);
        let value_result = sanitize_value(&mut value, policy, &mut findings);
        let description_result = description
            .as_mut()
            .is_none_or(|value| sanitize_value(value, policy, &mut findings));
        if !value_result || !description_result {
            omissions.push(DnaOmission {
                code: "dna.hygiene.observation_omitted".to_string(),
                class: "sensitive_observation".to_string(),
                message: "An observation was omitted by privacy or protected-reference policy"
                    .to_string(),
            });
            if findings
                .iter()
                .rev()
                .take(2)
                .any(|finding| finding.action == DnaHygieneAction::Block)
            {
                blocked = true;
            }
            return false;
        }
        observation.value = value;
        observation.description = description.and_then(|value| value.as_str().map(str::to_string));
        true
    });
    extensions.retain(|_, value| {
        let mut candidate = value.clone();
        let safe = sanitize_value(&mut candidate, policy, &mut findings);
        if safe {
            *value = candidate;
        } else {
            omissions.push(DnaOmission {
                code: "dna.hygiene.extension_omitted".to_string(),
                class: "provider_extension".to_string(),
                message: "A provider extension was omitted by hygiene policy".to_string(),
            });
            if findings
                .last()
                .is_some_and(|finding| finding.action == DnaHygieneAction::Block)
            {
                blocked = true;
            }
        }
        safe
    });
    let status = if blocked {
        DnaHygieneStatus::Blocked
    } else if findings.is_empty() {
        DnaHygieneStatus::Passed
    } else {
        DnaHygieneStatus::Sanitized
    };
    DnaHygieneEvidence {
        policy_id: policy.policy_id.clone(),
        status,
        raw_payload_retained: false,
        identifying_metadata_retained: false,
        findings,
    }
}

fn sanitize_value(
    value: &mut Value,
    policy: &DnaExtractionPolicy,
    findings: &mut Vec<DnaHygieneFinding>,
) -> bool {
    match value {
        Value::String(text) => sanitize_string(text, policy, findings),
        Value::Array(values) => values
            .iter_mut()
            .all(|value| sanitize_value(value, policy, findings)),
        Value::Object(values) => values
            .values_mut()
            .all(|value| sanitize_value(value, policy, findings)),
        _ => true,
    }
}

fn sanitize_string(
    text: &mut String,
    policy: &DnaExtractionPolicy,
    findings: &mut Vec<DnaHygieneFinding>,
) -> bool {
    if looks_like_secret(text) {
        findings.push(safe_hygiene_finding(
            "dna.hygiene.secret",
            "secret",
            policy.secret_action,
        ));
        return false;
    }
    if looks_like_pii(text) {
        findings.push(safe_hygiene_finding(
            "dna.hygiene.pii",
            "personally_identifying_information",
            policy.pii_action,
        ));
        return false;
    }
    for rule in &policy.protected_references {
        if rule.term.is_empty() || !text.to_lowercase().contains(&rule.term.to_lowercase()) {
            continue;
        }
        let action = policy.protected_reference_action;
        findings.push(safe_hygiene_finding(
            "dna.hygiene.protected_reference",
            "protected_reference",
            action,
        ));
        if action == DnaHygieneAction::Rewrite {
            let Some(replacement) = &rule.descriptive_replacement else {
                return false;
            };
            *text = replace_case_insensitive(text, &rule.term, replacement);
        } else {
            return false;
        }
    }
    true
}

fn safe_hygiene_finding(code: &str, class: &str, action: DnaHygieneAction) -> DnaHygieneFinding {
    DnaHygieneFinding {
        code: code.to_string(),
        class: class.to_string(),
        action,
        message: "A sensitive value was omitted or rewritten; the matched value was not retained"
            .to_string(),
    }
}

fn looks_like_secret(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("-----begin private key-----")
        || contains_token_with_tail(&lower, "github_pat_", 12)
        || contains_token_with_tail(&lower, "ghp_", 20)
        || contains_token_with_tail(&lower, "sk-", 20)
        || ["password=", "api_key=", "authorization: bearer "]
            .iter()
            .any(|marker| lower.contains(marker))
}

fn looks_like_pii(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let candidate = token.trim_matches(|character: char| {
            matches!(character, '<' | '>' | '(' | ')' | '[' | ']' | ',' | ';')
        });
        let mut parts = candidate.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        !local.is_empty()
            && domain.contains('.')
            && parts.next().is_none()
            && domain.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-')
            })
    })
}

fn contains_token_with_tail(text: &str, prefix: &str, minimum_tail: usize) -> bool {
    text.match_indices(prefix).any(|(offset, _)| {
        text[offset + prefix.len()..]
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
            .count()
            >= minimum_tail
    })
}

fn replace_case_insensitive(text: &str, term: &str, replacement: &str) -> String {
    let lower = text.to_lowercase();
    let term_lower = term.to_lowercase();
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = lower[cursor..].find(&term_lower) {
        let start = cursor + relative;
        output.push_str(&text[cursor..start]);
        output.push_str(replacement);
        cursor = start + term.len();
    }
    output.push_str(&text[cursor..]);
    output
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnaDimensionSimilarity {
    pub modality: DnaModality,
    pub dimension: String,
    pub score: f64,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDnaComparison {
    pub schema_version: String,
    pub left_artifact_id: String,
    pub right_artifact_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_score: Option<f64>,
    pub dimensions: Vec<DnaDimensionSimilarity>,
    #[serde(default)]
    pub unmatched_dimensions: Vec<String>,
    pub guidance: String,
    pub direct_imitation_allowed: bool,
    pub legal_clearance_claimed: bool,
}

impl ArtifactDnaComparison {
    pub fn compare(left: &ArtifactDna, right: &ArtifactDna) -> Result<Self> {
        left.validate()?;
        right.validate()?;
        let right_by_dimension = right
            .observations
            .iter()
            .filter(|observation| observation.similarity_eligible)
            .map(|observation| {
                (
                    (observation.modality, observation.dimension.as_str()),
                    observation,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut dimensions = Vec::new();
        let mut unmatched = Vec::new();
        for left_observation in left
            .observations
            .iter()
            .filter(|observation| observation.similarity_eligible)
        {
            let key = (
                left_observation.modality,
                left_observation.dimension.as_str(),
            );
            let Some(right_observation) = right_by_dimension.get(&key) else {
                unmatched.push(left_observation.dimension.clone());
                continue;
            };
            let Some(score) = value_similarity(&left_observation.value, &right_observation.value)
            else {
                unmatched.push(left_observation.dimension.clone());
                continue;
            };
            let left_weight = left
                .similarity_guidance
                .dimension_weights
                .get(&left_observation.dimension)
                .copied()
                .unwrap_or(1.0);
            let right_weight = right
                .similarity_guidance
                .dimension_weights
                .get(&right_observation.dimension)
                .copied()
                .unwrap_or(1.0);
            dimensions.push(DnaDimensionSimilarity {
                modality: left_observation.modality,
                dimension: left_observation.dimension.clone(),
                score: round_six(score),
                weight: round_six((left_weight + right_weight) / 2.0),
            });
        }
        dimensions.sort_by(|left, right| {
            (&left.modality, &left.dimension).cmp(&(&right.modality, &right.dimension))
        });
        unmatched.sort();
        unmatched.dedup();
        let weight_sum = dimensions
            .iter()
            .map(|dimension| dimension.weight)
            .sum::<f64>();
        let overall_score = (weight_sum > 0.0).then(|| {
            round_six(
                dimensions
                    .iter()
                    .map(|dimension| dimension.score * dimension.weight)
                    .sum::<f64>()
                    / weight_sum,
            )
        });
        Ok(Self {
            schema_version: ARTIFACT_DNA_COMPARISON_SCHEMA_V1.to_string(),
            left_artifact_id: left.source.artifact_id.clone(),
            right_artifact_id: right.source.artifact_id.clone(),
            overall_score,
            dimensions,
            unmatched_dimensions: unmatched,
            guidance: "Scores compare descriptive dimensions only; they do not authorize copying, establish ownership, or provide legal clearance".to_string(),
            direct_imitation_allowed: false,
            legal_clearance_claimed: false,
        })
    }
}

fn value_similarity(left: &Value, right: &Value) -> Option<f64> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            let left = left.as_f64()?;
            let right = right.as_f64()?;
            let scale = left.abs().max(right.abs()).max(1.0);
            Some((1.0 - (left - right).abs() / scale).clamp(0.0, 1.0))
        }
        (Value::String(left), Value::String(right)) => {
            Some(f64::from(left.eq_ignore_ascii_case(right)))
        }
        (Value::Bool(left), Value::Bool(right)) => Some(f64::from(left == right)),
        (Value::Array(left), Value::Array(right)) => {
            let left = left.iter().map(Value::to_string).collect::<BTreeSet<_>>();
            let right = right.iter().map(Value::to_string).collect::<BTreeSet<_>>();
            let union = left.union(&right).count();
            if union == 0 {
                Some(1.0)
            } else {
                Some(left.intersection(&right).count() as f64 / union as f64)
            }
        }
        (Value::Object(left), Value::Object(right)) => {
            let keys = left.keys().chain(right.keys()).collect::<BTreeSet<_>>();
            if keys.is_empty() {
                return Some(1.0);
            }
            let scores = keys
                .into_iter()
                .filter_map(|key| value_similarity(left.get(key)?, right.get(key)?))
                .collect::<Vec<_>>();
            (!scores.is_empty()).then(|| scores.iter().sum::<f64>() / scores.len() as f64)
        }
        _ => None,
    }
}

fn validate_observation(observation: &DnaObservation) -> Result<()> {
    if observation.id.trim().is_empty()
        || !is_namespaced(&observation.dimension)
        || observation.evidence.provider_id.trim().is_empty()
        || observation.evidence.provider_version.trim().is_empty()
    {
        anyhow::bail!("Artifact DNA observation identifiers must not be empty");
    }
    if !observation.evidence.confidence.is_finite()
        || !(0.0..=1.0).contains(&observation.evidence.confidence)
    {
        anyhow::bail!(
            "observation '{}' confidence must be between zero and one",
            observation.id
        );
    }
    Ok(())
}

fn validate_digest(digest: &DigestEvidence) -> Result<()> {
    if digest.algorithm != "sha256"
        || digest.value.len() != 64
        || !digest
            .value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        anyhow::bail!("Artifact DNA digest must be a lowercase SHA-256 value");
    }
    Ok(())
}

fn is_namespaced(value: &str) -> bool {
    let mut parts = value.split('.');
    parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_some_and(|part| !part.is_empty())
        && parts.all(|part| !part.is_empty())
}

fn digest_serialized<T: Serialize>(value: &T) -> Result<DigestEvidence> {
    let bytes = serde_json::to_vec(value)?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    Ok(DigestEvidence {
        algorithm: "sha256".to_string(),
        value: digest,
    })
}

fn round_six(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Format;

    fn source(store: &ArtifactStore, bytes: &[u8], format: Format) -> Artifact {
        store
            .put_bytes(
                bytes,
                ArtifactDescriptor::for_format(format, ArtifactStorageClass::Source),
            )
            .unwrap()
    }

    #[test]
    fn extraction_is_disabled_by_default() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let source = source(&store, b"# Heading\n\nBody", Format::Markdown);
        let outcome = ArtifactDnaEngine::with_builtins()
            .extract(&source, &store, &DnaExtractionPolicy::default())
            .unwrap();
        assert_eq!(outcome.status, DnaExtractionStatus::Skipped);
        assert!(outcome.dna.is_none());
    }

    #[test]
    fn deterministic_text_dna_does_not_retain_payload() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let source = source(
            &store,
            b"# A private heading\n\nPrivate body words remain private.",
            Format::Markdown,
        );
        let outcome = ArtifactDnaEngine::with_builtins()
            .extract(&source, &store, &DnaExtractionPolicy::explicit_local())
            .unwrap();
        let dna = outcome.dna.unwrap();
        let encoded = serde_json::to_string(&dna).unwrap();
        assert_eq!(outcome.status, DnaExtractionStatus::Complete);
        assert!(dna.modalities.contains(&DnaModality::Textual));
        assert!(dna.modalities.contains(&DnaModality::Layout));
        assert!(!encoded.contains("private heading"));
        assert!(!encoded.contains("Private body"));
        assert_eq!(dna.approval.state, DnaReviewState::Candidate);
        assert!(!dna.approval.human_review_required);
        assert!(outcome.artifact.is_some());
        store.verify(&source).unwrap();
    }

    #[test]
    fn svg_extraction_emits_geometry_palette_and_layout() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="1600"><rect fill="#112233"/><text fill="#ffeeaa">Safe synthetic title</text></svg>"##;
        let source = source(&store, svg, Format::Svg);
        let outcome = ArtifactDnaEngine::with_builtins()
            .extract(&source, &store, &DnaExtractionPolicy::explicit_local())
            .unwrap();
        let dna = outcome.dna.unwrap();
        assert!(dna.observations.iter().any(|observation| {
            observation.dimension == "layout.canvas.orientation" && observation.value == "portrait"
        }));
        assert!(dna.observations.iter().any(|observation| {
            observation.dimension == "visual.palette.hex"
                && observation.value == json!(["#112233", "#ffeeaa"])
        }));
    }

    #[test]
    fn png_header_geometry_is_extracted_without_a_decoder() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend_from_slice(&800_u32.to_be_bytes());
        png.extend_from_slice(&600_u32.to_be_bytes());
        let source = source(&store, &png, Format::Png);
        let dna = ArtifactDnaEngine::with_builtins()
            .extract(&source, &store, &DnaExtractionPolicy::explicit_local())
            .unwrap()
            .dna
            .unwrap();
        assert!(dna.observations.iter().any(|observation| {
            observation.dimension == "layout.canvas.aspect_ratio"
                && observation.value == json!(1.333333)
        }));
    }

    #[test]
    fn provider_output_is_sanitized_before_exposure() {
        struct UnsafeExtractor;
        impl ArtifactDnaExtractor for UnsafeExtractor {
            fn id(&self) -> &str {
                "fixture.unsafe"
            }
            fn version(&self) -> &str {
                "1.0.0"
            }
            fn locality(&self) -> DnaProviderLocality {
                DnaProviderLocality::Local
            }
            fn determinism(&self) -> DnaDeterminism {
                DnaDeterminism::Probabilistic
            }
            fn ai_assisted(&self) -> bool {
                true
            }
            fn requires_network(&self) -> bool {
                false
            }
            fn supports(&self, _: &Artifact) -> bool {
                true
            }
            fn extract(&self, _: &Artifact, _: &ArtifactStore) -> Result<DnaExtractionBatch> {
                let unsafe_observation = |id: &str, value: &str| DnaObservation {
                    id: id.to_string(),
                    modality: DnaModality::Visual,
                    dimension: format!("visual.texture.{id}"),
                    value: json!(value),
                    description: None,
                    evidence: DnaObservationEvidence {
                        origin: DnaEvidenceOrigin::Inferred,
                        provider_id: self.id().to_string(),
                        provider_version: self.version().to_string(),
                        determinism: self.determinism(),
                        confidence: 0.7,
                        scope: BTreeMap::new(),
                    },
                    similarity_eligible: true,
                };
                Ok(DnaExtractionBatch {
                    modalities: vec![DnaModality::Visual],
                    observations: vec![
                        unsafe_observation("reference", "Example Franchise texture"),
                        unsafe_observation("secret", "sk-fixturecredential012345678901234567890"),
                        unsafe_observation("pii", "creator@example.invalid"),
                    ],
                    ..DnaExtractionBatch::default()
                })
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let source = source(&store, b"safe", Format::Png);
        let mut policy = DnaExtractionPolicy::explicit_local();
        policy.allow_ai = true;
        policy.protected_references = vec![DnaProtectedReferenceRule {
            term: "Example Franchise".to_string(),
            descriptive_replacement: Some("weathered geometric".to_string()),
        }];
        let outcome = ArtifactDnaEngine::new()
            .with_extractor(Arc::new(UnsafeExtractor))
            .extract(&source, &store, &policy)
            .unwrap();
        let dna = outcome.dna.unwrap();
        let encoded = serde_json::to_string(&dna).unwrap();
        assert!(!encoded.contains("Example Franchise"));
        assert!(!encoded.contains("fixturecredential"));
        assert!(!encoded.contains("creator@example.invalid"));
        assert!(encoded.contains("weathered geometric"));
        assert_eq!(dna.observations.len(), 1);
        assert!(dna.approval.human_review_required);
        assert_eq!(dna.hygiene.status, DnaHygieneStatus::Sanitized);
    }

    #[test]
    fn remote_ai_extractors_require_each_explicit_permission() {
        struct RemoteExtractor;
        impl ArtifactDnaExtractor for RemoteExtractor {
            fn id(&self) -> &str {
                "fixture.remote-ai"
            }
            fn version(&self) -> &str {
                "1.0.0"
            }
            fn locality(&self) -> DnaProviderLocality {
                DnaProviderLocality::Remote
            }
            fn determinism(&self) -> DnaDeterminism {
                DnaDeterminism::Probabilistic
            }
            fn ai_assisted(&self) -> bool {
                true
            }
            fn requires_network(&self) -> bool {
                true
            }
            fn supports(&self, _: &Artifact) -> bool {
                true
            }
            fn extract(&self, _: &Artifact, _: &ArtifactStore) -> Result<DnaExtractionBatch> {
                panic!("policy must reject the extractor before execution")
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let source = source(&store, b"safe", Format::Png);
        let engine = ArtifactDnaEngine::new().with_extractor(Arc::new(RemoteExtractor));
        let mut policy = DnaExtractionPolicy::explicit_local();
        let outcome = engine.extract(&source, &store, &policy).unwrap();
        assert_eq!(outcome.status, DnaExtractionStatus::Skipped);
        assert!(outcome.diagnostics[0].contains("AI-assisted extraction"));
        policy.allow_ai = true;
        let outcome = engine.extract(&source, &store, &policy).unwrap();
        assert!(outcome.diagnostics[0].contains("network access"));
        policy.allow_network = true;
        let outcome = engine.extract(&source, &store, &policy).unwrap();
        assert!(outcome.diagnostics[0].contains("remote execution"));
    }

    #[test]
    fn similarity_is_explainable_and_not_clearance() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let left = source(
            &store,
            br##"<svg width="100" height="200"><rect fill="#112233"/></svg>"##,
            Format::Svg,
        );
        let right = source(
            &store,
            br##"<svg width="100" height="200"><rect fill="#112233"/></svg>"##,
            Format::Svg,
        );
        let engine = ArtifactDnaEngine::with_builtins();
        let policy = DnaExtractionPolicy::explicit_local();
        let left = engine.extract(&left, &store, &policy).unwrap().dna.unwrap();
        let right = engine
            .extract(&right, &store, &policy)
            .unwrap()
            .dna
            .unwrap();
        let comparison = ArtifactDnaComparison::compare(&left, &right).unwrap();
        assert_eq!(comparison.overall_score, Some(1.0));
        assert!(!comparison.dimensions.is_empty());
        assert!(!comparison.direct_imitation_allowed);
        assert!(!comparison.legal_clearance_claimed);
    }

    #[test]
    fn modality_fixtures_conform_to_the_rust_contract() {
        let fixtures = [
            include_str!("../../../tests/fixtures/artifact-dna/visual.json"),
            include_str!("../../../tests/fixtures/artifact-dna/sonic.json"),
            include_str!("../../../tests/fixtures/artifact-dna/textual.json"),
            include_str!("../../../tests/fixtures/artifact-dna/compound.json"),
        ];
        for fixture in fixtures {
            ArtifactDna::from_json(fixture).unwrap();
        }
    }
}
