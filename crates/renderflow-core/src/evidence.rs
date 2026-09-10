//! Versioned, machine-readable execution evidence.
//!
//! Renderflow owns this richer native model. Integrations may project artifact
//! records into a supported interchange contract without making that external
//! contract part of the artifact kernel's internal representation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::artifact::{Artifact, ArtifactStorageClass};
use crate::toolchain::ToolchainSnapshot;

pub const RUN_MANIFEST_SCHEMA_V1: &str = "renderflow.run/v1";
pub const ARTIFACT_MANIFEST_SCHEMA_V1: &str = "renderflow.artifact-manifest/v1";
pub const FLOW_ARTIFACT_SCHEMA_V1: &str = "flow.artifact/v1";

static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Planned,
    Complete,
    Partial,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Complete,
    Reused,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationState {
    Valid,
    ValidWithWarnings,
    Invalid,
    Unavailable,
    Skipped,
    NotRequested,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CacheDisposition {
    Source,
    Miss,
    Hit,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    Source,
    Intermediate,
    Terminal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FidelityDeclaration {
    Lossless,
    Partial,
    Lossy,
    PathDependent,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    RecoverableFailure,
    FatalFailure,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DigestEvidence {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProducerEvidence {
    pub system: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl ProducerEvidence {
    pub fn source() -> Self {
        Self {
            system: "renderflow".to_string(),
            transform: None,
            capability: Some("artifact.source".to_string()),
            provider: None,
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEvidence {
    pub artifact_id: String,
    /// Logical role from target/source intent (for example, `web` or `manuscript`).
    pub role: String,
    /// Lifecycle position inside this execution.
    pub lifecycle: ArtifactRole,
    pub locator: String,
    pub format: String,
    pub media_type: String,
    pub digest: DigestEvidence,
    pub size_bytes: u64,
    pub producer: ProducerEvidence,
    pub sources: Vec<String>,
    pub cache: CacheDisposition,
    pub validation: ValidationState,
    pub fidelity: FidelityDeclaration,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

impl ArtifactEvidence {
    pub fn from_artifact(
        artifact: &Artifact,
        role: impl Into<String>,
        lifecycle: ArtifactRole,
        locator: impl Into<String>,
        producer: ProducerEvidence,
        validation: ValidationState,
        fidelity: FidelityDeclaration,
    ) -> Self {
        let cache = match artifact.storage_class() {
            ArtifactStorageClass::Source => CacheDisposition::Source,
            ArtifactStorageClass::Cached => CacheDisposition::Hit,
            ArtifactStorageClass::Intermediate
            | ArtifactStorageClass::Terminal
            | ArtifactStorageClass::Ephemeral => CacheDisposition::Miss,
        };
        Self {
            artifact_id: artifact.id().to_string(),
            role: role.into(),
            lifecycle,
            locator: locator.into(),
            format: artifact.format().to_string(),
            media_type: artifact.media_type().to_string(),
            digest: DigestEvidence {
                algorithm: artifact.digest().algorithm().to_string(),
                value: artifact.digest().value().to_string(),
            },
            size_bytes: artifact.size_bytes(),
            producer,
            sources: artifact.sources().iter().map(ToString::to_string).collect(),
            cache,
            validation,
            fidelity,
            warnings: Vec::new(),
            metadata: artifact
                .metadata()
                .iter()
                .filter(|(key, _)| key.starts_with("renderflow.") && !is_sensitive_key(key))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        }
    }

    pub fn to_flow_v1(&self) -> FlowArtifactV1 {
        let mut seen_sources = BTreeSet::new();
        FlowArtifactV1 {
            schema_version: FLOW_ARTIFACT_SCHEMA_V1.to_string(),
            artifact_id: flow_artifact_id(&self.artifact_id),
            role: self.role.clone(),
            media_type: self.media_type.clone(),
            digest: self.digest.clone(),
            size_bytes: self.size_bytes,
            producer: FlowProducerV1 {
                owner: self.producer.system.clone(),
                capability_id: self
                    .producer
                    .capability
                    .clone()
                    .or_else(|| self.producer.transform.clone())
                    .unwrap_or_else(|| "artifact.produce".to_string()),
                provider_version: self
                    .producer
                    .version
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            },
            sources: self
                .sources
                .iter()
                .map(|source| flow_artifact_id(source))
                .filter(|source| seen_sources.insert(source.clone()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StepEvidence {
    pub step_id: String,
    pub transform: String,
    pub transform_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub input_artifacts: Vec<String>,
    pub output_artifacts: Vec<String>,
    pub configuration_digest: DigestEvidence,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: u64,
    pub duration_ms: u64,
    pub state: StepState,
    pub cache: CacheDisposition,
    pub validation: ValidationState,
    pub fidelity: FidelityDeclaration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExecutionDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    pub schema_version: String,
    pub run_id: String,
    pub output_dir: String,
    pub outputs: Vec<String>,
    pub artifacts: Vec<ArtifactEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    pub schema_version: String,
    pub run_id: String,
    pub execution_plan_digest: DigestEvidence,
    pub source_spec_digest: DigestEvidence,
    pub engine_version: String,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: u64,
    pub state: RunState,
    pub artifact_manifest: ArtifactManifest,
    pub steps: Vec<StepEvidence>,
    #[serde(default)]
    pub diagnostics: Vec<ExecutionDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<ToolchainSnapshot>,
}

impl RunManifest {
    pub fn flow_artifacts_v1(&self) -> Vec<FlowArtifactV1> {
        self.artifact_manifest
            .artifacts
            .iter()
            .map(ArtifactEvidence::to_flow_v1)
            .collect()
    }

    pub fn cache_hits(&self) -> Vec<String> {
        self.steps
            .iter()
            .filter(|step| step.cache == CacheDisposition::Hit)
            .flat_map(|step| step.output_artifacts.iter().cloned())
            .collect()
    }

    pub fn skipped_transforms(&self) -> Vec<String> {
        self.steps
            .iter()
            .filter(|step| step.state == StepState::Skipped)
            .map(|step| step.step_id.clone())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FlowProducerV1 {
    pub owner: String,
    pub capability_id: String,
    pub provider_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FlowArtifactV1 {
    pub schema_version: String,
    pub artifact_id: String,
    pub role: String,
    pub media_type: String,
    pub digest: DigestEvidence,
    pub size_bytes: u64,
    pub producer: FlowProducerV1,
    pub sources: Vec<String>,
}

pub fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

pub fn sha256_serialized<T: Serialize>(value: &T) -> anyhow::Result<DigestEvidence> {
    let bytes = serde_json::to_vec(value).context("failed to serialize evidence digest input")?;
    Ok(sha256_bytes(&bytes))
}

pub fn sha256_text(value: &str) -> DigestEvidence {
    sha256_bytes(value.as_bytes())
}

/// Redact common credential assignments before provider errors enter durable evidence.
pub(crate) fn redact_sensitive_text(value: &str) -> String {
    let mut redact_next = false;
    value
        .split_whitespace()
        .map(|word| {
            if redact_next {
                if word.eq_ignore_ascii_case("bearer") {
                    return word.to_string();
                }
                redact_next = false;
                return "[REDACTED]".to_string();
            }
            let lower = word.to_ascii_lowercase();
            if lower == "bearer" || lower.ends_with("authorization:") {
                redact_next = true;
                return word.to_string();
            }
            for marker in [
                "api_key=",
                "apikey=",
                "token=",
                "secret=",
                "password=",
                "credential=",
                "authorization=",
            ] {
                if let Some(index) = lower.find(marker) {
                    return format!(
                        "{}{}[REDACTED]",
                        &word[..index],
                        &word[index..index + marker.len()]
                    );
                }
            }
            word.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "credential",
        "authorization",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

pub fn run_id(plan_digest: &DigestEvidence, started_at_unix_ms: u64) -> String {
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let material = format!(
        "{}\0{}\0{}\0{}\0{}",
        plan_digest.algorithm,
        plan_digest.value,
        started_at_unix_ms,
        std::process::id(),
        sequence
    );
    let digest = sha256_text(&material);
    format!("run:sha256:{}", digest.value)
}

fn sha256_bytes(bytes: &[u8]) -> DigestEvidence {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    DigestEvidence {
        algorithm: "sha256".to_string(),
        value: format!("{:x}", hasher.finalize()),
    }
}

fn flow_artifact_id(native_id: &str) -> String {
    let suffix = native_id
        .strip_prefix("artifact:")
        .unwrap_or(native_id)
        .replace(':', "-")
        .to_ascii_lowercase();
    let sanitized: String = suffix
        .chars()
        .map(|character| {
            if character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-')
            {
                character
            } else {
                '-'
            }
        })
        .collect();
    format!("artifact:{sanitized}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct OutcomeFixture {
        name: String,
        run_state: RunState,
        step_state: StepState,
        cache: CacheDisposition,
        validation: ValidationState,
        severity: DiagnosticSeverity,
    }

    #[test]
    fn flow_projection_uses_contract_safe_artifact_ids() {
        assert_eq!(
            flow_artifact_id("artifact:sha256:ABC123"),
            "artifact:sha256-abc123"
        );
    }

    #[test]
    fn run_states_distinguish_non_successful_outcomes() {
        let values = [
            RunState::Complete,
            RunState::Partial,
            RunState::Failed,
            RunState::Cancelled,
        ];
        let serialized = serde_json::to_string(&values).unwrap();
        assert!(serialized.contains("partial"));
        assert!(serialized.contains("failed"));
        assert!(serialized.contains("cancelled"));
    }

    #[test]
    fn flow_projection_matches_the_supported_producer_shape() {
        let producer = FlowProducerV1 {
            owner: "renderflow".to_string(),
            capability_id: "document.render".to_string(),
            provider_version: "0.2.1".to_string(),
        };
        let value = serde_json::to_value(producer).unwrap();
        assert_eq!(value["owner"], "renderflow");
        assert_eq!(value["capability_id"], "document.render");
        assert_eq!(value["provider_version"], "0.2.1");
        assert!(value.get("system").is_none());
        assert!(value.get("capability").is_none());
        assert!(value.get("version").is_none());
    }

    #[test]
    fn flow_projection_matches_pinned_v1_compatibility_fixture() {
        let native = ArtifactEvidence {
            artifact_id: "artifact:sha256:ABC123".to_string(),
            role: "web".to_string(),
            lifecycle: ArtifactRole::Terminal,
            locator: "bundle:index.html".to_string(),
            format: "html".to_string(),
            media_type: "text/html".to_string(),
            digest: DigestEvidence {
                algorithm: "sha256".to_string(),
                value: "0".repeat(64),
            },
            size_bytes: 42,
            producer: ProducerEvidence {
                system: "renderflow".to_string(),
                transform: Some("html-render".to_string()),
                capability: Some("document.render".to_string()),
                provider: Some("pandoc".to_string()),
                version: Some("1.2.3".to_string()),
            },
            sources: vec!["artifact:sha256:SOURCE123".to_string()],
            cache: CacheDisposition::Miss,
            validation: ValidationState::Valid,
            fidelity: FidelityDeclaration::Lossless,
            warnings: Vec::new(),
            metadata: BTreeMap::new(),
        };
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/execution-evidence/flow-artifact-v1.json"
        ))
        .unwrap();

        assert_eq!(serde_json::to_value(native.to_flow_v1()).unwrap(), expected);
    }

    #[test]
    fn outcome_fixture_matrix_covers_required_states() {
        let fixtures: Vec<OutcomeFixture> = serde_json::from_str(include_str!(
            "../tests/fixtures/execution-evidence/outcome-matrix.json"
        ))
        .unwrap();
        assert_eq!(fixtures.len(), 6);
        assert!(fixtures
            .iter()
            .any(|fixture| fixture.run_state == RunState::Partial));
        assert!(fixtures
            .iter()
            .any(|fixture| fixture.run_state == RunState::Cancelled));
        assert!(fixtures
            .iter()
            .any(|fixture| fixture.step_state == StepState::Reused));
        assert!(fixtures
            .iter()
            .any(|fixture| fixture.cache == CacheDisposition::Hit));
        assert!(fixtures
            .iter()
            .any(|fixture| fixture.validation == ValidationState::Invalid));
        assert!(fixtures
            .iter()
            .any(|fixture| fixture.severity == DiagnosticSeverity::FatalFailure));
        assert!(fixtures.iter().all(|fixture| !fixture.name.is_empty()));
    }

    #[test]
    fn durable_diagnostics_redact_common_secret_assignments() {
        let redacted = redact_sensitive_text(
            "provider failed api_key=super-secret Authorization: Bearer abc123 token=xyz",
        );
        assert!(!redacted.contains("super-secret"));
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("xyz"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn checked_in_run_schema_tracks_the_runtime_version() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../schemas/renderflow-run-v1.schema.json"
        ))
        .unwrap();
        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            RUN_MANIFEST_SCHEMA_V1
        );
        assert_eq!(
            schema["$defs"]["artifact_manifest"]["properties"]["schema_version"]["const"],
            ARTIFACT_MANIFEST_SCHEMA_V1
        );
    }
}
