use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use anyhow::{Context, Result};
use rayon::prelude::*;
use tracing::{debug, warn};

use super::{Format, MultiTargetDag, TransformEdge};
use crate::transforms::aggregation::AggregationTransform;
use crate::transforms::Transform;

/// Executes a [`MultiTargetDag`] in correct dependency order.
///
/// Single-input edges ([`InputKind::Single`](super::InputKind::Single)) are
/// dispatched to a registered [`Transform`]; collection edges
/// ([`InputKind::Collection`](super::InputKind::Collection)) are dispatched to
/// a registered [`AggregationTransform`].
///
/// Independent edges — those whose source format becomes available at the same
/// time — are grouped into *waves* and executed in parallel using Rayon.
///
/// # Registration
///
/// Before calling [`execute`](DagExecutor::execute), register a transform for
/// every edge that appears in the DAG you intend to execute:
///
/// * **Single-input edges** – call
///   [`register_single`](DagExecutor::register_single) with an
///   `Arc`-wrapped [`Transform`] implementation that is [`Send`] + [`Sync`].
///
/// * **Collection edges** – call
///   [`register_aggregation`](DagExecutor::register_aggregation) with an
///   `Arc`-wrapped [`AggregationTransform`].
///
/// # Execution
///
/// [`execute`](DagExecutor::execute) starts with one piece of initial content
/// for the source format and repeatedly processes waves until no more edges can
/// run.  Each wave contains all edges whose source format has already been
/// produced.  Edges within the same wave are executed in parallel.
///
/// The returned map contains the produced content for every format that was
/// reached during execution, including the initial source format.
///
/// # Errors
///
/// * A transform for a DAG edge has not been registered.
/// * A registered transform returns an error during execution.
/// * Temporary file I/O fails when processing a collection edge.
///
/// # Example
///
/// ```rust
/// use renderflow::graph::{DagExecutor, Format, TransformEdge, TransformGraph};
/// use renderflow::transforms::Transform;
/// use anyhow::Result;
/// use std::sync::Arc;
///
/// struct UpperTransform;
/// impl Transform for UpperTransform {
///     fn apply(&self, input: String) -> Result<String> {
///         Ok(input.to_uppercase())
///     }
/// }
///
/// let mut graph = TransformGraph::new();
/// graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
/// let dag = graph
///     .build_multi_target_dag(Format::Markdown, &[Format::Html])
///     .unwrap();
///
/// let mut executor = DagExecutor::new();
/// executor.register_single(
///     Format::Markdown,
///     Format::Html,
///     Arc::new(UpperTransform),
/// );
///
/// let results = executor
///     .execute(&dag, Format::Markdown, "hello".to_string())
///     .unwrap();
/// assert_eq!(results[&Format::Html], "HELLO");
/// ```
pub struct DagExecutor {
    /// Single-input transforms keyed by `(from, to)` format pair.
    single_transforms: HashMap<(Format, Format), Arc<dyn Transform + Send + Sync>>,
    /// Collection-input transforms keyed by `(from, to)` format pair.
    aggregation_transforms: HashMap<(Format, Format), Arc<dyn AggregationTransform>>,
}

impl DagExecutor {
    /// Create an empty executor with no transforms registered.
    pub fn new() -> Self {
        Self {
            single_transforms: HashMap::new(),
            aggregation_transforms: HashMap::new(),
        }
    }

    /// Register a single-input transform for the `from → to` edge.
    ///
    /// If a transform is already registered for the same `(from, to)` pair it
    /// is replaced by the new one.
    pub fn register_single(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn Transform + Send + Sync>,
    ) -> &mut Self {
        self.single_transforms.insert((from, to), transform);
        self
    }

    /// Register a collection-input transform for the `from → to` edge.
    ///
    /// If a transform is already registered for the same `(from, to)` pair it
    /// is replaced by the new one.
    pub fn register_aggregation(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn AggregationTransform>,
    ) -> &mut Self {
        self.aggregation_transforms.insert((from, to), transform);
        self
    }

