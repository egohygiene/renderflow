use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::{Format, InputKind, MultiTargetDag, TransformEdge};
use crate::optimization::OptimizationMode;

// ── Node types ─────────────────────────────────────────────────────────────

/// Visual classification of a node in the execution plan graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// The initial input format (root of the DAG).
    Source,
    /// An intermediate format produced and consumed within the pipeline.
    Intermediate,
    /// A final output format (leaf of the DAG).
    Output,
}

// ── Edge types ─────────────────────────────────────────────────────────────

/// Visual / semantic classification of a transformation edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// A lossless, fully deterministic transformation.
    Lossless,
    /// A transformation that may discard information (quality < 1.0).
    Lossy,
    /// An aggregation transform that merges multiple inputs into one output.
    Aggregation,
}

// ── PlanNode ────────────────────────────────────────────────────────────────

/// A node in the canonical execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    /// The document format this node represents.
    pub format: String,
    /// How this node is classified within the execution plan.
    pub node_type: NodeType,
}

// ── PlanEdge ────────────────────────────────────────────────────────────────

/// An edge in the canonical execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEdge {
    /// Source format for this transformation.
    pub from: String,
    /// Target format produced by this transformation.
    pub to: String,
    /// Relative execution cost (lower is cheaper).
    pub cost: f32,
    /// Expected output quality in the range `[0.0, 1.0]`.
    pub quality: f32,
    /// Whether the transform consumes a single document or a collection.
    pub input_kind: String,
    /// Semantic classification of this edge.
    pub edge_type: EdgeType,
}

impl PlanEdge {
    pub(crate) fn from_transform_edge(e: &TransformEdge) -> Self {
        let edge_type = if e.input_kind.is_collection() {
            EdgeType::Aggregation
        } else if (e.quality - 1.0).abs() < f32::EPSILON {
            EdgeType::Lossless
        } else {
            EdgeType::Lossy
        };

        PlanEdge {
            from: e.from.to_string(),
            to: e.to.to_string(),
            cost: e.cost,
            quality: e.quality,
            input_kind: match e.input_kind {
                InputKind::Single => "single".to_string(),
                InputKind::Collection => "collection".to_string(),
            },
            edge_type,
        }
    }
}

// ── ExecutionWave ───────────────────────────────────────────────────────────

/// A group of independent transformations that can execute in parallel.
///
/// All edges within a wave have their source format available by the time the
/// wave begins — either as the initial source or as the output of a prior wave.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionWave {
    /// 1-based wave index.
    pub index: usize,
    /// The edges that execute during this wave.
    pub edges: Vec<PlanEdge>,
}

// ── ExecutionMetadata ───────────────────────────────────────────────────────

/// Summary statistics for an execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    /// Total number of format nodes in the plan.
    pub total_nodes: usize,
    /// Total number of transformation edges in the plan.
    pub total_edges: usize,
    /// Maximum chain depth from source to any output (longest path in hops).
    pub execution_depth: usize,
    /// Number of parallel execution waves.
    pub execution_waves: usize,
    /// Sum of all edge costs in execution order.
    pub estimated_cost: f32,
    /// Minimum edge quality across the critical (longest) path.
    pub estimated_quality: f32,
    /// Number of unique intermediate formats produced and reused.
    pub reused_intermediates: usize,
    /// Number of output (leaf) formats.
    pub output_count: usize,
}

// ── DiagnosticLevel ─────────────────────────────────────────────────────────

/// Severity of a planning diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

// ── PlanDiagnostic ──────────────────────────────────────────────────────────

/// A human-readable explanation of a planner decision or observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDiagnostic {
    /// Severity of this diagnostic.
    pub level: DiagnosticLevel,
    /// Human-readable message describing the observation.
    pub message: String,
}

