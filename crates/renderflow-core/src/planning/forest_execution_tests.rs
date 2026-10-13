//! Authored forest execution fixtures. These do not probe external tools.

use super::*;
use crate::spec::{ExecutionPolicy, OutputLayout, SPEC_V2_ID};

fn target(preset: &str) -> ResolvedTarget {
    let mut target = ResolvedTarget::generated(Format::Pdf);
    target.preset = Some(preset.to_owned());
    target
}

fn resolved(targets: Vec<ResolvedTarget>) -> ResolvedExecution {
    let source = SourceSpec {
        id: "source.main".to_owned(), role: None, kind: SourceKind::Artifact,
        path: Some("input.md".to_owned()), uri: None, members: Vec::new(),
        media_type: None, format: Some("markdown".to_owned()), sha256: None,
        geometry: None, detect: false, immutable: true,
    };
    let spec = SpecV2 {
        schema: SPEC_V2_ID.to_owned(), sources: vec![source.clone()],
        profiles: BTreeMap::new(), hygiene: BTreeMap::new(), publication: None,
        targets: TargetSelection::default(), execution: ExecutionPolicy::default(),
        output: OutputLayout::default(), variables: BTreeMap::new(), transforms: None,
    };
    let dag = TransformGraph::new().build_multi_target_dag(Format::Markdown, &[]).unwrap();
    ResolvedExecution {
        plan: ExecutionPlan::from_dag(&dag, Format::Markdown, &[], OptimizationMode::Speed),
        config_path: PathBuf::from("renderflow.yaml"), spec,
        source_version: SourceSpecVersion::V2, source, source_path: PathBuf::from("input.md"),
        source_format: Format::Markdown, source_members: Vec::new(), print_pages: None,
        fixed_epub_pages: None, targets, dag, executor: DagExecutor::new(),
        execution_groups: Vec::new(), tool_registry: ToolRegistry::builtins(),
        resume_checkpoints: false, cancellation: None, speech_observer: None,
    }
}

fn report(artifact: Artifact, provider: &str) -> DagExecutionReport {
    let step_id = "step:markdown-to-pdf".to_owned();
    let diagnostic = ExecutionDiagnostic {
        severity: DiagnosticSeverity::Info, code: "fixture.produced".to_owned(),
        message: "Fixture output".to_owned(), step_id: Some(step_id.clone()),
    };
    DagExecutionReport {
        artifacts: HashMap::from([(Format::Pdf, artifact.clone())]),
        steps: vec![StepEvidence {
            step_id, transform: "fixture.render".to_owned(), transform_version: "1".to_owned(),
            capability: Some("fixture.render.pdf".to_owned()), provider: Some(provider.to_owned()),
            input_artifacts: Vec::new(), output_artifacts: vec![artifact.id().to_string()],
            configuration_digest: sha256_serialized(&provider).unwrap(),
            started_at_unix_ms: 1, completed_at_unix_ms: 2, duration_ms: 1,
            state: StepState::Complete, cache: crate::evidence::CacheDisposition::NotApplicable,
            validation: ValidationState::NotRequested, fidelity: FidelityDeclaration::Lossless,
            skip_reason: None, diagnostics: vec![diagnostic.clone()],
        }],
        diagnostics: vec![diagnostic],
    }
}

