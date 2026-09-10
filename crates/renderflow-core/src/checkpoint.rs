//! Durable, corruption-detectable execution checkpoints and recovery decisions.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::artifact::{Artifact, ArtifactCollection, ArtifactStore};
use crate::evidence::{DigestEvidence, FidelityDeclaration, ValidationState, ValidatorEvidence};

pub const CHECKPOINT_SCHEMA_V1: &str = "renderflow.checkpoints/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    Retry,
    Reuse,
    Recompute,
    Skip,
    TerminalIncompatibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecoveryDecision {
    pub action: RecoveryAction,
    pub reason_code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_ids: Vec<String>,
}

impl RecoveryDecision {
    fn recompute(code: &str, message: impl Into<String>, key: Option<&str>) -> Self {
        Self {
            action: RecoveryAction::Recompute,
            reason_code: code.to_string(),
            message: message.into(),
            checkpoint_key: key.map(ToString::to_string),
            artifact_ids: Vec::new(),
        }
    }

    fn terminal(code: &str, message: impl Into<String>) -> Self {
        Self {
            action: RecoveryAction::TerminalIncompatibility,
            reason_code: code.to_string(),
            message: message.into(),
            checkpoint_key: None,
            artifact_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointContext {
    pub execution_plan_digest: DigestEvidence,
    pub source_spec_digest: DigestEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointArtifactRef {
    pub artifact_id: String,
    pub digest: DigestEvidence,
    pub size_bytes: u64,
}

impl From<&Artifact> for CheckpointArtifactRef {
    fn from(artifact: &Artifact) -> Self {
        Self {
            artifact_id: artifact.id().to_string(),
            digest: DigestEvidence {
                algorithm: artifact.digest().algorithm().to_string(),
                value: artifact.digest().value().to_string(),
            },
            size_bytes: artifact.size_bytes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointRequirement {
    pub checkpoint_key: String,
    pub step_id: String,
    pub input_artifacts: Vec<CheckpointArtifactRef>,
    pub transform: String,
    pub transform_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub configuration_digest: DigestEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StepCheckpoint {
    #[serde(flatten)]
    pub requirement: CheckpointRequirement,
    pub outputs: Vec<Artifact>,
    pub validation: ValidationState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_evidence: Vec<ValidatorEvidence>,
    pub fidelity: FidelityDeclaration,
    pub completed_at_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct CheckpointPayload {
    context: CheckpointContext,
    #[serde(default)]
    entries: BTreeMap<String, StepCheckpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct CheckpointEnvelope {
    schema_version: String,
    checksum: DigestEvidence,
    payload: CheckpointPayload,
}

#[derive(Debug, Clone)]
pub struct CheckpointStore {
    path: PathBuf,
    payload: CheckpointPayload,
}

impl CheckpointStore {
    pub fn reset(path: impl Into<PathBuf>, context: CheckpointContext) -> Result<Self> {
        let store = Self {
            path: path.into(),
            payload: CheckpointPayload {
                context,
                entries: BTreeMap::new(),
            },
        };
        store.persist()?;
        Ok(store)
    }

    pub fn open(path: impl Into<PathBuf>, context: CheckpointContext) -> Result<Self> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self {
                path,
                payload: CheckpointPayload {
                    context,
                    entries: BTreeMap::new(),
                },
            });
        }
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read checkpoint file '{}'", path.display()))?;
        let envelope: CheckpointEnvelope = serde_json::from_slice(&bytes)
            .with_context(|| format!("checkpoint file '{}' is not valid JSON", path.display()))?;
        if envelope.schema_version != CHECKPOINT_SCHEMA_V1 {
            anyhow::bail!(
                "unsupported checkpoint schema '{}' in '{}'",
                envelope.schema_version,
                path.display()
            );
        }
        let actual = digest_payload(&envelope.payload)?;
        if envelope.checksum != actual {
            anyhow::bail!("checkpoint checksum mismatch in '{}'", path.display());
        }
        Ok(Self {
            path,
            payload: envelope.payload,
        })
    }

    pub fn context_decision(&self, current: &CheckpointContext) -> RecoveryDecision {
        if self.payload.context.source_spec_digest != current.source_spec_digest {
            return RecoveryDecision::recompute(
                "checkpoint.source_spec_changed",
                "Source specification changed; dependent work must be recomputed",
                None,
            );
        }
        if self.payload.context.execution_plan_digest != current.execution_plan_digest {
            return RecoveryDecision::recompute(
                "checkpoint.plan_changed",
                "Execution plan changed; affected work must be recomputed",
                None,
            );
        }
        if self.payload.context.toolchain_fingerprint != current.toolchain_fingerprint {
            return RecoveryDecision::recompute(
                "checkpoint.toolchain_changed",
                "Selected toolchain changed; affected work must be recomputed",
                None,
            );
        }
        RecoveryDecision {
            action: RecoveryAction::Reuse,
            reason_code: "checkpoint.context_compatible".to_string(),
            message: "Checkpoint context is compatible".to_string(),
            checkpoint_key: None,
            artifact_ids: Vec::new(),
        }
    }

    pub fn adopt_context(&mut self, context: CheckpointContext) {
        self.payload.context = context;
    }

    pub fn is_empty(&self) -> bool {
        self.payload.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.payload.entries.len()
    }

    pub fn assess(
        &self,
        requirement: &CheckpointRequirement,
        store: &ArtifactStore,
    ) -> RecoveryDecision {
        let Some(entry) = self.payload.entries.get(&requirement.checkpoint_key) else {
            return RecoveryDecision::recompute(
                "checkpoint.missing",
                "No completed checkpoint exists for this node",
                Some(&requirement.checkpoint_key),
            );
        };
        if entry.requirement != *requirement {
            return RecoveryDecision::recompute(
                "checkpoint.requirement_changed",
                "Inputs, transform configuration, implementation, or toolchain changed",
                Some(&requirement.checkpoint_key),
            );
        }
        if !matches!(
            entry.validation,
            ValidationState::Valid | ValidationState::ValidWithWarnings
        ) {
            return RecoveryDecision::recompute(
                "checkpoint.validation_incompatible",
                "Checkpoint output was not successfully validated",
                Some(&requirement.checkpoint_key),
            );
        }
        for artifact in &entry.outputs {
            if !store.contains(artifact) {
                return RecoveryDecision::recompute(
                    "checkpoint.output_missing",
                    format!("Checkpoint output '{}' is missing", artifact.id()),
                    Some(&requirement.checkpoint_key),
                );
            }
            if let Err(error) = store.verify(artifact) {
                return RecoveryDecision::recompute(
                    "checkpoint.output_corrupt",
                    format!(
                        "Checkpoint output '{}' failed verification: {error}",
                        artifact.id()
                    ),
                    Some(&requirement.checkpoint_key),
                );
            }
        }
        RecoveryDecision {
            action: RecoveryAction::Reuse,
            reason_code: "checkpoint.compatible".to_string(),
            message: "Completed checkpoint is compatible".to_string(),
            checkpoint_key: Some(requirement.checkpoint_key.clone()),
            artifact_ids: entry
                .outputs
                .iter()
                .map(|artifact| artifact.id().to_string())
                .collect(),
        }
    }

    pub fn outputs(&self, checkpoint_key: &str) -> Option<ArtifactCollection> {
        self.payload.entries.get(checkpoint_key).map(|entry| {
            ArtifactCollection::new(
                entry
                    .outputs
                    .iter()
                    .cloned()
                    .map(|artifact| {
                        artifact.with_storage_class(crate::artifact::ArtifactStorageClass::Cached)
                    })
                    .collect(),
            )
        })
    }

    pub fn checkpoint(&self, checkpoint_key: &str) -> Option<&StepCheckpoint> {
        self.payload.entries.get(checkpoint_key)
    }

    pub fn record(&mut self, checkpoint: StepCheckpoint) -> Result<()> {
        self.payload
            .entries
            .insert(checkpoint.requirement.checkpoint_key.clone(), checkpoint);
        self.persist()
    }

    pub fn invalidate_step(&mut self, step_id: &str) -> Result<usize> {
        let before = self.payload.entries.len();
        self.payload
            .entries
            .retain(|_, entry| entry.requirement.step_id != step_id);
        let removed = before.saturating_sub(self.payload.entries.len());
        self.persist()?;
        Ok(removed)
    }

    pub fn invalidate_all(&mut self) -> Result<usize> {
        let removed = self.payload.entries.len();
        self.payload.entries.clear();
        self.persist()?;
        Ok(removed)
    }

    pub fn decisions(&self, store: &ArtifactStore) -> Vec<RecoveryDecision> {
        self.payload
            .entries
            .values()
            .map(|entry| self.assess(&entry.requirement, store))
            .collect()
    }

    pub fn decisions_for_source(
        &self,
        current_source_artifact_id: &str,
        store: &ArtifactStore,
    ) -> Vec<RecoveryDecision> {
        let output_ids = self
            .payload
            .entries
            .values()
            .flat_map(|entry| entry.outputs.iter())
            .map(|artifact| artifact.id().to_string())
            .collect::<BTreeSet<_>>();
        let root_input_ids = self
            .payload
            .entries
            .values()
            .flat_map(|entry| entry.requirement.input_artifacts.iter())
            .map(|artifact| artifact.artifact_id.clone())
            .filter(|artifact_id| !output_ids.contains(artifact_id))
            .collect::<BTreeSet<_>>();
        if root_input_ids.contains(current_source_artifact_id) {
            return self.decisions(store);
        }

        let mut invalidated = root_input_ids;
        loop {
            let before = invalidated.len();
            for entry in self.payload.entries.values() {
                if entry
                    .requirement
                    .input_artifacts
                    .iter()
                    .any(|input| invalidated.contains(&input.artifact_id))
                {
                    invalidated.extend(
                        entry
                            .outputs
                            .iter()
                            .map(|artifact| artifact.id().to_string()),
                    );
                }
            }
            if invalidated.len() == before {
                break;
            }
        }

        self.payload
            .entries
            .values()
            .map(|entry| {
                if entry
                    .requirement
                    .input_artifacts
                    .iter()
                    .any(|input| invalidated.contains(&input.artifact_id))
                {
                    RecoveryDecision::recompute(
                        "checkpoint.source_changed",
                        "Current source identity differs from this checkpoint dependency chain",
                        Some(&entry.requirement.checkpoint_key),
                    )
                } else {
                    self.assess(&entry.requirement, store)
                }
            })
            .collect()
    }

    fn persist(&self) -> Result<()> {
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create checkpoint directory '{}'",
                parent.display()
            )
        })?;
        let envelope = CheckpointEnvelope {
            schema_version: CHECKPOINT_SCHEMA_V1.to_string(),
            checksum: digest_payload(&self.payload)?,
            payload: self.payload.clone(),
        };
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .context("failed to create checkpoint temporary file")?;
        serde_json::to_writer_pretty(&mut temporary, &envelope)
            .context("failed to serialize checkpoints")?;
        temporary.flush().context("failed to flush checkpoints")?;
        temporary
            .as_file()
            .sync_all()
            .context("failed to sync checkpoints")?;
        temporary
            .persist(&self.path)
            .map_err(|error| error.error)
            .with_context(|| {
                format!(
                    "failed to atomically replace checkpoint file '{}'",
                    self.path.display()
                )
            })?;
        Ok(())
    }

    pub fn corruption_decision(error: &anyhow::Error) -> RecoveryDecision {
        RecoveryDecision::terminal(
            "checkpoint.corrupt",
            format!("Checkpoint state is corrupt or unreadable: {error}"),
        )
    }
}

pub fn checkpoint_key(requirement_material: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(requirement_material)?;
    Ok(format!("checkpoint:sha256:{:x}", Sha256::digest(bytes)))
}

fn digest_payload(payload: &CheckpointPayload) -> Result<DigestEvidence> {
    let bytes = serde_json::to_vec(payload)?;
    Ok(DigestEvidence {
        algorithm: "sha256".to_string(),
        value: format!("{:x}", Sha256::digest(bytes)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass};
    use crate::evidence::{sha256_text, unix_time_ms};
    use crate::graph::Format;

    fn context() -> CheckpointContext {
        CheckpointContext {
            execution_plan_digest: sha256_text("plan"),
            source_spec_digest: sha256_text("spec"),
            toolchain_fingerprint: Some("tools-v1".to_string()),
        }
    }

    fn fixture() -> (
        tempfile::TempDir,
        ArtifactStore,
        PathBuf,
        CheckpointRequirement,
        Artifact,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("artifacts")).unwrap();
        let input = store
            .put_bytes(
                b"source",
                ArtifactDescriptor::for_format(Format::Markdown, ArtifactStorageClass::Source),
            )
            .unwrap();
        let output = store
            .put_bytes(
                b"<p>source</p>",
                ArtifactDescriptor::for_format(Format::Html, ArtifactStorageClass::Intermediate)
                    .with_source(input.id().clone()),
            )
            .unwrap();
        let requirement = CheckpointRequirement {
            checkpoint_key: checkpoint_key(&(input.id().to_string(), "markdown-html-v1")).unwrap(),
            step_id: "step:markdown-to-html".to_string(),
            input_artifacts: vec![CheckpointArtifactRef::from(&input)],
            transform: "markdown-html".to_string(),
            transform_version: "1".to_string(),
            provider_id: Some("tool.fixture".to_string()),
            configuration_digest: sha256_text("config"),
            toolchain_fingerprint: Some("tools-v1".to_string()),
        };
        let path = directory.path().join("checkpoints.json");
        (directory, store, path, requirement, output)
    }

    #[test]
    fn compatible_checkpoint_is_idempotently_reusable() {
        let (_directory, store, path, requirement, output) = fixture();
        let mut checkpoints = CheckpointStore::open(&path, context()).unwrap();
        checkpoints
            .record(StepCheckpoint {
                requirement: requirement.clone(),
                outputs: vec![output],
                validation: ValidationState::Valid,
                validation_evidence: Vec::new(),
                fidelity: FidelityDeclaration::Lossless,
                completed_at_unix_ms: unix_time_ms(),
            })
            .unwrap();
        let reopened = CheckpointStore::open(path, context()).unwrap();
        assert_eq!(
            reopened.assess(&requirement, &store).action,
            RecoveryAction::Reuse
        );
        assert!(reopened
            .decisions_for_source("artifact:sha256:different-source", &store)
            .iter()
            .all(|decision| decision.action == RecoveryAction::Recompute));
        assert_eq!(
            reopened.assess(&requirement, &store).action,
            RecoveryAction::Reuse
        );
    }

    #[test]
    fn changed_configuration_and_corrupt_outputs_force_recompute() {
        let (_directory, store, path, requirement, output) = fixture();
        let mut checkpoints = CheckpointStore::open(&path, context()).unwrap();
        checkpoints
            .record(StepCheckpoint {
                requirement: requirement.clone(),
                outputs: vec![output.clone()],
                validation: ValidationState::Valid,
                validation_evidence: Vec::new(),
                fidelity: FidelityDeclaration::Lossless,
                completed_at_unix_ms: unix_time_ms(),
            })
            .unwrap();
        let mut changed = requirement.clone();
        changed.configuration_digest = sha256_text("different-config");
        assert_eq!(
            checkpoints.assess(&changed, &store).action,
            RecoveryAction::Recompute
        );
        fs::write(store.payload_path(&output).unwrap(), b"corrupt").unwrap();
        assert_eq!(
            checkpoints.assess(&requirement, &store).action,
            RecoveryAction::Recompute
        );
    }

    #[test]
    fn modified_checkpoint_payload_is_detected() {
        let (_directory, _store, path, requirement, output) = fixture();
        let mut checkpoints = CheckpointStore::open(&path, context()).unwrap();
        checkpoints
            .record(StepCheckpoint {
                requirement,
                outputs: vec![output],
                validation: ValidationState::Valid,
                validation_evidence: Vec::new(),
                fidelity: FidelityDeclaration::Lossless,
                completed_at_unix_ms: unix_time_ms(),
            })
            .unwrap();
        let original = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            original.replace("markdown-html", "tampered-transform"),
        )
        .unwrap();
        assert!(CheckpointStore::open(path, context()).is_err());
    }
}