impl PlanDiagnostic {
    fn info(message: impl Into<String>) -> Self {
        PlanDiagnostic {
            level: DiagnosticLevel::Info,
            message: message.into(),
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        PlanDiagnostic {
            level: DiagnosticLevel::Warning,
            message: message.into(),
        }
    }
}

// ── ExecutionPlan ───────────────────────────────────────────────────────────

/// The canonical description of a planned Renderflow execution.
///
/// `ExecutionPlan` is the source-of-truth that all graph renderers consume.
/// It is intentionally free of presentation concerns so that new renderers can
/// be added without touching planner logic.
///
/// # Example
///
/// ```rust
/// use renderflow::graph::{Format, TransformEdge, TransformGraph};
/// use renderflow::graph::ExecutionPlan;
/// use renderflow::optimization::OptimizationMode;
///
/// let mut graph = TransformGraph::new();
/// graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
/// graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
///
/// let dag = graph
///     .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
///     .unwrap();
///
/// let plan = ExecutionPlan::from_dag(&dag, Format::Markdown, &[Format::Pdf], OptimizationMode::Balanced);
/// assert_eq!(plan.metadata.total_edges, 2);
/// assert_eq!(plan.waves.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// The input format that starts the pipeline.
    pub source: String,
    /// The requested output formats.
    pub targets: Vec<String>,
    /// Optimization strategy used to select paths.
    pub optimization: String,
    /// All format nodes in the plan.
    pub nodes: Vec<PlanNode>,
    /// All transformation edges in the plan.
    pub edges: Vec<PlanEdge>,
    /// Grouped execution waves (parallel batches).
    pub waves: Vec<ExecutionWave>,
    /// Summary statistics.
    pub metadata: ExecutionMetadata,
    /// Planner observations and explanations.
    pub diagnostics: Vec<PlanDiagnostic>,
}

impl ExecutionPlan {
    /// Build a canonical `ExecutionPlan` from a [`MultiTargetDag`].
    ///
    /// The plan is derived entirely from the DAG structure; no external
    /// information is required by renderers.
    pub fn from_dag(
        dag: &MultiTargetDag,
        source: Format,
        targets: &[Format],
        optimization: OptimizationMode,
    ) -> Self {
        // ── collect nodes ──────────────────────────────────────────────────
        let target_set: HashSet<Format> = targets.iter().copied().collect();

        // Identify intermediate formats: produced but not the original source
        // and not a final target.  Count their usage as producer and consumer.
        let produced: HashSet<Format> = dag.all_edges().iter().map(|e| e.to).collect();
        let consumed: HashSet<Format> = dag.all_edges().iter().map(|e| e.from).collect();
        let intermediates: HashSet<Format> = produced
            .intersection(&consumed)
            .copied()
            .filter(|&f| f != source)
            .collect();

        let mut node_labels: Vec<String> =
            dag.graph.node_weights().map(|f| f.to_string()).collect();
        node_labels.sort();

        let nodes: Vec<PlanNode> = node_labels
            .iter()
            .map(|label| {
                let fmt: Format = label.parse().expect("node label must be a valid Format");
                let node_type = if fmt == source {
                    NodeType::Source
                } else if target_set.contains(&fmt) && !intermediates.contains(&fmt) {
                    NodeType::Output
                } else if intermediates.contains(&fmt) {
                    NodeType::Intermediate
                } else {
                    // Reachable leaf that was not explicitly requested.
                    NodeType::Output
                };
                PlanNode {
                    format: label.clone(),
                    node_type,
                }
            })
            .collect();

        // ── collect edges ──────────────────────────────────────────────────
        let order = dag.execution_order();
        let edges: Vec<PlanEdge> = order.iter().map(|e| PlanEdge::from_transform_edge(e)).collect();

        // ── compute waves ──────────────────────────────────────────────────
        let waves = Self::compute_waves(dag, source);

        // ── metadata ───────────────────────────────────────────────────────
        let total_cost: f32 = edges.iter().map(|e| e.cost).sum();
        let min_quality: f32 = edges
            .iter()
            .map(|e| e.quality)
            .fold(f32::INFINITY, f32::min);
        let estimated_quality = if min_quality.is_finite() {
            min_quality
        } else {
            1.0
        };

        let reused_intermediates = intermediates.len();
        let output_count = target_set.len();

        // Compute maximum depth: longest path from source to any target
        // (measured in number of edges).
        let depth = Self::compute_depth(dag, source);

        let metadata = ExecutionMetadata {
            total_nodes: nodes.len(),
            total_edges: edges.len(),
            execution_depth: depth,
            execution_waves: waves.len(),
            estimated_cost: total_cost,
            estimated_quality,
            reused_intermediates,
            output_count,
        };

        // ── diagnostics ────────────────────────────────────────────────────
        let diagnostics = Self::build_diagnostics(&edges, &intermediates, &waves, &optimization);

        ExecutionPlan {
            source: source.to_string(),
            targets: {
                let mut t: Vec<String> =
                    targets.iter().map(|f| f.to_string()).collect();
                t.sort();
                t
            },
            optimization: optimization.to_string(),
            nodes,
            edges,
            waves,
            metadata,
            diagnostics,
        }
    }