    /// Execute the DAG starting from `initial_content` for `source_format`.
    ///
    /// Returns a [`HashMap`] mapping every reachable format (including
    /// `source_format`) to the string content produced for it.
    ///
    /// Edges are processed in topological *waves*.  In each wave all edges
    /// whose source format is already available are collected and executed in
    /// parallel using Rayon.  The wave's outputs are added to the available
    /// set before the next wave begins.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered if any transform fails or if a
    /// required transform has not been registered.
    pub fn execute(
        &self,
        dag: &MultiTargetDag,
        source_format: Format,
        initial_content: String,
    ) -> Result<HashMap<Format, String>> {
        // Track the content produced for every format encountered so far.
        let mut available: HashMap<Format, String> = HashMap::new();
        available.insert(source_format, initial_content);

        let mut remaining: Vec<&TransformEdge> = dag.execution_order();

        loop {
            // Partition: edges ready to execute vs. those still waiting.
            let (wave, next_remaining): (Vec<_>, Vec<_>) =
                remaining.into_iter().partition(|e| available.contains_key(&e.from));

            if wave.is_empty() {
                if !next_remaining.is_empty() {
                    warn!(
                        unreachable = next_remaining.len(),
                        "Some DAG edges could not be executed because their \
                         source format was never produced"
                    );
                }
                break;
            }

            debug!(wave_size = wave.len(), "Executing DAG wave");

            // Execute all edges in the current wave in parallel.
            let wave_results: Result<Vec<(Format, String)>> = wave
                .into_par_iter()
                .map(|edge| self.execute_edge(edge, &available))
                .collect();

            // Propagate the first error; otherwise record the new outputs.
            for (format, content) in wave_results? {
                available.insert(format, content);
            }

            remaining = next_remaining;
        }

        Ok(available)
    }

    // ── private helpers ───────────────────────────────────────────────────────

    /// Dispatch a single edge to the appropriate executor.
    fn execute_edge(
        &self,
        edge: &TransformEdge,
        available: &HashMap<Format, String>,
    ) -> Result<(Format, String)> {
        let input = available
            .get(&edge.from)
            .expect("source format must be in available set when execute_edge is called")
            .clone();

        if edge.input_kind.is_single() {
            self.execute_single_edge(edge, input)
        } else {
            self.execute_collection_edge(edge, &[input])
        }
    }

    /// Execute a single-input edge by delegating to the registered transform.
    fn execute_single_edge(
        &self,
        edge: &TransformEdge,
        input: String,
    ) -> Result<(Format, String)> {
        let transform = self
            .single_transforms
            .get(&(edge.from, edge.to))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No single transform registered for {:?} → {:?}",
                    edge.from,
                    edge.to
                )
            })?;

        debug!(from = ?edge.from, to = ?edge.to, "Executing single transform");

        let output = transform.apply(input).with_context(|| {
            format!("Single transform {:?} → {:?} failed", edge.from, edge.to)
        })?;

        debug!(from = ?edge.from, to = ?edge.to, "Single transform completed");
        Ok((edge.to, output))
    }

    /// Execute a collection-input edge.
    ///
    /// Each input string is written to a temporary file so that
    /// [`AggregationTransform::aggregate`] receives file paths, as its
    /// contract requires.  The aggregated output is read back as a `String`.
    fn execute_collection_edge(
        &self,
        edge: &TransformEdge,
        inputs: &[String],
    ) -> Result<(Format, String)> {
        let transform = self
            .aggregation_transforms
            .get(&(edge.from, edge.to))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No aggregation transform registered for {:?} → {:?}",
                    edge.from,
                    edge.to
                )
            })?;

        debug!(
            from = ?edge.from,
            to = ?edge.to,
            inputs = inputs.len(),
            "Executing collection transform"
        );

        // Write each input to a temporary file so the aggregation transform
        // receives proper file paths.
        let temp_inputs: Vec<tempfile::NamedTempFile> = inputs
            .iter()
            .map(|content| {
                let mut f = tempfile::NamedTempFile::new()
                    .context("Failed to create temp file for aggregation input")?;
                f.write_all(content.as_bytes())
                    .context("Failed to write to aggregation input temp file")?;
                Ok(f)
            })
            .collect::<Result<Vec<_>>>()?;

        let input_paths: Vec<&str> = temp_inputs
            .iter()
            .map(|f| {
                f.path()
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Aggregation input temp file path is not valid UTF-8"))
            })
            .collect::<Result<Vec<_>>>()?;

        // Create a temp file to receive the aggregated output.
        let temp_output = tempfile::NamedTempFile::new()
            .context("Failed to create temp file for aggregation output")?;
        let output_path = temp_output
            .path()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Aggregation output temp file path is not valid UTF-8"))?;

        transform.aggregate(&input_paths, output_path).with_context(|| {
            format!(
                "Collection transform {:?} → {:?} failed",
                edge.from, edge.to
            )
        })?;

        let output = std::fs::read_to_string(temp_output.path()).with_context(|| {
            format!(
                "Failed to read aggregation output for {:?} → {:?}",
                edge.from, edge.to
            )
        })?;

        debug!(from = ?edge.from, to = ?edge.to, "Collection transform completed");
        Ok((edge.to, output))
    }
}

