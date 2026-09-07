use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rayon::prelude::*;
use tracing::{debug, warn};

use super::{Format, MultiTargetDag, TransformEdge};
use crate::artifact::{
    compute_artifact_node_hash, load_artifact_cache, save_artifact_cache, Artifact, ArtifactCache,
    ArtifactCollection, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore, ArtifactTransform,
    TextTransformAdapter,
};
use crate::transforms::aggregation::AggregationTransform;
use crate::transforms::Transform;

/// Executes a [`MultiTargetDag`] using file-backed, binary-safe artifacts.
///
/// The artifact-native executor is the canonical substrate. The legacy
/// [`execute`](Self::execute) method remains as a UTF-8 compatibility wrapper
/// for existing callers and transforms.
pub struct DagExecutor {
    /// Single-input artifact transforms keyed by `(from, to)` format pair.
    single_transforms: HashMap<(Format, Format), Arc<dyn ArtifactTransform>>,
    /// Collection-input transforms keyed by `(from, to)` format pair.
    aggregation_transforms: HashMap<(Format, Format), Arc<dyn AggregationTransform>>,
    /// Optional artifact-native DAG cache path.
    cache_path: Option<PathBuf>,
    /// Selected-provider fingerprint used to reject incompatible cache entries.
    toolchain_fingerprint: Option<String>,
    /// Optional per-execution parallelism bound from the canonical execution policy.
    max_parallel: Option<usize>,
}

impl DagExecutor {
    /// Create an empty executor with no transforms registered.
    pub fn new() -> Self {
        Self {
            single_transforms: HashMap::new(),
            aggregation_transforms: HashMap::new(),
            cache_path: None,
            toolchain_fingerprint: None,
            max_parallel: None,
        }
    }

    /// Configure the artifact-native on-disk DAG cache.
    ///
    /// Legacy string-cache files are intentionally treated as cache misses and
    /// replaced with the artifact-cache schema on the next successful save.
    pub fn with_cache(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_path = Some(path.into());
        self
    }