    // ── private helpers ────────────────────────────────────────────────────

    /// Group edges into parallel execution waves.
    ///
    /// A wave contains all edges whose source format has been made available
    /// either by the initial source or by a prior wave.
    fn compute_waves(dag: &MultiTargetDag, source: Format) -> Vec<ExecutionWave> {
        use std::collections::HashSet;

        let all_edges: Vec<&TransformEdge> = dag.execution_order();
        let mut available: HashSet<Format> = HashSet::new();
        available.insert(source);

        let mut waves: Vec<ExecutionWave> = Vec::new();
        let mut remaining: Vec<&TransformEdge> = all_edges;

        let mut wave_index = 1usize;
        while !remaining.is_empty() {
            let (ready, not_ready): (Vec<_>, Vec<_>) =
                remaining.into_iter().partition(|e| available.contains(&e.from));

            if ready.is_empty() {
                // Cycle or missing edge — emit remaining as a single wave to avoid
                // infinite loop.
                let fallback: Vec<PlanEdge> = not_ready
                    .iter()
                    .map(|e| PlanEdge::from_transform_edge(e))
                    .collect();
                if !fallback.is_empty() {
                    waves.push(ExecutionWave {
                        index: wave_index,
                        edges: fallback,
                    });
                }
                break;
            }

            for e in &ready {
                available.insert(e.to);
            }

            waves.push(ExecutionWave {
                index: wave_index,
                edges: ready
                    .iter()
                    .map(|e| PlanEdge::from_transform_edge(e))
                    .collect(),
            });

            wave_index += 1;
            remaining = not_ready;
        }

        waves
    }

    /// Compute the longest path depth (in edges) from `source` to any node.
    fn compute_depth(dag: &MultiTargetDag, source: Format) -> usize {
        use std::collections::HashMap;

        let mut depth: HashMap<Format, usize> = HashMap::new();
        depth.insert(source, 0);

        for edge in dag.execution_order() {
            let from_depth = *depth.get(&edge.from).unwrap_or(&0);
            let entry = depth.entry(edge.to).or_insert(0);
            *entry = (*entry).max(from_depth + 1);
        }

        depth.values().copied().max().unwrap_or(0)
    }

