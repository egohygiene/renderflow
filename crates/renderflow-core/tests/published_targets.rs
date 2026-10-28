//! Authored, unrun #430 receipt-join cases. These construct synthetic declarations;
//! they do not exercise providers or establish that any payload was produced.

use renderflow::evidence::{ArtifactRole, RunState, StepState, ValidationState};
use renderflow::graph::{ForestBranchState, ForestTargetOutcome};
use renderflow::{PublishedTargetsError, RunManifest};
use serde_json::json;

fn manifest() -> RunManifest {
    serde_json::from_value(json!({
        "schema_version": "renderflow.run/v1", "run_id": "synthetic-run",
        "execution_plan_digest": {"algorithm": "sha256", "value": "0".repeat(64)},
        "source_spec_digest": {"algorithm": "sha256", "value": "1".repeat(64)},
        "engine_version": "fixture", "started_at_unix_ms": 0, "completed_at_unix_ms": 1,
        "state": "complete", "steps": [], "diagnostics": [],
        "artifact_manifest": {
            "schema_version": "renderflow.artifact-manifest/v1", "run_id": "synthetic-run",
            "output_dir": "unused-synthetic-bundle", "outputs": ["unused-synthetic-bundle/result.txt"],
            "artifacts": [{
                "artifact_id": "synthetic-derivative", "role": "body", "lifecycle": "terminal",
                "locator": "bundle:result.txt", "format": "text", "media_type": "text/plain",
                "digest": {"algorithm": "sha256", "value": "2".repeat(64)}, "size_bytes": 8,
                "producer": {"system": "renderflow"}, "sources": ["synthetic-source"],
                "cache": "miss", "validation": "valid_with_warnings", "fidelity": "partial",
                "metadata": {"renderflow.subtitle": {"export_losses": ["timing"], "review_required": true}}
            }]
        },
        "artifact_forest": {
            "schema": "renderflow/artifact-forest-v1", "intermediates": "cache_only",
            "branches": [{
                "target_id": "synthetic-target", "format": "text", "requirement": "required",
                "state": "selected", "reason_code": "synthetic", "reason": "Receipt join fixture",
                "route": {"steps": [], "source_identity": true, "cumulative_quality": 1.0,
                    "cumulative_loss": 0.0, "hop_count": 0, "weighted_cost": 0.0,
                    "ranking": "synthetic zero-edge intent; hygiene may derive output", "explored_candidates": 0}
            }],
            "produced_artifacts": ["synthetic-derivative"],
            "execution": {"schema": "renderflow/forest-execution-v1", "targets": [{
                "target_id": "synthetic-target", "outcome": "published", "source_identity": false,
                "artifact_evidence_index": 0, "artifact_id": "synthetic-derivative", "locator": "bundle:result.txt"
            }]}
        }
    })).unwrap()
}

fn second_target(run: &mut RunManifest) {
    let forest = run.artifact_forest.as_mut().unwrap();
    let mut branch = forest.branches[0].clone();
    branch.target_id = "synthetic-second".into();
    forest.branches.push(branch);
    let mut target = forest.execution.as_ref().unwrap().targets[0].clone();
    target.target_id = "synthetic-second".into();
    target.artifact_evidence_index = Some(1);
    target.locator = Some("bundle:second.txt".into());
    forest.execution.as_mut().unwrap().targets.push(target);
    let mut record = run.artifact_manifest.artifacts[0].clone();
    record.locator = "bundle:second.txt".into();
    run.artifact_manifest.artifacts.push(record);
    run.artifact_manifest.outputs.push("unused-synthetic-bundle/second.txt".into());
}

fn target_error(run: &RunManifest) {
    assert!(matches!(run.published_targets(), Err(PublishedTargetsError::InconsistentTarget { .. })));
}

