use super::TransformEdge;

/// The result of a successful pathfinding query through the transformation
/// graph.
///
/// `steps` contains the ordered sequence of [`TransformEdge`]s that must be
/// applied in order to convert a document from the source format to the target
/// format.  `total_cost` is the sum of every edge cost along the path
/// (additive), and `total_quality` is the product of every edge quality value
/// along the path (multiplicative).
///
/// # Example
///
/// ```rust
/// use renderflow::graph::{Format, TransformEdge, TransformGraph, TransformPath};
///
/// let mut graph = TransformGraph::new();
/// graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
/// graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
///
/// let path = graph.find_path(Format::Markdown, Format::Pdf).unwrap();
/// assert_eq!(path.steps.len(), 2);
/// assert!((path.total_cost - 1.3).abs() < 1e-5);
/// assert!((path.total_quality - 0.85).abs() < 1e-5);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TransformPath {
    /// Ordered list of transformations to apply.
    pub steps: Vec<TransformEdge>,
    /// Sum of the cost of every step in the path (additive).
    pub total_cost: f32,
    /// Product of the quality of every step in the path (multiplicative).
    pub total_quality: f32,
}

impl TransformPath {
    /// Build a `TransformPath` from an ordered list of [`TransformEdge`]s.
    ///
    /// `total_cost` is computed as the sum of all edge costs; `total_quality`
    /// is computed as the product of all edge quality values.
    pub(super) fn from_steps(steps: Vec<TransformEdge>) -> Self {
        let total_cost = steps.iter().map(|e| e.cost).sum();
        let total_quality = steps.iter().map(|e| e.quality).product();
        Self { steps, total_cost, total_quality }
    }
}
