//! Frozen composition planning and an identity-preserving artifact-forest view.
//!
//! The common graph is a format-level projection, not a `MultiTargetDag` or an
//! instruction to execute ordinary format conversion. Locale identities and
//! real shared preparation dependencies live in the forest and stage records.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};

use crate::composition::{
    self as native, CompositionBranchPlan, CompositionBranchState, CompositionPlan,
    CompositionRequest, CompositionRun, LoadedComposition,
};
use crate::graph::execution_plan::{
    EdgeType, ExecutionMetadata, ExecutionWave, NodeType, PlanDiagnostic, PlanEdge, PlanNode,
};
use crate::graph::{
    ArtifactForest, DiagnosticLevel, ExecutionPlan, ForestBranch, ForestBranchState,
    ForestRouteEvidence,
};
use crate::process::ProcessCancellationToken;

pub const COMPOSITION_EXECUTION_PLAN_SCHEMA_V1: &str = "renderflow.composition-execution-plan/v1";
const SNAPSHOT_FORMAT: &str = "composition_snapshot";
const TRANSCRIPT_FORMAT: &str = "composition_transcript";
const PROVIDER: &str = "renderflow.core-composition";

/// Capture one canonical package and select its locale/output branches.
#[derive(Debug, Clone)]
pub struct CompositionPlanningRequest {
    pub input: PathBuf,
    pub request: CompositionRequest,
}

impl CompositionPlanningRequest {
    pub fn from_path(input: impl AsRef<Path>) -> Self {
        Self { input: input.as_ref().to_path_buf(), request: CompositionRequest::default() }
    }
}

/// A bounded, lazy preparation stage shared within one locale. Consumers are
/// planned terminal target IDs, not proof that execution reached the stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionStagePlan {
    pub stage_id: String,
    pub locale: String,
    pub kind: String,
    pub consumers: Vec<String>,
    pub dependencies: Vec<String>,
    /// At most one successful preparation is needed in this run. Terminal
    /// cache hits, cancellation, or earlier failure can make the actual count 0.
    pub expected_materializations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionExecutionPlan {
    pub schema: String,
    pub composition: CompositionPlan,
    pub execution: ExecutionPlan,
    pub stages: Vec<CompositionStagePlan>,
}

/// Only this private captured snapshot can execute the frozen plan. Serialized
/// plan evidence does not contain the measured glyph paths needed to draw.
pub struct ResolvedCompositionExecution {
    snapshot: LoadedComposition,
    native_plan: CompositionPlan,
    plan: CompositionExecutionPlan,
}

impl ResolvedCompositionExecution {
    pub fn plan(&self) -> &CompositionExecutionPlan { &self.plan }
    pub fn native_plan(&self) -> &CompositionPlan { &self.native_plan }
}

/// Resolve once from confined captured bytes. No output/cache publication,
/// external provider discovery, or general-format route selection occurs here.
pub fn resolve_composition(request: CompositionPlanningRequest) -> Result<ResolvedCompositionExecution> {
    let snapshot = LoadedComposition::load(&request.input)?;
    let native_plan = native::plan(&snapshot, &request.request)?;
    let plan = build_execution_plan(&native_plan)?;
    Ok(ResolvedCompositionExecution { snapshot, native_plan, plan })
}

/// Execute exactly the resolved in-process plan, retaining its private glyph
/// outlines. Changes to the source files after resolution cannot alter it.
pub fn execute_composition(
    resolved: &ResolvedCompositionExecution,
    output: &Path,
    cache_directory: Option<&Path>,
    cancellation: &ProcessCancellationToken,
) -> Result<CompositionRun> {
    native::execute_planned(&resolved.snapshot, resolved.native_plan.clone(), output, cache_directory, cancellation)
}

