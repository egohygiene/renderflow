//! Authored integration fixtures for pure forest planning; no runtime probes.

use super::*;
use crate::graph::execution_plan::PlanSourceArtifact;
use crate::intake::{DetectionConfidence, ResolvedArtifactProfile};

fn minimal_spec(targets: serde_json::Value) -> SpecV2 {
    serde_json::from_value(serde_json::json!({
        "schema": "renderflow/v2",
        "sources": [{"id": "source.main", "path": "input.md", "format": "markdown"}],
        "targets": targets,
    })).unwrap()
}

fn builtin_edge(from: Format, to: Format, quality: f32) -> TransformEdge {
    TransformEdge::new(from, to, 1.0, quality)
        .with_evidence("adapter", BUILTIN_ADAPTER_EVIDENCE)
}

fn planned(
    spec: &SpecV2,
    graph: &TransformGraph,
    available: &TransformGraph,
) -> forest::PlannedForest {
    let requested = resolve_target_intent(spec, graph, Format::Markdown).unwrap();
    plan_forest_routes(spec, graph, graph, available, Format::Markdown, requested).unwrap()
}

fn branch_for(forest: &ArtifactForest, format: Format) -> &ForestBranch {
    forest.branches.iter().find(|branch| branch.format == format.to_string()).unwrap()
}

#[test]
fn distinct_same_format_presets_keep_separate_route_groups() {
    let spec = minimal_spec(serde_json::json!({"exact": [
        {"id": "target.print", "format": "pdf", "preset": "print"},
        {"id": "target.screen", "format": "pdf", "preset": "screen"}
    ]}));
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 1.0));
    let forest = planned(&spec, &graph, &graph);
    assert_eq!(forest.routes.len(), 2);
    assert_ne!(forest.routes[0].0.identity(), forest.routes[1].0.identity());
    let groups = group_forest_routes(Format::Markdown, &forest.routes).unwrap();
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|(dag, targets)| dag.edge_count() == 1 && targets.len() == 1));
    assert_eq!(forest.evidence.branches.iter()
        .filter(|branch| branch.state == ForestBranchState::Selected).count(), 2);
}

#[test]
fn canonical_aliases_deduplicate_and_required_intent_wins() {
    let spec = minimal_spec(serde_json::json!({"exact": [
        {"format": "md", "requirement": "optional"},
        {"format": "markdown", "requirement": "required"},
        {"format": "md", "requirement": "optional"}
    ]}));
    let graph = TransformGraph::new();
    let requested = resolve_target_intent(&spec, &graph, Format::Markdown).unwrap();
    assert_eq!(requested.len(), 1);
    assert_eq!(requested[0].format, Format::Markdown);
    assert_eq!(requested[0].requirement, TargetRequirement::Required);
    let forest = planned(&spec, &graph, &graph);
    assert_eq!(forest.routes.len(), 1);
    assert!(forest.routes[0].1.path.steps.is_empty());
}

#[test]
fn exhaustive_reversible_graph_excludes_source_and_selects_each_identity_once() {
    let spec = minimal_spec(serde_json::json!({"all_reachable": true}));
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Html, 1.0));
    graph.add_transform(builtin_edge(Format::Html, Format::Markdown, 1.0));
    graph.add_transform(builtin_edge(Format::Html, Format::Pdf, 0.8));
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 1.0));
    let forest = planned(&spec, &graph, &graph);
    assert_eq!(forest.routes.len(), 2);
    assert_eq!(forest.routes.iter().map(|(target, _)| target.identity()).collect::<HashSet<_>>().len(), 2);
    for (target, route) in &forest.routes {
        assert_ne!(target.format, Format::Markdown);
        let mut visited = HashSet::from([Format::Markdown]);
        for edge in &route.path.steps { assert!(visited.insert(edge.to)); }
    }
    let source = branch_for(&forest.evidence, Format::Markdown);
    assert_eq!(source.state, ForestBranchState::Excluded);
    assert_eq!(source.reason_code, "branch.source_identity_excluded");
    let pdf = branch_for(&forest.evidence, Format::Pdf).route.as_ref().unwrap();
    assert_eq!(pdf.hop_count, 1);
    assert_eq!(pdf.cumulative_quality, 1.0);
}

