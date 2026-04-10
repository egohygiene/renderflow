use super::{Format, TransformEdge};

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;

/// A minimal directed acyclic graph that merges the cheapest transformation
/// paths for several output targets from a single source format.
///
/// Intermediate nodes (e.g. `Html` when producing both `Pdf` and `Docx` from
/// `Markdown`) are reused across paths so that each transformation step is
/// represented exactly once in the DAG.
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
/// // Markdown → Html is shared: only 3 unique edges total.
/// assert_eq!(dag.edge_count(), 3);
/// assert!(dag.contains_edge(Format::Markdown, Format::Html));
/// assert!(dag.contains_edge(Format::Html, Format::Pdf));
/// assert!(dag.contains_edge(Format::Html, Format::Docx));
///
/// // Execution order respects dependencies.
/// let order = dag.execution_order();
/// assert_eq!(order.len(), 3);
/// ```
pub struct MultiTargetDag {
    pub(super) graph: DiGraph<Format, TransformEdge>,
    pub(super) nodes: HashMap<Format, NodeIndex>,
}

impl MultiTargetDag {
    pub(super) fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            nodes: HashMap::new(),
        }
    }

    /// Return the [`NodeIndex`] for `format`, inserting a node when one does
    /// not already exist.
    pub(super) fn get_or_insert_node(&mut self, format: Format) -> NodeIndex {
        if let Some(&idx) = self.nodes.get(&format) {
            idx
        } else {
            let idx = self.graph.add_node(format);
            self.nodes.insert(format, idx);
            idx
        }
    }

    /// Merge `edge` into the DAG.
    ///
    /// If an edge between the same `(from, to)` pair already exists, the one
    /// with the lower cost is kept and the other is discarded.  This ensures
    /// that a shared intermediate step always executes via the cheapest
    /// available transformation.
    pub(super) fn merge_edge(&mut self, edge: TransformEdge) {
        let from_idx = self.get_or_insert_node(edge.from);
        let to_idx = self.get_or_insert_node(edge.to);

        // Check whether an edge for this (from, to) pair is already present.
        let existing_id = self
            .graph
            .edges(from_idx)
            .find(|e| e.target() == to_idx)
            .map(|e| e.id());

        if let Some(id) = existing_id {
            // Keep the cheaper edge.
            if edge.cost < self.graph[id].cost {
                self.graph[id] = edge;
            }
        } else {
            self.graph.add_edge(from_idx, to_idx, edge);
        }
    }

    /// Return the edges in a valid topological execution order.
    ///
    /// Each [`TransformEdge`] in the returned `Vec` is guaranteed to appear
    /// only after all edges that produce its source format.  An empty `Vec` is
    /// returned when the graph contains a cycle (which should never occur for a
    /// well-formed transformation DAG).
    pub fn execution_order(&self) -> Vec<&TransformEdge> {
        use petgraph::algo::toposort;

        let sorted_nodes = match toposort(&self.graph, None) {
            Ok(nodes) => nodes,
            Err(_) => return Vec::new(),
        };

        let mut result = Vec::new();
        for node in &sorted_nodes {
            for edge_ref in self.graph.edges(*node) {
                result.push(edge_ref.weight());
            }
        }
        result
    }

    /// Return all edges stored in the DAG (in arbitrary order).
    pub fn all_edges(&self) -> Vec<&TransformEdge> {
        self.graph.edge_weights().collect()
    }

    /// Return `true` when a direct edge from `from` to `to` exists in the DAG.
    pub fn contains_edge(&self, from: Format, to: Format) -> bool {
        let (Some(&fi), Some(&ti)) = (self.nodes.get(&from), self.nodes.get(&to)) else {
            return false;
        };
        self.graph.contains_edge(fi, ti)
    }

    /// Return the number of unique edges in the DAG.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Return the number of unique format nodes in the DAG.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Return all edges in the DAG whose [`InputKind`] is [`Collection`](InputKind::Collection).
    ///
    /// Collection edges represent aggregation-style transformations that consume
    /// multiple source documents simultaneously (e.g. pages → book).
    pub fn collection_edges(&self) -> Vec<&TransformEdge> {
        self.graph
            .edge_weights()
            .filter(|e| e.input_kind.is_collection())
            .collect()
    }

    /// Render a human-readable tree view of the DAG for CLI display.
    ///
    /// `source` is the originating [`Format`] (highlighted as the root node).
    /// The output lists nodes, edges, and the full execution order with costs
    /// and quality scores, making it easy to understand the planned
    /// transformation pipeline at a glance.
    ///
    /// # Example output
    ///
    /// ```text
    /// DAG Execution Plan
    /// ==================
    /// Source: markdown
    ///
    /// Nodes (3):
    ///   • markdown
    ///   • html
    ///   • pdf
    ///
    /// Edges (2):
    ///   markdown ──► html  [cost: 0.50, quality: 1.00]
    ///   html     ──► pdf   [cost: 0.80, quality: 0.85]
    ///
    /// Execution Order:
    ///   [1]  markdown  →  html  (cost: 0.50, quality: 1.00)
    ///   [2]  html      →  pdf   (cost: 0.80, quality: 0.85)
    /// ```
    pub fn to_tree(&self, source: Format) -> String {
        use petgraph::algo::toposort;
        use std::fmt::Write;

        let mut out = String::new();

        // ── header ────────────────────────────────────────────────────────────
        let _ = writeln!(out, "DAG Execution Plan");
        let _ = writeln!(out, "==================");
        let _ = writeln!(out, "Source: {}", source);

        // ── nodes ─────────────────────────────────────────────────────────────
        let _ = writeln!(out);
        let _ = writeln!(out, "Nodes ({}):", self.node_count());

        // Sort node labels for deterministic output.
        let mut node_labels: Vec<String> = self
            .graph
            .node_weights()
            .map(|f| f.to_string())
            .collect();
        node_labels.sort();
        for label in &node_labels {
            let _ = writeln!(out, "  • {}", label);
        }

        // ── edges ─────────────────────────────────────────────────────────────
        let _ = writeln!(out);
        let _ = writeln!(out, "Edges ({}):", self.edge_count());

        // Compute column width for aligned arrows.
        let max_from = self
            .graph
            .edge_weights()
            .map(|e| e.from.to_string().len())
            .max()
            .unwrap_or(0);

        let mut edge_lines: Vec<String> = self
            .graph
            .edge_weights()
            .map(|e| {
                format!(
                    "  {:<width$} ──► {}  [cost: {:.2}, quality: {:.2}]",
                    e.from.to_string(),
                    e.to,
                    e.cost,
                    e.quality,
                    width = max_from
                )
            })
            .collect();
        edge_lines.sort();
        for line in &edge_lines {
            let _ = writeln!(out, "{}", line);
        }

        // ── execution order ───────────────────────────────────────────────────
        let _ = writeln!(out);
        let _ = writeln!(out, "Execution Order:");

        let sorted_nodes = match toposort(&self.graph, None) {
            Ok(nodes) => nodes,
            Err(_) => {
                let _ = writeln!(out, "  (cycle detected – execution order unavailable)");
                return out;
            }
        };

        let mut step = 1usize;
        let max_from_exec = self
            .graph
            .edge_weights()
            .map(|e| e.from.to_string().len())
            .max()
            .unwrap_or(0);
        let max_to_exec = self
            .graph
            .edge_weights()
            .map(|e| e.to.to_string().len())
            .max()
            .unwrap_or(0);

        for node in &sorted_nodes {
            for edge_ref in self.graph.edges(*node) {
                let e = edge_ref.weight();
                let _ = writeln!(
                    out,
                    "  [{step}]  {:<fw$}  →  {:<tw$}  (cost: {:.2}, quality: {:.2})",
                    e.from.to_string(),
                    e.to.to_string(),
                    e.cost,
                    e.quality,
                    fw = max_from_exec,
                    tw = max_to_exec,
                );
                step += 1;
            }
        }

        out
    }

    /// Render the DAG as a [DOT language](https://graphviz.org/doc/info/lang.html)
    /// string suitable for use with Graphviz tools (e.g. `dot`, `neato`).
    ///
    /// `source` is the originating [`Format`]; it receives a distinct visual
    /// style (filled blue) in the output graph.  Target (leaf) nodes are
    /// highlighted green.  All other intermediate nodes use the default style.
    ///
    /// The generated string can be saved to a `.dot` file and rendered with:
    ///
    /// ```sh
    /// dot -Tsvg pipeline.dot -o pipeline.svg
    /// ```
    pub fn to_dot(&self, source: Format) -> String {
        use std::fmt::Write;

        let mut out = String::new();

        let _ = writeln!(out, "digraph renderflow {{");
        let _ = writeln!(out, "    rankdir=LR;");
        let _ = writeln!(out, "    node [shape=box fontname=\"monospace\"];");
        let _ = writeln!(out);

        // Identify leaf nodes (nodes with no outgoing edges in the DAG).
        let leaf_nodes: std::collections::HashSet<Format> = self
            .nodes
            .keys()
            .filter(|&&f| {
                let idx = self.nodes[&f];
                self.graph.edges(idx).next().is_none()
            })
            .copied()
            .collect();

        // Emit node declarations with styles.
        let mut node_labels: Vec<String> = self
            .graph
            .node_weights()
            .map(|f| f.to_string())
            .collect();
        node_labels.sort();
        for label in &node_labels {
            let format: Format = label.parse().expect("node label is always a valid Format");
            let style = if format == source {
                " style=filled fillcolor=lightblue".to_string()
            } else if leaf_nodes.contains(&format) {
                " style=filled fillcolor=lightgreen".to_string()
            } else {
                String::new()
            };
            let _ = writeln!(out, "    \"{label}\" [label=\"{label}\"{style}];");
        }

        let _ = writeln!(out);

        // Emit edges.
        let mut edge_lines: Vec<String> = self
            .graph
            .edge_weights()
            .map(|e| {
                format!(
                    "    \"{}\" -> \"{}\" [label=\"cost={:.2}\\nquality={:.2}\"];",
                    e.from, e.to, e.cost, e.quality
                )
            })
            .collect();
        edge_lines.sort();
        for line in &edge_lines {
            let _ = writeln!(out, "{}", line);
        }

        let _ = writeln!(out, "}}");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{InputKind, TransformGraph};

    fn build_graph() -> TransformGraph {
        let mut g = TransformGraph::new();
        // Markdown → Html (0.5 / 1.0)
        g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
        // Html → Pdf  (0.8 / 0.85)
        g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
        // Html → Docx (0.6 / 0.90)
        g.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));
        g
    }

    // ── build_multi_target_dag ────────────────────────────────────────────────

    #[test]
    fn test_dag_shared_intermediate_not_duplicated() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        // Markdown → Html is shared; only 3 unique edges total.
        assert_eq!(dag.edge_count(), 3);
    }

    #[test]
    fn test_dag_contains_expected_edges() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        assert!(dag.contains_edge(Format::Markdown, Format::Html));
        assert!(dag.contains_edge(Format::Html, Format::Pdf));
        assert!(dag.contains_edge(Format::Html, Format::Docx));
    }

    #[test]
    fn test_dag_single_target_equals_path() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
            .unwrap();

        assert_eq!(dag.edge_count(), 2);
        assert!(dag.contains_edge(Format::Markdown, Format::Html));
        assert!(dag.contains_edge(Format::Html, Format::Pdf));
    }

    #[test]
    fn test_dag_empty_targets_returns_empty_dag() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[])
            .unwrap();

        assert_eq!(dag.edge_count(), 0);
        assert_eq!(dag.node_count(), 0);
    }

    #[test]
    fn test_dag_unreachable_target_returns_none() {
        let mut g = TransformGraph::new();
        g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));

        // Epub is not reachable.
        let result = g
            .build_multi_target_dag(Format::Markdown, &[Format::Html, Format::Epub]);
        assert!(result.is_none());
    }

    // ── execution_order ───────────────────────────────────────────────────────

    #[test]
    fn test_execution_order_length() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let order = dag.execution_order();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_execution_order_source_before_dependents() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let order = dag.execution_order();

        // Find all three positions in a single pass.
        let mut md_html_pos = None;
        let mut html_pdf_pos = None;
        let mut html_docx_pos = None;
        for (i, e) in order.iter().enumerate() {
            match (e.from, e.to) {
                (Format::Markdown, Format::Html) => md_html_pos = Some(i),
                (Format::Html, Format::Pdf) => html_pdf_pos = Some(i),
                (Format::Html, Format::Docx) => html_docx_pos = Some(i),
                _ => {}
            }
        }

        let md_html_pos = md_html_pos.expect("Markdown→Html edge must be present");
        let html_pdf_pos = html_pdf_pos.expect("Html→Pdf edge must be present");
        let html_docx_pos = html_docx_pos.expect("Html→Docx edge must be present");

        assert!(
            md_html_pos < html_pdf_pos,
            "Markdown→Html must precede Html→Pdf"
        );
        assert!(
            md_html_pos < html_docx_pos,
            "Markdown→Html must precede Html→Docx"
        );
    }

    #[test]
    fn test_execution_order_no_duplicates() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let order = dag.execution_order();
        // Build a list of (from, to) pairs and ensure they are all unique.
        let mut seen = std::collections::HashSet::new();
        for edge in &order {
            let pair = (edge.from, edge.to);
            assert!(seen.insert(pair), "duplicate edge in execution order: {:?}", pair);
        }
    }

    // ── edge deduplication (cheaper edge wins) ────────────────────────────────

    #[test]
    fn test_merge_edge_deduplicates_and_keeps_cheaper() {
        let mut dag = MultiTargetDag::new();
        // Add the same (from, to) pair twice: expensive first, then cheaper.
        dag.merge_edge(TransformEdge::new(Format::Markdown, Format::Html, 2.0, 0.9));
        dag.merge_edge(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 0.95));

        // Only one edge should be stored.
        assert_eq!(dag.edge_count(), 1);
        // The cheaper edge (cost 1.0) must be kept.
        let edges = dag.all_edges();
        assert!((edges[0].cost - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_merge_edge_retains_existing_when_new_is_more_expensive() {
        let mut dag = MultiTargetDag::new();
        // Add the cheaper edge first, then try to replace it with a more expensive one.
        dag.merge_edge(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 0.95));
        dag.merge_edge(TransformEdge::new(Format::Markdown, Format::Html, 2.0, 0.9));

        assert_eq!(dag.edge_count(), 1);
        let edges = dag.all_edges();
        assert!((edges[0].cost - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_node_count_shared_intermediate() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        // Nodes: Markdown, Html, Pdf, Docx = 4
        assert_eq!(dag.node_count(), 4);
    }

    // ── all_edges ─────────────────────────────────────────────────────────────

    #[test]
    fn test_all_edges_count_matches_edge_count() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        assert_eq!(dag.all_edges().len(), dag.edge_count());
    }

    // ── collection_edges ──────────────────────────────────────────────────────

    #[test]
    fn test_collection_edges_empty_when_no_collection_edges() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        assert!(dag.collection_edges().is_empty());
    }

    #[test]
    fn test_collection_edges_returns_only_collection_edges() {
        let mut dag = MultiTargetDag::new();
        dag.merge_edge(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
        dag.merge_edge(TransformEdge::with_input_kind(
            Format::Markdown, Format::Epub, 1.0, 0.85, InputKind::Collection,
        ));

        let collection = dag.collection_edges();
        assert_eq!(collection.len(), 1);
        assert_eq!(collection[0].to, Format::Epub);
        assert_eq!(collection[0].input_kind, InputKind::Collection);
    }

    #[test]
    fn test_collection_edges_does_not_include_single_edges() {
        let mut dag = MultiTargetDag::new();
        dag.merge_edge(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
        dag.merge_edge(TransformEdge::with_input_kind(
            Format::Markdown, Format::Epub, 1.0, 0.85, InputKind::Collection,
        ));

        let collection = dag.collection_edges();
        assert!(collection.iter().all(|e| e.input_kind.is_collection()));
    }

    #[test]
    fn test_collection_edges_multiple() {
        let mut dag = MultiTargetDag::new();
        dag.merge_edge(TransformEdge::with_input_kind(
            Format::Markdown, Format::Epub, 1.0, 0.85, InputKind::Collection,
        ));
        dag.merge_edge(TransformEdge::with_input_kind(
            Format::Html, Format::Pdf, 0.8, 0.85, InputKind::Collection,
        ));
        dag.merge_edge(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));

        let collection = dag.collection_edges();
        assert_eq!(collection.len(), 2);
    }

    // ── to_tree ───────────────────────────────────────────────────────────────

    #[test]
    fn test_to_tree_contains_header() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let tree = dag.to_tree(Format::Markdown);
        assert!(tree.contains("DAG Execution Plan"), "header missing: {tree}");
        assert!(tree.contains("Source: markdown"), "source line missing: {tree}");
    }

    #[test]
    fn test_to_tree_lists_nodes() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let tree = dag.to_tree(Format::Markdown);
        assert!(tree.contains("markdown"), "markdown node missing");
        assert!(tree.contains("html"), "html node missing");
        assert!(tree.contains("pdf"), "pdf node missing");
        assert!(tree.contains("docx"), "docx node missing");
    }

    #[test]
    fn test_to_tree_lists_edges() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let tree = dag.to_tree(Format::Markdown);
        assert!(tree.contains("──►"), "edge arrow missing");
        // All three transforms should appear.
        assert!(tree.contains("Edges (3)"), "edge count missing: {tree}");
    }

    #[test]
    fn test_to_tree_lists_execution_order() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let tree = dag.to_tree(Format::Markdown);
        assert!(tree.contains("Execution Order:"), "execution order section missing");
        assert!(tree.contains("[1]"), "first step missing");
    }

    #[test]
    fn test_to_tree_empty_dag() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[])
            .unwrap();

        let tree = dag.to_tree(Format::Markdown);
        assert!(tree.contains("Nodes (0)"), "empty node count wrong: {tree}");
        assert!(tree.contains("Edges (0)"), "empty edge count wrong: {tree}");
    }

    // ── to_dot ────────────────────────────────────────────────────────────────

    #[test]
    fn test_to_dot_contains_digraph() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let dot = dag.to_dot(Format::Markdown);
        assert!(dot.starts_with("digraph renderflow {"), "missing digraph header: {dot}");
        assert!(dot.trim_end().ends_with('}'), "missing closing brace: {dot}");
    }

    #[test]
    fn test_to_dot_source_node_highlighted() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
            .unwrap();

        let dot = dag.to_dot(Format::Markdown);
        assert!(
            dot.contains("lightblue"),
            "source node should have lightblue fill: {dot}"
        );
    }

    #[test]
    fn test_to_dot_leaf_nodes_highlighted() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
            .unwrap();

        let dot = dag.to_dot(Format::Markdown);
        assert!(
            dot.contains("lightgreen"),
            "leaf node (pdf) should have lightgreen fill: {dot}"
        );
    }

    #[test]
    fn test_to_dot_contains_edges() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let dot = dag.to_dot(Format::Markdown);
        assert!(dot.contains("\"markdown\" -> \"html\""), "markdown→html edge missing: {dot}");
        assert!(dot.contains("\"html\" -> \"pdf\""), "html→pdf edge missing: {dot}");
        assert!(dot.contains("\"html\" -> \"docx\""), "html→docx edge missing: {dot}");
    }

    #[test]
    fn test_to_dot_edge_labels_include_cost_and_quality() {
        let g = build_graph();
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
            .unwrap();

        let dot = dag.to_dot(Format::Markdown);
        assert!(dot.contains("cost="), "cost label missing: {dot}");
        assert!(dot.contains("quality="), "quality label missing: {dot}");
    }
}
