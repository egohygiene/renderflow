//! Target identities and branch-local route selection for canonical planning.
use super::*;
use crate::graph::{DerivativePathConstraints, DerivativePathPolicy, SelectedDerivativePath, TransformPath};
use crate::graph::execution_plan::{ForestExecutionEvidence, ForestRouteEvidence,
    ForestTargetExecution, ForestTargetOutcome, PlanEdge, FOREST_EXECUTION_SCHEMA_V1};

pub(super) struct PlannedForest {
    pub routes: Vec<(ResolvedTarget, SelectedDerivativePath)>,
    pub evidence: ArtifactForest,
}

pub(super) fn selector_matches_identity(target: &ResolvedTarget, selector: &SelectorSet) -> bool {
    selector.formats.iter().any(|format| format.parse::<Format>().ok() == Some(target.format))
        || selector.families.iter().any(|family| format_in_family(target.format, family))
        || target.role.as_ref().is_some_and(|role| selector.roles.contains(role))
}

pub(super) fn selector_matches_edge(selector: &SelectorSet, edge: &TransformEdge) -> bool {
    edge.capability_id.as_ref().is_some_and(|id| selector.capabilities.contains(id))
        || edge.evidence.get("transform_id").is_some_and(|id| selector.transforms.contains(id))
        || edge.provider_id.as_ref().is_some_and(|id| selector.providers.contains(id))
        || edge.required_provider_ids.iter().any(|id| selector.providers.contains(id))
        || edge.variant_id.as_ref().is_some_and(|id| selector.variants.contains(id))
}

pub(super) fn target_edge_matches(target: &ResolvedTarget, edge: &TransformEdge) -> bool {
    target.capability.as_ref().is_none_or(|id| edge.capability_id.as_ref() == Some(id))
        && target.transform.as_ref().is_none_or(|id| edge.evidence.get("transform_id") == Some(id))
        && target.provider.as_ref().is_none_or(|id| edge.provider_id.as_ref() == Some(id))
        && target.variant.as_ref().is_none_or(|id| edge.variant_id.as_ref() == Some(id))
}

fn terminal_edge_allowed(target: &ResolvedTarget, edge: &TransformEdge) -> bool {
    target.include_selectors.iter().all(|selector|
        selector_matches_identity(target, selector) || selector_matches_edge(selector, edge))
        && target.exclude_selectors.iter().all(|selector|
            !selector_matches_identity(target, selector) && !selector_matches_edge(selector, edge))
}

fn exhaustive(spec: &SpecV2) -> bool {
    spec.targets.all_reachable || !spec.targets.profiles.is_empty()
}

fn select_route(spec: &SpecV2, graph: &TransformGraph, source: Format, target: &ResolvedTarget,
    policy: &DerivativePathPolicy) -> std::result::Result<Option<SelectedDerivativePath>, crate::graph::DerivativePathError> {
    if source == target.format && (!target.include_selectors.iter().all(|selector| selector_matches_identity(target, selector))
        || target.exclude_selectors.iter().any(|selector| selector_matches_identity(target, selector))) {
        return Ok(None);
    }
    let filtered = graph.filtered_by(|edge| edge.to != target.format || terminal_edge_allowed(target, edge));
    if exhaustive(spec) {
        filtered.select_derivative_path(source, target.format, &constraints(target), policy)
    } else {
        filtered.select_target_path(source, target.format, &constraints(target), policy)
    }
}

fn constraints(target: &ResolvedTarget) -> DerivativePathConstraints {
    DerivativePathConstraints {
        capability_id: target.capability.clone(), transform_id: target.transform.clone(),
        provider_id: target.provider.clone(), variant_id: target.variant.clone(),
    }
}

fn branch(target: &ResolvedTarget, state: ForestBranchState, code: &str, reason: &str) -> ForestBranch {
    ForestBranch {
        target_id: target.identity(), format: target.format.to_string(), role: target.role.clone(),
        preset: target.preset.clone(), template: target.template.clone(), variant: target.variant.clone(),
        requirement: match target.requirement { TargetRequirement::Required => "required", TargetRequirement::Optional => "optional" }.to_owned(),
        options: target.options.clone(), state, reason_code: code.to_owned(), reason: reason.to_owned(), route: None,
    }
}