#[test]
fn explicit_source_is_zero_edge_identity_but_render_changes_are_unsupported() {
    let graph = TransformGraph::new();
    let spec = minimal_spec(serde_json::json!({"exact": [{"format": "markdown"}]}));
    let forest = planned(&spec, &graph, &graph);
    let source = branch_for(&forest.evidence, Format::Markdown);
    assert_eq!(source.state, ForestBranchState::Selected);
    let route = source.route.as_ref().unwrap();
    assert!(route.source_identity);
    assert!(route.steps.is_empty());
    assert_eq!(route.cumulative_loss, 0.0);
    let changed = minimal_spec(serde_json::json!({"exact": [{"format": "markdown", "preset": "changed"}]}));
    let forest = planned(&changed, &graph, &graph);
    assert!(forest.routes.is_empty());
    assert_eq!(branch_for(&forest.evidence, Format::Markdown).reason_code,
        "branch.source_regeneration_unsupported");
}

#[test]
fn unavailable_provider_is_only_diagnostic_and_never_a_selected_output() {
    let spec = minimal_spec(serde_json::json!({"exact": [{"format": "pdf"}]}));
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 1.0)
        .with_provider("tool.missing", "render.pdf"));
    let available = TransformGraph::new();
    let forest = planned(&spec, &graph, &available);
    assert!(forest.routes.is_empty());
    assert!(forest.evidence.produced_artifacts.is_empty());
    let pdf = branch_for(&forest.evidence, Format::Pdf);
    assert_eq!(pdf.state, ForestBranchState::Unavailable);
    assert_eq!(pdf.reason_code, "branch.provider_unavailable");
    assert_eq!(pdf.route.as_ref().unwrap().steps[0].provider_id.as_deref(), Some("tool.missing"));
    let groups = group_forest_routes(Format::Markdown, &forest.routes).unwrap();
    assert!(groups.iter().all(|(dag, targets)| dag.edge_count() == 0 && targets.is_empty()));
}

#[test]
fn terminal_constraints_select_the_requested_provider_capability_transform_and_variant() {
    let spec = minimal_spec(serde_json::json!({"exact": [{
        "format": "pdf", "provider": "tool.selected", "capability": "render.pdf",
        "transform": "render.selected", "variant": "variant.print"
    }]}));
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 1.0)
        .with_provider("tool.other", "render.other")
        .with_evidence("transform_id", "render.other"));
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 0.9)
        .with_provider("tool.selected", "render.pdf")
        .with_evidence("transform_id", "render.selected")
        .with_variant("variant.print"));
    let forest = planned(&spec, &graph, &graph);
    assert_eq!(forest.routes.len(), 1);
    let edge = &forest.routes[0].1.path.steps[0];
    assert_eq!(edge.provider_id.as_deref(), Some("tool.selected"));
    assert_eq!(edge.capability_id.as_deref(), Some("render.pdf"));
    assert_eq!(edge.evidence.get("transform_id").map(String::as_str), Some("render.selected"));
    assert_eq!(edge.variant_id.as_deref(), Some("variant.print"));
    let mut unsupported = spec.clone();
    unsupported.targets.exact[0].capability = Some("render.unregistered".to_owned());
    let forest = planned(&unsupported, &graph, &graph);
    assert!(forest.routes.is_empty());
    assert_eq!(branch_for(&forest.evidence, Format::Pdf).reason_code, "branch.no_eligible_route");
}

#[test]
fn profile_provider_include_remains_a_terminal_constraint() {
    let mut spec = minimal_spec(serde_json::json!({"profiles": ["selected.provider"]}));
    spec.profiles.insert("selected.provider".to_owned(), DerivativeProfile {
        include: SelectorSet { providers: vec!["tool.a".to_owned()], ..SelectorSet::default() },
        ..DerivativeProfile::default()
    });
    spec.execution.optimization = OptimizationMode::Speed;
    let mut graph = TransformGraph::new();
    graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 10.0, 0.9)
        .with_provider("tool.a", "render.pdf"));
    graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 0.1, 1.0)
        .with_provider("tool.b", "render.pdf"));
    let forest = planned(&spec, &graph, &graph);
    assert_eq!(forest.routes.len(), 1);
    assert_eq!(forest.routes[0].1.path.steps[0].provider_id.as_deref(), Some("tool.a"));
    assert_eq!(forest.routes[0].0.include_selectors[0].providers, vec!["tool.a"]);
}