#[test]
fn distinct_pdf_profiles_keep_both_artifacts_and_namespaced_step_references() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(temporary.path()).unwrap();
    let print = target("print");
    let web = target("web");
    let first = store.put_bytes(b"print rendition", ArtifactDescriptor::for_format(
        Format::Pdf, ArtifactStorageClass::Intermediate)).unwrap();
    let second = store.put_bytes(b"web rendition", ArtifactDescriptor::for_format(
        Format::Pdf, ArtifactStorageClass::Intermediate)).unwrap();
    let mut first_report = report(first.clone(), "fixture.print");
    let mut second_report = report(second.clone(), "fixture.web");
    namespace_forest_steps(&mut first_report, &[print.clone()]).unwrap();
    namespace_forest_steps(&mut second_report, &[web.clone()]).unwrap();
    assert_ne!(first_report.steps[0].step_id, second_report.steps[0].step_id);
    for report in [&first_report, &second_report] {
        assert_eq!(report.diagnostics[0].step_id.as_deref(), Some(report.steps[0].step_id.as_str()));
        assert_eq!(report.steps[0].diagnostics[0].step_id.as_deref(), Some(report.steps[0].step_id.as_str()));
    }
    let mut targets = HashMap::new();
    let mut steps = HashMap::new();
    let mut artifacts = HashMap::new();
    record_forest_group(&first_report, &[print.clone()], &mut targets, &mut steps, &mut artifacts);
    record_forest_group(&second_report, &[web.clone()], &mut targets, &mut steps, &mut artifacts);
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[&print.identity()].id(), first.id());
    assert_eq!(targets[&web.identity()].id(), second.id());
    assert_eq!(artifacts.len(), 2);
    assert_ne!(steps[&print.identity()], steps[&web.identity()]);
}

#[test]
fn equal_bytes_keep_distinct_target_producers_and_count_each_publication() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(temporary.path()).unwrap();
    let print_metadata = serde_json::json!({"provider": "fixture.print", "review_required": true,
        "losses": ["print-specific-loss"], "observation": "print"});
    let web_metadata = serde_json::json!({"provider": "fixture.web", "review_required": false,
        "losses": ["web-specific-loss"], "observation": "web"});
    let artifact = store.put_bytes(b"same bytes", ArtifactDescriptor::for_format(
        Format::Pdf, ArtifactStorageClass::Intermediate)
        .with_metadata("renderflow.fixture", print_metadata.clone())).unwrap();
    let web_artifact = store.put_bytes(b"same bytes", ArtifactDescriptor::for_format(
        Format::Pdf, ArtifactStorageClass::Intermediate)
        .with_metadata("renderflow.fixture", web_metadata.clone())).unwrap();
    assert_eq!(artifact.id(), web_artifact.id());
    assert_ne!(artifact.metadata(), web_artifact.metadata());
    let print = target("print");
    let web = target("web");
    let mut first_report = report(artifact.clone(), "fixture.print");
    let mut second_report = report(web_artifact, "fixture.web");
    namespace_forest_steps(&mut first_report, &[print.clone()]).unwrap();
    namespace_forest_steps(&mut second_report, &[web.clone()]).unwrap();
    let mut targets = HashMap::new();
    let mut steps = HashMap::new();
    let mut artifacts = HashMap::new();
    record_forest_group(&first_report, &[print.clone()], &mut targets, &mut steps, &mut artifacts);
    record_forest_group(&second_report, &[web.clone()], &mut targets, &mut steps, &mut artifacts);
    first_report.steps.extend(second_report.steps);
    assert_eq!(targets.len(), 2);
    assert_eq!(artifacts.len(), 1);
    let mut resolved = resolved(vec![print.clone(), web.clone()]);
    let evidence = artifact_evidence(&resolved, &[], &first_report, &targets, &steps, &artifacts,
        &HashMap::new(), &[], &HashMap::new(), &HashMap::new(), &[], &HashMap::new());
    assert_eq!(evidence.len(), 2);
    for (target, provider, metadata) in [(&print, "fixture.print", &print_metadata),
        (&web, "fixture.web", &web_metadata)] {
        let item = evidence.iter().find(|item|
            item.metadata["renderflow.target.identity"] == serde_json::json!(target.identity())).unwrap();
        assert_eq!(item.producer.provider.as_deref(), Some(provider));
        assert_eq!(&item.metadata["renderflow.fixture"], metadata);
    }
    resolved.spec.execution.budgets.max_output_bytes = Some(artifact.size_bytes() * 2 - 1);
    assert!(validate_post_execution_budgets(&resolved, &targets, &artifacts, &[]).is_err());
    resolved.spec.execution.budgets.max_output_bytes = Some(artifact.size_bytes() * 2);
    resolved.spec.execution.budgets.max_storage_bytes = Some(artifact.size_bytes());
    resolved.spec.execution.budgets.max_artifacts = Some(1);
    assert!(validate_post_execution_budgets(&resolved, &targets, &artifacts, &[]).is_ok());
}

