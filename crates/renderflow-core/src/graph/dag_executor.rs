use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use rayon::prelude::*;
use tracing::{debug, warn};

use super::{Format, MultiTargetDag, TransformEdge};
use crate::artifact::{
    compute_artifact_node_hash, load_artifact_cache, save_artifact_cache, Artifact, ArtifactCache,
    ArtifactCollection, ArtifactCollectionTransform, ArtifactDescriptor, ArtifactStorageClass,
    ArtifactStore, ArtifactTransform, TextTransformAdapter,
};
use crate::evidence::{
    redact_sensitive_text, sha256_text, unix_time_ms, CacheDisposition, DiagnosticSeverity,
    ExecutionDiagnostic, FidelityDeclaration, StepEvidence, StepState, ValidationState,
};
use crate::transforms::aggregation::AggregationTransform;
use crate::transforms::plugin_v2::{
    PluginInputKind, PluginRuntimeOptions, PluginTransformV2, PluginV2ArtifactAdapter,
    PluginV2CollectionAdapter,
};
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
    /// Artifact-native ordered-collection transforms.
    collection_transforms: HashMap<(Format, Format), Arc<dyn ArtifactCollectionTransform>>,
    /// Optional artifact-native DAG cache path.
    cache_path: Option<PathBuf>,
    /// Selected-provider fingerprint used to reject incompatible cache entries.
    toolchain_fingerprint: Option<String>,
    /// Optional per-execution parallelism bound from the canonical execution policy.
    max_parallel: Option<usize>,
}

/// Artifact outputs and step evidence from one DAG execution.
pub struct DagExecutionReport {
    pub artifacts: HashMap<Format, Artifact>,
    pub steps: Vec<StepEvidence>,
    pub diagnostics: Vec<ExecutionDiagnostic>,
}

impl DagExecutionReport {
    fn into_result(self) -> Result<HashMap<Format, Artifact>> {
        ensure_steps_succeeded(&self.steps)?;
        Ok(self.artifacts)
    }
}

struct CollectionExecutionReport {
    artifacts: HashMap<Format, ArtifactCollection>,
    steps: Vec<StepEvidence>,
    diagnostics: Vec<ExecutionDiagnostic>,
}

impl CollectionExecutionReport {
    fn into_result(self) -> Result<HashMap<Format, ArtifactCollection>> {
        ensure_steps_succeeded(&self.steps)?;
        Ok(self.artifacts)
    }
}

struct ExecutedEdge {
    format: Format,
    artifacts: ArtifactCollection,
    evidence: StepEvidence,
}

struct SingleEdgeOutcome {
    artifact: Artifact,
    transform: String,
    transform_version: String,
    configuration_digest: crate::evidence::DigestEvidence,
    cache: CacheDisposition,
    fidelity: Option<FidelityDeclaration>,
}

fn ensure_steps_succeeded(steps: &[StepEvidence]) -> Result<()> {
    let has_failure = steps
        .iter()
        .any(|step| matches!(step.state, StepState::Failed | StepState::Cancelled));
    if !has_failure {
        return Ok(());
    }
    let failures = steps
        .iter()
        .filter(|step| matches!(step.state, StepState::Failed | StepState::Cancelled))
        .flat_map(|step| {
            step.diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
        })
        .collect::<Vec<_>>();
    if failures.is_empty() {
        anyhow::bail!("artifact DAG execution failed without diagnostic detail")
    } else {
        anyhow::bail!("artifact DAG execution failed: {}", failures.join("; "))
    }
}

fn edge_identity(edge: &TransformEdge) -> String {
    edge.evidence
        .get("transform_id")
        .cloned()
        .unwrap_or_else(|| format!("{}-to-{}", edge.from, edge.to))
}

fn edge_configuration_digest(edge: &TransformEdge) -> crate::evidence::DigestEvidence {
    sha256_text(&format!(
        "{}\0{}\0{}\0{}\0{:?}",
        edge.from,
        edge.to,
        edge.provider_id.as_deref().unwrap_or(""),
        edge.variant_id.as_deref().unwrap_or(""),
        edge.evidence
    ))
}

