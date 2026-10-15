//! Ordinary build admission for the existing frozen composition executor.
//!
//! The native package, target identities, renderer and receipts stay authoritative.
//! This adapter binds that execution to the effective build specification without
//! pretending locale branches are distinct formats in a general conversion DAG.

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::{ensure, Context, Result};

use super::{
    execute_composition, resolve_composition, CanonicalExecutionResult,
    CompositionPlanningRequest, PlanningRequest, ResolvedCompositionExecution,
};
use crate::composition::{hash_bytes, hash_json, CompositionBranchState};
use crate::evidence::{
    run_id, sha256_serialized, unix_time_ms, ArtifactManifest, DiagnosticSeverity,
    DigestEvidence, ExecutionDiagnostic, RunManifest, RunState,
    ARTIFACT_MANIFEST_SCHEMA_V1, RUN_MANIFEST_SCHEMA_V1,
};
use crate::graph::execution_plan::{
    ForestExecutionEvidence, ForestTargetExecution, ForestTargetOutcome,
    FOREST_EXECUTION_SCHEMA_V1,
};
use crate::graph::{DiagnosticLevel, ExecutionPlan, ForestBranchState};
use crate::process::ProcessCancellationToken;
use crate::spec::{LoadedSpec, SourceKind, SourceSpecVersion, SpecV2, TargetSpec};

pub(super) struct ResolvedCompositionBuild {
    native: ResolvedCompositionExecution,
    plan: ExecutionPlan,
    spec: SpecV2,
    source_version: SourceSpecVersion,
    source_path: PathBuf,
    output_root: PathBuf,
    cache_directory: Option<PathBuf>,
    cancellation: ProcessCancellationToken,
}

impl ResolvedCompositionBuild {
    pub(super) fn plan(&self) -> &ExecutionPlan { &self.plan }
    pub(super) fn spec(&self) -> &SpecV2 { &self.spec }
    pub(super) fn source_version(&self) -> SourceSpecVersion { self.source_version }
    pub(super) fn source_path(&self) -> &Path { &self.source_path }

    pub(super) fn predicted_output_paths(&self) -> Result<Vec<PathBuf>> {
        Ok(self.planned_outputs().iter().map(|path| self.output_root.join(path)).collect())
    }

    fn planned_outputs(&self) -> Vec<String> {
        self.native.native_plan().branches.iter()
            .filter(|branch| branch.state == CompositionBranchState::Planned)
            .map(|branch| format!("outputs/{}/page.{}", branch.locale, branch.output.extension()))
            .collect()
    }

    pub(super) fn with_cancellation_flag(mut self, cancellation: Arc<AtomicBool>) -> Self {
        self.cancellation = ProcessCancellationToken::from_shared(cancellation);
        self
    }
}