#[test]
fn borrowed_publications_preserve_same_content_destinations_and_native_metadata() {
    let mut run = manifest();
    second_target(&mut run);
    let mut retained = run.artifact_manifest.artifacts[0].clone();
    retained.lifecycle = ArtifactRole::Intermediate;
    retained.locator = "artifact-store:retained".into();
    run.artifact_manifest.artifacts.push(retained);
    let outputs = run.published_targets().unwrap();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].artifact.artifact_id, outputs[1].artifact.artifact_id);
    assert_ne!(outputs[0].artifact.locator, outputs[1].artifact.locator);
    assert_ne!(outputs[0].branch.target_id, outputs[1].branch.target_id);
    assert!(std::ptr::eq(outputs[0].artifact, &run.artifact_manifest.artifacts[0]));
    assert_eq!(outputs[0].artifact.metadata["renderflow.subtitle"]["export_losses"], json!(["timing"]));
    assert_eq!(outputs[0].artifact.metadata["renderflow.subtitle"]["review_required"], true);
    assert_eq!(run.flow_artifacts_v1().len(), 3, "legacy projection still covers all inventory");
}

#[test]
fn missing_legacy_evidence_and_unknown_contracts_are_errors_not_empty_success() {
    let mut run = manifest();
    run.artifact_forest = None;
    assert_eq!(run.published_targets().unwrap_err(), PublishedTargetsError::MissingForest);
    run = manifest();
    run.artifact_forest.as_mut().unwrap().execution = None;
    assert_eq!(run.published_targets().unwrap_err(), PublishedTargetsError::MissingExecution);
    run = manifest();
    run.artifact_manifest.schema_version = "unsupported".into();
    assert!(matches!(run.published_targets(), Err(PublishedTargetsError::UnsupportedSchema { .. })));
    run = manifest();
    run.artifact_manifest.run_id = "other-run".into();
    assert!(matches!(run.published_targets(), Err(PublishedTargetsError::InconsistentManifest { .. })));
}

#[test]
fn dry_run_intent_has_no_publications_and_cannot_reference_a_payload() {
    let mut run = manifest();
    run.state = RunState::Planned;
    let forest = run.artifact_forest.as_mut().unwrap();
    forest.produced_artifacts.clear();
    let target = &mut forest.execution.as_mut().unwrap().targets[0];
    target.outcome = ForestTargetOutcome::Planned;
    target.artifact_evidence_index = None;
    target.artifact_id = None;
    target.locator = None;
    assert!(run.published_targets().unwrap().is_empty());
    run.artifact_forest.as_mut().unwrap().execution.as_mut().unwrap().targets[0].artifact_id = Some("invented".into());
    target_error(&run);
}

#[test]
fn unsuccessful_run_state_does_not_erase_retained_publications() {
    for state in [RunState::Partial, RunState::Failed, RunState::Cancelled] {
        let mut run = manifest();
        run.state = state;
        assert_eq!(run.published_targets().unwrap().len(), 1);
        assert_eq!(run.state, state);
    }
}

#[test]
fn retained_unpublished_candidates_and_excluded_branches_are_not_deliverables() {
    let mut run = manifest();
    second_target(&mut run);
    let forest = run.artifact_forest.as_mut().unwrap();
    let target = &mut forest.execution.as_mut().unwrap().targets[1];
    target.outcome = ForestTargetOutcome::NotPublished;
    target.locator = None;
    run.artifact_manifest.artifacts[1].validation = ValidationState::Invalid;
    assert_eq!(run.published_targets().unwrap().len(), 1);
    let forest = run.artifact_forest.as_mut().unwrap();
    forest.branches[1].state = ForestBranchState::Excluded;
    let target = &mut forest.execution.as_mut().unwrap().targets[1];
    target.outcome = ForestTargetOutcome::NotSelected;
    target.artifact_evidence_index = None;
    target.artifact_id = None;
    assert_eq!(run.published_targets().unwrap().len(), 1);
    run.artifact_forest.as_mut().unwrap().execution.as_mut().unwrap().targets[0].outcome = ForestTargetOutcome::Planned;
    target_error(&run);
}

#[test]
fn branch_membership_order_and_publication_references_must_agree() {
    for mutation in 0..10 {
        let mut run = manifest();
        second_target(&mut run);
        let forest = run.artifact_forest.as_mut().unwrap();
        let rows = &mut forest.execution.as_mut().unwrap().targets;
        match mutation {
            0 => { forest.branches[1].target_id = forest.branches[0].target_id.clone(); rows[1].target_id = rows[0].target_id.clone(); }
            1 => rows.swap(0, 1),
            2 => rows[0].target_id = "unknown-target".into(),
            3 => rows[0].artifact_evidence_index = None,
            4 => rows[0].artifact_evidence_index = Some(99),
            5 => rows[0].artifact_id = Some("different-record".into()),
            6 => rows[0].locator = Some("bundle:different.txt".into()),
            7 => { rows[1].artifact_evidence_index = Some(0); rows[1].locator = rows[0].locator.clone(); }
            8 => forest.branches[0].state = ForestBranchState::Unavailable,
            _ => { rows[1].locator = rows[0].locator.clone(); run.artifact_manifest.artifacts[1].locator = run.artifact_manifest.artifacts[0].locator.clone(); }
        }
        target_error(&run);
    }
    let mut run = manifest();
    run.artifact_forest.as_mut().unwrap().execution.as_mut().unwrap().targets.clear();
    assert!(matches!(run.published_targets(), Err(PublishedTargetsError::InconsistentManifest { .. })));
}

