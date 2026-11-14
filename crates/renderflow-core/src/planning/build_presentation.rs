//! Frozen presentation admission to the common build/forest contract.
use super::{CanonicalExecutionResult, PlanningRequest};
use crate::artifact::{ArtifactDigest, ArtifactId, CanonicalFormat};
use crate::evidence::*;
use crate::graph::execution_plan::{
    EdgeType, ExecutionMetadata, ExecutionWave, ForestExecutionEvidence, ForestTargetExecution,
    ForestTargetOutcome, NodeType, PlanEdge, PlanNode, FOREST_EXECUTION_SCHEMA_V1,
};
use crate::graph::{
    ArtifactForest, ExecutionPlan, ForestBranch, ForestBranchState, ForestRouteEvidence,
};
use crate::presentation::{self, render, PresentationRequest, PresentationTarget};
use crate::process::ProcessCancellationToken;
use crate::spec::{LoadedSpec, SourceSpecVersion, SpecV2, TargetSpec};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};

pub(super) struct ResolvedPresentationBuild {
    request: PresentationRequest,
    captured: render::Captured,
    spec: SpecV2,
    plan: ExecutionPlan,
    cancellation: ProcessCancellationToken,
}

impl ResolvedPresentationBuild {
    pub(super) fn plan(&self) -> &ExecutionPlan {
        &self.plan
    }
    pub(super) fn spec(&self) -> &SpecV2 {
        &self.spec
    }
    pub(super) fn source_version(&self) -> SourceSpecVersion {
        SourceSpecVersion::V2
    }
    pub(super) fn source_path(&self) -> &Path {
        &self.request.input
    }
    pub(super) fn predicted_output_paths(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .request
            .targets
            .iter()
            .map(|t| self.request.output_dir.join(filename(*t)))
            .collect())
    }
    pub(super) fn with_cancellation_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancellation = ProcessCancellationToken::from_shared(flag);
        self
    }
}

fn name(target: PresentationTarget) -> &'static str {
    match target {
        PresentationTarget::Source => "presentation_source",
        PresentationTarget::Web => "presentation_web",
        PresentationTarget::Pdf => "pdf",
    }
}
fn filename(target: PresentationTarget) -> &'static str {
    match target {
        PresentationTarget::Source => "presentation-source.zip",
        PresentationTarget::Web => "presentation-web.zip",
        PresentationTarget::Pdf => "presentation.pdf",
    }
}

pub(super) fn resolve(
    loaded: LoadedSpec,
    selection: &PlanningRequest,
) -> Result<ResolvedPresentationBuild> {
    ensure!(
        loaded.source_version == SourceSpecVersion::V2
            && !selection.all_reachable
            && selection.profile.is_none()
            && selection.exclude.is_empty()
            && selection.optimization.is_none(),
        "presentation.build.selection: use a v2 source and exact targets only"
    );
    let mut spec = loaded.spec;
    if let Some(target) = &selection.target {
        spec.targets.exact = vec![TargetSpec {
            format: Some(target.clone()),
            ..TargetSpec::default()
        }];
    }
    ensure!(
        spec.validate().is_empty(),
        "presentation.build.spec: unsupported specification"
    );
    let config = fs::canonicalize(&selection.config_path)?;
    let parent = config.parent().context("presentation.config.parent")?;
    let relative = |s: &str| {
        let p = Path::new(s);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            parent.join(p)
        }
    };
    let input = relative(
        spec.sources[0]
            .path
            .as_deref()
            .context("presentation.source.path")?,
    );
    let output_dir = relative(&spec.output.bundle_root);
    let policy = spec.execution.presentation.clone().unwrap_or_default();
    let mut targets = crate::spec::presentation::targets(&spec.targets.exact)?;
    targets.sort();
    let request = PresentationRequest {
        input,
        output_dir,
        targets,
        provider: policy.provider,
        include_speaker_notes: policy.include_speaker_notes,
        allow_unqualified: policy.allow_unqualified,
    };
    let captured = render::capture(&request)?;
    spec.sources[0].path = Some(
        request
            .input
            .to_str()
            .context("presentation.input.utf8")?
            .into(),
    );
    spec.output.bundle_root = request
        .output_dir
        .to_str()
        .context("presentation.output.utf8")?
        .into();
    let plan = project(&spec, &request, &captured.plan)?;
    Ok(ResolvedPresentationBuild {
        request,
        captured,
        spec,
        plan,
        cancellation: ProcessCancellationToken::new(),
    })
}