#[test]
fn same_id_hygiene_candidates_keep_metadata_decisions_scoped_to_each_target() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(temporary.path()).unwrap();
    let safe = store.put_bytes(b"same bytes", ArtifactDescriptor::for_format(
        Format::Pdf, ArtifactStorageClass::Intermediate)
        .with_metadata("renderflow.fixture", "safe metadata")).unwrap();
    let sensitive = store.put_bytes(b"same bytes", ArtifactDescriptor::for_format(
        Format::Pdf, ArtifactStorageClass::Intermediate)
        .with_metadata("renderflow.fixture", "fixture-blocked-marker")).unwrap();
    assert_eq!(safe.id(), sensitive.id());
    assert_ne!(hygiene_candidate_key(&safe).unwrap(), hygiene_candidate_key(&sensitive).unwrap());
    assert_eq!(hygiene_candidate_key(&safe).unwrap(), hygiene_candidate_key(&safe.clone()).unwrap());

    let mut policy = HygienePolicy::default();
    policy.secrets.markers = vec!["fixture-blocked-marker".to_owned()];
    let engine = HygieneEngine::new();
    let safe_outcome = engine.apply("fixture", &policy, &safe, &store).unwrap();
    let blocked_outcome = engine.apply("fixture", &policy, &sensitive, &store).unwrap();
    assert!(safe_outcome.releasable());
    assert!(!blocked_outcome.releasable());
    // Metadata and decisions differ although output bytes and lineage are equal.
    assert_eq!(safe_outcome.artifact.id(), blocked_outcome.artifact.id());
    let safe_target = target("safe");
    let blocked_target = target("blocked");
    let mut evidence = HashMap::new();
    let mut blocked = HashSet::new();
    assert!(!record_target_hygiene(&safe_target.identity(), &safe_outcome, &mut evidence, &mut blocked));
    assert!(record_target_hygiene(&blocked_target.identity(), &blocked_outcome, &mut evidence, &mut blocked));
    assert_eq!(blocked, HashSet::from([blocked_target.identity()]));
    assert_eq!(evidence.len(), 2);

    let mut safe_report = report(safe_outcome.artifact.clone(), "fixture.safe");
    let mut blocked_report = report(blocked_outcome.artifact.clone(), "fixture.blocked");
    namespace_forest_steps(&mut safe_report, &[safe_target.clone()]).unwrap();
    namespace_forest_steps(&mut blocked_report, &[blocked_target.clone()]).unwrap();
    let mut targets = HashMap::new();
    let mut steps = HashMap::new();
    let mut artifacts = HashMap::new();
    record_forest_group(&safe_report, &[safe_target.clone()], &mut targets, &mut steps, &mut artifacts);
    record_forest_group(&blocked_report, &[blocked_target.clone()], &mut targets, &mut steps, &mut artifacts);
    safe_report.steps.extend(blocked_report.steps);
    let resolved = resolved(vec![safe_target.clone(), blocked_target.clone()]);
    let outputs = HashMap::from([(safe_target.identity(), "bundle:safe.pdf".to_owned())]);
    let manifest_records = artifact_evidence(&resolved, &[], &safe_report, &targets, &steps, &artifacts,
        &outputs, &[], &HashMap::new(), &evidence, &[], &HashMap::new());
    assert_eq!(manifest_records.len(), 2);
    for (target, outcome) in [(&safe_target, &safe_outcome), (&blocked_target, &blocked_outcome)] {
        let item = manifest_records.iter().find(|item|
            item.metadata["renderflow.target.identity"] == serde_json::json!(target.identity())).unwrap();
        assert_eq!(item.hygiene.as_ref(), Some(&outcome.evidence));
        assert_eq!(item.metadata["renderflow.fixture"], outcome.artifact.metadata()["renderflow.fixture"]);
        assert_eq!(item.metadata["renderflow.target.published"], outcome.releasable());
    }
}