/// Project native intent without reloading inputs, measuring fonts, or rendering.
/// The scoped adapter interprets these options; ordinary v2 target.options and
/// the format-keyed general DAG retain their existing contracts.
pub(crate) fn build_execution_plan(plan: &CompositionPlan) -> Result<CompositionExecutionPlan> {
    let stages = build_stages(plan)?;
    let mut branches = Vec::new();
    let mut unique_edges = BTreeMap::<String, PlanEdge>::new();
    let mut wave_edges = BTreeMap::<usize, BTreeMap<String, PlanEdge>>::new();
    let mut node_types = BTreeMap::from([(SNAPSHOT_FORMAT.to_string(), NodeType::Source)]);
    let mut targets = BTreeSet::new();
    let mut maximum_depth = 0;
    for branch in &plan.branches {
        let selected = branch.state == CompositionBranchState::Planned;
        let route = if selected {
            let steps = branch_steps(plan, branch, &stages)?;
            maximum_depth = maximum_depth.max(steps.len());
            for (index, edge) in steps.iter().enumerate() {
                let key = native::hash_json(edge)?;
                unique_edges.insert(key.clone(), edge.clone());
                wave_edges.entry(index + 1).or_default().insert(key, edge.clone());
                node_types.entry(edge.to.clone()).or_insert(NodeType::Output);
                if edge.from != SNAPSHOT_FORMAT {
                    node_types.insert(edge.from.clone(), NodeType::Intermediate);
                }
            }
            targets.insert(branch.output.name().to_string());
            Some(ForestRouteEvidence {
                hop_count: steps.len(), weighted_cost: steps.len() as f64, steps,
                source_identity: false, cumulative_quality: 0.0, cumulative_loss: 1.0,
                ranking: "fixed composition adapter dependencies; unit structural cost and unqualified quality floor, not searched or measured fidelity".into(),
                explored_candidates: 0,
            })
        } else { None };
        let (state, reason_code, reason) = match branch.state {
            CompositionBranchState::Planned => (ForestBranchState::Selected,
                "composition.branch.selected", "Selected the frozen composition adapter branch; shared preparation is lazy and publication remains unobserved"),
            CompositionBranchState::Blocked => (ForestBranchState::Unsupported,
                "composition.branch.blocked", "Native composition diagnostics block this branch; no preparation stage or executable route is selected"),
            CompositionBranchState::Unavailable => (ForestBranchState::Unavailable,
                "composition.branch.unavailable", "No adopted composition provider implements this output"),
        };
        let options = BTreeMap::from([
            ("composition_id".into(), serde_json::json!(plan.composition_id)),
            ("locale".into(), serde_json::json!(branch.locale)),
            ("content_revision".into(), serde_json::json!(branch.content_revision)),
            ("content_sha256".into(), serde_json::json!(branch.content_sha256)),
            ("cache_key".into(), serde_json::json!(branch.cache_key)),
            ("raster".into(), serde_json::json!(branch.raster)),
            ("pdf".into(), serde_json::json!(branch.pdf)),
            ("diagnostics".into(), serde_json::json!(branch.diagnostics)),
        ]);
        branches.push(ForestBranch {
            target_id: branch.target_id.clone(), format: branch.output.name().into(),
            preset: None, template: None, variant: None, role: Some("composition_derivative".into()),
            requirement: "required".into(), options, state,
            reason_code: reason_code.into(), reason: reason.into(), route,
        });
    }
    let identity = format!("sha256:{}", native::hash_json(&serde_json::json!({
        "schema": "renderflow.composition-forest-identity/v1", "composition": plan, "stages": stages,
    }))?);
    let forest = ArtifactForest {
        identity, profiles: vec![plan.profile.clone()], intermediates: "cache_only".into(), branches,
        policy: serde_json::json!({
            "adapter": "frozen_composition", "graph_view": "format_projection_only",
            "identity_authority": "branch.target_id and stage.stage_id", "runtime_qualified": false,
            "quality_policy": "0 is an unqualified conservative placeholder, not measured fidelity",
            "cost_policy": "1 per structural dependency step, not measured runtime or route optimization",
            "sharing": "lazy transcript and SVG preparation; at most one locale retained; cache hits do not imply preparation",
            "intermediate_storage": "ephemeral_per_locale; no retained or persistent-cache intermediate artifacts",
            "native_plan_schema": plan.schema, "review_required": true,
        }),
        ..ArtifactForest::default()
    };
    let edges = unique_edges.into_values().collect::<Vec<_>>();
    let waves = wave_edges.into_iter().map(|(index, edges)| ExecutionWave {
        index, edges: edges.into_values().collect(),
    }).collect::<Vec<_>>();
    let nodes = node_types.into_iter().map(|(format, node_type)| PlanNode { format, node_type }).collect::<Vec<_>>();
    let metadata = ExecutionMetadata {
        total_nodes: nodes.len(), total_edges: edges.len(), execution_depth: maximum_depth,
        execution_waves: waves.len(), estimated_cost: edges.len() as f32, estimated_quality: 0.0,
        reused_intermediates: stages.iter().filter(|stage| stage.consumers.len() > 1).count(),
        output_count: plan.branches.iter().filter(|branch| branch.state == CompositionBranchState::Planned).count(),
    };
    let execution = ExecutionPlan {
        source: SNAPSHOT_FORMAT.into(), targets: targets.into_iter().collect(),
        optimization: "fixed_composition_dependencies".into(), nodes, edges, waves, metadata,
        diagnostics: vec![PlanDiagnostic { level: DiagnosticLevel::Warning,
            message: "Format nodes are a compatibility projection; the forest and stage IDs preserve locales. Waves express dependencies, not observed concurrency. Structural unit costs and zero quality are unqualified placeholders. This plan uses the frozen composition executor, not the general format DAG; it does not establish a Flow adapter or publication approval.".into() }],
        source_artifact: None, source_collection: None, toolchain: None, artifact_forest: Some(forest),
    };
    Ok(CompositionExecutionPlan { schema: COMPOSITION_EXECUTION_PLAN_SCHEMA_V1.into(),
        composition: plan.clone(), execution, stages })
}

