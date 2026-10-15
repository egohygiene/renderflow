//! Correlate native composition receipts with the common publication contract.
//!
//! This projection checks structured evidence; it does not observe filesystem
//! bytes, establish visual fidelity, or implement Flow's acceptance boundary.

use std::collections::BTreeSet;

use anyhow::{ensure, Context, Result};

use super::composition::build_execution_plan;
use crate::artifact::{ArtifactDigest, ArtifactId, CanonicalFormat};
use crate::composition::{
    hash_bytes, hash_json, render, CompositionBranchOutcome, CompositionBranchPlan, CompositionBranchRun,
    CompositionBranchState, CompositionDiagnostic, CompositionOutput, CompositionRun, CompositionRunState,
    DiagnosticSeverity as CompositionSeverity, COMPOSITION_PLAN_SCHEMA_V5,
    COMPOSITION_PROFILE, COMPOSITION_RUN_SCHEMA_V5,
};
use crate::evidence::{
    ArtifactEvidence, ArtifactRole, CacheDisposition, DiagnosticSeverity, DigestEvidence,
    ExecutionDiagnostic, FidelityDeclaration, RunManifest, RunState, StepEvidence, StepState,
    ValidationState, ARTIFACT_MANIFEST_SCHEMA_V1, RUN_MANIFEST_SCHEMA_V1,
};
use crate::graph::execution_plan::{
    ForestExecutionEvidence, ForestTargetExecution, ForestTargetOutcome, FOREST_EXECUTION_SCHEMA_V1,
};