#[test]
fn naming_preserves_distinct_renditions_and_refuses_same_run_replace() {
    let mut resolved = resolved(vec![target("print"), target("web")]);
    resolved.spec.output.naming_template = "{target.role}.{ext}".to_owned();
    resolved.spec.output.collision = CollisionPolicy::Replace;
    assert!(render_output_paths(&resolved).unwrap_err().to_string().contains("same run"));
    resolved.spec.output.naming_template = "{target.preset}/{target.variant}/{target.identity}.{ext}".to_owned();
    let paths = render_output_paths(&resolved).unwrap();
    assert_ne!(paths[0], paths[1]);
    assert!(paths[0].to_string_lossy().contains("print/default/target-"));
    assert!(paths[1].to_string_lossy().contains("web/default/target-"));
}

#[test]
fn dedupe_reserves_generated_names_against_later_target_names() {
    let mut first = target("first");
    let mut second = target("second");
    let mut third = target("third");
    first.role = Some("pdf".to_owned());
    second.role = Some("pdf".to_owned());
    third.role = Some("pdf-2".to_owned());
    let mut resolved = resolved(vec![first, second, third]);
    resolved.spec.output.naming_template = "{target.role}.{ext}".to_owned();
    resolved.spec.output.collision = CollisionPolicy::Dedupe;
    let paths = render_output_paths(&resolved).unwrap();
    assert_eq!(paths.iter().collect::<HashSet<_>>().len(), 3);
    assert!(paths[1].ends_with("pdf-2.pdf"));
    assert!(paths[2].ends_with("pdf-2-2.pdf"));
}

fn outcome_branch(id: &str, state: ForestBranchState, source_identity: bool) -> ForestBranch {
    let selected = state == ForestBranchState::Selected;
    let steps = if source_identity { Vec::new() } else {
        vec![crate::graph::execution_plan::PlanEdge::from_transform_edge(
            &TransformEdge::new(Format::Text, Format::Fountain, 1.0, 0.75)
                .with_provider("tool.fixture", "fixture.recover"))]
    };
    ForestBranch {
        target_id: id.to_owned(), format: if source_identity { "text" } else { "fountain" }.to_owned(),
        preset: None, template: None, variant: None, role: Some(id.to_owned()),
        requirement: "optional".to_owned(), options: BTreeMap::new(), state,
        reason_code: "fixture.branch".to_owned(), reason: "Authored fixture branch".to_owned(),
        route: selected.then(|| crate::graph::ForestRouteEvidence {
            hop_count: steps.len(), steps, source_identity,
            cumulative_quality: if source_identity { 1.0 } else { 0.75 },
            cumulative_loss: if source_identity { 0.0 } else { 0.25 },
            weighted_cost: 1.0, ranking: "fixture".to_owned(), explored_candidates: 1,
        }),
    }
}

fn outcome_forest() -> ArtifactForest {
    ArtifactForest {
        identity: "sha256:fixture-forest".to_owned(), intermediates: "cache_only".to_owned(),
        policy: serde_json::json!({"fixture": true}),
        branches: vec![
            outcome_branch("target.first", ForestBranchState::Selected, false),
            outcome_branch("target.second", ForestBranchState::Selected, false),
            outcome_branch("target.rejected", ForestBranchState::Selected, false),
            outcome_branch("target.original", ForestBranchState::Selected, true),
            outcome_branch("target.excluded", ForestBranchState::Excluded, false),
            outcome_branch("target.unavailable", ForestBranchState::Unavailable, false),
            outcome_branch("target.unsupported", ForestBranchState::Unsupported, false),
            outcome_branch("target.budget", ForestBranchState::BudgetPruned, false),
        ],
        ..ArtifactForest::default()
    }
}