fn project(
    spec: &SpecV2,
    request: &PresentationRequest,
    native: &presentation::PresentationPlan,
) -> Result<ExecutionPlan> {
    let mut edges = Vec::new();
    let mut branches = Vec::new();
    for target in &request.targets {
        let output = name(*target);
        let available = *target == PresentationTarget::Source
            || (request.provider.is_some()
                && request.allow_unqualified
                && (*target != PresentationTarget::Pdf
                    || request
                        .provider
                        .as_ref()
                        .is_some_and(|p| p.browser.is_some())));
        let edge = PlanEdge {
            from: "presentation_snapshot".into(),
            to: output.into(),
            cost: 1.0,
            quality: 0.0,
            input_kind: "single".into(),
            edge_type: EdgeType::Lossy,
            provider_id: Some(
                if *target == PresentationTarget::Source {
                    "renderflow.presentation"
                } else {
                    "tool.slidev"
                }
                .into(),
            ),
            capability_id: Some(format!("presentation.{output}")),
            required_provider_ids: Vec::new(),
            variant_id: None,
            evidence: BTreeMap::from([
                ("profile".into(), presentation::PROFILE_VERSION.into()),
                ("runtime_qualified".into(), "false".into()),
                ("quality_meaning".into(), "unqualified placeholder".into()),
            ]),
        };
        let route = available.then(|| ForestRouteEvidence {
            steps: vec![edge.clone()],
            source_identity: false,
            cumulative_quality: 0.0,
            cumulative_loss: 1.0,
            hop_count: 1,
            weighted_cost: 1.0,
            ranking: "fixed presentation profile; not a searched or qualified fidelity route"
                .into(),
            explored_candidates: 0,
        });
        if available {
            edges.push(edge);
        }
        let target_id = format!(
            "presentation:{}",
            sha256_serialized(
                &json!({"target":output,"native":native,"policy":spec.execution.presentation})
            )?
            .value
        );
        branches.push(ForestBranch { target_id, format:output.into(), role:Some("presentation".into()), preset:None,
            template:None, variant:None, requirement:"required".into(),
            options:BTreeMap::from([("file".into(),json!(filename(*target))), ("native_plan".into(),json!(native))]),
            state:if available { ForestBranchState::Selected } else { ForestBranchState::Unavailable },
            reason_code:if available { "presentation.selected" } else { "presentation.provider_required" }.into(),
            reason:if available { "Frozen presentation branch; publication and review remain unobserved" }
                else { "Native target requires explicit unqualified opt-in, a pinned local provider and a browser for PDF" }.into(), route });
    }
    let identity = format!(
        "sha256:{}",
        sha256_serialized(&json!({"spec":spec,"native":native,"branches":branches}))?.value
    );
    let count = edges.len();
    let mut nodes = vec![PlanNode {
        format: "presentation_snapshot".into(),
        node_type: NodeType::Source,
    }];
    nodes.extend(request.targets.iter().map(|target| PlanNode {
        format: name(*target).into(),
        node_type: NodeType::Output,
    }));
    Ok(ExecutionPlan {
        source: "presentation_snapshot".into(),
        targets: request.targets.iter().map(|t| name(*t).into()).collect(),
        optimization: "fixed_presentation_dependencies".into(),
        nodes,
        edges: edges.clone(),
        waves: if edges.is_empty() {
            Vec::new()
        } else {
            vec![ExecutionWave { index: 1, edges }]
        },
        metadata: ExecutionMetadata {
            total_nodes: 1 + request.targets.len(),
            total_edges: count,
            execution_depth: usize::from(count > 0),
            execution_waves: usize::from(count > 0),
            estimated_cost: count as f32,
            estimated_quality: 0.0,
            reused_intermediates: 0,
            output_count: count,
        },
        diagnostics: Vec::new(),
        source_artifact: None,
        source_collection: None,
        toolchain: None,
        artifact_forest: Some(ArtifactForest {
            identity,
            profiles: vec![presentation::PROFILE_VERSION.into()],
            intermediates: "cache_only".into(),
            branches,
            policy: json!({"adapter":"frozen_presentation","native_plan":native,"runtime_qualified":false,
                "review_is_declared":true,"sharing":"one captured model/assets, one local Slidev project; no second job engine",
                "limits":"native process/network/resource limitations remain those of the standalone adapter"}),
            ..ArtifactForest::default()
        }),
    })
}

