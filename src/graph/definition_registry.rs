use super::{definition::TransformDefinition, Format, TransformGraph};

/// A runtime registry of pluggable format-to-format transform definitions.
///
/// `TransformDefinitionRegistry` decouples the engine from hardcoded
/// transformation logic: callers register [`TransformDefinition`]s at runtime
/// and call [`build_graph`](Self::build_graph) to produce a
/// [`TransformGraph`] that can be used for pathfinding and DAG construction.
///
/// Multiple definitions for the same `(from, to)` pair are allowed; all of
/// them are added to the graph as parallel edges.  Pathfinding algorithms
/// on the resulting graph will select the optimal edge according to the chosen
/// [`OptimizationMode`](crate::optimization::OptimizationMode).
///
/// # Example
///
/// ```rust
/// use renderflow::graph::{Format, TransformDefinition, TransformDefinitionRegistry};
///
/// let mut registry = TransformDefinitionRegistry::new();
/// registry
///     .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"))
///     .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"));
///
/// let graph = registry.build_graph();
/// assert!(graph.has_transform(Format::Markdown, Format::Html));
/// assert!(graph.has_transform(Format::Html, Format::Pdf));
/// ```
pub struct TransformDefinitionRegistry {
    definitions: Vec<TransformDefinition>,
}

impl TransformDefinitionRegistry {
    /// Create an empty registry with no registered definitions.
    pub fn new() -> Self {
        Self { definitions: Vec::new() }
    }

    /// Append a [`TransformDefinition`] to the registry and return `&mut self`
    /// for chaining.
    ///
    /// Definitions are stored in registration order.  When multiple definitions
    /// cover the same `(from, to)` pair they are all added to the graph as
    /// parallel edges.
    pub fn register(&mut self, def: TransformDefinition) -> &mut Self {
        self.definitions.push(def);
        self
    }

    /// Return all definitions whose source format matches `from`.
    ///
    /// Returns an empty `Vec` when no matching definitions are registered.
    pub fn definitions_from(&self, from: Format) -> Vec<&TransformDefinition> {
        self.definitions.iter().filter(|d| d.from == from).collect()
    }

    /// Return all definitions that produce the target format `to`.
    ///
    /// Returns an empty `Vec` when no matching definitions are registered.
    pub fn definitions_to(&self, to: Format) -> Vec<&TransformDefinition> {
        self.definitions.iter().filter(|d| d.to == to).collect()
    }

    /// Return a slice of all registered definitions in registration order.
    pub fn all_definitions(&self) -> &[TransformDefinition] {
        &self.definitions
    }

    /// Build a [`TransformGraph`] from all registered definitions.
    ///
    /// Each [`TransformDefinition`] is converted to a [`TransformEdge`](super::TransformEdge)
    /// and added to a fresh graph.  The resulting graph can be used directly
    /// with [`TransformGraph::find_path`], [`TransformGraph::build_multi_target_dag`],
    /// and all other pathfinding APIs.
    pub fn build_graph(&self) -> TransformGraph {
        let mut graph = TransformGraph::new();
        for def in &self.definitions {
            graph.add_transform(def.to_edge());
        }
        graph
    }

    /// Create a registry pre-populated with the standard document transformation
    /// definitions.
    ///
    /// The standard set covers the most common Pandoc-based conversion routes:
    ///
    /// | From       | To     | Cost | Quality | Label         |
    /// |------------|--------|------|---------|---------------|
    /// | Markdown   | Html   | 0.5  | 1.00    | pandoc        |
    /// | Html       | Pdf    | 0.8  | 0.85    | wkhtmltopdf   |
    /// | Html       | Docx   | 0.6  | 0.90    | pandoc        |
    /// | Markdown   | Pdf    | 1.5  | 0.75    | pandoc        |
    /// | Markdown   | Docx   | 1.2  | 0.80    | pandoc        |
    /// | Markdown   | Epub   | 1.0  | 0.85    | pandoc        |
    /// | Markdown   | Rst    | 1.0  | 0.90    | pandoc        |
    /// | Markdown   | Latex  | 1.0  | 0.90    | pandoc        |
    ///
    /// Additional definitions can be appended with [`register`](Self::register)
    /// after calling this constructor.
    pub fn with_standard_definitions() -> Self {
        let mut registry = Self::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html,  0.5, 1.00, "pandoc"))
            .register(TransformDefinition::new(Format::Html,     Format::Pdf,   0.8, 0.85, "wkhtmltopdf"))
            .register(TransformDefinition::new(Format::Html,     Format::Docx,  0.6, 0.90, "pandoc"))
            .register(TransformDefinition::new(Format::Markdown, Format::Pdf,   1.5, 0.75, "pandoc"))
            .register(TransformDefinition::new(Format::Markdown, Format::Docx,  1.2, 0.80, "pandoc"))
            .register(TransformDefinition::new(Format::Markdown, Format::Epub,  1.0, 0.85, "pandoc"))
            .register(TransformDefinition::new(Format::Markdown, Format::Rst,   1.0, 0.90, "pandoc"))
            .register(TransformDefinition::new(Format::Markdown, Format::Latex, 1.0, 0.90, "pandoc"));
        registry
    }
}