impl Default for DagExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Format, InputKind, TransformEdge, TransformGraph};
    use anyhow::{bail, Result};
    use std::sync::Arc;

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Build a small graph:  Markdown → Html → {Pdf, Docx}
    fn build_graph() -> TransformGraph {
        let mut g = TransformGraph::new();
        g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
        g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
        g.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));
        g
    }

    /// A [`Transform`] that appends a fixed string to its input.
    struct AppendTransform(String);

    impl Transform for AppendTransform {
        fn name(&self) -> &str {
            "AppendTransform"
        }

        fn apply(&self, input: String) -> Result<String> {
            Ok(format!("{}{}", input, self.0))
        }
    }

    /// A [`Transform`] that always returns an error.
    struct FailingTransform;

    impl Transform for FailingTransform {
        fn name(&self) -> &str {
            "FailingTransform"
        }

        fn apply(&self, _input: String) -> Result<String> {
            bail!("intentional transform failure")
        }
    }

    /// An [`AggregationTransform`] that writes all inputs joined by commas.
    struct JoinAggregation;

    impl AggregationTransform for JoinAggregation {
        fn name(&self) -> &str {
            "join"
        }

        fn aggregate(&self, inputs: &[&str], output_path: &str) -> Result<()> {
            // Read each input file and join the contents.
            let parts: Result<Vec<String>> = inputs
                .iter()
                .map(|p| std::fs::read_to_string(p).context("Failed to read aggregation input"))
                .collect();
            std::fs::write(output_path, parts?.join(","))?;
            Ok(())
        }
    }

    fn arc_single<T: Transform + Send + Sync + 'static>(t: T) -> Arc<dyn Transform + Send + Sync> {
        Arc::new(t)
    }

    fn arc_agg<T: AggregationTransform + 'static>(t: T) -> Arc<dyn AggregationTransform> {
        Arc::new(t)
    }

    // ── basic single transform ────────────────────────────────────────────────

    #[test]
    fn test_execute_single_transform_produces_output() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[Format::Html])
            .unwrap();

        let mut executor = DagExecutor::new();
        executor.register_single(
            Format::Markdown,
            Format::Html,
            arc_single(AppendTransform("→html".to_string())),
        );

        let results = executor
            .execute(&dag, Format::Markdown, "content".to_string())
            .unwrap();

        assert_eq!(results[&Format::Html], "content→html");
    }

    #[test]
    fn test_execute_includes_source_format_in_results() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[Format::Html])
            .unwrap();

        let mut executor = DagExecutor::new();
        executor.register_single(
            Format::Markdown,
            Format::Html,
            arc_single(AppendTransform("→html".to_string())),
        );

        let results = executor
            .execute(&dag, Format::Markdown, "source".to_string())
            .unwrap();

        assert!(
            results.contains_key(&Format::Markdown),
            "source format must be present in results"
        );
        assert_eq!(results[&Format::Markdown], "source");
    }

    // ── multi-level DAG (dependency order) ────────────────────────────────────

    #[test]
    fn test_execute_multi_level_respects_dependency_order() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
            .unwrap();

        let mut executor = DagExecutor::new();
        executor
            .register_single(
                Format::Markdown,
                Format::Html,
                arc_single(AppendTransform("→html".to_string())),
            )
            .register_single(
                Format::Html,
                Format::Pdf,
                arc_single(AppendTransform("→pdf".to_string())),
            );

        let results = executor
            .execute(&dag, Format::Markdown, "start".to_string())
            .unwrap();

        // Html must have been produced from Markdown before Pdf was produced from Html.
        assert_eq!(results[&Format::Html], "start→html");
        assert_eq!(results[&Format::Pdf], "start→html→pdf");
    }

    // ── parallel wave (independent edges) ────────────────────────────────────

    #[test]
    fn test_execute_independent_edges_both_produced() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx])
            .unwrap();

        let mut executor = DagExecutor::new();
        executor
            .register_single(
                Format::Markdown,
                Format::Html,
                arc_single(AppendTransform("→html".to_string())),
            )
            .register_single(
                Format::Html,
                Format::Pdf,
                arc_single(AppendTransform("→pdf".to_string())),
            )
            .register_single(
                Format::Html,
                Format::Docx,
                arc_single(AppendTransform("→docx".to_string())),
            );

        let results = executor
            .execute(&dag, Format::Markdown, "start".to_string())
            .unwrap();

        assert_eq!(results[&Format::Html], "start→html");
        assert_eq!(results[&Format::Pdf], "start→html→pdf");
        assert_eq!(results[&Format::Docx], "start→html→docx");
    }

    // ── empty DAG ─────────────────────────────────────────────────────────────

    #[test]
    fn test_execute_empty_dag_returns_only_source() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[])
            .unwrap();

        let executor = DagExecutor::new();
        let results = executor
            .execute(&dag, Format::Markdown, "content".to_string())
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[&Format::Markdown], "content");
    }

    // ── missing transform → error ─────────────────────────────────────────────

    #[test]
    fn test_execute_missing_single_transform_returns_error() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[Format::Html])
            .unwrap();

        let executor = DagExecutor::new(); // no transforms registered

        let err = executor
            .execute(&dag, Format::Markdown, "content".to_string())
            .unwrap_err();

        assert!(
            err.to_string().contains("No single transform registered"),
            "error message: {}",
            err
        );
    }

    #[test]
    fn test_execute_missing_aggregation_transform_returns_error() {
        let mut g = TransformGraph::new();
        g.add_transform(TransformEdge::with_input_kind(
            Format::Markdown,
            Format::Epub,
            1.0,
            0.85,
            InputKind::Collection,
        ));
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Epub])
            .unwrap();

        let executor = DagExecutor::new(); // no aggregation registered

        let err = executor
            .execute(&dag, Format::Markdown, "content".to_string())
            .unwrap_err();

        assert!(
            err.to_string().contains("No aggregation transform registered"),
            "error message: {}",
            err
        );
    }

    // ── failing transform → error propagated ─────────────────────────────────

    #[test]
    fn test_execute_failing_transform_returns_error() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[Format::Html])
            .unwrap();

        let mut executor = DagExecutor::new();
        executor.register_single(
            Format::Markdown,
            Format::Html,
            arc_single(FailingTransform),
        );

        let err = executor
            .execute(&dag, Format::Markdown, "content".to_string())
            .unwrap_err();

        // The full error chain (anyhow's alternate format) must contain the
        // root cause message emitted by FailingTransform.
        let chain = format!("{:#}", err);
        assert!(
            chain.contains("intentional transform failure"),
            "error chain: {}",
            chain
        );
    }

    #[test]
    fn test_execute_error_context_identifies_edge() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[Format::Html])
            .unwrap();

        let mut executor = DagExecutor::new();
        executor.register_single(
            Format::Markdown,
            Format::Html,
            arc_single(FailingTransform),
        );

        let err = executor
            .execute(&dag, Format::Markdown, "content".to_string())
            .unwrap_err();

        // The error chain must identify the failing edge.
        let chain = format!("{:#}", err);
        assert!(
            chain.contains("Markdown") || chain.contains("Html"),
            "error chain should identify the edge, got: {chain}"
        );
    }

    // ── collection edge ───────────────────────────────────────────────────────

    #[test]
    fn test_execute_collection_transform_produces_output() {
        let mut g = TransformGraph::new();
        g.add_transform(TransformEdge::with_input_kind(
            Format::Markdown,
            Format::Epub,
            1.0,
            0.85,
            InputKind::Collection,
        ));
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Epub])
            .unwrap();

        let mut executor = DagExecutor::new();
        executor.register_aggregation(
            Format::Markdown,
            Format::Epub,
            arc_agg(JoinAggregation),
        );

        let results = executor
            .execute(&dag, Format::Markdown, "page content".to_string())
            .unwrap();

        assert!(results.contains_key(&Format::Epub));
        assert_eq!(results[&Format::Epub], "page content");
    }

    // ── register_single replaces existing ─────────────────────────────────────

    #[test]
    fn test_register_single_replaces_previous() {
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[Format::Html])
            .unwrap();

        let mut executor = DagExecutor::new();
        executor
            .register_single(
                Format::Markdown,
                Format::Html,
                arc_single(AppendTransform("→old".to_string())),
            )
            .register_single(
                Format::Markdown,
                Format::Html,
                arc_single(AppendTransform("→new".to_string())),
            );

        let results = executor
            .execute(&dag, Format::Markdown, "x".to_string())
            .unwrap();

        assert_eq!(results[&Format::Html], "x→new", "second registration must win");
    }

    // ── register_aggregation replaces existing ────────────────────────────────

    #[test]
    fn test_register_aggregation_replaces_previous() {
        let mut g = TransformGraph::new();
        g.add_transform(TransformEdge::with_input_kind(
            Format::Markdown,
            Format::Epub,
            1.0,
            0.85,
            InputKind::Collection,
        ));
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Epub])
            .unwrap();

        struct PrefixAggregation(&'static str);
        impl AggregationTransform for PrefixAggregation {
            fn name(&self) -> &str {
                self.0
            }
            fn aggregate(&self, _inputs: &[&str], output_path: &str) -> Result<()> {
                std::fs::write(output_path, self.0)?;
                Ok(())
            }
        }

        let mut executor = DagExecutor::new();
        executor
            .register_aggregation(
                Format::Markdown,
                Format::Epub,
                arc_agg(PrefixAggregation("first")),
            )
            .register_aggregation(
                Format::Markdown,
                Format::Epub,
                arc_agg(PrefixAggregation("second")),
            );

        let results = executor
            .execute(&dag, Format::Markdown, "input".to_string())
            .unwrap();

        assert_eq!(results[&Format::Epub], "second", "second registration must win");
    }

    // ── default ───────────────────────────────────────────────────────────────

    #[test]
    fn test_default_is_empty() {
        let executor = DagExecutor::default();
        let dag = build_graph()
            .build_multi_target_dag(Format::Markdown, &[])
            .unwrap();

        let results = executor
            .execute(&dag, Format::Markdown, "content".to_string())
            .unwrap();

        // Default executor, empty DAG: just the source.
        assert_eq!(results.len(), 1);
    }
}