fn build_stages(plan: &CompositionPlan) -> Result<Vec<CompositionStagePlan>> {
    let mut seen_targets = BTreeSet::new();
    let mut locales = BTreeMap::<&str, Vec<&CompositionBranchPlan>>::new();
    for branch in &plan.branches {
        ensure!(seen_targets.insert(&branch.target_id), "composition.plan.duplicate_target: target IDs must be unique");
        if branch.state == CompositionBranchState::Planned {
            locales.entry(&branch.locale).or_default().push(branch);
        }
    }
    let mut stages = Vec::new();
    for (locale, branches) in locales {
        let first = branches.first().context("composition.plan.empty_locale")?;
        let transcript_id = format!("stage-{}", native::hash_json(&serde_json::json!({
            "schema": "renderflow.composition-stage/v1", "kind": "transcript", "profile": plan.profile,
            "renderflow_version": plan.renderflow_version, "composition_sha256": plan.composition_sha256,
            "inputs": plan.inputs, "locale": locale, "content_sha256": first.content_sha256,
        }))?);
        let mut consumers = branches.iter().map(|branch| branch.target_id.clone()).collect::<Vec<_>>();
        consumers.sort();
        stages.push(CompositionStagePlan { stage_id: transcript_id.clone(), locale: locale.into(),
            kind: "transcript".into(), consumers, dependencies: Vec::new(), expected_materializations: 1 });
        let visual = branches.iter().filter(|branch| branch.output.visual()).collect::<Vec<_>>();
        if let Some(first_visual) = visual.first() {
            let typography = first_visual.typography.as_ref().context("composition.plan.typography_missing: selected visual branch requires measured typography")?;
            let report_sha256 = native::hash_json(typography)?;
            for branch in &visual {
                ensure!(branch.typography.as_ref().map(native::hash_json).transpose()?.as_deref() == Some(report_sha256.as_str()),
                    "composition.plan.typography_mismatch: one locale must share the same measured report");
            }
            let svg_id = format!("stage-{}", native::hash_json(&serde_json::json!({
                "schema": "renderflow.composition-stage/v1", "kind": "svg", "transcript_stage": transcript_id,
                "typography_sha256": report_sha256,
            }))?);
            let mut consumers = visual.iter().map(|branch| branch.target_id.clone()).collect::<Vec<_>>();
            consumers.sort();
            stages.push(CompositionStagePlan { stage_id: svg_id, locale: locale.into(), kind: "svg".into(),
                consumers, dependencies: vec![transcript_id], expected_materializations: 1 });
        }
    }
    Ok(stages)
}

fn branch_steps(plan: &CompositionPlan, branch: &CompositionBranchPlan, stages: &[CompositionStagePlan]) -> Result<Vec<PlanEdge>> {
    let stage = |kind: &str| stages.iter().find(|stage| stage.locale == branch.locale && stage.kind == kind)
        .with_context(|| format!("composition.plan.stage_missing: {} {kind}", branch.locale));
    let transcript = stage("transcript")?;
    let mut steps = vec![stage_edge(plan, transcript, SNAPSHOT_FORMAT, TRANSCRIPT_FORMAT)];
    let from = if branch.output.visual() {
        steps.push(stage_edge(plan, stage("svg")?, TRANSCRIPT_FORMAT, "svg"));
        "svg"
    } else { TRANSCRIPT_FORMAT };
    if branch.output != native::CompositionOutput::Svg {
        let mut edge = edge(plan, from, branch.output.name(), &format!("composition.{}", branch.output.name()));
        edge.evidence.insert("target_id".into(), branch.target_id.clone());
        edge.evidence.insert("locale".into(), branch.locale.clone());
        edge.evidence.insert("cache_key".into(), branch.cache_key.clone());
        steps.push(edge);
    }
    Ok(steps)
}