fn outcome_artifact(target: Option<&str>, id: &str, locator: &str, published: bool) -> ArtifactEvidence {
    let mut metadata = BTreeMap::from([("renderflow.screenplay_recovery".to_owned(),
        serde_json::json!({"lossy": true, "review_required": true,
            "qualification": "authored_unqualified", "candidate_sha256": "b".repeat(64)}))]);
    if let Some(target) = target {
        metadata.insert("renderflow.target.identity".to_owned(), serde_json::json!(target));
        metadata.insert("renderflow.target.published".to_owned(), serde_json::json!(published));
    }
    ArtifactEvidence {
        artifact_id: id.to_owned(), role: target.unwrap_or("retained").to_owned(),
        lifecycle: if target.is_some() { ArtifactRole::Terminal } else { ArtifactRole::Intermediate },
        locator: locator.to_owned(), format: "fountain".to_owned(), media_type: "text/plain".to_owned(),
        digest: crate::evidence::DigestEvidence { algorithm: "sha256".to_owned(), value: "a".repeat(64) },
        size_bytes: 7, producer: ProducerEvidence {
            system: "renderflow".to_owned(), transform: Some("fixture.recover".to_owned()),
            capability: Some("fixture.recovery".to_owned()), provider: Some("tool.fixture".to_owned()),
            version: Some("1".to_owned()),
        },
        sources: vec!["artifact:source".to_owned()], cache: crate::evidence::CacheDisposition::Miss,
        validation: ValidationState::Valid, validation_evidence: Vec::new(),
        fidelity: FidelityDeclaration::Lossy, hygiene: None, warnings: Vec::new(), metadata,
    }
}

#[test]
fn publication_outcomes_keep_same_content_destinations_and_full_native_evidence() {
    use crate::graph::ForestTargetOutcome;
    let mut resolved = resolved(Vec::new());
    resolved.plan.attach_artifact_forest(outcome_forest());
    resolved.spec.output.bundle_root = "dist/fixture".to_owned();
    let mut source = outcome_artifact(None, "artifact:source", "artifact-store:source", false);
    source.lifecycle = ArtifactRole::Source;
    let retained = outcome_artifact(None, "artifact:retained", "bundle:evidence.json", false);
    let first = outcome_artifact(Some("target.first"), "artifact:same", "bundle:first.fountain", true);
    let mut second = outcome_artifact(Some("target.second"), "artifact:same", "bundle:second.fountain", true);
    second.producer.provider = Some("tool.other-fixture".to_owned());
    let mut rejected = outcome_artifact(Some("target.rejected"), "artifact:rejected", "artifact-store:rejected", false);
    rejected.validation = ValidationState::Invalid;
    let original = outcome_artifact(Some("target.original"), "artifact:source", "bundle:original.txt", true);
    let artifacts = vec![source, retained, first, second, rejected, original];
    let manifest = build_run_manifest(&resolved, 1, RunState::Partial,
        ["first.fountain", "second.fountain", "original.txt", "evidence.json"].into_iter()
            .map(|name| format!("dist/fixture/{name}")).collect(),
        artifacts.clone(), Vec::new(), Vec::new()).unwrap();
    let forest = manifest.artifact_forest.as_ref().unwrap();
    assert_eq!(forest.produced_artifacts, ["artifact:same"]);
    assert_eq!(forest.branches, resolved.plan.artifact_forest.as_ref().unwrap().branches);
    let execution = forest.execution.as_ref().unwrap();
    assert_eq!(execution.schema, crate::graph::FOREST_EXECUTION_SCHEMA_V1);
    assert_eq!(execution.targets.len(), forest.branches.len());
    for (index, target) in execution.targets.iter().enumerate() {
        assert_eq!(target.target_id, forest.branches[index].target_id);
    }
    for (target_index, artifact_index, locator) in [(0, 2, "bundle:first.fountain"), (1, 3, "bundle:second.fountain")] {
        let target = &execution.targets[target_index];
        assert_eq!(target.outcome, ForestTargetOutcome::Published);
        assert_eq!(target.artifact_evidence_index, Some(artifact_index));
        assert_eq!(target.locator.as_deref(), Some(locator));
        let native = &manifest.artifact_manifest.artifacts[target.artifact_evidence_index.unwrap()];
        assert_eq!(native, &artifacts[artifact_index]);
        assert_eq!(native.sources, ["artifact:source"]);
        assert_eq!(native.fidelity, FidelityDeclaration::Lossy);
        assert_eq!(native.metadata["renderflow.screenplay_recovery"]["review_required"], true);
    }
    assert_eq!(execution.targets[2].outcome, ForestTargetOutcome::NotPublished);
    assert_eq!(execution.targets[2].artifact_evidence_index, Some(4));
    assert!(execution.targets[2].locator.is_none());
    assert_eq!(execution.targets[3].outcome, ForestTargetOutcome::Published);
    assert!(execution.targets[3].source_identity);
    assert!(execution.targets[4..].iter().all(|target| target.outcome == ForestTargetOutcome::NotSelected
        && target.artifact_evidence_index.is_none() && target.locator.is_none()));
    assert!(resolved.plan.artifact_forest.as_ref().unwrap().execution.is_none());
}

