use super::{Format, InputKind};

/// Metadata attached to a directed edge in the [`TransformGraph`](super::TransformGraph).
///
/// Each `TransformEdge` describes a single transformation that converts a
/// document from one [`Format`] to another.  The `cost` and `quality` fields
/// allow graph-search algorithms to prefer cheaper or higher-quality paths when
/// multiple routes between two formats exist.
///
/// The `input_kind` field indicates whether the transformation operates on a
/// single source document ([`InputKind::Single`]) or aggregates a collection
/// of source documents into one output ([`InputKind::Collection`]).
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
    /// Whether this transformation consumes a single input or a collection.
    pub input_kind: InputKind,
}

impl TransformEdge {
    /// Create a new `TransformEdge` with [`InputKind::Single`].
    ///
    /// # Parameters
    ///
    /// * `from`    – source [`Format`]
    /// * `to`      – target [`Format`]
    /// * `cost`    – relative execution cost (lower is cheaper)
    /// * `quality` – expected output quality in the range `[0.0, 1.0]`; values
    ///   outside this range are clamped automatically.
    pub fn new(from: Format, to: Format, cost: f32, quality: f32) -> Self {
        Self { from, to, cost, quality: quality.clamp(0.0, 1.0), input_kind: InputKind::Single }
    }

    /// Create a new `TransformEdge` with an explicit [`InputKind`].
    ///
    /// Use this when registering a collection-based transform (e.g. pages → book).
    ///
    /// # Parameters
    ///
    /// * `from`       – source [`Format`]
    /// * `to`         – target [`Format`]
    /// * `cost`       – relative execution cost (lower is cheaper)
    /// * `quality`    – expected output quality in the range `[0.0, 1.0]`
    /// * `input_kind` – whether this edge consumes a single input or a collection
    pub fn with_input_kind(from: Format, to: Format, cost: f32, quality: f32, input_kind: InputKind) -> Self {
        Self { from, to, cost, quality: quality.clamp(0.0, 1.0), input_kind }
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
        assert_eq!(edge.input_kind, InputKind::Single);
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

    #[test]
    fn test_new_defaults_to_single_input_kind() {
        let edge = TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0);
        assert_eq!(edge.input_kind, InputKind::Single);
    }

    #[test]
    fn test_with_input_kind_collection() {
        let edge = TransformEdge::with_input_kind(
            Format::Markdown, Format::Epub, 1.0, 0.85, InputKind::Collection,
        );
        assert_eq!(edge.input_kind, InputKind::Collection);
        assert_eq!(edge.from, Format::Markdown);
        assert_eq!(edge.to, Format::Epub);
    }

    #[test]
    fn test_with_input_kind_single_matches_new() {
        let a = TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0);
        let b = TransformEdge::with_input_kind(Format::Markdown, Format::Html, 0.5, 1.0, InputKind::Single);
        assert_eq!(a, b);
    }

    #[test]
    fn test_collection_edge_inequality_with_single() {
        let single = TransformEdge::new(Format::Markdown, Format::Epub, 1.0, 0.85);
        let collection = TransformEdge::with_input_kind(
            Format::Markdown, Format::Epub, 1.0, 0.85, InputKind::Collection,
        );
        assert_ne!(single, collection);
    }
}