fn stage_edge(plan: &CompositionPlan, stage: &CompositionStagePlan, from: &str, to: &str) -> PlanEdge {
    let mut edge = edge(plan, from, to, &format!("composition.{}", stage.kind));
    edge.evidence.insert("stage_id".into(), stage.stage_id.clone());
    edge.evidence.insert("locale".into(), stage.locale.clone());
    edge.evidence.insert("materialization_kind".into(), stage.kind.clone());
    edge
}

fn edge(plan: &CompositionPlan, from: &str, to: &str, capability: &str) -> PlanEdge {
    PlanEdge { from: from.into(), to: to.into(), cost: 1.0, quality: 0.0,
        input_kind: if from == SNAPSHOT_FORMAT { "collection" } else { "single" }.into(),
        edge_type: if from == SNAPSHOT_FORMAT { EdgeType::Aggregation } else { EdgeType::Lossy },
        provider_id: Some(PROVIDER.into()), capability_id: Some(capability.into()),
        required_provider_ids: Vec::new(), variant_id: None,
        evidence: BTreeMap::from([
            ("transform_id".into(), plan.profile.clone()),
            ("qualification".into(), "unqualified_structural_dependency".into()),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{CompositionOutput, LocaleSelector};

    fn fixture(targets: Vec<CompositionOutput>) -> CompositionPlan {
        fixture_request(CompositionRequest { targets, ..CompositionRequest::default() })
    }

    fn fixture_request(mut request: CompositionRequest) -> CompositionPlan {
        let input = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/page-composition/measured-typography/page.composition.json");
        let loaded = LoadedComposition::load(input).unwrap();
        request.locales = LocaleSelector::Subset { locales: vec!["en".into(), "de".into()] };
        native::plan(&loaded, &request).unwrap()
    }

    #[test]
    fn same_format_locales_keep_distinct_targets_and_stages() {
        let native = fixture(vec![CompositionOutput::TranscriptJson]);
        let projected = build_execution_plan(&native).unwrap();
        let forest = projected.execution.artifact_forest.as_ref().unwrap();
        assert_eq!(forest.branches.len(), 2);
        assert_ne!(forest.branches[0].target_id, forest.branches[1].target_id);
        assert_ne!(forest.branches[0].options["locale"], forest.branches[1].options["locale"]);
        assert_eq!(projected.execution.targets, vec!["transcript_json"]);
        assert_eq!(projected.execution.metadata.output_count, 2);
        assert_eq!(projected.stages.len(), 2);
        assert_ne!(projected.stages[0].stage_id, projected.stages[1].stage_id);
        assert!(forest.execution.is_none() && forest.produced_artifacts.is_empty());
        let again = build_execution_plan(&native).unwrap();
        assert_eq!(forest.identity, again.execution.artifact_forest.unwrap().identity);
    }

    #[test]
    fn shared_preparation_identity_ignores_selected_output_and_pixel_settings() {
        let svg = build_execution_plan(&fixture(vec![CompositionOutput::Svg])).unwrap();
        let mut request = CompositionRequest {
            targets: vec![CompositionOutput::Svg, CompositionOutput::Html, CompositionOutput::Png],
            ..CompositionRequest::default()
        };
        request.raster.dpi = 144;
        let multi_native = fixture_request(request);
        let multi = build_execution_plan(&multi_native).unwrap();
        assert_eq!(svg.stages.iter().map(|stage| &stage.stage_id).collect::<Vec<_>>(),
            multi.stages.iter().map(|stage| &stage.stage_id).collect::<Vec<_>>());
        assert!(multi.stages.iter().all(|stage| stage.consumers.len() == 3 && stage.expected_materializations == 1));
        assert!(multi.stages.iter().filter(|stage| stage.kind == "svg").all(|stage| stage.dependencies.len() == 1));
    }

    #[test]
    fn unavailable_branches_have_no_stage_or_execution_route() {
        let native = fixture(vec![CompositionOutput::Ora]);
        let projected = build_execution_plan(&native).unwrap();
        assert!(projected.stages.is_empty());
        assert!(projected.execution.edges.is_empty());
        assert!(projected.execution.artifact_forest.unwrap().branches.iter().all(|branch|
            branch.state == ForestBranchState::Unavailable && branch.route.is_none()));
    }
}