#[test]
fn provider_exclusion_does_not_remove_a_different_provider_of_the_same_format() {
    let spec = minimal_spec(serde_json::json!({
        "exact": [
            {"id": "target.a", "format": "pdf", "provider": "tool.a"},
            {"id": "target.b", "format": "pdf", "provider": "tool.b"}
        ],
        "exclude": {"providers": ["tool.a"]}
    }));
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 1.0)
        .with_provider("tool.a", "render.pdf"));
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 1.0)
        .with_provider("tool.b", "render.pdf"));
    let requested = resolve_target_intent(&spec, &graph, Format::Markdown).unwrap();
    let excluded_id = requested.iter().find(|target| target.provider.as_deref() == Some("tool.a"))
        .unwrap().identity();
    let forest = planned(&spec, &graph, &graph);
    assert_eq!(forest.routes.len(), 1);
    assert_eq!(forest.routes[0].0.provider.as_deref(), Some("tool.b"));
    assert_eq!(forest.routes[0].1.path.steps[0].provider_id.as_deref(), Some("tool.b"));
    let excluded = forest.evidence.branches.iter().find(|branch| branch.target_id == excluded_id).unwrap();
    assert_eq!(excluded.state, ForestBranchState::Excluded);
    assert_eq!(excluded.reason_code, "branch.selector_excluded");
}

#[test]
fn targeted_speed_keeps_optimization_priority_while_exhaustive_prefers_quality() {
    let mut spec = minimal_spec(serde_json::json!({"exact": [{"format": "pdf"}]}));
    spec.execution.optimization = OptimizationMode::Speed;
    let mut graph = TransformGraph::new();
    graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 10.0, 1.0)
        .with_provider("tool.quality", "render.pdf"));
    graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 0.1, 0.5)
        .with_provider("tool.fast", "render.pdf"));
    let targeted = planned(&spec, &graph, &graph);
    assert_eq!(targeted.routes[0].1.path.steps[0].provider_id.as_deref(), Some("tool.fast"));
    spec.targets.all_reachable = true;
    let exhaustive = planned(&spec, &graph, &graph);
    assert_eq!(exhaustive.routes[0].1.path.steps[0].provider_id.as_deref(), Some("tool.quality"));
    assert_ne!(targeted.evidence.policy["ranking"], exhaustive.evidence.policy["ranking"]);
}

#[test]
fn cumulative_loss_budget_applies_to_the_whole_route() {
    let mut spec = minimal_spec(serde_json::json!({"exact": [{"format": "pdf"}]}));
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Html, 0.9));
    graph.add_transform(builtin_edge(Format::Html, Format::Pdf, 0.9));
    spec.execution.derivative_planning.max_loss = Some(0.15);
    let rejected = planned(&spec, &graph, &graph);
    assert!(rejected.routes.is_empty());
    assert_eq!(branch_for(&rejected.evidence, Format::Pdf).state, ForestBranchState::Unsupported);
    spec.execution.derivative_planning.max_loss = Some(0.25);
    let accepted = planned(&spec, &graph, &graph);
    let route = branch_for(&accepted.evidence, Format::Pdf).route.as_ref().unwrap();
    assert_eq!(route.hop_count, 2);
    assert!((route.cumulative_loss - 0.19).abs() < 0.000_001);
}

#[test]
fn generic_options_are_explicitly_unsupported_instead_of_rendering_defaults() {
    let spec = minimal_spec(serde_json::json!({"exact": [{
        "format": "pdf", "options": {"quality": "custom"}
    }]}));
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 1.0));
    let forest = planned(&spec, &graph, &graph);
    assert!(forest.routes.is_empty());
    let pdf = branch_for(&forest.evidence, Format::Pdf);
    assert_eq!(pdf.state, ForestBranchState::Unsupported);
    assert_eq!(pdf.reason_code, "branch.options_unsupported");
    assert_eq!(pdf.options["quality"], "custom");
}

#[test]
fn effective_hop_budget_uses_the_stricter_execution_depth() {
    let mut spec = minimal_spec(serde_json::json!({"exact": [{"format": "pdf"}]}));
    spec.execution.derivative_planning.max_hops = 8;
    spec.execution.budgets.max_depth = Some(1);
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Html, 1.0));
    graph.add_transform(builtin_edge(Format::Html, Format::Pdf, 1.0));
    let forest = planned(&spec, &graph, &graph);
    assert!(forest.routes.is_empty());
    assert_eq!(forest.evidence.policy["effective_max_hops"], 1);
    spec.execution.budgets.max_depth = Some(2);
    assert_eq!(planned(&spec, &graph, &graph).routes.len(), 1);
}

#[test]
fn exhausted_search_is_an_explicit_nonselected_branch() {
    let mut spec = minimal_spec(serde_json::json!({"exact": [{"format": "pdf"}]}));
    spec.execution.derivative_planning.max_candidates = 1;
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Html, 1.0));
    graph.add_transform(builtin_edge(Format::Html, Format::Pdf, 1.0));
    let forest = planned(&spec, &graph, &graph);
    assert!(forest.routes.is_empty());
    let pdf = branch_for(&forest.evidence, Format::Pdf);
    assert_eq!(pdf.state, ForestBranchState::Unsupported);
    assert_eq!(pdf.reason_code, "branch.route_search_failed");
    assert!(pdf.route.is_none());
}