#[test]
fn planned_failed_and_cancelled_runs_never_infer_target_publication() {
    use crate::graph::ForestTargetOutcome;
    for state in [RunState::Planned, RunState::Failed, RunState::Cancelled, RunState::Partial] {
        let mut forest = outcome_forest();
        // Planned paths and a retained canonical record are not selected-target publication.
        let artifacts = [outcome_artifact(Some("target.first"), "artifact:retained", "bundle:retained.json", false)];
        attach_forest_execution(&mut forest, state, "dist", &["dist/retained.json".to_owned()], &artifacts, &[]);
        assert!(forest.produced_artifacts.is_empty());
        let targets = &forest.execution.as_ref().unwrap().targets;
        for target in &targets[..4] {
            assert_eq!(target.outcome, if state == RunState::Planned {
                ForestTargetOutcome::Planned
            } else { ForestTargetOutcome::NotPublished });
            assert!(target.locator.is_none());
            if state == RunState::Planned { assert!(target.artifact_evidence_index.is_none()); }
        }
        assert!(targets[4..].iter().all(|target| target.outcome == ForestTargetOutcome::NotSelected));
    }
}

#[test]
fn publication_requires_target_materialization_and_actual_output_membership() {
    use crate::graph::ForestTargetOutcome;
    for (published, locator, outputs) in [
        (false, "bundle:first.fountain", vec!["dist/first.fountain".to_owned()]),
        (true, "bundle:first.fountain", Vec::new()),
        (true, "artifact-store:first", vec!["dist/first.fountain".to_owned()]),
    ] {
        let mut forest = outcome_forest();
        let artifact = outcome_artifact(Some("target.first"), "artifact:one", locator, published);
        attach_forest_execution(&mut forest, RunState::Complete, "dist", &outputs, &[artifact], &[]);
        assert_eq!(forest.execution.as_ref().unwrap().targets[0].outcome, ForestTargetOutcome::NotPublished);
        assert!(forest.produced_artifacts.is_empty());
    }
}

