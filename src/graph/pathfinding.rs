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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Format, TransformEdge};

    fn edge(from: Format, to: Format, cost: f32, quality: f32) -> TransformEdge {
        TransformEdge::new(from, to, cost, quality)
    }

    // ── single-step paths ─────────────────────────────────────────────────────

    #[test]
    fn test_single_step_quality_preserved() {
        let steps = vec![edge(Format::Markdown, Format::Html, 1.0, 0.9)];
        let path = TransformPath::from_steps(steps);
        assert!((path.total_quality - 0.9).abs() < 1e-5,
            "single-step quality must equal the edge quality");
    }

    #[test]
    fn test_single_step_cost_preserved() {
        let steps = vec![edge(Format::Markdown, Format::Html, 2.5, 1.0)];
        let path = TransformPath::from_steps(steps);
        assert!((path.total_cost - 2.5).abs() < 1e-5);
    }

    // ── multi-step quality is multiplicative ──────────────────────────────────

    #[test]
    fn test_two_step_quality_is_product() {
        // 0.9 * 0.8 = 0.72
        let steps = vec![
            edge(Format::Markdown, Format::Html, 1.0, 0.9),
            edge(Format::Html, Format::Pdf, 1.0, 0.8),
        ];
        let path = TransformPath::from_steps(steps);
        assert!((path.total_quality - 0.72).abs() < 1e-5,
            "two-step quality must be the product of both edge qualities");
    }

    #[test]
    fn test_two_step_cost_is_sum() {
        let steps = vec![
            edge(Format::Markdown, Format::Html, 0.5, 1.0),
            edge(Format::Html, Format::Pdf, 0.8, 0.85),
        ];
        let path = TransformPath::from_steps(steps);
        assert!((path.total_cost - 1.3).abs() < 1e-5);
    }

    #[test]
    fn test_three_step_quality_propagates_multiplicatively() {
        // 0.9 * 0.8 * 0.7 = 0.504
        let steps = vec![
            edge(Format::Markdown, Format::Html, 1.0, 0.9),
            edge(Format::Html, Format::Rst, 1.0, 0.8),
            edge(Format::Rst, Format::Pdf, 1.0, 0.7),
        ];
        let path = TransformPath::from_steps(steps);
        assert!((path.total_quality - 0.504_f32).abs() < 1e-5,
            "three-step quality must be the product of all three edge qualities");
    }

    // ── perfect quality path ──────────────────────────────────────────────────

    #[test]
    fn test_all_quality_one_yields_product_one() {
        let steps = vec![
            edge(Format::Markdown, Format::Html, 1.0, 1.0),
            edge(Format::Html, Format::Pdf, 1.0, 1.0),
        ];
        let path = TransformPath::from_steps(steps);
        assert!((path.total_quality - 1.0).abs() < 1e-5,
            "product of 1.0 values must equal 1.0");
    }

    // ── zero quality collapses path quality ───────────────────────────────────

    #[test]
    fn test_zero_quality_edge_collapses_path_quality() {
        let steps = vec![
            edge(Format::Markdown, Format::Html, 1.0, 0.9),
            edge(Format::Html, Format::Pdf, 1.0, 0.0),
        ];
        let path = TransformPath::from_steps(steps);
        assert!((path.total_quality - 0.0).abs() < 1e-5,
            "a zero-quality edge must bring total path quality to 0.0");
    }

    // ── step count ────────────────────────────────────────────────────────────

    #[test]
    fn test_steps_count_matches_input() {
        let steps = vec![
            edge(Format::Markdown, Format::Html, 1.0, 1.0),
            edge(Format::Html, Format::Pdf, 1.0, 1.0),
            edge(Format::Pdf, Format::Epub, 1.0, 1.0),
        ];
        let path = TransformPath::from_steps(steps);
        assert_eq!(path.steps.len(), 3);
    }
}