pub(crate) fn run_manifest(run: &CompositionRun) -> Result<RunManifest> {
    ensure!(run.schema == COMPOSITION_RUN_SCHEMA_V5
        && run.plan.schema == COMPOSITION_PLAN_SCHEMA_V5
        && run.plan.profile == COMPOSITION_PROFILE,
        "composition.evidence.version: unsupported native receipt contract");
    ensure!(run.artifact_manifest.schema_version == ARTIFACT_MANIFEST_SCHEMA_V1
        && run.artifact_manifest.run_id == run.run_id
        && run.artifact_manifest.output_dir == ".",
        "composition.evidence.inventory: native inventory does not belong to this bundle");
    ensure!(!run.runtime_qualified && run.review_required && !run.visual_determinism_qualified
        && !run.plan.runtime_qualified && run.plan.review_required && !run.plan.visual_determinism_qualified,
        "composition.evidence.qualification: projection cannot establish runtime or visual approval");
    ensure!(hash_bytes(&receipt_bytes(&run.plan)?) == run.plan_sha256,
        "composition.evidence.plan_digest: native plan bytes differ from their receipt digest");
    ensure!(valid_sha256(&run.plan.composition_sha256),
        "composition.evidence.source_digest: invalid canonical manifest digest");
    ensure!(!run.plan.branches.is_empty() && run.branches.len() == run.plan.branches.len(),
        "composition.evidence.branches: require one outcome per native planning branch");

    let wrapper = build_execution_plan(&run.plan)?;
    let execution_plan_digest = sha256(&receipt_bytes(&wrapper)?);
    let mut forest = wrapper.execution.artifact_forest.clone()
        .context("composition.evidence.forest: common plan omitted publication branches")?;
    ensure!(forest.branches.len() == run.branches.len(),
        "composition.evidence.forest: common and native branch counts differ");
    let source_ids = validate_sources(run)?;
    let mut seen_targets = BTreeSet::new();
    let mut seen_records = BTreeSet::new();
    let mut seen_outputs = BTreeSet::new();
    let mut produced = BTreeSet::new();
    let mut steps = Vec::new();
    let mut targets = Vec::new();
    for ((planned, observed), common) in run.plan.branches.iter().zip(&run.branches).zip(&forest.branches) {
        ensure!(planned.target_id == observed.target_id && common.target_id == planned.target_id
            && valid_sha256(&planned.cache_key)
            && seen_targets.insert(planned.target_id.clone()),
            "composition.evidence.target: branch identity or deterministic order differs");
        let step_id = format!("composition.branch:{}", planned.target_id);
        let record = if observed.outcome == CompositionBranchOutcome::Published {
            ensure!(planned.state == CompositionBranchState::Planned,
                "composition.evidence.publication: unselected branch claims publication");
            let index = observed.artifact_evidence_index
                .context("composition.evidence.publication: missing native record index")?;
            ensure!(seen_records.insert(index),
                "composition.evidence.publication: record reused by multiple target identities");
            let record = run.artifact_manifest.artifacts.get(index)
                .context("composition.evidence.publication: record index is out of bounds")?;
            validate_publication(run, planned, observed, record, &source_ids)?;
            let locator = record.locator.strip_prefix("bundle:")
                .context("composition.evidence.publication: missing bundle locator")?;
            ensure!(seen_outputs.insert(locator.to_owned()),
                "composition.evidence.publication: duplicate publication locator");
            produced.insert(record.artifact_id.clone());
            Some(record)
        } else {
            ensure!(observed.artifact_evidence_index.is_none() && observed.artifact_id.is_none()
                && observed.locator.is_none() && observed.cache == CacheDisposition::NotApplicable,
                "composition.evidence.publication: unpublished branch carries publication references");
            ensure!(match observed.outcome {
                CompositionBranchOutcome::Blocked => planned.state == CompositionBranchState::Blocked,
                CompositionBranchOutcome::Unavailable => planned.state == CompositionBranchState::Unavailable,
                CompositionBranchOutcome::Failed => planned.state == CompositionBranchState::Planned,
                CompositionBranchOutcome::Cancelled => true,
                CompositionBranchOutcome::Published => false,
            }, "composition.evidence.outcome: native outcome disagrees with planning state");
            None
        };
        let state = match observed.outcome {
            CompositionBranchOutcome::Published if observed.cache == CacheDisposition::Hit => StepState::Reused,
            CompositionBranchOutcome::Published => StepState::Complete,
            CompositionBranchOutcome::Blocked | CompositionBranchOutcome::Unavailable => StepState::Skipped,
            CompositionBranchOutcome::Failed => StepState::Failed,
            CompositionBranchOutcome::Cancelled => StepState::Cancelled,
        };
        let validation = record.map(|artifact| artifact.validation).unwrap_or(match observed.outcome {
            CompositionBranchOutcome::Blocked => ValidationState::Invalid,
            CompositionBranchOutcome::Unavailable => ValidationState::Unavailable,
            _ => ValidationState::NotRequested,
        });
        steps.push(StepEvidence {
            step_id: step_id.clone(), transform: run.plan.profile.clone(),
            transform_version: run.plan.renderflow_version.clone(),
            capability: Some(format!("composition.{}", planned.output.name())),
            provider: Some("renderflow.core-composition".into()),
            input_artifacts: if matches!(state, StepState::Complete | StepState::Reused | StepState::Failed) {
                source_ids.iter().cloned().collect()
            } else { Vec::new() },
            output_artifacts: record.map(|artifact| vec![artifact.artifact_id.clone()]).unwrap_or_default(),
            configuration_digest: DigestEvidence { algorithm: "sha256".into(), value: planned.cache_key.clone() },
            started_at_unix_ms: observed.started_at_unix_ms,
            completed_at_unix_ms: observed.completed_at_unix_ms,
            duration_ms: observed.completed_at_unix_ms.saturating_sub(observed.started_at_unix_ms),
            state, cache: observed.cache, validation,
            fidelity: record.map(|artifact| artifact.fidelity).unwrap_or(FidelityDeclaration::Unknown),
            skip_reason: match observed.outcome {
                CompositionBranchOutcome::Blocked => Some("composition branch blocked by native diagnostics".into()),
                CompositionBranchOutcome::Unavailable => Some("composition adapter unavailable".into()),
                _ => None,
            },
            diagnostics: observed.diagnostics.iter().map(|diagnostic| diagnostic_evidence(diagnostic, Some(&step_id))).collect(),
        });
        targets.push(ForestTargetExecution {
            target_id: planned.target_id.clone(),
            outcome: if record.is_some() { ForestTargetOutcome::Published }
                else if planned.state != CompositionBranchState::Planned { ForestTargetOutcome::NotSelected }
                else { ForestTargetOutcome::NotPublished },
            source_identity: false,
            artifact_evidence_index: observed.artifact_evidence_index,
            artifact_id: observed.artifact_id.clone(), locator: observed.locator.clone(),
            producing_step_id: record.map(|_| step_id),
        });
    }
    ensure!(run.artifact_manifest.artifacts.iter().enumerate().all(|(index, artifact)|
        artifact.lifecycle == ArtifactRole::Source
            || (artifact.lifecycle == ArtifactRole::Terminal && seen_records.contains(&index))),
        "composition.evidence.inventory: unreferenced terminal or fabricated intermediate record");
    let declared_outputs = run.artifact_manifest.outputs.iter().cloned().collect::<BTreeSet<_>>();
    ensure!(declared_outputs.len() == run.artifact_manifest.outputs.len() && declared_outputs == seen_outputs,
        "composition.evidence.outputs: inventory output list differs from actual publication references");
    let published = seen_records.len();
    ensure!(match run.state {
        CompositionRunState::Complete => published == run.branches.len(),
        CompositionRunState::Partial => published > 0 && published < run.branches.len(),
        CompositionRunState::Failed => published == 0,
        CompositionRunState::Cancelled => true,
    }, "composition.evidence.state: native run state disagrees with published branches");
    forest.produced_artifacts = produced.into_iter().collect();
    forest.execution = Some(ForestExecutionEvidence { schema: FOREST_EXECUTION_SCHEMA_V1.into(), targets });
    let mut diagnostics = run.plan.diagnostics.iter().map(|item| diagnostic_evidence(item, None)).collect::<Vec<_>>();
    diagnostics.push(ExecutionDiagnostic {
        severity: DiagnosticSeverity::Info, code: "composition.evidence.scope".into(), step_id: None,
        message: "Common projection of the native composition receipt; source_spec_digest identifies canonical manifest bytes only, while execution_plan_digest identifies exact execution-plan.json bytes including captured input pins and effective locale/output plans. Shared stages are in-memory observations, not published or cached artifacts. Projection checks receipt correlation, not filesystem bytes, visual quality, creative approval, or Flow acceptance.".into(),
    });
    let mut seen_stages = BTreeSet::new();
    for observed in &run.shared_intermediates {
        let stage = wrapper.stages.iter().find(|stage|
            stage.locale == observed.locale && stage.kind == observed.kind)
            .context("composition.evidence.stage: observed intermediate has no planned stage")?;
        let consumers = observed.consumer_target_ids.iter().cloned().collect::<BTreeSet<_>>();
        ensure!(seen_stages.insert(stage.stage_id.clone()) && observed.materializations == 1
            && valid_sha256(&observed.sha256) && !consumers.is_empty()
            && consumers.len() == observed.consumer_target_ids.len()
            && u64::from(observed.reuse_count) + 1 == consumers.len() as u64
            && consumers.iter().all(|id| stage.consumers.contains(id)
                && run.branches.iter().find(|branch| &branch.target_id == id).is_some_and(|branch|
                    match branch.outcome {
                        CompositionBranchOutcome::Published => branch.cache == CacheDisposition::Miss,
                        CompositionBranchOutcome::Failed | CompositionBranchOutcome::Cancelled => true,
                        CompositionBranchOutcome::Blocked | CompositionBranchOutcome::Unavailable => false,
                    })),
            "composition.evidence.stage: inconsistent shared materialization evidence");
        for target_id in &consumers {
            let (planned, branch) = run.plan.branches.iter().zip(&run.branches)
                .find(|(_, branch)| &branch.target_id == target_id)
                .context("composition.evidence.stage: consumer branch missing")?;
            let direct_output = (observed.kind == "svg" && planned.output == CompositionOutput::Svg)
                || (observed.kind == "transcript" && planned.output == CompositionOutput::TranscriptJson);
            if direct_output && branch.outcome == CompositionBranchOutcome::Published
                && branch.cache == CacheDisposition::Miss {
                let index = branch.artifact_evidence_index
                    .context("composition.evidence.stage: direct publication record missing")?;
                let record = run.artifact_manifest.artifacts.get(index)
                    .context("composition.evidence.stage: direct publication record out of bounds")?;
                ensure!(observed.sha256 == record.digest.value && observed.size_bytes == record.size_bytes,
                    "composition.evidence.stage: shared bytes differ from their direct terminal publication");
            }
        }
        diagnostics.push(ExecutionDiagnostic {
            severity: DiagnosticSeverity::Info, code: "composition.shared_stage.observed".into(),
            step_id: None,
            message: serde_json::to_string(&serde_json::json!({
                "schema": "renderflow.composition-shared-stage-observation/v1",
                "stage_id": stage.stage_id,
                "locale": observed.locale, "kind": observed.kind,
                "materializations": observed.materializations, "reuse_count": observed.reuse_count,
                "sha256": observed.sha256, "size_bytes": observed.size_bytes,
                "consumer_target_ids": observed.consumer_target_ids,
                "storage": "in_memory_only", "runtime_qualified": false,
                "meaning": "successful shared-stage accesses; downstream branch publication can still fail",
            }))?,
        });
    }
    Ok(RunManifest {
        schema_version: RUN_MANIFEST_SCHEMA_V1.into(), run_id: run.run_id.clone(),
        execution_plan_digest,
        source_spec_digest: DigestEvidence { algorithm: "sha256".into(), value: run.plan.composition_sha256.clone() },
        engine_version: run.plan.renderflow_version.clone(),
        started_at_unix_ms: run.started_at_unix_ms, completed_at_unix_ms: run.completed_at_unix_ms,
        state: match run.state {
            CompositionRunState::Complete => RunState::Complete,
            CompositionRunState::Partial => RunState::Partial,
            CompositionRunState::Failed => RunState::Failed,
            CompositionRunState::Cancelled => RunState::Cancelled,
        },
        artifact_manifest: run.artifact_manifest.clone(), steps, diagnostics,
        toolchain: None, artifact_forest: Some(forest),
    })
}

