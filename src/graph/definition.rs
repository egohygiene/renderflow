use super::{Format, InputKind, TransformEdge};

/// A pluggable definition of a format-to-format transformation.
///
/// Unlike [`TransformEdge`], which is a graph artifact used internally by
/// [`TransformGraph`](super::TransformGraph), a `TransformDefinition`
/// describes a concrete conversion capability that can be registered at runtime
/// and later materialized into graph edges via
/// [`TransformDefinitionRegistry::build_graph`](super::TransformDefinitionRegistry::build_graph).
///
/// The optional `label` field identifies the underlying tool or method (e.g.
/// `"pandoc"` or `"wkhtmltopdf"`), which helps with diagnostics and lets
/// callers register multiple competing definitions for the same format pair.
///
/// The `input_kind` field controls whether the definition expects a single
/// source document or a collection of source documents as input.
///
/// # Example
///
/// ```rust
/// use renderflow::graph::{Format, InputKind, TransformDefinition};
///
/// let def = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc");
/// assert_eq!(def.from, Format::Markdown);
/// assert_eq!(def.to, Format::Html);
/// assert_eq!(def.label, "pandoc");
/// assert_eq!(def.input_kind, InputKind::Single);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TransformDefinition {
    /// Source format for this definition.
    pub from: Format,
    /// Target format produced by this definition.
    pub to: Format,
    /// Relative cost of applying this transformation (lower is cheaper).
    pub cost: f32,
    /// Expected quality of the output on a 0.0–1.0 scale (higher is better).
    pub quality: f32,
    /// Human-readable label identifying the tool or method (e.g. `"pandoc"`).
    pub label: String,
    /// Whether this definition consumes a single input or a collection.
    pub input_kind: InputKind,
}

impl TransformDefinition {
    /// Create a new `TransformDefinition` with [`InputKind::Single`].
    ///
    /// # Parameters
    ///
    /// * `from`    – source [`Format`]
    /// * `to`      – target [`Format`]
    /// * `cost`    – relative execution cost (lower is cheaper)
    /// * `quality` – expected output quality in the range `[0.0, 1.0]`; values
    ///   outside this range are clamped automatically.
    /// * `label`   – human-readable name identifying the conversion tool or method
    pub fn new(from: Format, to: Format, cost: f32, quality: f32, label: impl Into<String>) -> Self {
        Self {
            from,
            to,
            cost,
            quality: quality.clamp(0.0, 1.0),
            label: label.into(),
            input_kind: InputKind::Single,
        }
    }

    /// Create a new `TransformDefinition` with an explicit [`InputKind`].
    ///
    /// Use this when registering a collection-based transform (e.g. pages → book).
    ///
    /// # Parameters
    ///
    /// * `from`       – source [`Format`]
    /// * `to`         – target [`Format`]
    /// * `cost`       – relative execution cost (lower is cheaper)
    /// * `quality`    – expected output quality in the range `[0.0, 1.0]`
    /// * `label`      – human-readable name identifying the conversion tool or method
    /// * `input_kind` – whether this definition consumes a single input or a collection
    pub fn with_input_kind(
        from: Format,
        to: Format,
        cost: f32,
        quality: f32,
        label: impl Into<String>,
        input_kind: InputKind,
    ) -> Self {
        Self {
            from,
            to,
            cost,
            quality: quality.clamp(0.0, 1.0),
            label: label.into(),
            input_kind,
        }
    }