fn route_evidence(selected: &SelectedDerivativePath) -> ForestRouteEvidence {
    ForestRouteEvidence {
        steps: selected.path.steps.iter().map(PlanEdge::from_transform_edge).collect(),
        source_identity: selected.path.steps.is_empty(), cumulative_quality: selected.evidence.total_quality,
        cumulative_loss: selected.evidence.cumulative_loss, hop_count: selected.evidence.hops,
        weighted_cost: selected.evidence.optimization_cost, ranking: selected.evidence.ranking_policy.clone(),
        explored_candidates: selected.evidence.explored_candidates,
    }
}

pub(super) fn plan_forest_routes(
    spec: &SpecV2, graph: &TransformGraph, policy_graph: &TransformGraph,
    available_graph: &TransformGraph, source: Format, requested: Vec<ResolvedTarget>,
) -> Result<PlannedForest> {
    let configured = &spec.execution.derivative_planning;
    let policy = DerivativePathPolicy {
        max_hops: spec.execution.budgets.max_depth.map(|depth| configured.max_hops.min(depth as usize)).unwrap_or(configured.max_hops),
        max_candidates: configured.max_candidates, max_loss: configured.max_loss,
        optimization: spec.execution.optimization,
    };
    let mut routes = Vec::new();
    let mut branches = Vec::new();
    for target in requested {
        // Adapters must explicitly interpret options. Treating arbitrary JSON as
        // evidence while rendering defaults would make distinct targets a lie.
        if !target.options.is_empty() {
            branches.push(branch(&target, ForestBranchState::Unsupported, "branch.options_unsupported", "this adapter contract does not interpret target.options; use a supported preset/template or a registered transform variant"));
            continue;
        }
        if target.format == source && (target.preset.is_some() || target.template.is_some()) {
            branches.push(branch(&target, ForestBranchState::Unsupported, "branch.source_regeneration_unsupported", "an explicit source-format target is an identity export; regeneration with render settings requires a distinct registered artifact capability"));
            continue;
        }
        let selected = match select_route(spec, available_graph, source, &target, &policy) {
            Ok(Some(selected)) => selected,
            Ok(None) => {
                let policy_route = select_route(spec, policy_graph, source, &target, &policy);
                if let Ok(Some(route)) = policy_route {
                    let mut evidence = branch(&target, ForestBranchState::Unavailable, "branch.provider_unavailable", "no eligible route has all required providers available; this branch is diagnostic only");
                    evidence.route = Some(route_evidence(&route));
                    branches.push(evidence);
                } else if let Err(error) = policy_route {
                    branches.push(branch(&target, ForestBranchState::Unsupported, "branch.route_search_failed", &error.to_string()));
                } else if !target.exclude_selectors.is_empty() && graph.transforms_to(target.format).iter().any(|edge|
                    target_edge_matches(&target, edge) && !terminal_edge_allowed(&target, edge)) {
                    branches.push(branch(&target, ForestBranchState::Excluded, "branch.selector_excluded", "terminal routes matching this identity were excluded by profile/spec/CLI selectors"));
                } else {
                    branches.push(branch(&target, ForestBranchState::Unsupported, "branch.no_eligible_route", "no simple source-to-target route satisfies terminal selectors, execution policy, and the cumulative loss/hop budget"));
                }
                continue;
            }
            Err(error) => {
                branches.push(branch(&target, ForestBranchState::Unsupported, "branch.route_search_failed", &error.to_string()));
                continue;
            }
        };
        if (target.preset.is_some() || target.template.is_some()) && !selected.path.steps.last().is_some_and(|edge|
            edge.evidence.get("adapter").map(String::as_str) == Some(BUILTIN_ADAPTER_EVIDENCE)) {
            branches.push(branch(&target, ForestBranchState::Unsupported, "branch.render_settings_unsupported", "the selected adapter does not interpret target preset/template settings"));
            continue;
        }
        let over_budget = if let Some(limit) = spec.execution.budgets.max_artifacts {
            let mut tentative = routes.clone();
            tentative.push((target.clone(), selected.clone()));
            let groups = group_forest_routes(source, &tentative)?;
            let estimate = 1_u64 + groups.iter().map(|(dag, _)| dag.node_count().saturating_sub(1) as u64).sum::<u64>();
            estimate > limit
        } else { false };
        if over_budget {
            let mut evidence = branch(&target, ForestBranchState::BudgetPruned, "branch.max_artifacts", "pruned by execution.budgets.max_artifacts in stable target identity order");
            evidence.route = Some(route_evidence(&selected));
            branches.push(evidence);
            continue;
        }
        let mut evidence = branch(&target, ForestBranchState::Selected, "branch.selected", "selected one bounded simple path from the original source; route evidence records the exact ranking policy");
        evidence.route = Some(route_evidence(&selected));
        branches.push(evidence);
        routes.push((target, selected));
    }
    let mut exclusions = spec.targets.exclude.clone();
    for name in &spec.targets.profiles {
        if let Some(profile) = spec.profiles.get(name) { merge_selector_set(&mut exclusions, &profile.exclude); }
    }
    for format in graph.reachable_from(source) {
        let target = ResolvedTarget::generated(format);
        if selector_matches(&target, &exclusions, graph) && !branches.iter().any(|branch| branch.target_id == target.identity()) {
            branches.push(branch(&target, ForestBranchState::Excluded, "branch.selector_excluded", "matched an explicit profile/spec/CLI exclusion selector"));
        }
    }
    if !branches.iter().any(|branch| branch.format == source.to_string()) {
        let target = ResolvedTarget::generated(source);
        branches.push(branch(&target, ForestBranchState::Excluded, "branch.source_identity_excluded", "exhaustive expansion excludes the source format; request it explicitly to export the original artifact without a conversion cycle"));
    }
    branches.sort_by(|a, b| a.format.cmp(&b.format).then_with(|| a.target_id.cmp(&b.target_id)));
    Ok(PlannedForest {
        routes,
        evidence: ArtifactForest {
            schema: "renderflow/artifact-forest-v1".to_owned(), identity: String::new(),
            policy: serde_json::json!({
                "planning": configured, "effective_max_hops": policy.max_hops,
                "ranking": if exhaustive(spec) { "cumulative_fidelity_then_hops_then_optimization_then_identity_v1" } else { "optimization_then_cumulative_fidelity_then_hops_then_identity_v1" },
                "source_identity": "excluded_unless_explicit; explicit_identity_export_has_zero_steps",
                "deduplication": "canonical_format_and_publication_identity_and_render_configuration",
                "shared_prefixes": "identical_selected_edges_and_render_settings_only",
            }),
            profiles: spec.targets.profiles.iter().map(|name| format!("{}@{}", name,
                spec.profiles.get(name).map(|profile| profile.schema.as_str()).unwrap_or("unknown"))).collect(),
            intermediates: match spec.targets.intermediates { IntermediatePolicy::CacheOnly => "cache_only", IntermediatePolicy::Retain => "retain" }.to_owned(),
            branches, produced_artifacts: Vec::new(), execution: None,
        },
    })
}