pub(super) fn execute(
    resolved: ResolvedPresentationBuild,
    dry_run: bool,
) -> Result<CanonicalExecutionResult> {
    let started = unix_time_ms();
    if resolved.cancellation.is_cancelled() {
        return cancelled(resolved);
    }
    if dry_run {
        return finish(&resolved, RunState::Planned, None, started);
    }
    ensure!(
        resolved
            .plan
            .artifact_forest
            .as_ref()
            .context("presentation.forest")?
            .branches
            .iter()
            .all(|b| b.state == ForestBranchState::Selected),
        "presentation.build.unavailable: required native target is unavailable"
    );
    let receipt = render::build_captured(
        &resolved.request,
        &resolved.captured,
        resolved.cancellation.clone(),
    )?;
    finish(&resolved, RunState::Complete, Some(receipt), started)
}

pub(super) fn cancelled(resolved: ResolvedPresentationBuild) -> Result<CanonicalExecutionResult> {
    finish(&resolved, RunState::Cancelled, None, unix_time_ms())
}

fn record(
    format: &str,
    digest: &str,
    size: u64,
    sources: &[ArtifactId],
    locator: String,
    lifecycle: ArtifactRole,
) -> Result<ArtifactEvidence> {
    let hash = ArtifactDigest::sha256(digest.to_owned());
    let id = ArtifactId::from_record(&hash, &CanonicalFormat::new(format)?, sources);
    Ok(ArtifactEvidence {
        artifact_id: id.to_string(),
        role: "presentation".into(),
        lifecycle,
        locator,
        format: format.into(),
        media_type: if format == "pdf" {
            "application/pdf"
        } else if format.starts_with("presentation_") {
            "application/zip"
        } else {
            "application/json"
        }
        .into(),
        digest: DigestEvidence {
            algorithm: "sha256".into(),
            value: digest.into(),
        },
        size_bytes: size,
        producer: ProducerEvidence {
            system: "renderflow".into(),
            transform: Some("presentation".into()),
            capability: Some("presentation".into()),
            provider: Some("renderflow.presentation".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
        },
        sources: sources.iter().map(ToString::to_string).collect(),
        cache: CacheDisposition::NotApplicable,
        validation: ValidationState::ValidWithWarnings,
        validation_evidence: Vec::new(),
        fidelity: FidelityDeclaration::Unknown,
        hygiene: None,
        warnings: vec!["presentation.runtime_and_visual_qualification_unobserved".into()],
        metadata: BTreeMap::new(),
    })
}

fn finish(
    resolved: &ResolvedPresentationBuild,
    state: RunState,
    receipt: Option<Value>,
    started: u64,
) -> Result<CanonicalExecutionResult> {
    let plan_digest = sha256_serialized(&resolved.plan)?;
    let id = run_id(&plan_digest, started);
    let mut forest = resolved
        .plan
        .artifact_forest
        .clone()
        .context("presentation.forest")?;
    let mut records = Vec::new();
    let mut outputs = Vec::new();
    let mut observed = Vec::new();
    if let Some(receipt) = receipt {
        ensure!(
            receipt["status"] == "completed"
                && receipt["plan"] == serde_json::to_value(&resolved.captured.plan)?,
            "presentation.receipt: native result differs from the frozen plan"
        );
        let source = record(
            "presentation",
            &resolved.captured.plan.input_sha256,
            resolved.captured.input_size,
            &[],
            "memory:presentation-input".into(),
            ArtifactRole::Source,
        )?;
        let source_id: ArtifactId = serde_json::from_value(json!(source.artifact_id))?;
        records.push(source);
        let files = receipt["files"]
            .as_array()
            .context("presentation.receipt.files")?;
        for file in files {
            let path = file["path"].as_str().context("presentation.receipt.path")?;
            presentation::relative_path(path)?;
            let bytes = presentation::read_file(
                &resolved.request.output_dir.join(path),
                256 * 1024 * 1024,
            )?;
            ensure!(
                file["sha256"] == presentation::sha256(&bytes)
                    && file["size_bytes"] == bytes.len() as u64,
                "presentation.receipt.changed: exact published bytes differ"
            );
            outputs.push(path.to_owned());
        }
        for branch in &forest.branches {
            let path = branch.options["file"]
                .as_str()
                .context("presentation.branch.file")?;
            let file = files
                .iter()
                .find(|f| f["path"] == path)
                .context("presentation.branch.missing")?;
            let mut evidence = record(
                &branch.format,
                file["sha256"].as_str().context("presentation.digest")?,
                file["size_bytes"].as_u64().context("presentation.size")?,
                std::slice::from_ref(&source_id),
                format!("bundle:{path}"),
                ArtifactRole::Terminal,
            )?;
            evidence
                .metadata
                .insert("renderflow.target.identity".into(), json!(branch.target_id));
            evidence
                .metadata
                .insert("renderflow.target.published".into(), json!(true));
            evidence.metadata.insert(
                "renderflow.presentation.native_receipt".into(),
                json!("bundle:bundle.json"),
            );
            evidence.producer.provider = branch
                .route
                .as_ref()
                .and_then(|r| r.steps.first())
                .and_then(|e| e.provider_id.clone());
            let index = records.len() + observed.len();
            forest.produced_artifacts.push(evidence.artifact_id.clone());
            observed.push((index, evidence));
        }
    }
    let targets = forest
        .branches
        .iter()
        .enumerate()
        .map(|(index, branch)| {
            let item = observed.get(index);
            ForestTargetExecution {
                target_id: branch.target_id.clone(),
                outcome: if branch.state != ForestBranchState::Selected {
                    ForestTargetOutcome::NotSelected
                } else if item.is_some() {
                    ForestTargetOutcome::Published
                } else if state == RunState::Planned {
                    ForestTargetOutcome::Planned
                } else {
                    ForestTargetOutcome::NotPublished
                },
                source_identity: false,
                artifact_evidence_index: item.map(|(i, _)| *i),
                artifact_id: item.map(|(_, a)| a.artifact_id.clone()),
                locator: item.map(|(_, a)| a.locator.clone()),
                producing_step_id: None,
            }
        })
        .collect();
    records.extend(observed.into_iter().map(|(_, e)| e));
    forest.execution = Some(ForestExecutionEvidence {
        schema: FOREST_EXECUTION_SCHEMA_V1.into(),
        targets,
    });
    let manifest = RunManifest { schema_version:RUN_MANIFEST_SCHEMA_V1.into(), run_id:id.clone(), execution_plan_digest:plan_digest,
        source_spec_digest:sha256_serialized(&resolved.spec)?, engine_version:env!("CARGO_PKG_VERSION").into(),
        started_at_unix_ms:started, completed_at_unix_ms:unix_time_ms().max(started), state,
        artifact_manifest:ArtifactManifest { schema_version:ARTIFACT_MANIFEST_SCHEMA_V1.into(), run_id:id,
            output_dir:resolved.spec.output.bundle_root.clone(), outputs:outputs.clone(), artifacts:records }, steps:Vec::new(),
        diagnostics:vec![ExecutionDiagnostic { severity:DiagnosticSeverity::Warning, code:"presentation.build.scope".into(), step_id:None,
            message:"Frozen presentation profile. File hashes/publication correlation are scoped observations; runtime, visual/accessibility quality, declared review and publishing approval remain separate. Native errors preserve partial files without a common completion receipt.".into() }],
        toolchain:None, artifact_forest:Some(forest) };
    let manifest_path = if state == RunState::Complete {
        render::write_new(
            &resolved.request.output_dir.join("build-plan.json"),
            &serde_json::to_vec(&resolved.plan)?,
        )?;
        let path = resolved.request.output_dir.join("renderflow-run.json");
        render::write_new(&path, &serde_json::to_vec_pretty(&manifest)?)?;
        Some(path.display().to_string())
    } else {
        None
    };
    let paths = if state == RunState::Planned {
        resolved.predicted_output_paths()?
    } else {
        outputs
            .iter()
            .map(|p| resolved.request.output_dir.join(p))
            .collect()
    };
    Ok(CanonicalExecutionResult {
        plan: resolved.plan.clone(),
        output_dir: resolved.spec.output.bundle_root.clone(),
        outputs: paths.iter().map(|p| p.display().to_string()).collect(),
        diagnostics: manifest
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect(),
        toolchain: None,
        run_manifest: manifest,
        manifest_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_build_uses_captured_deck_and_source_edits_cannot_replace_it() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("deck.json"),
            include_bytes!("../../../../examples/presentation/synthetic.presentation.json"),
        )
        .unwrap();
        fs::write(root.path().join("build.yml"),"schema: renderflow/v2\nsources:\n  - id: deck\n    kind: presentation\n    path: deck.json\ntargets:\n  exact:\n    - format: presentation_source\noutput:\n  bundle_root: out\n").unwrap();
        let resolved =
            super::super::build::resolve(PlanningRequest::from_path(root.path().join("build.yml")))
                .unwrap();
        let identity = resolved
            .plan()
            .artifact_forest
            .as_ref()
            .unwrap()
            .identity
            .clone();
        assert!(resolved.graph().is_err());
        fs::write(root.path().join("deck.json"), b"changed after resolution").unwrap();
        let result = super::super::build::execute(resolved, false).unwrap();
        let forest = result.run_manifest.artifact_forest.unwrap();
        assert_eq!(forest.identity, identity);
        assert_eq!(
            forest.execution.unwrap().targets[0].outcome,
            ForestTargetOutcome::Published
        );
        assert_eq!(forest.produced_artifacts.len(), 1);
        assert!(root.path().join("out/renderflow-run.json").is_file());
    }
    #[test]
    fn unavailable_native_targets_never_become_published() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("deck.json"),
            include_bytes!("../../../../examples/presentation/synthetic.presentation.json"),
        )
        .unwrap();
        let text="schema: renderflow/v2\nsources:\n  - id: deck\n    kind: presentation\n    path: deck.json\ntargets:\n  exact:\n    - format: presentation_web\noutput:\n  bundle_root: out\n";
        fs::write(root.path().join("build.yml"), text).unwrap();
        let request = PlanningRequest::from_path(root.path().join("build.yml"));
        let resolved = super::super::build::resolve(request.clone()).unwrap();
        let dry = super::super::build::execute(resolved, true).unwrap();
        assert!(dry.run_manifest.artifact_manifest.artifacts.is_empty());
        assert!(!root.path().join("out").exists());
        let resolved = super::super::build::resolve(request).unwrap();
        assert!(super::super::build::execute(resolved, false).is_err());
        assert!(!root.path().join("out").exists());
    }
}