fn validate_sources(run: &CompositionRun) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    let mut roles = BTreeSet::new();
    for record in &run.artifact_manifest.artifacts {
        ensure!(record.digest.algorithm == "sha256" && valid_sha256(&record.digest.value),
            "composition.evidence.digest: invalid artifact digest declaration");
        let sources = record.sources.iter().map(|source| -> Result<ArtifactId> {
            ensure!(source.strip_prefix("artifact:sha256:").is_some_and(valid_sha256),
                "composition.evidence.identity: invalid parent artifact identity");
            Ok(serde_json::from_value(serde_json::Value::String(source.clone()))?)
        }).collect::<Result<Vec<_>>>()?;
        let digest = ArtifactDigest::sha256(record.digest.value.clone());
        let format = CanonicalFormat::new(record.format.clone())?;
        ensure!(record.artifact_id == ArtifactId::from_record(&digest, &format, &sources).as_str(),
            "composition.evidence.identity: artifact identity differs from digest, format, and ordered lineage");
        if record.lifecycle != ArtifactRole::Source { continue; }
        ensure!(record.sources.is_empty(),
            "composition.evidence.source: source artifact cannot have parent lineage");
        let input = run.plan.inputs.iter().find(|input| format!("{}:{}", input.kind, input.id) == record.role)
            .context("composition.evidence.source: source role absent from captured input plan")?;
        ensure!(roles.insert(record.role.clone()) && record.digest.value == input.sha256
            && record.size_bytes == input.size_bytes
            && record.locator == format!("bundle:inputs/{}", input.sha256),
            "composition.evidence.source: source record differs from captured input pins");
        ids.insert(record.artifact_id.clone());
    }
    ensure!(run.state == CompositionRunState::Cancelled || roles.len() == run.plan.inputs.len(),
        "composition.evidence.source: native inventory omitted captured inputs");
    Ok(ids)
}