    /// Attach a selected-provider fingerprint to cache compatibility.
    pub fn with_toolchain_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.toolchain_fingerprint = Some(fingerprint.into());
        self
    }

    /// Bound parallel transform execution for this executor instance.
    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = Some(max_parallel.max(1));
        self
    }

    /// Register an existing UTF-8 text transform through the compatibility adapter.
    pub fn register_single(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn Transform + Send + Sync>,
    ) -> &mut Self {
        self.single_transforms
            .insert((from, to), Arc::new(TextTransformAdapter::new(transform)));
        self
    }

    /// Register a text transform with configuration-aware cache identity.
    ///
    /// Embedders that know configuration affecting transform output can use this
    /// seam while the versioned Transform v2 contract is developed in #357.
    pub fn register_single_with_identity(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn Transform + Send + Sync>,
        cache_identity: impl Into<String>,
    ) -> &mut Self {
        self.single_transforms.insert(
            (from, to),
            Arc::new(TextTransformAdapter::with_identity(
                transform,
                cache_identity,
            )),
        );
        self
    }

    /// Register an artifact-native transform that may consume arbitrary bytes.
    pub fn register_artifact(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn ArtifactTransform>,
    ) -> &mut Self {
        self.single_transforms.insert((from, to), transform);
        self
    }

    /// Register a collection-input transform for the `from → to` edge.
    pub fn register_aggregation(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn AggregationTransform>,
    ) -> &mut Self {
        self.aggregation_transforms.insert((from, to), transform);
        self
    }

    /// Execute a DAG using the legacy UTF-8 `String` API.
    ///
    /// Existing text transforms continue to work unchanged, but callers that
    /// expect binary output must use [`execute_artifact`](Self::execute_artifact)
    /// or [`execute_artifacts`](Self::execute_artifacts).
    pub fn execute(
        &self,
        dag: &MultiTargetDag,
        source_format: Format,
        initial_content: String,
    ) -> Result<HashMap<Format, String>> {
        let temporary_directory = if self.cache_path.is_none() {
            Some(tempfile::tempdir().context("Failed to create legacy DAG work directory")?)
        } else {
            None
        };

        let store_root = if let Some(cache_path) = &self.cache_path {
            Self::legacy_store_path(cache_path)
        } else {
            temporary_directory
                .as_ref()
                .expect("temporary directory exists when no cache is configured")
                .path()
                .join("artifacts")
        };
        let store = ArtifactStore::new(store_root)?;
        let source = store.put_bytes(
            initial_content.as_bytes(),
            ArtifactDescriptor::for_format(source_format, ArtifactStorageClass::Source),
        )?;
        let artifacts = self.execute_artifact(dag, source_format, source, &store)?;

        artifacts
            .into_iter()
            .map(|(format, artifact)| {
                let text = store.read_text(&artifact).with_context(|| {
                    format!(
                        "Legacy String DAG API cannot return binary '{}' output; use execute_artifact",
                        format
                    )
                })?;
                Ok((format, text))
            })
            .collect()
    }

    /// Execute a DAG from one binary-safe source artifact.
    pub fn execute_artifact(
        &self,
        dag: &MultiTargetDag,
        source_format: Format,
        initial_artifact: Artifact,
        store: &ArtifactStore,
    ) -> Result<HashMap<Format, Artifact>> {
        let collections = self.execute_artifacts(
            dag,
            source_format,
            ArtifactCollection::one(initial_artifact),
            store,
        )?;

        collections
            .into_iter()
            .map(|(format, collection)| {
                let artifact = collection.into_one().with_context(|| {
                    format!(
                        "Format '{}' produced a collection where a single artifact was expected",
                        format
                    )
                })?;
                Ok((format, artifact))
            })
            .collect()
    }

    /// Execute a DAG from an ordered source artifact collection.
    ///
    /// Single-input edges require exactly one artifact. Collection edges receive
    /// every artifact in declared order as file-backed paths and may therefore
    /// aggregate binary inputs without converting them to text.
    pub fn execute_artifacts(
        &self,
        dag: &MultiTargetDag,
        source_format: Format,
        initial_artifacts: ArtifactCollection,
        store: &ArtifactStore,
    ) -> Result<HashMap<Format, ArtifactCollection>> {
        if initial_artifacts.is_empty() {
            anyhow::bail!("Artifact DAG execution requires at least one source artifact");
        }

        let cache: Option<Mutex<ArtifactCache>> = self
            .cache_path
            .as_deref()
            .map(|path| Mutex::new(load_artifact_cache(path)));

        let thread_pool = self
            .max_parallel
            .map(|threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .context("Failed to create bounded DAG execution thread pool")
            })
            .transpose()?;

        let mut available: HashMap<Format, ArtifactCollection> = HashMap::new();
        available.insert(source_format, initial_artifacts);
        let mut remaining: Vec<&TransformEdge> = dag.execution_order();

        loop {
            let (wave, next_remaining): (Vec<_>, Vec<_>) = remaining
                .into_iter()
                .partition(|edge| available.contains_key(&edge.from));

            if wave.is_empty() {
                if !next_remaining.is_empty() {
                    warn!(
                        unreachable = next_remaining.len(),
                        "Some DAG edges could not execute because their source format was never produced"
                    );
                }
                break;
            }

            debug!(wave_size = wave.len(), "Executing artifact DAG wave");
            let execute_wave = || {
                wave.into_par_iter()
                    .map(|edge| self.execute_edge(edge, &available, store, cache.as_ref()))
                    .collect::<Result<Vec<(Format, ArtifactCollection)>>>()
            };
            let wave_results = if let Some(pool) = &thread_pool {
                pool.install(execute_wave)
            } else {
                execute_wave()
            };

            for (format, artifacts) in wave_results? {
                available.insert(format, artifacts);
            }
            remaining = next_remaining;
        }

        if let (Some(cache_path), Some(cache_mutex)) = (&self.cache_path, cache) {
            match cache_mutex.into_inner() {
                Ok(cache) => {
                    if let Err(error) = save_artifact_cache(&cache, cache_path) {
                        warn!(
                            error = %error,
                            path = %cache_path.display(),
                            "Failed to save artifact DAG cache"
                        );
                    }
                }
                Err(error) => {
                    warn!(
                        error = %error,
                        "Artifact DAG cache mutex was poisoned; cache not saved"
                    );
                }
            }
        }

        Ok(available)
    }

    fn execute_edge(
        &self,
        edge: &TransformEdge,
        available: &HashMap<Format, ArtifactCollection>,
        store: &ArtifactStore,
        cache: Option<&Mutex<ArtifactCache>>,
    ) -> Result<(Format, ArtifactCollection)> {
        let inputs = available.get(&edge.from).ok_or_else(|| {
            anyhow::anyhow!(
                "Source format '{}' was not available for DAG edge",
                edge.from
            )
        })?;

        if edge.input_kind.is_single() {
            let input = inputs.clone().into_one().with_context(|| {
                format!(
                    "Single transform {:?} → {:?} requires exactly one artifact",
                    edge.from, edge.to
                )
            })?;
            let output = self.execute_single_edge(edge, &input, store, cache)?;
            Ok((edge.to, ArtifactCollection::one(output)))
        } else {
            let output = self.execute_collection_edge(edge, inputs, store)?;
            Ok((edge.to, ArtifactCollection::one(output)))
        }
    }

    fn execute_single_edge(
        &self,
        edge: &TransformEdge,
        input: &Artifact,
        store: &ArtifactStore,
        cache: Option<&Mutex<ArtifactCache>>,
    ) -> Result<Artifact> {
        let transform = self
            .single_transforms
            .get(&(edge.from, edge.to))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No artifact transform registered for {:?} → {:?}",
                    edge.from,
                    edge.to
                )
            })?;
        let mut cache_identity = transform.cache_identity();
        if let Some(fingerprint) = &self.toolchain_fingerprint {
            cache_identity.push_str("\0toolchain=");
            cache_identity.push_str(fingerprint);
        }
        let cache_key = compute_artifact_node_hash(input, edge.from, edge.to, &cache_identity);

        if let Some(cache_mutex) = cache {
            if let Ok(guard) = cache_mutex.lock() {
                if let Some(cached) = guard.get(&cache_key) {
                    if store.contains(cached) {
                        debug!(
                            from = ?edge.from,
                            to = ?edge.to,
                            artifact = %cached.id(),
                            "Artifact cache hit; skipping transform"
                        );
                        return Ok(cached
                            .clone()
                            .with_storage_class(ArtifactStorageClass::Cached));
                    }
                }
            }
        }

        debug!(
            from = ?edge.from,
            to = ?edge.to,
            transform = %transform.name(),
            "Executing artifact transform"
        );
        let output = transform.apply(input, edge.to, store).with_context(|| {
            format!(
                "Artifact transform {:?} → {:?} ({}) failed",
                edge.from,
                edge.to,
                transform.name()
            )
        })?;
        self.validate_output_format(edge, &output)?;

        if let Some(cache_mutex) = cache {
            if let Ok(mut guard) = cache_mutex.lock() {
                guard.insert(cache_key, output.clone());
            }
        }
        Ok(output)
    }

    fn execute_collection_edge(
        &self,
        edge: &TransformEdge,
        inputs: &ArtifactCollection,
        store: &ArtifactStore,
    ) -> Result<Artifact> {
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

        let input_paths: Vec<PathBuf> = inputs
            .iter()
            .map(|artifact| store.payload_path(artifact))
            .collect::<Result<Vec<_>>>()?;
        let input_path_strings: Vec<&str> = input_paths
            .iter()
            .map(|path| {
                path.to_str().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Artifact-store path '{}' is not valid UTF-8",
                        path.display()
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let suffix = format!(".{}", edge.to);
        let temporary_output = tempfile::Builder::new()
            .prefix("aggregate-")
            .suffix(&suffix)
            .tempfile_in(store.temporary_directory())
            .context("Failed to create aggregation output temporary file")?
            .into_temp_path();
        let output_path = temporary_output
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Aggregation output path is not valid UTF-8"))?;

        debug!(
            from = ?edge.from,
            to = ?edge.to,
            transform = %transform.name(),
            inputs = inputs.len(),
            "Executing artifact collection transform"
        );
        transform
            .aggregate(&input_path_strings, output_path)
            .with_context(|| {
                format!(
                    "Collection transform {:?} → {:?} ({}) failed",
                    edge.from,
                    edge.to,
                    transform.name()
                )
            })?;

        let sources = inputs.iter().map(|artifact| artifact.id().clone());
        let output = store.import_path(
            &temporary_output,
            ArtifactDescriptor::for_format(edge.to, ArtifactStorageClass::Intermediate)
                .with_sources(sources)
                .with_metadata("renderflow.transform", transform.name()),
        )?;
        self.validate_output_format(edge, &output)?;
        Ok(output)
    }

    fn validate_output_format(&self, edge: &TransformEdge, output: &Artifact) -> Result<()> {
        if output.format().as_str() != edge.to.to_string() {
            anyhow::bail!(
                "Transform {:?} → {:?} returned artifact format '{}'",
                edge.from,
                edge.to,
                output.format()
            );
        }
        Ok(())
    }

    fn legacy_store_path(cache_path: &Path) -> PathBuf {
        let parent = cache_path
            .parent()
            .filter(|candidate| !candidate.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let stem = cache_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("dag-cache");
        parent.join(format!(".{}-artifacts", stem))
    }
}

impl Default for DagExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::{bail, Result};

    use super::*;
    use crate::graph::{InputKind, TransformGraph};

    struct AppendTransform(&'static str);

    impl Transform for AppendTransform {
        fn name(&self) -> &str {
            "append"
        }

        fn apply(&self, input: String) -> Result<String> {
            Ok(format!("{}{}", input, self.0))
        }
    }

    struct BinaryCopyTransform;

    impl ArtifactTransform for BinaryCopyTransform {
        fn name(&self) -> &str {
            "binary-copy"
        }

        fn apply(
            &self,
            input: &Artifact,
            output_format: Format,
            store: &ArtifactStore,
        ) -> Result<Artifact> {
            let mut reader = store.open(input)?;
            store.put_reader(
                &mut reader,
                ArtifactDescriptor::for_format(output_format, ArtifactStorageClass::Intermediate)
                    .with_source(input.id().clone()),
            )
        }
    }

    struct CountingBinaryTransform {
        executions: Arc<AtomicUsize>,
    }

    impl ArtifactTransform for CountingBinaryTransform {
        fn name(&self) -> &str {
            "counting-binary"
        }

        fn cache_identity(&self) -> String {
            "counting-binary:v1".to_string()
        }

        fn apply(
            &self,
            input: &Artifact,
            output_format: Format,
            store: &ArtifactStore,
        ) -> Result<Artifact> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            let mut reader = store.open(input)?;
            store.put_reader(
                &mut reader,
                ArtifactDescriptor::for_format(output_format, ArtifactStorageClass::Intermediate)
                    .with_source(input.id().clone()),
            )
        }
    }

    struct OrderedJoinAggregation;

    impl AggregationTransform for OrderedJoinAggregation {
        fn name(&self) -> &str {
            "ordered-join"
        }

        fn aggregate(&self, inputs: &[&str], output_path: &str) -> Result<()> {
            let mut output = std::fs::File::create(output_path)?;
            for path in inputs {
                output.write_all(&std::fs::read(path)?)?;
            }
            Ok(())
        }
    }

    struct FailingAggregation;

    impl AggregationTransform for FailingAggregation {
        fn name(&self) -> &str {
            "failing-aggregation"
        }

        fn aggregate(&self, _inputs: &[&str], output_path: &str) -> Result<()> {
            std::fs::write(output_path, b"partial")?;
            bail!("intentional failure")
        }
    }

    fn one_edge(from: Format, to: Format, input_kind: InputKind) -> MultiTargetDag {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::with_input_kind(
            from, to, 1.0, 1.0, input_kind,
        ));
        graph
            .build_multi_target_dag(from, &[to])
            .expect("edge must be reachable")
    }

    #[test]
    fn legacy_text_api_runs_through_artifact_adapter() {
        let dag = one_edge(Format::Markdown, Format::Html, InputKind::Single);
        let mut executor = DagExecutor::new();
        executor.register_single(
            Format::Markdown,
            Format::Html,
            Arc::new(AppendTransform("!")),
        );
        let results = executor
            .execute(&dag, Format::Markdown, "hello".to_string())
            .unwrap();
        assert_eq!(results[&Format::Html], "hello!");
    }

    #[test]
    fn binary_artifact_traverses_graph_without_utf8_conversion() {
        let dag = one_edge(Format::Png, Format::Webp, InputKind::Single);
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let input_bytes = [0_u8, 159, 255, 1, 2, 3];
        let source = store
            .put_bytes(
                &input_bytes,
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        let mut executor = DagExecutor::new();
        executor.register_artifact(Format::Png, Format::Webp, Arc::new(BinaryCopyTransform));

        let results = executor
            .execute_artifact(&dag, Format::Png, source.clone(), &store)
            .unwrap();
        let output = &results[&Format::Webp];
        assert_eq!(store.read_bytes(output).unwrap(), input_bytes);
        assert_eq!(output.sources(), &[source.id().clone()]);
        assert_ne!(output.id(), source.id());
    }

    #[test]
    fn ordered_collections_are_first_class_aggregation_inputs() {
        let dag = one_edge(Format::Png, Format::Pdf, InputKind::Collection);
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let first = store
            .put_bytes(
                b"page-one|",
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        let second = store
            .put_bytes(
                b"page-two",
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        let mut executor = DagExecutor::new();
        executor.register_aggregation(Format::Png, Format::Pdf, Arc::new(OrderedJoinAggregation));

        let results = executor
            .execute_artifacts(
                &dag,
                Format::Png,
                ArtifactCollection::new(vec![first.clone(), second.clone()]),
                &store,
            )
            .unwrap();
        let output = results[&Format::Pdf].clone().into_one().unwrap();
        assert_eq!(store.read_bytes(&output).unwrap(), b"page-one|page-two");
        assert_eq!(output.sources(), &[first.id().clone(), second.id().clone()]);
    }

    #[test]
    fn artifact_cache_reuses_stored_payload_by_artifact_identity() {
        let dag = one_edge(Format::Png, Format::Webp, InputKind::Single);
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let cache_path = directory.path().join("dag-cache.json");
        let source = store
            .put_bytes(
                &[0, 255, 4, 5],
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        let executions = Arc::new(AtomicUsize::new(0));
        let mut executor = DagExecutor::new().with_cache(&cache_path);
        executor.register_artifact(
            Format::Png,
            Format::Webp,
            Arc::new(CountingBinaryTransform {
                executions: Arc::clone(&executions),
            }),
        );

        executor
            .execute_artifact(&dag, Format::Png, source.clone(), &store)
            .unwrap();
        executor
            .execute_artifact(&dag, Format::Png, source, &store)
            .unwrap();
        assert_eq!(executions.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn failed_collection_transform_does_not_publish_partial_artifact() {
        let dag = one_edge(Format::Png, Format::Pdf, InputKind::Collection);
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let source = store
            .put_bytes(
                b"page",
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        let mut executor = DagExecutor::new();
        executor.register_aggregation(Format::Png, Format::Pdf, Arc::new(FailingAggregation));

        let before = count_artifact_objects(&store);
        let result =
            executor.execute_artifacts(&dag, Format::Png, ArtifactCollection::one(source), &store);
        assert!(result.is_err());
        assert_eq!(before, count_artifact_objects(&store));
    }

    fn count_artifact_objects(store: &ArtifactStore) -> usize {
        fn count_files(path: &Path) -> usize {
            std::fs::read_dir(path)
                .map(|entries| {
                    entries
                        .filter_map(std::result::Result::ok)
                        .map(|entry| {
                            let path = entry.path();
                            if path.is_dir() {
                                count_files(&path)
                            } else {
                                usize::from(path.is_file())
                            }
                        })
                        .sum()
                })
                .unwrap_or(0)
        }

        count_files(&store.root().join("objects/sha256"))
    }
}