    /// Convert this definition into a [`TransformEdge`] for use in a
    /// [`TransformGraph`](super::TransformGraph).
    pub fn to_edge(&self) -> TransformEdge {
        TransformEdge::with_input_kind(self.from, self.to, self.cost, self.quality, self.input_kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── construction ──────────────────────────────────────────────────────────

    #[test]
    fn test_definition_fields() {
        let def = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc");
        assert_eq!(def.from, Format::Markdown);
        assert_eq!(def.to, Format::Html);
        assert_eq!(def.cost, 0.5);
        assert!((def.quality - 1.0).abs() < 1e-5);
        assert_eq!(def.label, "pandoc");
        assert_eq!(def.input_kind, InputKind::Single);
    }

    #[test]
    fn test_quality_clamped_above_one() {
        let def = TransformDefinition::new(Format::Markdown, Format::Pdf, 1.0, 1.5, "tool");
        assert!((def.quality - 1.0).abs() < 1e-5, "quality above 1.0 must be clamped");
    }

    #[test]
    fn test_quality_clamped_below_zero() {
        let def = TransformDefinition::new(Format::Markdown, Format::Pdf, 1.0, -0.5, "tool");
        assert!((def.quality - 0.0).abs() < 1e-5, "quality below 0.0 must be clamped");
    }

    #[test]
    fn test_quality_at_boundary_zero_not_clamped() {
        let def = TransformDefinition::new(Format::Markdown, Format::Html, 1.0, 0.0, "tool");
        assert!((def.quality - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_quality_at_boundary_one_not_clamped() {
        let def = TransformDefinition::new(Format::Markdown, Format::Html, 1.0, 1.0, "tool");
        assert!((def.quality - 1.0).abs() < 1e-5);
    }

    // ── clone / equality ──────────────────────────────────────────────────────

    #[test]
    fn test_definition_clone() {
        let def = TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf");
        let cloned = def.clone();
        assert_eq!(def, cloned);
    }

    #[test]
    fn test_definition_equality() {
        let a = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc");
        let b = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc");
        assert_eq!(a, b);
    }

    #[test]
    fn test_definition_inequality_by_label() {
        let a = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc");
        let b = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "other");
        assert_ne!(a, b);
    }

    #[test]
    fn test_definition_inequality_by_format() {
        let a = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc");
        let b = TransformDefinition::new(Format::Markdown, Format::Pdf, 0.5, 1.0, "pandoc");
        assert_ne!(a, b);
    }

    // ── with_input_kind ───────────────────────────────────────────────────────

    #[test]
    fn test_with_input_kind_collection() {
        let def = TransformDefinition::with_input_kind(
            Format::Markdown, Format::Epub, 1.0, 0.85, "book-assembler", InputKind::Collection,
        );
        assert_eq!(def.input_kind, InputKind::Collection);
        assert_eq!(def.label, "book-assembler");
    }

    #[test]
    fn test_new_defaults_to_single_input_kind() {
        let def = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc");
        assert_eq!(def.input_kind, InputKind::Single);
    }

    #[test]
    fn test_definition_inequality_by_input_kind() {
        let a = TransformDefinition::new(Format::Markdown, Format::Epub, 1.0, 0.85, "tool");
        let b = TransformDefinition::with_input_kind(
            Format::Markdown, Format::Epub, 1.0, 0.85, "tool", InputKind::Collection,
        );
        assert_ne!(a, b);
    }

    // ── to_edge ───────────────────────────────────────────────────────────────

    #[test]
    fn test_to_edge_preserves_from_to_cost_quality() {
        let def = TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf");
        let edge = def.to_edge();
        assert_eq!(edge.from, Format::Html);
        assert_eq!(edge.to, Format::Pdf);
        assert!((edge.cost - 0.8).abs() < 1e-5);
        assert!((edge.quality - 0.85).abs() < 1e-5);
    }

    #[test]
    fn test_to_edge_from_definition_with_clamped_quality() {
        // Quality of 1.5 is clamped to 1.0 at construction time; edge must reflect that.
        let def = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.5, "tool");
        let edge = def.to_edge();
        assert!((edge.quality - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_to_edge_propagates_input_kind_single() {
        let def = TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc");
        let edge = def.to_edge();
        assert_eq!(edge.input_kind, InputKind::Single);
    }

    #[test]
    fn test_to_edge_propagates_input_kind_collection() {
        let def = TransformDefinition::with_input_kind(
            Format::Markdown, Format::Epub, 1.0, 0.85, "book-assembler", InputKind::Collection,
        );
        let edge = def.to_edge();
        assert_eq!(edge.input_kind, InputKind::Collection);
    }
}