impl Default for TransformDefinitionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── construction ──────────────────────────────────────────────────────────

    #[test]
    fn test_new_registry_has_no_definitions() {
        let registry = TransformDefinitionRegistry::new();
        assert!(registry.all_definitions().is_empty());
    }

    #[test]
    fn test_default_is_empty() {
        let registry = TransformDefinitionRegistry::default();
        assert!(registry.all_definitions().is_empty());
    }

    // ── register ─────────────────────────────────────────────────────────────

    #[test]
    fn test_register_single_definition() {
        let mut registry = TransformDefinitionRegistry::new();
        registry.register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"));
        assert_eq!(registry.all_definitions().len(), 1);
    }

    #[test]
    fn test_register_multiple_definitions() {
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"))
            .register(TransformDefinition::new(Format::Html, Format::Docx, 0.6, 0.90, "pandoc"));
        assert_eq!(registry.all_definitions().len(), 3);
    }

    #[test]
    fn test_register_duplicate_pair_is_allowed() {
        // Two competing definitions for the same format pair must both be stored.
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Pdf, 1.5, 0.75, "pandoc"))
            .register(TransformDefinition::new(Format::Markdown, Format::Pdf, 2.0, 0.95, "latex"));
        assert_eq!(registry.all_definitions().len(), 2);
    }

    #[test]
    fn test_register_preserves_order() {
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "first"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "second"));
        let defs = registry.all_definitions();
        assert_eq!(defs[0].label, "first");
        assert_eq!(defs[1].label, "second");
    }

    // ── definitions_from ─────────────────────────────────────────────────────

    #[test]
    fn test_definitions_from_returns_matching_source() {
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"))
            .register(TransformDefinition::new(Format::Markdown, Format::Pdf, 1.5, 0.75, "pandoc"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"));

        let from_md = registry.definitions_from(Format::Markdown);
        assert_eq!(from_md.len(), 2);
        assert!(from_md.iter().all(|d| d.from == Format::Markdown));
    }

    #[test]
    fn test_definitions_from_empty_for_unknown_format() {
        let registry = TransformDefinitionRegistry::new();
        assert!(registry.definitions_from(Format::Pdf).is_empty());
    }

    #[test]
    fn test_definitions_from_does_not_include_wrong_source() {
        let mut registry = TransformDefinitionRegistry::new();
        registry.register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "tool"));
        assert!(registry.definitions_from(Format::Markdown).is_empty());
    }

    // ── definitions_to ────────────────────────────────────────────────────────

    #[test]
    fn test_definitions_to_returns_matching_target() {
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"))
            .register(TransformDefinition::new(Format::Markdown, Format::Pdf, 1.5, 0.75, "pandoc"));

        let to_pdf = registry.definitions_to(Format::Pdf);
        assert_eq!(to_pdf.len(), 2);
        assert!(to_pdf.iter().all(|d| d.to == Format::Pdf));
    }

    #[test]
    fn test_definitions_to_empty_for_unknown_format() {
        let registry = TransformDefinitionRegistry::new();
        assert!(registry.definitions_to(Format::Epub).is_empty());
    }

    #[test]
    fn test_definitions_to_does_not_include_wrong_target() {
        let mut registry = TransformDefinitionRegistry::new();
        registry.register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "tool"));
        assert!(registry.definitions_to(Format::Pdf).is_empty());
    }

    // ── build_graph ───────────────────────────────────────────────────────────

    #[test]
    fn test_build_graph_from_empty_registry_is_empty() {
        let registry = TransformDefinitionRegistry::new();
        let graph = registry.build_graph();
        assert!(graph.transforms_from(Format::Markdown).is_empty());
    }

    #[test]
    fn test_build_graph_contains_registered_edges() {
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"));

        let graph = registry.build_graph();
        assert!(graph.has_transform(Format::Markdown, Format::Html));
        assert!(graph.has_transform(Format::Html, Format::Pdf));
    }

    #[test]
    fn test_build_graph_edge_count_matches_definitions() {
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"))
            .register(TransformDefinition::new(Format::Html, Format::Docx, 0.6, 0.90, "pandoc"));

        let graph = registry.build_graph();
        // 3 definitions → 3 edges in the graph
        assert_eq!(graph.transforms_from(Format::Markdown).len(), 1);
        assert_eq!(graph.transforms_from(Format::Html).len(), 2);
    }

    #[test]
    fn test_build_graph_pathfinding_works() {
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"));

        let graph = registry.build_graph();
        let path = graph.find_path(Format::Markdown, Format::Pdf);
        assert!(path.is_some(), "path from Markdown to Pdf must exist");
        let path = path.unwrap();
        assert_eq!(path.steps.len(), 2);
    }

    #[test]
    fn test_build_graph_dag_construction_works() {
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Markdown, Format::Html, 0.5, 1.0, "pandoc"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"))
            .register(TransformDefinition::new(Format::Html, Format::Docx, 0.6, 0.90, "pandoc"));

        let graph = registry.build_graph();
        let dag = graph
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .expect("all targets must be reachable");

        // Markdown→Html is shared: 3 unique edges total.
        assert_eq!(dag.edge_count(), 3);
        assert!(dag.contains_edge(Format::Markdown, Format::Html));
        assert!(dag.contains_edge(Format::Html, Format::Pdf));
        assert!(dag.contains_edge(Format::Html, Format::Docx));
    }

    // ── with_standard_definitions ─────────────────────────────────────────────

    #[test]
    fn test_standard_definitions_are_non_empty() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        assert!(!registry.all_definitions().is_empty());
    }

    #[test]
    fn test_standard_definitions_include_markdown_to_html() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        let defs = registry.definitions_from(Format::Markdown);
        assert!(
            defs.iter().any(|d| d.to == Format::Html),
            "standard definitions must include Markdown→Html"
        );
    }

    #[test]
    fn test_standard_definitions_include_html_to_pdf() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        let defs = registry.definitions_to(Format::Pdf);
        assert!(
            defs.iter().any(|d| d.from == Format::Html),
            "standard definitions must include Html→Pdf"
        );
    }

    #[test]
    fn test_standard_definitions_build_graph_connects_markdown_to_pdf() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        let graph = registry.build_graph();
        let path = graph.find_path(Format::Markdown, Format::Pdf);
        assert!(path.is_some(), "standard graph must connect Markdown to Pdf");
    }

    #[test]
    fn test_standard_definitions_build_graph_connects_markdown_to_docx() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        let graph = registry.build_graph();
        let path = graph.find_path(Format::Markdown, Format::Docx);
        assert!(path.is_some(), "standard graph must connect Markdown to Docx");
    }

    #[test]
    fn test_standard_definitions_build_graph_connects_markdown_to_epub() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        let graph = registry.build_graph();
        let path = graph.find_path(Format::Markdown, Format::Epub);
        assert!(path.is_some(), "standard graph must connect Markdown to Epub");
    }

    #[test]
    fn test_standard_definitions_build_graph_connects_markdown_to_latex() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        let graph = registry.build_graph();
        let path = graph.find_path(Format::Markdown, Format::Latex);
        assert!(path.is_some(), "standard graph must connect Markdown to Latex");
    }

    #[test]
    fn test_standard_definitions_build_graph_connects_markdown_to_rst() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        let graph = registry.build_graph();
        let path = graph.find_path(Format::Markdown, Format::Rst);
        assert!(path.is_some(), "standard graph must connect Markdown to Rst");
    }

    #[test]
    fn test_standard_definitions_supports_multi_target_dag() {
        let registry = TransformDefinitionRegistry::with_standard_definitions();
        let graph = registry.build_graph();
        let dag = graph
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx, Format::Epub])
            .expect("all targets reachable from standard definitions");
        assert!(dag.edge_count() > 0);
    }

    // ── extensibility (runtime registration after factory) ────────────────────

    #[test]
    fn test_custom_definition_added_after_standard() {
        let mut registry = TransformDefinitionRegistry::with_standard_definitions();
        let before = registry.all_definitions().len();
        registry.register(TransformDefinition::new(
            Format::Html,
            Format::Epub,
            0.7,
            0.88,
            "custom-tool",
        ));
        assert_eq!(registry.all_definitions().len(), before + 1);
    }

    #[test]
    fn test_custom_graph_without_standard_definitions() {
        // Callers can build a fully custom graph without any standard definitions.
        let mut registry = TransformDefinitionRegistry::new();
        registry
            .register(TransformDefinition::new(Format::Rst, Format::Html, 1.0, 0.9, "rst2html"))
            .register(TransformDefinition::new(Format::Html, Format::Pdf, 0.8, 0.85, "wkhtmltopdf"));

        let graph = registry.build_graph();
        let path = graph.find_path(Format::Rst, Format::Pdf);
        assert!(path.is_some(), "custom graph must connect Rst to Pdf via Html");
        assert_eq!(path.unwrap().steps.len(), 2);
    }
}