pub(super) fn resolve(
    config_path: PathBuf,
    loaded: LoadedSpec,
    request: &PlanningRequest,
) -> Result<ResolvedCompositionBuild> {
    ensure!(!request.all_reachable && request.profile.is_none() && request.exclude.is_empty()
        && request.optimization.is_none(),
        "composition.build.overrides: --all, --profile, --exclude and --optimization are unsupported; select exact native outputs with --target or targets.exact");
    let mut spec = loaded.spec;
    if let Some(target) = &request.target {
        spec.targets.exact = vec![TargetSpec { format: Some(target.clone()), ..TargetSpec::default() }];
    }
    let diagnostics = spec.validate();
    ensure!(diagnostics.is_empty(), "composition.build.spec: {}", diagnostics.iter()
        .map(|diagnostic| format!("{} [{}]: {}", diagnostic.path, diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>().join("; "));
    ensure!(loaded.source_version == SourceSpecVersion::V2 && spec.sources.len() == 1
        && spec.sources[0].kind == SourceKind::Composition,
        "composition.build.source: require one local composition source in renderflow/v2");

    // Resolve the already-read configuration's location, never output/cache state
    // or host providers. Captured package reads happen in resolve_composition.
    let config_path = fs::canonicalize(config_path)
        .context("composition.build.config: cannot resolve configuration location")?;
    let config_parent = config_path.parent().context("composition.build.config: configuration has no parent")?;
    let source_path = relative_to(config_parent, spec.sources[0].path.as_deref()
        .context("composition.build.source: missing local package path")?);
    let output_root = relative_to(config_parent, &spec.output.bundle_root);
    reject_parent_traversal(&output_root, "output.bundle_root")?;
    let mut policy = spec.execution.composition.clone().unwrap_or_default();
    let cache_directory = policy.cache_directory.as_deref()
        .map(|path| relative_to(config_parent, path));
    if let Some(path) = &cache_directory {
        reject_parent_traversal(path, "execution.composition.cache_directory")?;
        ensure!(!path.starts_with(&output_root),
            "composition.build.cache: an explicit cache_directory must be outside the output bundle; omit it for the bundle's default .artifact-store or use a separate shared cache");
        policy.cache_directory = Some(path_string(path)?);
    }
    let native_request = policy.request(&spec.targets.exact)?;
    spec.sources[0].path = Some(path_string(&source_path)?);
    spec.output.bundle_root = path_string(&output_root)?;
    spec.execution.composition = Some(policy);
    let native = resolve_composition(CompositionPlanningRequest {
        input: source_path.clone(), request: native_request,
    })?;
    let mut plan = native.plan().execution.clone();
    let source_spec_digest = sha256_serialized(&spec)?;
    let mut native_plan_bytes = serde_json::to_vec_pretty(native.plan())?;
    native_plan_bytes.push(b'\n');
    let native_plan_digest = DigestEvidence {
        algorithm: "sha256".into(), value: hash_bytes(&native_plan_bytes),
    };
    let forest = plan.artifact_forest.as_mut()
        .context("composition.build.forest: native planning omitted its forest")?;
    let native_forest_identity = forest.identity.clone();
    forest.policy.as_object_mut()
        .context("composition.build.forest: native policy is not an object")?
        .insert("build".into(), serde_json::json!({
            "schema": "renderflow.composition-build-binding/v1",
            "source_spec_digest": source_spec_digest,
            "native_execution_plan_digest": native_plan_digest,
            "native_forest_identity": native_forest_identity,
            "build_plan_file": "build-plan.json",
            "build_plan_serialization": "compact_json_without_trailing_newline",
            "native_execution_plan_file": "execution-plan.json",
            "source_spec_meaning": "effective normalized renderflow/v2 specification after request overrides and config-relative path resolution",
            "publication": "fresh bundle; outputs/{locale}/page.{native_extension}",
        }));
    forest.identity = format!("sha256:{}", hash_json(&serde_json::json!({
        "schema": "renderflow.composition-build-forest/v1",
        "native_identity": native_forest_identity,
        "policy": forest.policy,
    }))?);
    Ok(ResolvedCompositionBuild {
        native, plan, spec, source_version: loaded.source_version, source_path,
        output_root, cache_directory, cancellation: ProcessCancellationToken::new(),
    })
}

pub(super) fn execute(resolved: ResolvedCompositionBuild, dry_run: bool) -> Result<CanonicalExecutionResult> {
    if resolved.cancellation.is_cancelled() { return cancelled(resolved); }
    if dry_run {
        let outputs = resolved.predicted_output_paths()?.iter()
            .map(|path| path.display().to_string()).collect();
        let manifest = unexecuted_manifest(&resolved, RunState::Planned)?;
        return Ok(result(&resolved, manifest, None, outputs));
    }
    // Native execution refuses an existing bundle before publication and keeps
    // all native sidecars intact, including partial and cancelled receipts.
    let native_run = execute_composition(&resolved.native, &resolved.output_root,
        resolved.cache_directory.as_deref(), &resolved.cancellation)?;
    let mut manifest = native_run.run_manifest()?;
    // The native receipt remains portable with output_dir=".". The common
    // build result anchors its unchanged bundle-relative locators for SDKs.
    manifest.artifact_manifest.output_dir = resolved.spec.output.bundle_root.clone();
    manifest.execution_plan_digest = sha256_serialized(&resolved.plan)?;
    manifest.source_spec_digest = sha256_serialized(&resolved.spec)?;
    let planned_forest = resolved.plan.artifact_forest.as_ref()
        .context("composition.build.forest: frozen build plan omitted its forest")?;
    let observed_forest = manifest.artifact_forest.as_mut()
        .context("composition.build.forest: native receipt omitted its forest")?;
    ensure!(observed_forest.branches == planned_forest.branches,
        "composition.build.forest: execution branches differ from the frozen build plan");
    observed_forest.identity = planned_forest.identity.clone();
    observed_forest.policy = planned_forest.policy.clone();
    manifest.diagnostics.retain(|diagnostic| diagnostic.code != "composition.evidence.scope");
    manifest.diagnostics.push(scope_diagnostic());

    // Use exactly provider_plan's compact serialization: its digest identifies
    // the persisted bytes, not a pretty-printed or newline-modified equivalent.
    write_new(&resolved.output_root, "build-plan.json", &serde_json::to_vec(&resolved.plan)?)?;
    let manifest_path = write_new(&resolved.output_root, "renderflow-run.json", &serde_json::to_vec_pretty(&manifest)?)?;
    let outputs = manifest.artifact_manifest.outputs.iter()
        .map(|path| resolved.output_root.join(path).display().to_string()).collect();
    Ok(result(&resolved, manifest, Some(manifest_path.display().to_string()), outputs))
}

/// A pre-execution cancellation has no observed artifacts, stage accesses, or
/// files. Retain the resolved intent in memory for CLI/SDK consumers only.
pub(super) fn cancelled(resolved: ResolvedCompositionBuild) -> Result<CanonicalExecutionResult> {
    let manifest = unexecuted_manifest(&resolved, RunState::Cancelled)?;
    Ok(result(&resolved, manifest, None, Vec::new()))
}

fn unexecuted_manifest(resolved: &ResolvedCompositionBuild, state: RunState) -> Result<RunManifest> {
    let started_at_unix_ms = unix_time_ms();
    let execution_plan_digest = sha256_serialized(&resolved.plan)?;
    let run_id = run_id(&execution_plan_digest, started_at_unix_ms);
    let mut forest = resolved.plan.artifact_forest.clone()
        .context("composition.build.forest: frozen build plan omitted its forest")?;
    forest.produced_artifacts.clear();
    forest.execution = Some(ForestExecutionEvidence {
        schema: FOREST_EXECUTION_SCHEMA_V1.into(),
        targets: forest.branches.iter().map(|branch| ForestTargetExecution {
            target_id: branch.target_id.clone(),
            outcome: if branch.state != ForestBranchState::Selected { ForestTargetOutcome::NotSelected }
                else if state == RunState::Planned { ForestTargetOutcome::Planned }
                else { ForestTargetOutcome::NotPublished },
            source_identity: false, artifact_evidence_index: None, artifact_id: None,
            locator: None, producing_step_id: None,
        }).collect(),
    });
    let mut diagnostics = resolved.plan.diagnostics.iter().map(|diagnostic| ExecutionDiagnostic {
        severity: match diagnostic.level {
            DiagnosticLevel::Info => DiagnosticSeverity::Info,
            DiagnosticLevel::Warning => DiagnosticSeverity::Warning,
            DiagnosticLevel::Error => DiagnosticSeverity::FatalFailure,
        },
        code: "composition.build.plan".into(), message: diagnostic.message.clone(), step_id: None,
    }).collect::<Vec<_>>();
    diagnostics.push(scope_diagnostic());
    diagnostics.push(ExecutionDiagnostic {
        severity: if state == RunState::Cancelled { DiagnosticSeverity::Cancelled } else { DiagnosticSeverity::Info },
        code: if state == RunState::Cancelled { "execution.cancelled" } else { "execution.dry_run" }.into(),
        message: "No composition execution or publication occurred; output and cache directories were not accessed".into(),
        step_id: None,
    });
    Ok(RunManifest {
        schema_version: RUN_MANIFEST_SCHEMA_V1.into(), run_id: run_id.clone(),
        execution_plan_digest, source_spec_digest: sha256_serialized(&resolved.spec)?,
        engine_version: env!("CARGO_PKG_VERSION").into(), started_at_unix_ms,
        completed_at_unix_ms: unix_time_ms().max(started_at_unix_ms), state,
        artifact_manifest: ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_V1.into(), run_id,
            output_dir: resolved.spec.output.bundle_root.clone(),
            outputs: if state == RunState::Planned { resolved.planned_outputs() } else { Vec::new() },
            artifacts: Vec::new(),
        },
        steps: Vec::new(), diagnostics, toolchain: None, artifact_forest: Some(forest),
    })
}

fn scope_diagnostic() -> ExecutionDiagnostic {
    ExecutionDiagnostic {
        severity: DiagnosticSeverity::Info, code: "composition.build.evidence.scope".into(), step_id: None,
        message: "Build execution_plan_digest identifies exact compact build-plan.json bytes; source_spec_digest identifies the effective normalized renderflow/v2 specification. Forest policy binds the untouched native execution-plan.json and its captured input pins. Native receipts retain their original digest meanings. The common output_dir anchors unchanged bundle-relative artifact locators and publication joins; planned output names are not observed artifacts. Correlated receipts do not establish visual quality, creative approval, runtime qualification, or Flow acceptance.".into(),
    }
}

fn result(resolved: &ResolvedCompositionBuild, manifest: RunManifest,
    manifest_path: Option<String>, outputs: Vec<String>) -> CanonicalExecutionResult {
    CanonicalExecutionResult {
        plan: resolved.plan.clone(), output_dir: resolved.spec.output.bundle_root.clone(), outputs,
        diagnostics: manifest.diagnostics.iter().map(|diagnostic| diagnostic.message.clone()).collect(),
        toolchain: None, run_manifest: manifest, manifest_path,
    }
}

fn relative_to(parent: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    let absolute = if path.is_absolute() { path.to_path_buf() } else { parent.join(path) };
    // Preserve parent components for explicit refusal; strip only harmless dot
    // components so a cache cannot hide inside the bundle as "out/./cache".
    absolute.components().filter(|component| *component != Component::CurDir).collect()
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str().map(str::to_owned).context("composition.build.path: non-UTF8 paths are unsupported")
}

fn reject_parent_traversal(path: &Path, field: &str) -> Result<()> {
    ensure!(!path.components().any(|component| component == Component::ParentDir),
        "composition.build.path: {field} cannot contain '..'; use a path inside the configuration directory or an absolute destination");
    Ok(())
}

fn write_new(root: &Path, filename: &str, bytes: &[u8]) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(root)?;
    ensure!(metadata.is_dir() && !metadata.file_type().is_symlink(),
        "composition.build.publication: output bundle is no longer a regular directory");
    let destination = root.join(filename);
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(&destination).map_err(|error| error.error)?;
    Ok(destination)
}
