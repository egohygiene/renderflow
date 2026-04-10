mod definition;
mod definition_registry;
mod format;
mod input_kind;
mod multi_target;
mod pathfinding;
mod transform_edge;

pub use definition::TransformDefinition;
pub use definition_registry::TransformDefinitionRegistry;
pub use format::Format;
pub use input_kind::InputKind;
pub use multi_target::MultiTargetDag;
pub use pathfinding::TransformPath;
pub use transform_edge::TransformEdge;

use crate::optimization::OptimizationMode;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;

/// A directed graph of document format transformations.
///
/// [`Format`] variants are represented as nodes and [`TransformEdge`] values
/// are stored as directed edge weights.  Multiple edges between the same pair
/// of nodes are allowed, enabling alternative transformation paths that differ
/// in cost or quality.
///
/// # Example
///
/// ```rust
/// use renderflow::graph::{Format, TransformEdge, TransformGraph};
///
/// let mut graph = TransformGraph::new();
/// graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 0.9));
/// graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
///
/// let routes = graph.transforms_from(Format::Markdown);
/// assert_eq!(routes.len(), 2);
/// ```
pub struct TransformGraph {
    graph: DiGraph<Format, TransformEdge>,
    nodes: HashMap<Format, NodeIndex>,
}

impl TransformGraph {
    /// Create an empty `TransformGraph`.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            nodes: HashMap::new(),
        }
    }

    /// Return the [`NodeIndex`] for `format`, inserting a node if one does not
    /// already exist.
    fn get_or_insert_node(&mut self, format: Format) -> NodeIndex {
        if let Some(&idx) = self.nodes.get(&format) {
            idx
        } else {
            let idx = self.graph.add_node(format);
            self.nodes.insert(format, idx);
            idx
        }
    }

    /// Add a transformation to the graph.
    ///
    /// If nodes for the source or target [`Format`] do not yet exist they are
    /// created automatically.  Duplicate edges (same source, target, cost, and
    /// quality) are allowed.
    pub fn add_transform(&mut self, edge: TransformEdge) {
        let from_idx = self.get_or_insert_node(edge.from);
        let to_idx = self.get_or_insert_node(edge.to);
        self.graph.add_edge(from_idx, to_idx, edge);
    }

    /// Add a collection-based transformation to the graph.
    ///
    /// Equivalent to calling [`add_transform`](Self::add_transform) with a
    /// [`TransformEdge`] whose [`input_kind`](TransformEdge::input_kind) is
    /// [`InputKind::Collection`].  Use this for aggregation-style transforms
    /// where multiple source documents are combined into a single output
    /// (e.g. pages → book).
    pub fn add_collection_transform(&mut self, from: Format, to: Format, cost: f32, quality: f32) {
        self.add_transform(TransformEdge::with_input_kind(from, to, cost, quality, InputKind::Collection));
    }

    /// Return all [`TransformEdge`]s whose source is `from`.
    ///
    /// Returns an empty `Vec` when no outgoing edges exist for the given format.
    pub fn transforms_from(&self, from: Format) -> Vec<&TransformEdge> {
        let Some(&node_idx) = self.nodes.get(&from) else {
            return Vec::new();
        };
        self.graph
            .edges(node_idx)
            .map(|e| e.weight())
            .collect()
    }

    /// Return all [`TransformEdge`]s that produce the target format `to`.
    ///
    /// Returns an empty `Vec` when no incoming edges exist for the given format.
    pub fn transforms_to(&self, to: Format) -> Vec<&TransformEdge> {
        let Some(&node_idx) = self.nodes.get(&to) else {
            return Vec::new();
        };
        use petgraph::Direction;
        self.graph
            .edges_directed(node_idx, Direction::Incoming)
            .map(|e| e.weight())
            .collect()
    }

    /// Return `true` when at least one direct transformation from `from` to
    /// `to` has been registered.
    pub fn has_transform(&self, from: Format, to: Format) -> bool {
        let (Some(&from_idx), Some(&to_idx)) =
            (self.nodes.get(&from), self.nodes.get(&to))
        else {
            return false;
        };
        self.graph.contains_edge(from_idx, to_idx)
    }

    /// Reconstruct the cheapest directed edge between each consecutive pair of
    /// nodes in `node_path` and return them as an ordered `Vec<TransformEdge>`.
    ///
    /// When multiple parallel edges connect the same pair of nodes the one with
    /// the lowest cost is chosen, which is consistent with the cost function
    /// used by the pathfinding algorithms.
    fn edges_from_node_path(&self, node_path: &[NodeIndex]) -> Vec<TransformEdge> {
        node_path
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                self.graph
                    .edges(a)
                    .filter(|e| e.target() == b)
                    .min_by(|x, y| {
                        x.weight()
                            .cost
                            .partial_cmp(&y.weight().cost)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .expect("node path contains a pair with no connecting edge")
                    .weight()
                    .clone()
            })
            .collect()
    }

    /// Like [`edges_from_node_path`], but picks the edge with the lowest
    /// `mode`-specific weight when multiple parallel edges connect the same
    /// pair of nodes.
    fn edges_from_node_path_with_mode(
        &self,
        node_path: &[NodeIndex],
        mode: OptimizationMode,
    ) -> Vec<TransformEdge> {
        node_path
            .windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                self.graph
                    .edges(a)
                    .filter(|e| e.target() == b)
                    .min_by(|x, y| {
                        let wx = mode.edge_weight(x.weight().cost, x.weight().quality);
                        let wy = mode.edge_weight(y.weight().cost, y.weight().quality);
                        wx.partial_cmp(&wy).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .expect("node path contains a pair with no connecting edge")
                    .weight()
                    .clone()
            })
            .collect()
    }

    /// Find the lowest-cost path from `from` to `to` using Dijkstra's
    /// algorithm.
    ///
    /// Cost is treated as additive (sum of edge costs) and the path is
    /// selected to minimise total cost.  Quality is computed multiplicatively
    /// along the chosen path and stored in the returned [`TransformPath`].
    ///
    /// Returns `None` when no path exists between the two formats.
    pub fn find_path(&self, from: Format, to: Format) -> Option<TransformPath> {
        self.find_path_with_mode(from, to, OptimizationMode::Speed)
    }

    /// Find the optimal path from `from` to `to` according to `mode`.
    ///
    /// The edge weights passed to A\* are determined by
    /// [`OptimizationMode::edge_weight`], so the selected path will be the
    /// one that minimizes cost (`Speed`), maximizes quality (`Quality`), or
    /// balances both (`Balanced`).
    ///
    /// Returns `None` when no path exists between the two formats.
    pub fn find_path_with_mode(
        &self,
        from: Format,
        to: Format,
        mode: OptimizationMode,
    ) -> Option<TransformPath> {
        use petgraph::algo::astar;

        let (&from_idx, &to_idx) =
            match (self.nodes.get(&from), self.nodes.get(&to)) {
                (Some(f), Some(t)) => (f, t),
                _ => return None,
            };

        let (_cost, node_path) = astar(
            &self.graph,
            from_idx,
            |n| n == to_idx,
            |e| mode.edge_weight(e.weight().cost, e.weight().quality),
            |_| 0.0_f32,
        )?;

        Some(TransformPath::from_steps(
            self.edges_from_node_path_with_mode(&node_path, mode),
        ))
    }

    /// Return all simple paths (no repeated nodes) from `from` to `to`.
    ///
    /// The returned [`Vec`] is sorted by `total_cost` ascending so callers can
    /// easily compare candidate pipelines.  An empty `Vec` is returned when no
    /// path exists.
    pub fn find_all_paths(&self, from: Format, to: Format) -> Vec<TransformPath> {
        use petgraph::algo::all_simple_paths;

        let (&from_idx, &to_idx) =
            match (self.nodes.get(&from), self.nodes.get(&to)) {
                (Some(f), Some(t)) => (f, t),
                _ => return Vec::new(),
            };

        let mut paths: Vec<TransformPath> =
            all_simple_paths::<Vec<_>, _, std::collections::hash_map::RandomState>(
                &self.graph, from_idx, to_idx, 0, None,
            )
            .map(|node_path: Vec<NodeIndex>| {
                TransformPath::from_steps(self.edges_from_node_path(&node_path))
            })
            .collect();

        paths.sort_by(|a, b| {
            a.total_cost
                .partial_cmp(&b.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        paths
    }

    /// Build a minimal DAG that covers all `targets` reachable from `from`.
    ///
    /// For each target the cheapest path is computed independently via
    /// [`find_path`](Self::find_path).  All resulting edges are then merged
    /// into a single [`MultiTargetDag`], deduplicating any edges that are
    /// shared across paths.  When two paths contribute an edge for the same
    /// `(from, to)` pair the cheaper edge is kept.
    ///
    /// Returns `None` when at least one target is unreachable from `from`.
    /// Returns `Some` with an empty DAG when `targets` is empty.
    ///
    /// # Example
    ///
    /// ```rust
    /// use renderflow::graph::{Format, TransformEdge, TransformGraph};
    ///
    /// let mut graph = TransformGraph::new();
    /// graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    /// graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf,  0.8, 0.85));
    /// graph.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));
    ///
    /// let dag = graph
    ///     .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
    ///     .expect("all targets must be reachable");
    ///
    /// // Markdown → Html is shared: 3 unique edges, not 4.
    /// assert_eq!(dag.edge_count(), 3);
    ///
    /// let order = dag.execution_order();
    /// assert_eq!(order.len(), 3);
    /// ```
    pub fn build_multi_target_dag(
        &self,
        from: Format,
        targets: &[Format],
    ) -> Option<MultiTargetDag> {
        self.build_multi_target_dag_with_mode(from, targets, OptimizationMode::Speed)
    }

    /// Like [`build_multi_target_dag`](Self::build_multi_target_dag), but
    /// selects paths according to the given [`OptimizationMode`].
    pub fn build_multi_target_dag_with_mode(
        &self,
        from: Format,
        targets: &[Format],
        mode: OptimizationMode,
    ) -> Option<MultiTargetDag> {
        let mut dag = MultiTargetDag::new();
        for &target in targets {
            let path = self.find_path_with_mode(from, target, mode)?;
            for edge in path.steps {
                dag.merge_edge(edge);
            }
        }
        Some(dag)
    }

    /// Return the Pareto-optimal frontier of paths from `from` to `to`.
    ///
    /// All simple paths between the two formats are enumerated first.  Any
    /// path that is dominated (worse or equal cost **and** worse or equal
    /// quality compared to another candidate, with at least one strict
    /// difference) is discarded.  The surviving non-dominated paths are sorted
    /// by `total_cost` ascending and capped at `cap` entries to prevent an
    /// explosion for densely connected graphs.
    ///
    /// Returns an empty `Vec` when no path exists or when `from`/`to` are
    /// unknown formats.
    ///
    /// # Example
    ///
    /// ```rust
    /// use renderflow::graph::{Format, TransformEdge, TransformGraph};
    ///
    /// let mut graph = TransformGraph::new();
    /// // Direct path: cost=5.0, quality=0.99
    /// graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 5.0, 0.99));
    /// // Indirect path via Html: total cost=1.3, quality=0.85
    /// graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    /// graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
    ///
    /// // Neither path dominates the other (cheap+low-quality vs expensive+high-quality).
    /// let frontier = graph.find_pareto_paths(Format::Markdown, Format::Pdf, 10);
    /// assert_eq!(frontier.len(), 2);
    /// ```
    pub fn find_pareto_paths(
        &self,
        from: Format,
        to: Format,
        cap: usize,
    ) -> Vec<TransformPath> {
        use crate::optimization::pareto_frontier;

        let candidates = self.find_all_paths(from, to);
        pareto_frontier(&candidates, Some(cap))
    }
}

impl Default for TransformGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimization::OptimizationMode;

    fn markdown_to_pdf() -> TransformEdge {
        TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 0.9)
    }

    fn markdown_to_html() -> TransformEdge {
        TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0)
    }

    fn html_to_pdf() -> TransformEdge {
        TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85)
    }

    // ── construction ──────────────────────────────────────────────────────────

    #[test]
    fn test_new_graph_is_empty() {
        let graph = TransformGraph::new();
        assert!(graph.transforms_from(Format::Markdown).is_empty());
    }

    #[test]
    fn test_default_is_empty() {
        let graph = TransformGraph::default();
        assert!(graph.transforms_from(Format::Pdf).is_empty());
    }

    // ── add_transform / has_transform ─────────────────────────────────────────

    #[test]
    fn test_add_single_transform_creates_edge() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        assert!(graph.has_transform(Format::Markdown, Format::Pdf));
    }

    #[test]
    fn test_has_transform_returns_false_for_missing_edge() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        assert!(!graph.has_transform(Format::Markdown, Format::Html));
    }

    #[test]
    fn test_has_transform_returns_false_for_unknown_format() {
        let graph = TransformGraph::new();
        assert!(!graph.has_transform(Format::Markdown, Format::Pdf));
    }

    #[test]
    fn test_multiple_transforms_from_same_source() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        graph.add_transform(markdown_to_html());

        assert!(graph.has_transform(Format::Markdown, Format::Pdf));
        assert!(graph.has_transform(Format::Markdown, Format::Html));
    }

    // ── transforms_from ───────────────────────────────────────────────────────

    #[test]
    fn test_transforms_from_returns_all_outgoing_edges() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        graph.add_transform(markdown_to_html());

        let edges = graph.transforms_from(Format::Markdown);
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_transforms_from_returns_correct_targets() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        graph.add_transform(markdown_to_html());

        let targets: Vec<Format> = graph
            .transforms_from(Format::Markdown)
            .into_iter()
            .map(|e| e.to)
            .collect();
        assert!(targets.contains(&Format::Pdf));
        assert!(targets.contains(&Format::Html));
    }

    #[test]
    fn test_transforms_from_empty_for_unknown_format() {
        let graph = TransformGraph::new();
        assert!(graph.transforms_from(Format::Epub).is_empty());
    }

    #[test]
    fn test_transforms_from_empty_for_sink_node() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        // Pdf is a sink — no outgoing edges.
        assert!(graph.transforms_from(Format::Pdf).is_empty());
    }

    // ── transforms_to ─────────────────────────────────────────────────────────

    #[test]
    fn test_transforms_to_returns_all_incoming_edges() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        graph.add_transform(html_to_pdf());

        let edges = graph.transforms_to(Format::Pdf);
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_transforms_to_correct_sources() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        graph.add_transform(html_to_pdf());

        let sources: Vec<Format> = graph
            .transforms_to(Format::Pdf)
            .into_iter()
            .map(|e| e.from)
            .collect();
        assert!(sources.contains(&Format::Markdown));
        assert!(sources.contains(&Format::Html));
    }

    #[test]
    fn test_transforms_to_empty_for_unknown_format() {
        let graph = TransformGraph::new();
        assert!(graph.transforms_to(Format::Pdf).is_empty());
    }

    #[test]
    fn test_transforms_to_empty_for_source_only_node() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());
        // Markdown is a source — no incoming edges.
        assert!(graph.transforms_to(Format::Markdown).is_empty());
    }

    // ── edge metadata ─────────────────────────────────────────────────────────

    #[test]
    fn test_edge_cost_and_quality_preserved() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 2.5, 0.75));

        let edges = graph.transforms_from(Format::Markdown);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].cost, 2.5);
        assert_eq!(edges[0].quality, 0.75);
    }

    // ── multi-hop paths ───────────────────────────────────────────────────────

    #[test]
    fn test_graph_traversal_multi_hop() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        // Markdown → HTML → PDF: verify both hops exist.
        let first_hop = graph.transforms_from(Format::Markdown);
        assert_eq!(first_hop.len(), 1);
        assert_eq!(first_hop[0].to, Format::Html);

        let second_hop = graph.transforms_from(Format::Html);
        assert_eq!(second_hop.len(), 1);
        assert_eq!(second_hop[0].to, Format::Pdf);
    }

    // ── all formats as nodes ──────────────────────────────────────────────────

    #[test]
    fn test_all_format_variants_can_be_nodes() {
        let formats = [
            Format::Markdown,
            Format::Html,
            Format::Pdf,
            Format::Docx,
            Format::Epub,
            Format::Rst,
            Format::Latex,
        ];
        let mut graph = TransformGraph::new();
        // Add each format as both source and target to ensure every variant
        // can be inserted as a node without panicking.
        for &src in &formats {
            for &dst in &formats {
                if src != dst {
                    graph.add_transform(TransformEdge::new(src, dst, 1.0, 1.0));
                }
            }
        }
        // Every format should now have outgoing edges to every other format.
        for &fmt in &formats {
            assert_eq!(
                graph.transforms_from(fmt).len(),
                formats.len() - 1,
                "expected {} outgoing edges from {:?}",
                formats.len() - 1,
                fmt
            );
        }
    }

    // ── find_path ─────────────────────────────────────────────────────────────

    #[test]
    fn test_find_path_direct_single_hop() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());

        let path = graph.find_path(Format::Markdown, Format::Pdf).unwrap();
        assert_eq!(path.steps.len(), 1);
        assert_eq!(path.steps[0].from, Format::Markdown);
        assert_eq!(path.steps[0].to, Format::Pdf);
        assert!((path.total_cost - 1.0).abs() < 1e-5);
        assert!((path.total_quality - 0.9).abs() < 1e-5);
    }

    #[test]
    fn test_find_path_multi_hop() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_html()); // cost 0.5, quality 1.0
        graph.add_transform(html_to_pdf()); // cost 0.8, quality 0.85

        let path = graph.find_path(Format::Markdown, Format::Pdf).unwrap();
        assert_eq!(path.steps.len(), 2);
        assert_eq!(path.steps[0].to, Format::Html);
        assert_eq!(path.steps[1].to, Format::Pdf);
        assert!((path.total_cost - 1.3).abs() < 1e-5);
        assert!((path.total_quality - 0.85).abs() < 1e-5);
    }

    #[test]
    fn test_find_path_prefers_lower_cost() {
        let mut graph = TransformGraph::new();
        // Direct path — more expensive.
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 5.0, 0.9));
        // Indirect path via HTML — cheaper overall (0.5 + 0.8 = 1.3).
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        let path = graph.find_path(Format::Markdown, Format::Pdf).unwrap();
        // The indirect path (total_cost 1.3) should be chosen over the direct
        // path (total_cost 5.0).
        assert_eq!(path.steps.len(), 2);
        assert!((path.total_cost - 1.3).abs() < 1e-5);
    }

    #[test]
    fn test_find_path_returns_none_when_no_path() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_html());
        // No edge from Html → Pdf, so Markdown → Pdf has no path.
        assert!(graph.find_path(Format::Markdown, Format::Pdf).is_none());
    }

    #[test]
    fn test_find_path_returns_none_for_unknown_format() {
        let graph = TransformGraph::new();
        assert!(graph.find_path(Format::Markdown, Format::Pdf).is_none());
    }

    #[test]
    fn test_find_path_cost_additive() {
        let mut graph = TransformGraph::new();
        // Three hops: Markdown → Html (1.0) → Pdf (2.0) — total 3.0
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 2.0, 1.0));

        let path = graph.find_path(Format::Markdown, Format::Pdf).unwrap();
        assert!((path.total_cost - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_find_path_quality_multiplicative() {
        let mut graph = TransformGraph::new();
        // quality: 0.9 * 0.8 = 0.72
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 0.9));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 1.0, 0.8));

        let path = graph.find_path(Format::Markdown, Format::Pdf).unwrap();
        assert!((path.total_quality - 0.72).abs() < 1e-5);
    }

    #[test]
    fn test_find_path_chooses_cheapest_parallel_edge() {
        let mut graph = TransformGraph::new();
        // Two parallel edges between the same nodes with different costs.
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 3.0, 0.9));
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 0.7));

        let path = graph.find_path(Format::Markdown, Format::Pdf).unwrap();
        assert!((path.total_cost - 1.0).abs() < 1e-5);
    }

    // ── find_all_paths ────────────────────────────────────────────────────────

    #[test]
    fn test_find_all_paths_single_path() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf());

        let paths = graph.find_all_paths(Format::Markdown, Format::Pdf);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].steps.len(), 1);
    }

    #[test]
    fn test_find_all_paths_returns_both_direct_and_indirect() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_pdf()); // direct
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf()); // indirect via Html

        let paths = graph.find_all_paths(Format::Markdown, Format::Pdf);
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_find_all_paths_sorted_by_cost_ascending() {
        let mut graph = TransformGraph::new();
        // Direct (cost 5.0) and indirect (cost 1.3).
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 5.0, 0.9));
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        let paths = graph.find_all_paths(Format::Markdown, Format::Pdf);
        assert_eq!(paths.len(), 2);
        // Cheaper path comes first.
        assert!(paths[0].total_cost <= paths[1].total_cost);
        assert!((paths[0].total_cost - 1.3).abs() < 1e-5);
        assert!((paths[1].total_cost - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_find_all_paths_empty_when_no_path() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_html());

        let paths = graph.find_all_paths(Format::Markdown, Format::Pdf);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_find_all_paths_empty_for_unknown_format() {
        let graph = TransformGraph::new();
        assert!(graph.find_all_paths(Format::Markdown, Format::Pdf).is_empty());
    }

    #[test]
    fn test_find_all_paths_metrics_correct() {
        let mut graph = TransformGraph::new();
        // cost 0.5, quality 1.0 then cost 0.8, quality 0.85
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        let paths = graph.find_all_paths(Format::Markdown, Format::Pdf);
        assert_eq!(paths.len(), 1);
        assert!((paths[0].total_cost - 1.3).abs() < 1e-5);
        assert!((paths[0].total_quality - 0.85).abs() < 1e-5);
    }

    // ── find_path_with_mode ───────────────────────────────────────────────────

    #[test]
    fn test_find_path_with_mode_speed_minimizes_cost() {
        let mut graph = TransformGraph::new();
        // Direct path: cost=5.0, quality=0.99
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 5.0, 0.99));
        // Indirect path via Html: total cost=1.3, total quality=0.85
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        let path = graph
            .find_path_with_mode(Format::Markdown, Format::Pdf, OptimizationMode::Speed)
            .unwrap();
        // Speed mode should choose the cheaper indirect path (1.3 < 5.0).
        assert_eq!(path.steps.len(), 2);
        assert!((path.total_cost - 1.3).abs() < 1e-5);
    }

    #[test]
    fn test_find_path_with_mode_quality_maximizes_quality() {
        let mut graph = TransformGraph::new();
        // Direct path: cost=1.0, quality=0.5 (poor quality)
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 0.5));
        // Indirect path via Html: total cost=1.3, quality=0.85 (better quality)
        graph.add_transform(markdown_to_html()); // quality 1.0
        graph.add_transform(html_to_pdf()); // quality 0.85

        let path = graph
            .find_path_with_mode(Format::Markdown, Format::Pdf, OptimizationMode::Quality)
            .unwrap();
        // Quality mode should prefer the path with higher total quality (0.85 > 0.5).
        assert_eq!(path.steps.len(), 2);
        assert!(path.total_quality > 0.5 + 1e-5);
    }

    #[test]
    fn test_find_path_with_mode_balanced_chooses_balanced_path() {
        let mut graph = TransformGraph::new();
        // Path A: cheap but poor quality (cost=0.5, quality=0.3)
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 0.5, 0.3));
        // Path B: expensive but high quality (cost=5.0, quality=1.0)
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 5.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.0, 1.0));

        let path_a = TransformPath::from_steps(vec![TransformEdge::new(
            Format::Markdown,
            Format::Pdf,
            0.5,
            0.3,
        )]);
        let path_b = TransformPath::from_steps(vec![
            TransformEdge::new(Format::Markdown, Format::Html, 5.0, 1.0),
            TransformEdge::new(Format::Html, Format::Pdf, 0.0, 1.0),
        ]);

        // Balanced scores: A = -0.5*0.5 + 0.5*0.3 = -0.1, B = -0.5*5.0 + 0.5*1.0 = -2.0
        // So A has a higher balanced score than B in this extreme case.
        assert!(
            OptimizationMode::Balanced.score(&path_a)
                > OptimizationMode::Balanced.score(&path_b)
        );
    }

    #[test]
    fn test_find_path_with_mode_returns_none_for_unknown_format() {
        let graph = TransformGraph::new();
        assert!(graph
            .find_path_with_mode(Format::Markdown, Format::Pdf, OptimizationMode::Quality)
            .is_none());
    }

    #[test]
    fn test_find_path_with_mode_speed_same_as_find_path() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        let path_default = graph.find_path(Format::Markdown, Format::Pdf).unwrap();
        let path_speed = graph
            .find_path_with_mode(Format::Markdown, Format::Pdf, OptimizationMode::Speed)
            .unwrap();
        assert_eq!(path_default.steps.len(), path_speed.steps.len());
        assert!((path_default.total_cost - path_speed.total_cost).abs() < 1e-5);
    }

    // ── find_pareto_paths ─────────────────────────────────────────────────────

    #[test]
    fn test_find_pareto_paths_both_non_dominated() {
        let mut graph = TransformGraph::new();
        // Direct path: cost=5.0, quality=0.99 — high quality, high cost
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 5.0, 0.99));
        // Indirect path via Html: total cost=1.3, quality=0.85 — cheaper, lower quality
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        // Neither path dominates the other.
        let frontier = graph.find_pareto_paths(Format::Markdown, Format::Pdf, 10);
        assert_eq!(frontier.len(), 2, "both paths should be on the frontier");
    }

    #[test]
    fn test_find_pareto_paths_dominated_path_excluded() {
        let mut graph = TransformGraph::new();
        // Path A: cost=1.3, quality=0.85 (indirect via Html)
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());
        // Path B: cost=5.0, quality=0.3 — dominated by A (worse cost AND worse quality)
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 5.0, 0.3));

        let frontier = graph.find_pareto_paths(Format::Markdown, Format::Pdf, 10);
        assert_eq!(frontier.len(), 1, "dominated path should be filtered out");
        assert!((frontier[0].total_cost - 1.3).abs() < 1e-5);
    }

    #[test]
    fn test_find_pareto_paths_cap_limits_results() {
        let mut graph = TransformGraph::new();
        // Direct path: cost=5.0, quality=0.99
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 5.0, 0.99));
        // Indirect path via Html: cost=1.3, quality=0.85
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        // Cap at 1 — only cheapest non-dominated path returned.
        let frontier = graph.find_pareto_paths(Format::Markdown, Format::Pdf, 1);
        assert_eq!(frontier.len(), 1);
        assert!((frontier[0].total_cost - 1.3).abs() < 1e-5);
    }

    #[test]
    fn test_find_pareto_paths_empty_when_no_path() {
        let mut graph = TransformGraph::new();
        graph.add_transform(markdown_to_html());
        // No Html → Pdf edge; no path exists.
        let frontier = graph.find_pareto_paths(Format::Markdown, Format::Pdf, 10);
        assert!(frontier.is_empty());
    }

    #[test]
    fn test_find_pareto_paths_empty_for_unknown_format() {
        let graph = TransformGraph::new();
        assert!(graph
            .find_pareto_paths(Format::Markdown, Format::Pdf, 10)
            .is_empty());
    }

    #[test]
    fn test_find_pareto_paths_sorted_by_cost_ascending() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 5.0, 0.99));
        graph.add_transform(markdown_to_html());
        graph.add_transform(html_to_pdf());

        let frontier = graph.find_pareto_paths(Format::Markdown, Format::Pdf, 10);
        assert_eq!(frontier.len(), 2);
        assert!(frontier[0].total_cost <= frontier[1].total_cost);
    }
}