fn same_render_settings(a: &ResolvedTarget, b: &ResolvedTarget) -> bool {
    a.preset == b.preset && a.template == b.template && a.options == b.options
        && a.variant == b.variant
        // A configured terminal cannot become another rendition's intermediate:
        // that intermediate normally uses its adapter's unconfigured defaults.
        && (a.format == b.format || (a.preset.is_none() && a.template.is_none()))
}

pub(super) fn group_forest_routes(
    source: Format, routes: &[(ResolvedTarget, SelectedDerivativePath)],
) -> Result<Vec<(MultiTargetDag, Vec<ResolvedTarget>)>> {
    let mut groups: Vec<(Vec<TransformPath>, Vec<ResolvedTarget>)> = Vec::new();
    for (target, selected) in routes {
        let group = groups.iter_mut().find(|(paths, targets)| {
            if !targets.iter().all(|other| same_render_settings(other, target)) { return false; }
            let mut combined = paths.clone();
            combined.push(selected.path.clone());
            MultiTargetDag::from_selected_paths(source, &combined).is_ok()
        });
        if let Some((paths, targets)) = group {
            paths.push(selected.path.clone()); targets.push(target.clone());
        } else {
            groups.push((vec![selected.path.clone()], vec![target.clone()]));
        }
    }
    if groups.is_empty() { groups.push((Vec::new(), Vec::new())); }
    groups.into_iter().map(|(paths, targets)| Ok((MultiTargetDag::from_selected_paths(source, &paths)?, targets))).collect()
}

pub(super) fn finalize_forest_identity(plan: &mut ExecutionPlan, spec: &SpecV2) -> Result<()> {
    if let Some(forest) = plan.artifact_forest.as_mut() { forest.identity.clear(); }
    let digest = sha256_serialized(&serde_json::json!({ "schema": "renderflow/forest-identity-v1", "spec": spec, "plan": &*plan }))?;
    if let Some(forest) = plan.artifact_forest.as_mut() { forest.identity = format!("sha256:{}", digest.value); }
    Ok(())
}

