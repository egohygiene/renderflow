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
    let artifact = store.put_bytes(b"same bytes", ArtifactDescriptor::for_format(
        Format::Pdf, ArtifactStorageClass::Intermediate)).unwrap();
    let print = target("print");
    let web = target("web");
    let mut first_report = report(artifact.clone(), "fixture.print");
    let mut second_report = report(artifact.clone(), "fixture.web");
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
    for (target, provider) in [(&print, "fixture.print"), (&web, "fixture.web")] {
        let item = evidence.iter().find(|item|
            item.metadata["renderflow.target.identity"] == serde_json::json!(target.identity())).unwrap();
        assert_eq!(item.producer.provider.as_deref(), Some(provider));
    }
    resolved.spec.execution.budgets.max_output_bytes = Some(artifact.size_bytes() * 2 - 1);
    assert!(validate_post_execution_budgets(&resolved, &targets, &artifacts, &[]).is_err());
    resolved.spec.execution.budgets.max_output_bytes = Some(artifact.size_bytes() * 2);
    resolved.spec.execution.budgets.max_storage_bytes = Some(artifact.size_bytes());
    resolved.spec.execution.budgets.max_artifacts = Some(1);
    assert!(validate_post_execution_budgets(&resolved, &targets, &artifacts, &[]).is_ok());
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