fn validate_publication(run: &CompositionRun, planned: &CompositionBranchPlan,
    observed: &CompositionBranchRun, record: &ArtifactEvidence, source_ids: &BTreeSet<String>) -> Result<()> {
    let locator = format!("bundle:outputs/{}/page.{}", planned.locale, planned.output.extension());
    ensure!(valid_sha256(&planned.cache_key) && record.lifecycle == ArtifactRole::Terminal
        && observed.artifact_id.as_deref() == Some(record.artifact_id.as_str())
        && observed.locator.as_deref() == Some(record.locator.as_str())
        && record.locator == locator && record.role == planned.target_id
        && record.format == planned.output.name() && record.media_type == planned.output.media_type()
        && matches!(observed.cache, CacheDisposition::Hit | CacheDisposition::Miss)
        && record.cache == observed.cache
        && record.producer.system == "renderflow"
        && record.producer.provider.as_deref() == Some("renderflow.core-composition")
        && record.producer.transform.as_deref() == Some(run.plan.profile.as_str())
        && record.producer.capability.as_deref() == Some(format!("composition.{}", planned.output.name()).as_str())
        && record.producer.version.as_deref() == Some(run.plan.renderflow_version.as_str())
        && record.validation == ValidationState::ValidWithWarnings
        && record.fidelity == FidelityDeclaration::Partial
        && record.sources.iter().cloned().collect::<BTreeSet<_>>() == *source_ids
        && record.sources.len() == source_ids.len(),
        "composition.evidence.publication: native record identity, locator, output kind, cache, or lineage mismatch");
    let metadata = record.metadata.get("renderflow.composition")
        .context("composition.evidence.metadata: published artifact lacks native composition evidence")?;
    let typography = planned.typography.as_ref().map(|report| -> Result<_> {
        Ok(serde_json::json!({
            "schema": report.schema, "provider_profile": report.provider_profile,
            "report_sha256": hash_json(report)?, "runtime_qualified": false,
        }))
    }).transpose()?;
    let renderer = serde_json::json!({
        "id": render::COMPOSITION_RENDERER_ID,
        "version": render::COMPOSITION_RENDERER_VERSION,
    });
    ensure!(metadata.get("schema").and_then(serde_json::Value::as_str) == Some("renderflow.composition-artifact/v5")
        && metadata.get("target_id").and_then(serde_json::Value::as_str) == Some(planned.target_id.as_str())
        && metadata.get("locale").and_then(serde_json::Value::as_str) == Some(planned.locale.as_str())
        && metadata.get("profile").and_then(serde_json::Value::as_str) == Some(run.plan.profile.as_str())
        && metadata.get("version").and_then(serde_json::Value::as_str) == Some(run.plan.renderflow_version.as_str())
        && metadata.get("renderer") == Some(&renderer)
        && metadata.get("typography") == Some(&serde_json::to_value(&typography)?)
        && metadata.get("composition_sha256").and_then(serde_json::Value::as_str) == Some(run.plan.composition_sha256.as_str())
        && metadata.get("cache_key").and_then(serde_json::Value::as_str) == Some(planned.cache_key.as_str())
        && metadata.get("content_sha256") == Some(&serde_json::to_value(&planned.content_sha256)?)
        && metadata.get("content_revision") == Some(&serde_json::to_value(&planned.content_revision)?)
        && metadata.get("raster") == Some(&serde_json::to_value(&planned.raster)?)
        && metadata.get("pdf") == Some(&serde_json::to_value(&planned.pdf)?)
        && metadata.get("review_required").and_then(serde_json::Value::as_bool) == Some(true)
        && metadata.get("runtime_qualified").and_then(serde_json::Value::as_bool) == Some(false)
        && metadata.get("visual_determinism_qualified").and_then(serde_json::Value::as_bool) == Some(false)
        && metadata.get("losses").and_then(serde_json::Value::as_array).is_some_and(|losses| !losses.is_empty()),
        "composition.evidence.metadata: published native identity or review evidence differs");
    Ok(())
}

fn diagnostic_evidence(diagnostic: &CompositionDiagnostic, step_id: Option<&str>) -> ExecutionDiagnostic {
    ExecutionDiagnostic {
        severity: match diagnostic.severity {
            CompositionSeverity::Info => DiagnosticSeverity::Info,
            CompositionSeverity::Warning => DiagnosticSeverity::Warning,
            CompositionSeverity::Error => DiagnosticSeverity::RecoverableFailure,
        },
        code: diagnostic.code.clone(), message: diagnostic.message.clone(), step_id: step_id.map(str::to_owned),
    }
}

fn receipt_bytes(value: &impl serde::Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> DigestEvidence {
    DigestEvidence { algorithm: "sha256".into(), value: hash_bytes(bytes) }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