/// Join planned target membership to observed manifest records. Publication is
/// established by the successful materialization locator and output list; a
/// selected route, cached candidate, or successful transform alone is insufficient.
/// NotPublished makes no claim about why publication did not occur: run state,
/// validation evidence and diagnostics retain their independent meanings.
pub(super) fn attach_forest_execution(
    forest: &mut ArtifactForest,
    state: RunState,
    output_root: &str,
    outputs: &[String],
    artifacts: &[ArtifactEvidence],
    steps: &[StepEvidence],
) {
    let published_locators = outputs.iter().map(|output|
        bundle_locator(Path::new(output_root), Path::new(output))).collect::<HashSet<_>>();
    let mut produced = BTreeSet::new();
    let targets = forest.branches.iter().map(|branch| {
        let selected = branch.state == ForestBranchState::Selected;
        let planned_source_identity = branch.route.as_ref()
            .is_some_and(|route| route.source_identity && route.steps.is_empty());
        let observed = (selected && state != RunState::Planned).then(|| artifacts.iter()
            .enumerate().find(|(_, artifact)| artifact.lifecycle == ArtifactRole::Terminal
                && artifact.metadata.get("renderflow.target.identity")
                    .and_then(serde_json::Value::as_str) == Some(branch.target_id.as_str())))
            .flatten();
        // A zero-edge target can still become a derivative through publication
        // hygiene. Observed record identity and digest, not route intent, decide
        // whether a candidate remains the original source. Without a candidate,
        // this flag describes planning intent only and implies no actual export.
        let source_identity = observed.map(|(_, artifact)| artifacts.iter().any(|source|
            source.lifecycle == ArtifactRole::Source && source.artifact_id == artifact.artifact_id
                && source.digest == artifact.digest)).unwrap_or(planned_source_identity);
        let published = observed.is_some_and(|(_, artifact)| artifact.metadata
            .get("renderflow.target.published").and_then(serde_json::Value::as_bool) == Some(true)
            && artifact.locator.starts_with("bundle:")
            && published_locators.contains(&artifact.locator));
        let outcome = if !selected { ForestTargetOutcome::NotSelected }
            else if state == RunState::Planned { ForestTargetOutcome::Planned }
            else if published { ForestTargetOutcome::Published }
            else { ForestTargetOutcome::NotPublished };
        if published && !source_identity {
            if let Some((_, artifact)) = observed { produced.insert(artifact.artifact_id.clone()); }
        }
        let producing_step_id = observed.and_then(|(_, artifact)| artifact.metadata
            .get("renderflow.target.producing_step_id").and_then(serde_json::Value::as_str)
            .filter(|id| steps.iter().any(|step| step.step_id == *id
                && step.output_artifacts.contains(&artifact.artifact_id))).map(str::to_owned));
        ForestTargetExecution {
            target_id: branch.target_id.clone(), outcome, source_identity,
            artifact_evidence_index: observed.map(|(index, _)| index),
            artifact_id: observed.map(|(_, artifact)| artifact.artifact_id.clone()),
            locator: observed.filter(|_| published).map(|(_, artifact)| artifact.locator.clone()),
            producing_step_id,
        }
    }).collect();
    forest.produced_artifacts = produced.into_iter().collect();
    forest.execution = Some(ForestExecutionEvidence {
        schema: FOREST_EXECUTION_SCHEMA_V1.to_owned(), targets,
    });
}

/// Hygiene observes record metadata and media type as well as payload bytes.
/// ArtifactId alone does not commit to those fields, so reuse is scoped to the
/// complete candidate record within this run's single effective hygiene policy.
pub(super) fn hygiene_candidate_key(candidate: &Artifact) -> Result<String> {
    Ok(sha256_serialized(candidate)?.value)
}

/// Metadata-sensitive hygiene decisions belong to a target, even when different
/// decisions produce the same payload/format/lineage artifact ID.
pub(super) fn record_target_hygiene(
    identity: &str,
    outcome: &HygieneOutcome,
    evidence: &mut HashMap<String, HygieneEvidence>,
    blocked: &mut HashSet<String>,
) -> bool {
    evidence.insert(identity.to_owned(), outcome.evidence.clone());
    let failed = !outcome.releasable();
    if failed { blocked.insert(identity.to_owned()); }
    failed
}