    /// Generate diagnostics that explain planner decisions.
    fn build_diagnostics(
        edges: &[PlanEdge],
        intermediates: &HashSet<Format>,
        waves: &[ExecutionWave],
        optimization: &OptimizationMode,
    ) -> Vec<PlanDiagnostic> {
        let mut diags: Vec<PlanDiagnostic> = Vec::new();

        // Optimization mode explanation
        let opt_explanation = match optimization {
            OptimizationMode::Speed => {
                "Optimization mode 'speed': paths were selected to minimise execution cost."
            }
            OptimizationMode::Quality => {
                "Optimization mode 'quality': paths were selected to maximise output quality."
            }
            OptimizationMode::Balanced => {
                "Optimization mode 'balanced': paths were selected using equally weighted cost and quality."
            }
            OptimizationMode::Pareto => {
                "Optimization mode 'pareto': the Pareto-optimal frontier was used; the lowest-cost non-dominated path was selected."
            }
        };
        diags.push(PlanDiagnostic::info(opt_explanation));

        // Shared intermediates
        if !intermediates.is_empty() {
            let names: Vec<String> = {
                let mut v: Vec<String> =
                    intermediates.iter().map(|f| f.to_string()).collect();
                v.sort();
                v
            };
            diags.push(PlanDiagnostic::info(format!(
                "Shared intermediate format(s) reused across targets: {}. \
                 These are produced once and consumed by multiple downstream transforms.",
                names.join(", ")
            )));
        }

        // Parallel waves
        if waves.len() > 1 {
            diags.push(PlanDiagnostic::info(format!(
                "{} execution waves identified. Transforms within the same wave \
                 are independent and will run in parallel.",
                waves.len()
            )));
        }

        // Lossy edges
        let lossy: Vec<&PlanEdge> = edges
            .iter()
            .filter(|e| e.edge_type == EdgeType::Lossy)
            .collect();
        if !lossy.is_empty() {
            for e in &lossy {
                diags.push(PlanDiagnostic::warning(format!(
                    "Transform '{}' → '{}' is lossy (quality: {:.2}). \
                     Some information may be discarded during this conversion.",
                    e.from, e.to, e.quality
                )));
            }
        }

        // Collection (aggregation) edges
        let agg: Vec<&PlanEdge> = edges
            .iter()
            .filter(|e| e.edge_type == EdgeType::Aggregation)
            .collect();
        if !agg.is_empty() {
            for e in &agg {
                diags.push(PlanDiagnostic::info(format!(
                    "Transform '{}' → '{}' is a collection (aggregation) edge: \
                     multiple source documents are combined into one output.",
                    e.from, e.to
                )));
            }
        }

        diags
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{TransformGraph};

    fn build_plan() -> ExecutionPlan {
        let mut g = TransformGraph::new();
        g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
        g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
        g.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));

        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        ExecutionPlan::from_dag(
            &dag,
            Format::Markdown,
            &[Format::Pdf, Format::Docx],
            OptimizationMode::Balanced,
        )
    }

    #[test]
    fn test_plan_metadata_counts() {
        let plan = build_plan();
        assert_eq!(plan.metadata.total_nodes, 4);
        assert_eq!(plan.metadata.total_edges, 3);
    }

    #[test]
    fn test_plan_source_and_targets() {
        let plan = build_plan();
        assert_eq!(plan.source, "markdown");
        let mut targets = plan.targets.clone();
        targets.sort();
        assert!(targets.contains(&"pdf".to_string()));
        assert!(targets.contains(&"docx".to_string()));
    }

    #[test]
    fn test_plan_waves_computed() {
        let plan = build_plan();
        // Wave 1: markdown → html
        // Wave 2: html → pdf, html → docx (parallel)
        assert_eq!(plan.waves.len(), 2);
        assert_eq!(plan.waves[0].edges.len(), 1);
        assert_eq!(plan.waves[1].edges.len(), 2);
    }

    #[test]
    fn test_plan_node_types() {
        let plan = build_plan();
        let source_node = plan.nodes.iter().find(|n| n.format == "markdown").unwrap();
        assert_eq!(source_node.node_type, NodeType::Source);

        let intermediate_node = plan.nodes.iter().find(|n| n.format == "html").unwrap();
        assert_eq!(intermediate_node.node_type, NodeType::Intermediate);

        let output_node = plan.nodes.iter().find(|n| n.format == "pdf").unwrap();
        assert_eq!(output_node.node_type, NodeType::Output);
    }

    #[test]
    fn test_plan_edge_types() {
        let plan = build_plan();
        let md_html = plan
            .edges
            .iter()
            .find(|e| e.from == "markdown" && e.to == "html")
            .unwrap();
        assert_eq!(md_html.edge_type, EdgeType::Lossless);

        let html_pdf = plan
            .edges
            .iter()
            .find(|e| e.from == "html" && e.to == "pdf")
            .unwrap();
        assert_eq!(html_pdf.edge_type, EdgeType::Lossy);
    }

    #[test]
    fn test_plan_reused_intermediates() {
        let plan = build_plan();
        // Html is produced once and used by both pdf and docx.
        assert_eq!(plan.metadata.reused_intermediates, 1);
    }

    #[test]
    fn test_plan_optimization_diagnostic_present() {
        let plan = build_plan();
        let has_opt_diag = plan
            .diagnostics
            .iter()
            .any(|d| d.message.contains("balanced"));
        assert!(has_opt_diag);
    }

    #[test]
    fn test_plan_lossy_diagnostic_present() {
        let plan = build_plan();
        let has_lossy = plan
            .diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Warning && d.message.contains("lossy"));
        assert!(has_lossy);
    }

    #[test]
    fn test_plan_depth() {
        let plan = build_plan();
        // markdown(0) → html(1) → pdf(2)  ⟹ depth = 2
        assert_eq!(plan.metadata.execution_depth, 2);
    }

    #[test]
    fn test_plan_single_edge() {
        let mut g = TransformGraph::new();
        g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Html])
            .unwrap();
        let plan = ExecutionPlan::from_dag(
            &dag,
            Format::Markdown,
            &[Format::Html],
            OptimizationMode::Speed,
        );
        assert_eq!(plan.waves.len(), 1);
        assert_eq!(plan.metadata.execution_depth, 1);
    }

    #[test]
    fn test_plan_serialization_roundtrip() {
        let plan = build_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let deserialized: ExecutionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source, plan.source);
        assert_eq!(deserialized.metadata.total_edges, plan.metadata.total_edges);
    }
}
