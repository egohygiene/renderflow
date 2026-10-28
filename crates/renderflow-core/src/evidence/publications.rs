//! Borrowed publication views over existing native evidence; no I/O or new wire format.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    ArtifactEvidence, ArtifactRole, RunManifest, RunState, StepEvidence, StepState,
    ValidationState, ARTIFACT_MANIFEST_SCHEMA_V1, RUN_MANIFEST_SCHEMA_V1,
};
use crate::graph::{
    ForestBranch, ForestBranchState, ForestTargetExecution, ForestTargetOutcome,
    FOREST_EXECUTION_SCHEMA_V1,
};

/// One explicitly published target and its complete borrowed native evidence.
///
/// The artifact retains validation, fidelity, hygiene, lineage and family metadata.
/// `execution.source_identity` distinguishes unchanged source exports from new
/// derivatives; route intent alone cannot do so after publication hygiene.
/// Locators and digests remain declarations until the caller checks actual files.
#[derive(Debug, Clone, Copy)]
pub struct PublishedTarget<'a> {
    pub branch: &'a ForestBranch,
    pub execution: &'a ForestTargetExecution,
    pub artifact: &'a ArtifactEvidence,
    pub producing_step: Option<&'a StepEvidence>,
}

/// Structural publication evidence could not be joined without guessing.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum PublishedTargetsError {
    #[error("run has no artifact-forest evidence")]
    MissingForest,
    #[error("artifact forest has no explicit target execution evidence")]
    MissingExecution,
    #[error("expected {expected}, found {actual}")]
    UnsupportedSchema { expected: &'static str, actual: String },
    #[error("inconsistent publication manifest: {reason}")]
    InconsistentManifest { reason: &'static str },
    #[error("inconsistent publication target {target_id}: {reason}")]
    InconsistentTarget { target_id: String, reason: &'static str },
}

fn schema(actual: &str, expected: &'static str) -> Result<(), PublishedTargetsError> {
    if actual != expected {
        return Err(PublishedTargetsError::UnsupportedSchema { expected, actual: actual.into() });
    }
    Ok(())
}

pub(super) fn published_targets(
    manifest: &RunManifest,
) -> Result<Vec<PublishedTarget<'_>>, PublishedTargetsError> {
    use PublishedTargetsError::InconsistentManifest;

    schema(&manifest.schema_version, RUN_MANIFEST_SCHEMA_V1)?;
    let inventory = &manifest.artifact_manifest;
    schema(&inventory.schema_version, ARTIFACT_MANIFEST_SCHEMA_V1)?;
    if manifest.run_id.is_empty() || manifest.run_id != inventory.run_id {
        return Err(InconsistentManifest { reason: "run and inventory identities differ or are empty" });
    }
    let forest = manifest.artifact_forest.as_ref().ok_or(PublishedTargetsError::MissingForest)?;
    schema(&forest.schema, "renderflow/artifact-forest-v1")?;
    let execution = forest.execution.as_ref().ok_or(PublishedTargetsError::MissingExecution)?;
    schema(&execution.schema, FOREST_EXECUTION_SCHEMA_V1)?;
    if execution.targets.len() != forest.branches.len() {
        return Err(InconsistentManifest { reason: "one execution row per planning branch is required" });
    }

    let mut steps: BTreeMap<&str, Option<&StepEvidence>> = BTreeMap::new();
    for step in &manifest.steps {
        // Duplicate IDs cannot identify a unique producing step. Only reject them
        // if a published row actually references that ambiguous ID.
        steps.entry(step.step_id.as_str()).and_modify(|value| *value = None).or_insert(Some(step));
    }
    let sources = inventory.artifacts.iter().filter(|record| record.lifecycle == ArtifactRole::Source)
        .map(|record| (record.artifact_id.as_str(), record.digest.algorithm.as_str(), record.digest.value.as_str()))
        .collect::<BTreeSet<_>>();
    let mut target_ids = BTreeSet::new();
    let mut record_indices = BTreeSet::new();
    let mut locators = BTreeSet::new();
    let mut produced = BTreeSet::new();
    let mut publications = Vec::new();
    for (branch, target) in forest.branches.iter().zip(&execution.targets) {
        let invalid = |reason| PublishedTargetsError::InconsistentTarget {
            target_id: branch.target_id.clone(), reason,
        };
        if branch.target_id.is_empty() || target.target_id != branch.target_id
            || !target_ids.insert(branch.target_id.as_str()) {
            return Err(invalid("branch and execution IDs must be unique and agree in planning order"));
        }
        let selected = branch.state == ForestBranchState::Selected;
        let planned = manifest.state == RunState::Planned;
        let valid_outcome = match target.outcome {
            ForestTargetOutcome::NotSelected => !selected,
            ForestTargetOutcome::Planned => selected && planned,
            ForestTargetOutcome::Published | ForestTargetOutcome::NotPublished => selected && !planned,
        };
        if !valid_outcome {
            return Err(invalid("execution outcome contradicts planning or run state"));
        }
        if target.outcome != ForestTargetOutcome::Published {
            // NotPublished may reference a retained, rejected candidate. That
            // index is not publication, and cannot supply a published locator.
            if target.locator.is_some() {
                return Err(invalid("unpublished target carries a publication locator"));
            }
            if matches!(target.outcome, ForestTargetOutcome::Planned | ForestTargetOutcome::NotSelected)
                && (target.artifact_evidence_index.is_some() || target.artifact_id.is_some()
                    || target.producing_step_id.is_some()) {
                return Err(invalid("unobserved target carries artifact references"));
            }
            continue;
        }
        let index = target.artifact_evidence_index.ok_or_else(|| invalid("missing artifact index"))?;
        let record = inventory.artifacts.get(index).ok_or_else(|| invalid("artifact index is out of bounds"))?;
        if !record_indices.insert(index) || !locators.insert(record.locator.as_str()) {
            return Err(invalid("published targets must have distinct records and destinations"));
        }
        if record.lifecycle != ArtifactRole::Terminal || record.format != branch.format
            || target.artifact_id.as_deref() != Some(record.artifact_id.as_str())
            || target.locator.as_deref() != Some(record.locator.as_str())
            || !record.locator.strip_prefix("bundle:").is_some_and(|path| !path.is_empty())
            || record.validation == ValidationState::Invalid {
            return Err(invalid("publication does not match a non-invalid terminal bundle record"));
        }
        // Generic graph records carry these markers; composition retains its own
        // native namespace. Preserve both without imposing a family-specific shape.
        if record.metadata.get("renderflow.target.identity")
            .is_some_and(|value| value.as_str() != Some(target.target_id.as_str()))
            || record.metadata.get("renderflow.target.published")
                .is_some_and(|value| value.as_bool() != Some(true)) {
            return Err(invalid("native target markers contradict publication"));
        }
        let source_identity = sources.contains(&(
            record.artifact_id.as_str(), record.digest.algorithm.as_str(), record.digest.value.as_str(),
        ));
        if target.source_identity != source_identity {
            return Err(invalid("source identity disagrees with the observed source records"));
        }
        if !source_identity {
            produced.insert(record.artifact_id.as_str());
        }
        let producing_step = match target.producing_step_id.as_deref() {
            Some(id) => {
                let step = steps.get(id).copied().flatten()
                    .ok_or_else(|| invalid("producing step is missing or ambiguous"))?;
                if !matches!(step.state, StepState::Complete | StepState::Reused)
                    || !step.output_artifacts.contains(&record.artifact_id) {
                    return Err(invalid("producing step does not report this artifact as a successful output"));
                }
                Some(step)
            }
            None => None,
        };
        publications.push(PublishedTarget { branch, execution: target, artifact: record, producing_step });
    }
    let declared = forest.produced_artifacts.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if declared.len() != forest.produced_artifacts.len() || declared != produced {
        return Err(InconsistentManifest { reason: "produced derivative IDs differ from published target records" });
    }
    Ok(publications)
}
