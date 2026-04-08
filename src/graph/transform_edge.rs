use super::Format;

/// Metadata attached to a directed edge in the [`TransformGraph`](super::TransformGraph).
///
/// Each `TransformEdge` describes a single transformation that converts a
/// document from one [`Format`] to another.  The `cost` and `quality` fields
/// allow graph-search algorithms to prefer cheaper or higher-quality paths when
/// multiple routes between two formats exist.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformEdge {
    /// The source format for this transformation.
    pub from: Format,
    /// The target format produced by this transformation.
    pub to: Format,
    /// Relative cost of applying this transformation (lower is cheaper).
    pub cost: f32,
    /// Expected quality of the output on a 0.0–1.0 scale (higher is better).
    pub quality: f32,
}

impl TransformEdge {
    /// Create a new `TransformEdge`.
    ///
    /// # Parameters
    ///
    /// * `from`    – source [`Format`]
    /// * `to`      – target [`Format`]
    /// * `cost`    – relative execution cost (lower is cheaper)
    /// * `quality` – expected output quality in the range `[0.0, 1.0]`; values
    ///   outside this range are clamped automatically.
    pub fn new(from: Format, to: Format, cost: f32, quality: f32) -> Self {
        Self { from, to, cost, quality: quality.clamp(0.0, 1.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_edge_fields() {
        let edge = TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 0.9);
        assert_eq!(edge.from, Format::Markdown);
        assert_eq!(edge.to, Format::Pdf);
        assert_eq!(edge.cost, 1.0);
        assert_eq!(edge.quality, 0.9);
    }

    #[test]
    fn test_transform_edge_clone() {
        let edge = TransformEdge::new(Format::Html, Format::Pdf, 2.0, 0.8);
        let cloned = edge.clone();
        assert_eq!(edge, cloned);
    }

    #[test]
    fn test_transform_edge_equality() {
        let a = TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0);
        let b = TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0);
        assert_eq!(a, b);
    }

    #[test]
    fn test_transform_edge_inequality() {
        let a = TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0);
        let b = TransformEdge::new(Format::Markdown, Format::Pdf, 0.5, 1.0);
        assert_ne!(a, b);
    }

    #[test]
    fn test_quality_clamped_above_one() {
        let edge = TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.5);
        assert!((edge.quality - 1.0).abs() < 1e-5, "quality above 1.0 must be clamped to 1.0");
    }

    #[test]
    fn test_quality_clamped_below_zero() {
        let edge = TransformEdge::new(Format::Markdown, Format::Html, 1.0, -0.5);
        assert!((edge.quality - 0.0).abs() < 1e-5, "quality below 0.0 must be clamped to 0.0");
    }

    #[test]
    fn test_quality_at_boundaries_not_clamped() {
        let edge_zero = TransformEdge::new(Format::Markdown, Format::Html, 1.0, 0.0);
        let edge_one = TransformEdge::new(Format::Html, Format::Pdf, 1.0, 1.0);
        assert!((edge_zero.quality - 0.0).abs() < 1e-5);
        assert!((edge_one.quality - 1.0).abs() < 1e-5);
    }
}
