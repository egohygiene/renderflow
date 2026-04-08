mod format;
mod transform_edge;

pub use format::Format;
pub use transform_edge::TransformEdge;

use petgraph::graph::{DiGraph, NodeIndex};
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
}

impl Default for TransformGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