#[test]
fn unavailable_route_search_limit_preserves_diagnostic_plan() {
    let mut spec = minimal_spec(serde_json::json!({"exact": [{"format": "pdf"}]}));
    spec.execution.derivative_planning.max_candidates = 1;
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Html, 1.0));
    graph.add_transform(builtin_edge(Format::Html, Format::Pdf, 1.0)
        .with_provider("tool.unavailable", "render.pdf"));
    let available = TransformGraph::new();
    let forest = planned(&spec, &graph, &available);
    assert!(forest.routes.is_empty());
    let pdf = branch_for(&forest.evidence, Format::Pdf);
    assert_eq!(pdf.state, ForestBranchState::Unsupported);
    assert_eq!(pdf.reason_code, "branch.route_search_failed");
    assert!(pdf.route.is_none());
}

fn evidence_plan(spec: &SpecV2) -> ExecutionPlan {
    let mut graph = TransformGraph::new();
    graph.add_transform(builtin_edge(Format::Markdown, Format::Pdf, 1.0));
    let forest = planned(spec, &graph, &graph);
    let groups = group_forest_routes(Format::Markdown, &forest.routes).unwrap();
    let targets = forest.routes.iter().map(|(target, _)| target.format).collect::<Vec<_>>();
    let mut plan = ExecutionPlan::from_dag(&groups[0].0, Format::Markdown, &targets, spec.execution.optimization);
    plan.source_artifact = Some(PlanSourceArtifact {
        artifact_id: "artifact.source".to_owned(), digest: "sha256:source-one".to_owned(), size_bytes: 12,
        profile: ResolvedArtifactProfile {
            format: Some("markdown".to_owned()), media_type: "text/markdown".to_owned(),
            confidence: DetectionConfidence::High, container_format: None, stream_formats: Vec::new(),
            families: vec!["document".to_owned()], metadata: BTreeMap::new(),
            available_operations: Vec::new(), signals: Vec::new(), conflicts: Vec::new(),
        },
    });
    plan.toolchain = Some(ToolchainSnapshot {
        schema: crate::toolchain::TOOLCHAIN_SCHEMA.to_owned(), fingerprint: "sha256:toolchain-one".to_owned(),
        operating_system: "fixture".to_owned(), architecture: "fixture".to_owned(), selected_tools: Vec::new(),
    });
    plan.attach_artifact_forest(forest.evidence);
    plan
}

#[test]
fn forest_digest_binds_source_spec_target_options_and_toolchain_and_is_idempotent() {
    let spec = minimal_spec(serde_json::json!({"exact": [{"format": "pdf"}]}));
    let mut base = evidence_plan(&spec);
    finalize_forest_identity(&mut base, &spec).unwrap();
    let baseline = base.artifact_forest.as_ref().unwrap().identity.clone();
    finalize_forest_identity(&mut base, &spec).unwrap();
    assert_eq!(base.artifact_forest.as_ref().unwrap().identity, baseline);

    let mut source_changed = base.clone();
    source_changed.source_artifact.as_mut().unwrap().digest = "sha256:source-two".to_owned();
    finalize_forest_identity(&mut source_changed, &spec).unwrap();
    assert_ne!(source_changed.artifact_forest.as_ref().unwrap().identity, baseline);

    let mut policy_changed = spec.clone();
    policy_changed.execution.derivative_planning.max_hops = 3;
    let mut policy_plan = base.clone();
    finalize_forest_identity(&mut policy_plan, &policy_changed).unwrap();
    assert_ne!(policy_plan.artifact_forest.as_ref().unwrap().identity, baseline);

    let mut options_changed = spec.clone();
    options_changed.targets.exact[0].options.insert("custom".to_owned(), serde_json::json!(true));
    let mut options_plan = base.clone();
    finalize_forest_identity(&mut options_plan, &options_changed).unwrap();
    assert_ne!(options_plan.artifact_forest.as_ref().unwrap().identity, baseline);

    let mut toolchain_changed = base.clone();
    toolchain_changed.toolchain.as_mut().unwrap().fingerprint = "sha256:toolchain-two".to_owned();
    finalize_forest_identity(&mut toolchain_changed, &spec).unwrap();
    assert_ne!(toolchain_changed.artifact_forest.as_ref().unwrap().identity, baseline);
}