#[test]
fn producer_reference_is_bound_to_the_recorded_target_step_and_artifact() {
    let mut forest = outcome_forest();
    let mut artifact = outcome_artifact(Some("target.first"), "artifact:one", "bundle:first.fountain", true);
    artifact.metadata.insert("renderflow.target.producing_step_id".to_owned(), serde_json::json!("step:fixture"));
    let mut step = StepEvidence {
        step_id: "step:fixture".to_owned(), transform: "fixture.recover".to_owned(), transform_version: "1".to_owned(),
        capability: Some("fixture.recovery".to_owned()), provider: Some("tool.fixture".to_owned()),
        input_artifacts: vec!["artifact:source".to_owned()], output_artifacts: vec!["artifact:one".to_owned()],
        configuration_digest: sha256_serialized(&"fixture").unwrap(), started_at_unix_ms: 1,
        completed_at_unix_ms: 2, duration_ms: 1, state: StepState::Complete,
        cache: crate::evidence::CacheDisposition::Miss, validation: ValidationState::Valid,
        fidelity: FidelityDeclaration::Lossy, skip_reason: None, diagnostics: Vec::new(),
    };
    let outputs = ["dist/first.fountain".to_owned()];
    attach_forest_execution(&mut forest, RunState::Complete, "dist", &outputs, &[artifact.clone()], &[step.clone()]);
    assert_eq!(forest.execution.as_ref().unwrap().targets[0].producing_step_id.as_deref(), Some("step:fixture"));
    step.output_artifacts = vec!["artifact:another".to_owned()];
    attach_forest_execution(&mut forest, RunState::Complete, "dist", &outputs, &[artifact], &[step]);
    assert!(forest.execution.as_ref().unwrap().targets[0].producing_step_id.is_none());
}

#[test]
fn zero_edge_source_intent_counts_hygiene_lineage_as_a_produced_derivative() {
    use crate::graph::ForestTargetOutcome;
    let temporary = tempfile::tempdir().unwrap();
    let store = ArtifactStore::new(temporary.path()).unwrap();
    let source = store.put_bytes(b"unchanged payload", ArtifactDescriptor::for_format(
        Format::Text, ArtifactStorageClass::Source)).unwrap();
    // Even byte-preserving hygiene creates a distinct source-rooted record.
    let hygienic = store.put_bytes(b"unchanged payload", ArtifactDescriptor::for_format(
        Format::Text, ArtifactStorageClass::Terminal).with_source(source.id().clone())
        .with_metadata("renderflow.hygiene.policy", "fixture.strip-metadata")).unwrap();
    assert_eq!(source.digest(), hygienic.digest());
    assert_ne!(source.id(), hygienic.id());
    let source_evidence = ArtifactEvidence::from_artifact(&source, "source", ArtifactRole::Source,
        "artifact-store:source", ProducerEvidence::source(), ValidationState::NotRequested,
        FidelityDeclaration::Lossless);
    for (artifact, is_original) in [(&source, true), (&hygienic, false)] {
        let mut forest = outcome_forest();
        let mut terminal = ArtifactEvidence::from_artifact(artifact, "target.original", ArtifactRole::Terminal,
            "bundle:original.txt", ProducerEvidence::source(), ValidationState::Valid,
            FidelityDeclaration::Lossless);
        terminal.metadata.insert("renderflow.target.identity".to_owned(), serde_json::json!("target.original"));
        terminal.metadata.insert("renderflow.target.published".to_owned(), serde_json::json!(true));
        attach_forest_execution(&mut forest, RunState::Complete, "dist",
            &["dist/original.txt".to_owned()], &[source_evidence.clone(), terminal], &[]);
        let outcome = &forest.execution.as_ref().unwrap().targets[3];
        assert_eq!(outcome.outcome, ForestTargetOutcome::Published);
        assert_eq!(outcome.source_identity, is_original);
        assert!(forest.branches[3].route.as_ref().unwrap().source_identity);
        assert_eq!(forest.produced_artifacts, if is_original { Vec::new() }
            else { vec![hygienic.id().to_string()] });
    }
}

#[test]
fn nested_and_standalone_forest_execution_schema_contracts_match() {
    let standalone: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/renderflow-forest-execution-v1.schema.json")).unwrap();
    let forest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/renderflow-artifact-forest-v1.schema.json")).unwrap();
    let run: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/renderflow-run-v1.schema.json")).unwrap();
    assert_eq!(standalone["properties"]["schema"]["const"], crate::graph::FOREST_EXECUTION_SCHEMA_V1);
    let nested = &forest["properties"]["execution"];
    assert_eq!(nested, &run["$defs"]["artifact_forest"]["properties"]["execution"]);
    for key in ["type", "additionalProperties", "description", "required", "properties"] {
        assert_eq!(nested[key], standalone[key]);
    }
}