fn edge_fidelity(edge: &TransformEdge) -> FidelityDeclaration {
    if edge.input_kind.is_collection() {
        FidelityDeclaration::PathDependent
    } else if (edge.quality - 1.0).abs() < f32::EPSILON {
        FidelityDeclaration::Lossless
    } else {
        FidelityDeclaration::Lossy
    }
}

fn failed_step(
    edge: &TransformEdge,
    inputs: Option<&ArtifactCollection>,
    error: &anyhow::Error,
    started_at_unix_ms: u64,
    duration_ms: u64,
) -> StepEvidence {
    let message = redact_sensitive_text(&error.to_string());
    let step_id = format!("step:{}-to-{}", edge.from, edge.to);
    StepEvidence {
        step_id: step_id.clone(),
        transform: edge_identity(edge),
        transform_version: edge
            .variant_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        capability: edge.capability_id.clone(),
        provider: edge.provider_id.clone(),
        input_artifacts: inputs
            .into_iter()
            .flat_map(|collection| collection.iter())
            .map(|artifact| artifact.id().to_string())
            .collect(),
        output_artifacts: Vec::new(),
        configuration_digest: edge_configuration_digest(edge),
        started_at_unix_ms,
        completed_at_unix_ms: unix_time_ms(),
        duration_ms,
        state: StepState::Failed,
        cache: CacheDisposition::Miss,
        validation: if message.contains("returned artifact format") {
            ValidationState::Invalid
        } else {
            ValidationState::Unavailable
        },
        fidelity: edge_fidelity(edge),
        skip_reason: None,
        diagnostics: vec![ExecutionDiagnostic {
            severity: DiagnosticSeverity::FatalFailure,
            code: "execution.transform_failed".to_string(),
            message,
            step_id: Some(step_id),
        }],
    }
}

fn skipped_step(edge: &TransformEdge, reason: &str) -> StepEvidence {
    let timestamp = unix_time_ms();
    StepEvidence {
        step_id: format!("step:{}-to-{}", edge.from, edge.to),
        transform: edge_identity(edge),
        transform_version: edge
            .variant_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        capability: edge.capability_id.clone(),
        provider: edge.provider_id.clone(),
        input_artifacts: Vec::new(),
        output_artifacts: Vec::new(),
        configuration_digest: edge_configuration_digest(edge),
        started_at_unix_ms: timestamp,
        completed_at_unix_ms: timestamp,
        duration_ms: 0,
        state: StepState::Skipped,
        cache: CacheDisposition::NotApplicable,
        validation: ValidationState::Skipped,
        fidelity: edge_fidelity(edge),
        skip_reason: Some(reason.to_string()),
        diagnostics: Vec::new(),
    }
}