#[test]
fn conflicting_terminal_record_claims_are_refused() {
    for mutation in 0..6 {
        let mut run = manifest();
        let record = &mut run.artifact_manifest.artifacts[0];
        match mutation {
            0 => record.lifecycle = ArtifactRole::Intermediate,
            1 => record.format = "pdf".into(),
            2 => record.validation = ValidationState::Invalid,
            3 => { record.locator = "artifact-store:candidate".into(); run.artifact_forest.as_mut().unwrap().execution.as_mut().unwrap().targets[0].locator = Some(record.locator.clone()); }
            4 => { record.metadata.insert("renderflow.target.identity".into(), json!("another-target")); }
            _ => { record.metadata.insert("renderflow.target.published".into(), json!(false)); }
        }
        target_error(&run);
    }
}

#[test]
fn source_identity_uses_observed_records_including_zero_edge_hygiene_derivatives() {
    let mut run = manifest();
    let mut source = run.artifact_manifest.artifacts[0].clone();
    source.lifecycle = ArtifactRole::Source;
    source.locator = "artifact-store:original".into();
    source.artifact_id = "synthetic-source".into();
    run.artifact_manifest.artifacts.push(source);
    assert!(!run.published_targets().unwrap()[0].execution.source_identity,
        "same bytes with a distinct hygiene record remain a derivative after zero-edge planning");
    run.artifact_manifest.artifacts[1].artifact_id = "synthetic-derivative".into();
    target_error(&run);
    let forest = run.artifact_forest.as_mut().unwrap();
    forest.execution.as_mut().unwrap().targets[0].source_identity = true;
    forest.produced_artifacts.clear();
    assert!(run.published_targets().unwrap()[0].execution.source_identity);
    run.artifact_manifest.artifacts[1].digest.value = "3".repeat(64);
    target_error(&run);
}

#[test]
fn producing_step_references_are_exact_and_preserve_reuse() {
    let mut run = manifest();
    run.steps.push(serde_json::from_value(json!({
        "step_id": "synthetic-step", "transform": "fixture", "transform_version": "1",
        "input_artifacts": ["synthetic-source"], "output_artifacts": ["synthetic-derivative"],
        "configuration_digest": {"algorithm": "sha256", "value": "4".repeat(64)},
        "started_at_unix_ms": 0, "completed_at_unix_ms": 1, "duration_ms": 1,
        "state": "reused", "cache": "hit", "validation": "valid_with_warnings", "fidelity": "partial"
    })).unwrap());
    run.artifact_forest.as_mut().unwrap().execution.as_mut().unwrap().targets[0].producing_step_id = Some("synthetic-step".into());
    assert!(std::ptr::eq(run.published_targets().unwrap()[0].producing_step.unwrap(), &run.steps[0]));
    for mutation in 0..4 {
        let mut invalid = run.clone();
        match mutation {
            0 => invalid.steps.clear(),
            1 => invalid.steps.push(invalid.steps[0].clone()),
            2 => invalid.steps[0].state = StepState::Failed,
            _ => invalid.steps[0].output_artifacts.clear(),
        }
        target_error(&invalid);
    }
}

#[test]
fn produced_derivative_summary_cannot_omit_duplicate_or_invent_ids() {
    for ids in [vec![], vec!["invented"], vec!["synthetic-derivative", "synthetic-derivative"]] {
        let mut run = manifest();
        run.artifact_forest.as_mut().unwrap().produced_artifacts = ids.into_iter().map(str::to_owned).collect();
        assert!(matches!(run.published_targets(), Err(PublishedTargetsError::InconsistentManifest { .. })));
    }
}