impl DagExecutor {
    /// Create an empty executor with no transforms registered.
    pub fn new() -> Self {
        Self {
            single_transforms: HashMap::new(),
            aggregation_transforms: HashMap::new(),
            collection_transforms: HashMap::new(),
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
    /// seam for legacy transforms that have not migrated to the v2 plugin SDK.
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

    /// Register a versioned artifact-native plugin on a canonical graph edge.
    pub fn register_plugin_v2(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn PluginTransformV2>,
        options: PluginRuntimeOptions,
    ) -> Result<&mut Self> {
        if self.single_transforms.contains_key(&(from, to))
            || self.collection_transforms.contains_key(&(from, to))
            || self.aggregation_transforms.contains_key(&(from, to))
        {
            anyhow::bail!(
                "a transform is already registered for '{}' to '{}'; use replace_plugin_v2 for an explicit replacement",
                from,
                to
            );
        }
        self.install_plugin_v2(from, to, transform, options)
    }

    /// Explicitly replace the transform assigned to a canonical graph edge.
    pub fn replace_plugin_v2(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn PluginTransformV2>,
        options: PluginRuntimeOptions,
    ) -> Result<&mut Self> {
        match transform.descriptor().input_kind {
            PluginInputKind::Single => {
                let adapter = PluginV2ArtifactAdapter::new(transform, options)?;
                self.collection_transforms.remove(&(from, to));
                self.aggregation_transforms.remove(&(from, to));
                self.single_transforms.insert((from, to), Arc::new(adapter));
            }
            PluginInputKind::OrderedCollection => {
                let adapter = PluginV2CollectionAdapter::new(transform, options)?;
                self.single_transforms.remove(&(from, to));
                self.aggregation_transforms.remove(&(from, to));
                self.collection_transforms
                    .insert((from, to), Arc::new(adapter));
            }
        }
        Ok(self)
    }

    fn install_plugin_v2(
        &mut self,
        from: Format,
        to: Format,
        transform: Arc<dyn PluginTransformV2>,
        options: PluginRuntimeOptions,
    ) -> Result<&mut Self> {
        match transform.descriptor().input_kind {
            PluginInputKind::Single => {
                let adapter = PluginV2ArtifactAdapter::new(transform, options)?;
                self.single_transforms.insert((from, to), Arc::new(adapter));
            }
            PluginInputKind::OrderedCollection => {
                let adapter = PluginV2CollectionAdapter::new(transform, options)?;
                self.collection_transforms
                    .insert((from, to), Arc::new(adapter));
            }
        }
        Ok(self)
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
        self.execute_artifact_with_evidence(dag, source_format, initial_artifact, store)?
            .into_result()
    }

    /// Execute one artifact while retaining machine-readable step evidence.
    pub fn execute_artifact_with_evidence(
        &self,
        dag: &MultiTargetDag,
        source_format: Format,
        initial_artifact: Artifact,
        store: &ArtifactStore,
    ) -> Result<DagExecutionReport> {
        let report = self.execute_artifacts_with_evidence(
            dag,
            source_format,
            ArtifactCollection::one(initial_artifact),
            store,
        )?;

        let artifacts = report
            .artifacts
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
            .collect::<Result<HashMap<_, _>>>()?;
        Ok(DagExecutionReport {
            artifacts,
            steps: report.steps,
            diagnostics: report.diagnostics,
        })
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
        self.execute_artifacts_with_evidence(dag, source_format, initial_artifacts, store)?
            .into_result()
    }

    fn execute_artifacts_with_evidence(
        &self,
        dag: &MultiTargetDag,
        source_format: Format,
        initial_artifacts: ArtifactCollection,
        store: &ArtifactStore,
    ) -> Result<CollectionExecutionReport> {
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
        let mut steps = Vec::new();
        let mut diagnostics = Vec::new();

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
                    for edge in &next_remaining {
                        let step = skipped_step(edge, "required input artifact was not produced");
                        diagnostics.push(ExecutionDiagnostic {
                            severity: DiagnosticSeverity::RecoverableFailure,
                            code: "execution.step_skipped".to_string(),
                            message: format!(
                                "Skipped transform {} → {} because its input was unavailable",
                                edge.from, edge.to
                            ),
                            step_id: Some(step.step_id.clone()),
                        });
                        steps.push(step);
                    }
                }
                break;
            }

            debug!(wave_size = wave.len(), "Executing artifact DAG wave");
            let execute_wave = || {
                wave.into_par_iter()
                    .map(|edge| {
                        let started_at_unix_ms = unix_time_ms();
                        let started = Instant::now();
                        (
                            edge,
                            self.execute_edge(edge, &available, store, cache.as_ref()),
                            started_at_unix_ms,
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let wave_results = if let Some(pool) = &thread_pool {
                pool.install(execute_wave)
            } else {
                execute_wave()
            };

            for (edge, outcome, started_at_unix_ms, duration_ms) in wave_results {
                match outcome {
                    Ok(executed) => {
                        available.insert(executed.format, executed.artifacts);
                        steps.push(executed.evidence);
                    }
                    Err(error) => {
                        let step = failed_step(
                            edge,
                            available.get(&edge.from),
                            &error,
                            started_at_unix_ms,
                            duration_ms,
                        );
                        diagnostics.push(ExecutionDiagnostic {
                            severity: DiagnosticSeverity::FatalFailure,
                            code: "execution.transform_failed".to_string(),
                            message: step
                                .diagnostics
                                .first()
                                .map(|diagnostic| diagnostic.message.clone())
                                .unwrap_or_else(|| "Transform failed".to_string()),
                            step_id: Some(step.step_id.clone()),
                        });
                        steps.push(step);
                    }
                }
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

        Ok(CollectionExecutionReport {
            artifacts: available,
            steps,
            diagnostics,
        })
    }

    fn execute_edge(
        &self,
        edge: &TransformEdge,
        available: &HashMap<Format, ArtifactCollection>,
        store: &ArtifactStore,
        cache: Option<&Mutex<ArtifactCache>>,
    ) -> Result<ExecutedEdge> {
        let started_at_unix_ms = unix_time_ms();
        let started = Instant::now();
        let inputs = available.get(&edge.from).ok_or_else(|| {
            anyhow::anyhow!(
                "Source format '{}' was not available for DAG edge",
                edge.from
            )
        })?;

        let input_artifacts = inputs
            .iter()
            .map(|artifact| artifact.id().to_string())
            .collect::<Vec<_>>();
        let (artifacts, transform, transform_version, configuration_digest, cache, fidelity) =
            if edge.input_kind.is_single() {
                let input = inputs.clone().into_one().with_context(|| {
                    format!(
                        "Single transform {:?} → {:?} requires exactly one artifact",
                        edge.from, edge.to
                    )
                })?;
                let outcome = self.execute_single_edge(edge, &input, store, cache)?;
                (
                    ArtifactCollection::one(outcome.artifact),
                    outcome.transform,
                    outcome.transform_version,
                    outcome.configuration_digest,
                    outcome.cache,
                    outcome.fidelity,
                )
            } else {
                let (output, transform, transform_version, fidelity) =
                    self.execute_collection_edge(edge, inputs, store)?;
                (
                    ArtifactCollection::one(output),
                    transform.clone(),
                    transform_version,
                    sha256_text(&transform),
                    CacheDisposition::NotApplicable,
                    fidelity,
                )
            };
        let completed_at_unix_ms = unix_time_ms();
        let output_artifacts = artifacts
            .iter()
            .map(|artifact| artifact.id().to_string())
            .collect();
        let fidelity = fidelity.unwrap_or_else(|| edge_fidelity(edge));
        Ok(ExecutedEdge {
            format: edge.to,
            artifacts,
            evidence: StepEvidence {
                step_id: format!("step:{}-to-{}", edge.from, edge.to),
                transform,
                transform_version,
                capability: edge.capability_id.clone(),
                provider: edge.provider_id.clone(),
                input_artifacts,
                output_artifacts,
                configuration_digest,
                started_at_unix_ms,
                completed_at_unix_ms,
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                state: if cache == CacheDisposition::Hit {
                    StepState::Reused
                } else {
                    StepState::Complete
                },
                cache,
                validation: ValidationState::Valid,
                fidelity,
                skip_reason: None,
                diagnostics: Vec::new(),
            },
        })
    }

    fn execute_single_edge(
        &self,
        edge: &TransformEdge,
        input: &Artifact,
        store: &ArtifactStore,
        cache: Option<&Mutex<ArtifactCache>>,
    ) -> Result<SingleEdgeOutcome> {
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

        if transform.cacheable() {
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
                            return Ok(SingleEdgeOutcome {
                                artifact: cached
                                    .clone()
                                    .with_storage_class(ArtifactStorageClass::Cached),
                                transform: transform.name().to_string(),
                                transform_version: transform.version().to_string(),
                                configuration_digest: sha256_text(&cache_identity),
                                cache: CacheDisposition::Hit,
                                fidelity: transform.fidelity(),
                            });
                        }
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

        if transform.cacheable() {
            if let Some(cache_mutex) = cache {
                if let Ok(mut guard) = cache_mutex.lock() {
                    guard.insert(cache_key, output.clone());
                }
            }
        }
        Ok(SingleEdgeOutcome {
            artifact: output,
            transform: transform.name().to_string(),
            transform_version: transform.version().to_string(),
            configuration_digest: sha256_text(&cache_identity),
            cache: if transform.cacheable() {
                CacheDisposition::Miss
            } else {
                CacheDisposition::NotApplicable
            },
            fidelity: transform.fidelity(),
        })
    }

    fn execute_collection_edge(
        &self,
        edge: &TransformEdge,
        inputs: &ArtifactCollection,
        store: &ArtifactStore,
    ) -> Result<(Artifact, String, String, Option<FidelityDeclaration>)> {
        if let Some(transform) = self.collection_transforms.get(&(edge.from, edge.to)) {
            let output = transform.apply(inputs, edge.to, store).with_context(|| {
                format!(
                    "Collection plugin {:?} → {:?} ({}) failed",
                    edge.from,
                    edge.to,
                    transform.name()
                )
            })?;
            self.validate_output_format(edge, &output)?;
            return Ok((
                output,
                transform.name().to_string(),
                transform.version().to_string(),
                transform.fidelity(),
            ));
        }

        let transform = self
            .aggregation_transforms
            .get(&(edge.from, edge.to))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No collection transform registered for {:?} → {:?}",
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
        Ok((
            output,
            transform.name().to_string(),
            "unstable-v1".to_string(),
            None,
        ))
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

    struct WrongFormatTransform;

    impl ArtifactTransform for WrongFormatTransform {
        fn name(&self) -> &str {
            "wrong-format"
        }

        fn apply(
            &self,
            input: &Artifact,
            _output_format: Format,
            store: &ArtifactStore,
        ) -> Result<Artifact> {
            let mut reader = store.open(input)?;
            store.put_reader(
                &mut reader,
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Intermediate)
                    .with_source(input.id().clone()),
            )
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
    fn execution_evidence_reports_cache_miss_then_hit() {
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
        let mut executor = DagExecutor::new().with_cache(&cache_path);
        executor.register_artifact(
            Format::Png,
            Format::Webp,
            Arc::new(CountingBinaryTransform {
                executions: Arc::new(AtomicUsize::new(0)),
            }),
        );

        let first = executor
            .execute_artifact_with_evidence(&dag, Format::Png, source.clone(), &store)
            .unwrap();
        let second = executor
            .execute_artifact_with_evidence(&dag, Format::Png, source, &store)
            .unwrap();

        assert_eq!(first.steps[0].cache, CacheDisposition::Miss);
        assert_eq!(first.steps[0].state, StepState::Complete);
        assert_eq!(second.steps[0].cache, CacheDisposition::Hit);
        assert_eq!(second.steps[0].state, StepState::Reused);
        assert!(second.diagnostics.is_empty());
    }

    #[test]
    fn partial_execution_records_failure_and_downstream_skip() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Png, Format::Webp, 1.0, 1.0));
        graph.add_collection_transform(Format::Png, Format::Pdf, 1.0, 1.0);
        graph.add_transform(TransformEdge::new(Format::Pdf, Format::Html, 1.0, 1.0));
        let dag = graph
            .build_multi_target_dag(Format::Png, &[Format::Webp, Format::Html])
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let source = store
            .put_bytes(
                b"page",
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        let mut executor = DagExecutor::new();
        executor.register_artifact(Format::Png, Format::Webp, Arc::new(BinaryCopyTransform));
        executor.register_aggregation(Format::Png, Format::Pdf, Arc::new(FailingAggregation));

        let report = executor
            .execute_artifact_with_evidence(&dag, Format::Png, source, &store)
            .unwrap();

        assert!(report.artifacts.contains_key(&Format::Webp));
        assert!(!report.artifacts.contains_key(&Format::Html));
        assert!(report
            .steps
            .iter()
            .any(|step| step.state == StepState::Complete));
        assert!(report
            .steps
            .iter()
            .any(|step| step.state == StepState::Failed));
        assert!(report
            .steps
            .iter()
            .any(|step| step.state == StepState::Skipped));
    }

    #[test]
    fn output_contract_validation_failure_is_structured() {
        let dag = one_edge(Format::Png, Format::Webp, InputKind::Single);
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let source = store
            .put_bytes(
                b"image",
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        let mut executor = DagExecutor::new();
        executor.register_artifact(Format::Png, Format::Webp, Arc::new(WrongFormatTransform));

        let report = executor
            .execute_artifact_with_evidence(&dag, Format::Png, source, &store)
            .unwrap();

        assert_eq!(report.steps[0].state, StepState::Failed);
        assert_eq!(report.steps[0].validation, ValidationState::Invalid);
        assert_eq!(report.diagnostics[0].code, "execution.transform_failed");
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
