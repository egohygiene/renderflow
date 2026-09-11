//! Canonical application-layer planning and execution lifecycle.
//!
//! All CLI and SDK build modes normalize v1/v2 intent here before execution.
//! Execution consumes a previously resolved [`ResolvedExecution`] and never
//! performs implicit target/path re-planning.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::{Context, Result};

use crate::adapters::strategy::{
    document_input_format, output_type_for_format, StrategyArtifactTransform,
};
use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore};
use crate::checkpoint::CheckpointContext;
use crate::evidence::{
    redact_sensitive_text, run_id, sha256_serialized, unix_time_ms, ArtifactEvidence,
    ArtifactManifest, ArtifactRole, DiagnosticSeverity, ExecutionDiagnostic, FidelityDeclaration,
    ProducerEvidence, RunManifest, RunState, StepEvidence, StepState, ValidationState,
    ARTIFACT_MANIFEST_SCHEMA_V1, RUN_MANIFEST_SCHEMA_V1,
};
use crate::graph::capability::{FormatCapabilityRegistry, FormatFamily};
use crate::graph::{
    ArtifactForest, DagExecutionReport, DagExecutor, DiagnosticLevel, ExecutionPlan, ForestBranch,
    ForestBranchState, Format, MultiTargetDag, TransformEdge, TransformGraph,
};
use crate::hygiene::{HygieneEngine, HygieneEvidence};
use crate::intake::{IntakeEngine, IntakeRequest, ResolvedArtifactProfile};
use crate::optimization::OptimizationMode;
use crate::spec::{
    load_spec, AiPolicy, CollisionPolicy, DerivativeProfile, HygienePolicy, IntermediatePolicy,
    RejectedLossClass, SelectorSet, SourceKind, SourceSpec, SourceSpecVersion, SpecV2,
    TargetRequirement, TargetSelection, TargetSpec, ValidationFailureMode,
};
use crate::super_resolution::{select_upscayl_variants, UpscaylModelCatalog};
use crate::toolchain::{
    transform_capability_id, ToolDeterminism, ToolId, ToolLocality, ToolRegistry,
    ToolRuntimeContext, ToolchainSnapshot,
};
use crate::transforms::yaml_loader::build_graph_executor_and_tools_from_yaml;
use crate::validation::{ArtifactValidationOutcome, ValidationRegistry};

const BUILTIN_ADAPTER_EVIDENCE: &str = "builtin.strategy";

#[derive(Debug, Clone)]
pub struct PlanningRequest {
    pub config_path: PathBuf,
    pub target: Option<String>,
    pub all_reachable: bool,
    pub profile: Option<String>,
    pub exclude: SelectorSet,
    pub optimization: Option<OptimizationMode>,
}

impl PlanningRequest {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
            target: None,
            all_reachable: false,
            profile: None,
            exclude: SelectorSet::default(),
            optimization: None,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self.all_reachable = false;
        self.profile = None;
        self
    }

    pub fn with_all_reachable(mut self) -> Self {
        self.target = None;
        self.profile = None;
        self.all_reachable = true;
        self
    }

    pub fn with_profile(mut self, profile: impl Into<String>) -> Self {
        self.target = None;
        self.all_reachable = false;
        self.profile = Some(profile.into());
        self
    }

    pub fn with_exclude(mut self, expression: &str) -> Result<Self> {
        let (kind, value) = expression.split_once(':').ok_or_else(|| {
            anyhow::anyhow!("invalid exclude selector '{expression}'; expected kind:value")
        })?;
        let destination = match kind {
            "format" => &mut self.exclude.formats,
            "family" => &mut self.exclude.families,
            "capability" => &mut self.exclude.capabilities,
            "provider" => &mut self.exclude.providers,
            "role" => &mut self.exclude.roles,
            "transform" => &mut self.exclude.transforms,
            "variant" => &mut self.exclude.variants,
            _ => anyhow::bail!("unknown exclude selector kind '{kind}'"),
        };
        destination.push(value.to_string());
        Ok(self)
    }

    pub fn with_optimization(mut self, optimization: OptimizationMode) -> Self {
        self.optimization = Some(optimization);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub format: Format,
    pub id: Option<String>,
    pub role: Option<String>,
    pub preset: Option<String>,
    pub template: Option<String>,
    pub variant: Option<String>,
    pub requirement: TargetRequirement,
    pub options: std::collections::BTreeMap<String, serde_json::Value>,
}

impl ResolvedTarget {
    fn generated(format: Format) -> Self {
        Self {
            format,
            id: None,
            role: Some(format.to_string()),
            preset: None,
            template: None,
            variant: None,
            requirement: TargetRequirement::Optional,
            options: Default::default(),
        }
    }

    fn from_spec(format: Format, target: &TargetSpec) -> Self {
        Self {
            format,
            id: target.id.clone(),
            role: target.role.clone().or_else(|| Some(format.to_string())),
            preset: target.preset.clone(),
            template: target.template.clone(),
            variant: target.variant.clone(),
            requirement: target.requirement,
            options: target.options.clone(),
        }
    }
}

pub struct ResolvedExecution {
    plan: ExecutionPlan,
    spec: SpecV2,
    source_version: SourceSpecVersion,
    source: SourceSpec,
    source_path: PathBuf,
    source_format: Format,
    source_profile: ResolvedArtifactProfile,
    targets: Vec<ResolvedTarget>,
    dag: MultiTargetDag,
    executor: DagExecutor,
    tool_registry: ToolRegistry,
    resume_checkpoints: bool,
    cancellation: Option<Arc<AtomicBool>>,
}

impl ResolvedExecution {
    pub fn plan(&self) -> &ExecutionPlan {
        &self.plan
    }

    pub fn spec(&self) -> &SpecV2 {
        &self.spec
    }

    pub fn source_version(&self) -> SourceSpecVersion {
        self.source_version
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn source_format(&self) -> Format {
        self.source_format
    }

    pub fn target_formats(&self) -> Vec<Format> {
        self.targets.iter().map(|target| target.format).collect()
    }

    pub fn targets(&self) -> &[ResolvedTarget] {
        &self.targets
    }

    pub fn dag(&self) -> &MultiTargetDag {
        &self.dag
    }

    pub fn predicted_output_paths(&self) -> Result<Vec<PathBuf>> {
        render_output_paths(self)
    }

    pub(crate) fn with_resume(mut self, resume: bool) -> Self {
        self.resume_checkpoints = resume;
        self
    }

    pub(crate) fn with_cancellation_flag(mut self, cancellation: Arc<AtomicBool>) -> Self {
        self.cancellation = Some(cancellation);
        self
    }
}

#[derive(Debug, Clone)]
pub struct CanonicalExecutionResult {
    /// Exact frozen plan resolved before execution.
    pub plan: ExecutionPlan,
    pub output_dir: String,
    pub outputs: Vec<String>,
    pub diagnostics: Vec<String>,
    pub toolchain: Option<ToolchainSnapshot>,
    /// Authoritative evidence derived from the actual executor outcome.
    pub run_manifest: RunManifest,
    /// Persisted run-manifest path. Dry runs are side-effect free and return `None`.
    pub manifest_path: Option<String>,
}

impl CanonicalExecutionResult {
    pub fn is_success(&self) -> bool {
        matches!(
            self.run_manifest.state,
            RunState::Planned | RunState::Complete
        )
    }
}

fn effective_hygiene_policy(spec: &SpecV2) -> Result<Option<(String, HygienePolicy)>> {
    let policy_id = if let Some(policy_id) = &spec.execution.hygiene_policy {
        Some(policy_id.clone())
    } else {
        let profile_policies = spec
            .targets
            .profiles
            .iter()
            .filter_map(|profile_id| spec.profiles.get(profile_id))
            .filter_map(|profile| profile.hygiene_policy.as_deref())
            .collect::<HashSet<_>>();
        match profile_policies.len() {
            0 => None,
            1 => profile_policies.iter().next().map(|value| (*value).to_string()),
            _ => anyhow::bail!(
                "selected derivative profiles declare conflicting hygiene policies; set execution.hygiene_policy explicitly"
            ),
        }
    };
    policy_id
        .map(|policy_id| {
            let policy = spec.hygiene.get(&policy_id).cloned().with_context(|| {
                format!("hygiene policy '{policy_id}' is not declared in the specification")
            })?;
            Ok((policy_id, policy))
        })
        .transpose()
}

pub fn resolve(request: PlanningRequest) -> Result<ResolvedExecution> {
    let config_path = request
        .config_path
        .to_str()
        .context("Config path contains non-UTF8 characters")?;
    let loaded = load_spec(config_path)?;
    let source_version = loaded.source_version;
    let mut spec = loaded.spec;
    apply_request_overrides(&mut spec, &request)?;
    compose_selected_profiles(&mut spec)?;

    let source = select_primary_source(&spec)?;
    let source_path = resolve_path_relative_to_config(
        &request.config_path,
        source.path.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "canonical execution currently requires a local source path for '{}'",
                source.id
            )
        })?,
    );
    if !source_path.is_file() {
        anyhow::bail!(
            "source artifact '{}' does not exist or is not a file",
            source_path.display()
        );
    }
    // Universal intake establishes immutable byte identity and multi-signal
    // evidence before the graph is selected. The temporary CAS is only a
    // planning staging area; execution imports the same bytes into its durable
    // run store and verifies the digest through normal artifact handling.
    let intake_staging = tempfile::tempdir().context("failed to create intake staging store")?;
    let intake_store = ArtifactStore::new(intake_staging.path())?;
    let mut intake_request = IntakeRequest::from_path(&source_path);
    if let Some(format) = &source.format {
        intake_request = intake_request.with_format(format.clone());
    }
    if let Some(media_type) = &source.media_type {
        intake_request = intake_request.with_media_type(media_type.clone());
    }
    let source_intake = IntakeEngine::new()
        .intake(&intake_request, &intake_store)
        .context("failed to establish source artifact identity before planning")?;
    let source_format = resolve_source_format(&source, &source_path, &source_intake.profile)?;

    let (mut graph, mut executor, mut tool_registry) =
        if let Some(transforms_path) = &spec.transforms {
            let transforms_path =
                resolve_path_relative_to_config(&request.config_path, transforms_path);
            let transforms_path_string = transforms_path
                .to_str()
                .context("transform registry path contains non-UTF8 characters")?;
            build_graph_executor_and_tools_from_yaml(transforms_path_string).with_context(|| {
                format!(
                    "failed to load transform registry '{}'",
                    transforms_path.display()
                )
            })?
        } else {
            (
                TransformGraph::new(),
                DagExecutor::new(),
                ToolRegistry::builtins(),
            )
        };

    register_builtin_strategy_edges(&mut graph, &mut tool_registry)?;
    let policy_graph = apply_execution_policy(&graph, &tool_registry, &spec);
    let mut targets = resolve_target_intent(&spec, &policy_graph, source_format)?;
    if targets.is_empty() {
        anyhow::bail!("target selection resolved to no executable artifact formats");
    }

    let provider_inventory = tool_registry.assess_ids_current(policy_graph.provider_ids());
    let available_graph =
        policy_graph.filtered_by_available_providers(&provider_inventory.available_ids());
    let available_formats = available_graph
        .reachable_from(source_format)
        .into_iter()
        .collect::<HashSet<_>>();
    let mut pruned_unavailable = Vec::new();
    targets.retain(|target| {
        let available = available_formats.contains(&target.format);
        if !available
            && (spec.targets.all_reachable || target.requirement == TargetRequirement::Optional)
        {
            pruned_unavailable.push(target.clone());
            false
        } else {
            true
        }
    });
    let mut pruned_budget = Vec::new();
    if let Some(max_artifacts) = spec.execution.budgets.max_artifacts {
        let limit = usize::try_from(max_artifacts).unwrap_or(usize::MAX);
        if targets.len() > limit {
            pruned_budget.extend(targets[limit..].iter().cloned());
            targets.truncate(limit);
        }
    }
    if targets.is_empty() {
        anyhow::bail!(
            "target selection resolved to no available branches; inspect the artifact-forest plan or relax provider/policy constraints"
        );
    }
    let target_formats: Vec<Format> = targets.iter().map(|target| target.format).collect();
    let optimization = spec.execution.optimization;
    let (dag, used_blocked_provider_fallback) = match available_graph
        .build_multi_target_dag_with_mode(source_format, &target_formats, optimization)
    {
        Some(dag) => (dag, false),
        None => {
            let dag = policy_graph
                .build_multi_target_dag_with_mode(source_format, &target_formats, optimization)
                .ok_or_else(|| {
                    unsupported_targets_error(&policy_graph, source_format, &target_formats)
                })?;
            (dag, true)
        }
    };

    register_builtin_strategy_executors(
        &mut executor,
        &dag,
        &spec,
        &targets,
        source_format,
        &source_path,
    )?;

    let mut plan = ExecutionPlan::from_dag(&dag, source_format, &target_formats, optimization);
    plan.attach_artifact_forest(build_artifact_forest(
        &spec,
        &policy_graph,
        source_format,
        &targets,
        &pruned_unavailable,
        &pruned_budget,
    ));
    plan.attach_source_artifact(&source_intake);
    if !source_intake.profile.conflicts.is_empty() {
        plan.add_tool_diagnostic(format!(
            "source intake reported {} conflicting format signal set(s); '{}' was selected",
            source_intake.profile.conflicts.len(),
            source_format
        ));
    }
    if source_version == SourceSpecVersion::V1 {
        plan.add_tool_diagnostic(
            "v1 configuration normalized into renderflow/v2 before canonical planning",
        );
    }
    if used_blocked_provider_fallback {
        plan.add_tool_diagnostic(
            "one or more selected paths require providers unavailable on this host; dry-run remains inspectable but execution preflight will fail until dependencies are available",
        );
    }

    let selected_ids = selected_provider_ids(&dag);
    let selected_inventory = tool_registry.assess_ids_current(selected_ids.iter());
    for blocked in selected_inventory
        .tools
        .iter()
        .filter(|availability| !availability.is_available())
    {
        plan.add_tool_diagnostic(format!(
            "selected provider unavailable: {}",
            blocked.summary()
        ));
    }
    if selected_inventory
        .tools
        .iter()
        .all(|tool| tool.is_available())
    {
        let context = ToolRuntimeContext::current();
        let snapshot = tool_registry.fingerprint_for_dag(&selected_inventory, &dag, &context)?;
        plan.attach_toolchain(snapshot);
    }

    let upscayl = select_upscayl_variants(&spec, &UpscaylModelCatalog::builtins());
    for diagnostic in upscayl.diagnostics {
        plan.add_tool_diagnostic(format!("{}: {}", diagnostic.code, diagnostic.message));
    }
    if !upscayl.variants.is_empty() {
        let names = upscayl
            .variants
            .iter()
            .map(|model| model.variant_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        plan.add_tool_diagnostic(format!(
            "resolved provider variants for {}: {}. Same-format derivative execution remains gated on the Transform v2 node-identity contract (#357).",
            upscayl.capability_id, names
        ));
    }

    Ok(ResolvedExecution {
        plan,
        spec,
        source_version,
        source,
        source_path,
        source_format,
        source_profile: source_intake.profile,
        targets,
        dag,
        executor,
        tool_registry,
        resume_checkpoints: false,
        cancellation: None,
    })
}

pub fn execute(mut resolved: ResolvedExecution, dry_run: bool) -> Result<CanonicalExecutionResult> {
    let started_at_unix_ms = unix_time_ms();
    let predicted = resolved.predicted_output_paths()?;
    if dry_run {
        let run_manifest = build_run_manifest(
            &resolved,
            started_at_unix_ms,
            RunState::Planned,
            predicted
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            Vec::new(),
            Vec::new(),
            plan_diagnostics(&resolved),
        )?;
        return Ok(CanonicalExecutionResult {
            plan: resolved.plan.clone(),
            output_dir: resolved.spec.output.bundle_root.clone(),
            outputs: run_manifest.artifact_manifest.outputs.clone(),
            diagnostics: resolved
                .plan
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect(),
            toolchain: resolved.plan.toolchain.clone(),
            run_manifest,
            manifest_path: None,
        });
    }

    let output_root = PathBuf::from(&resolved.spec.output.bundle_root);
    fs::create_dir_all(&output_root).with_context(|| {
        format!(
            "failed to create output directory '{}'",
            output_root.display()
        )
    })?;
    if let Err(error) = preflight_selected_providers(&resolved)
        .and_then(|_| validate_pre_execution_budgets(&resolved))
    {
        return failed_execution_result(
            &resolved,
            &output_root,
            started_at_unix_ms,
            Vec::new(),
            Vec::new(),
            "execution.preflight_failed",
            error,
        );
    }

    let state_parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let state_dir = state_parent.join(".renderflow");
    if let Err(error) = fs::create_dir_all(&state_dir)
        .with_context(|| format!("failed to create state directory '{}'", state_dir.display()))
    {
        return failed_execution_result(
            &resolved,
            &output_root,
            started_at_unix_ms,
            Vec::new(),
            Vec::new(),
            "execution.state_store_failed",
            error,
        );
    }
    let store = match ArtifactStore::new(state_dir.join("artifacts")) {
        Ok(store) => store,
        Err(error) => {
            return failed_execution_result(
                &resolved,
                &output_root,
                started_at_unix_ms,
                Vec::new(),
                Vec::new(),
                "execution.artifact_store_failed",
                error,
            )
        }
    };
    let source_artifact = match store.import_path(
        &resolved.source_path,
        ArtifactDescriptor::for_format(resolved.source_format, ArtifactStorageClass::Source)
            .with_metadata("renderflow.source_id", resolved.source.id.clone())
            .with_metadata("renderflow.intake.schema", crate::intake::INTAKE_SCHEMA_V1)
            .with_metadata(
                "renderflow.intake.profile",
                serde_json::to_value(&resolved.source_profile)?,
            ),
    ) {
        Ok(artifact) => artifact,
        Err(error) => {
            return failed_execution_result(
                &resolved,
                &output_root,
                started_at_unix_ms,
                Vec::new(),
                Vec::new(),
                "execution.source_import_failed",
                error,
            )
        }
    };
    if resolved
        .plan
        .source_artifact
        .as_ref()
        .is_some_and(|planned| planned.digest != source_artifact.digest().to_string())
    {
        return failed_execution_result(
            &resolved,
            &output_root,
            started_at_unix_ms,
            vec![source_artifact_evidence(&resolved, &source_artifact)],
            Vec::new(),
            "execution.source_changed_after_intake",
            anyhow::anyhow!(
                "source bytes changed after intake and before transform execution; re-plan the run"
            ),
        );
    }

    let executor = std::mem::take(&mut resolved.executor);
    let checkpoint_context = CheckpointContext {
        execution_plan_digest: sha256_serialized(&resolved.plan)?,
        source_spec_digest: sha256_serialized(&resolved.spec)?,
        toolchain_fingerprint: resolved
            .plan
            .toolchain
            .as_ref()
            .map(|snapshot| snapshot.fingerprint.clone()),
    };
    let mut executor = executor
        .with_cache(state_dir.join("canonical-cache.json"))
        .with_checkpoints(
            state_dir.join("checkpoints.json"),
            checkpoint_context,
            resolved.resume_checkpoints,
        )
        .with_max_parallel(resolved.spec.execution.max_parallel);
    if let Some(cancellation) = &resolved.cancellation {
        executor = executor.with_cancellation_flag(cancellation.clone());
    }
    if let Some(snapshot) = &resolved.plan.toolchain {
        executor = executor.with_toolchain_fingerprint(snapshot.fingerprint.clone());
        if let Err(error) = fs::write(
            state_dir.join("toolchain.json"),
            serde_json::to_vec_pretty(snapshot)?,
        ) {
            return failed_execution_result(
                &resolved,
                &output_root,
                started_at_unix_ms,
                vec![source_artifact_evidence(&resolved, &source_artifact)],
                Vec::new(),
                "execution.toolchain_evidence_failed",
                error.into(),
            );
        }
    }
    let mut report = match executor.execute_artifact_with_evidence(
        &resolved.dag,
        resolved.source_format,
        source_artifact.clone(),
        &store,
    ) {
        Ok(report) => report,
        Err(error) => {
            let source_evidence = source_artifact_evidence(&resolved, &source_artifact);
            return failed_execution_result(
                &resolved,
                &output_root,
                started_at_unix_ms,
                vec![source_evidence],
                Vec::new(),
                "execution.executor_failed",
                error,
            );
        }
    };
    enrich_step_versions(&mut report.steps, resolved.plan.toolchain.as_ref());

    let mut diagnostics = plan_diagnostics(&resolved);
    diagnostics.append(&mut report.diagnostics);
    let mut hygiene_evidence = HashMap::<String, HygieneEvidence>::new();
    let mut hygiene_blocked = HashSet::<String>::new();
    let mut hygiene_failures = false;
    if let Some((policy_id, policy)) = effective_hygiene_policy(&resolved.spec)? {
        let engine = HygieneEngine::new();
        let mut target_formats = resolved
            .targets
            .iter()
            .map(|target| target.format)
            .collect::<Vec<_>>();
        target_formats.sort_by_key(ToString::to_string);
        target_formats.dedup();
        for format in target_formats {
            let Some(candidate) = report.artifacts.get(&format).cloned() else {
                continue;
            };
            let started = unix_time_ms();
            let outcome = engine.apply(&policy_id, &policy, &candidate, &store)?;
            let completed = unix_time_ms();
            let step_id = format!("hygiene:{policy_id}:{format}");
            for finding in &outcome.evidence.findings {
                diagnostics.push(ExecutionDiagnostic {
                    severity: if finding.blocking {
                        DiagnosticSeverity::RecoverableFailure
                    } else {
                        DiagnosticSeverity::Warning
                    },
                    code: finding.code.clone(),
                    message: finding.message.clone(),
                    step_id: Some(step_id.clone()),
                });
            }
            if !outcome.releasable() {
                hygiene_failures = true;
                hygiene_blocked.insert(outcome.artifact.id().to_string());
            }
            let changed = !outcome.evidence.changed_field_classes.is_empty();
            report.steps.push(StepEvidence {
                step_id,
                transform: "publication.hygiene".to_string(),
                transform_version: env!("CARGO_PKG_VERSION").to_string(),
                capability: Some("artifact.publication-hygiene".to_string()),
                provider: Some("renderflow.core-hygiene".to_string()),
                input_artifacts: vec![candidate.id().to_string()],
                output_artifacts: vec![outcome.artifact.id().to_string()],
                configuration_digest: sha256_serialized(&policy)?,
                started_at_unix_ms: started,
                completed_at_unix_ms: completed,
                duration_ms: completed.saturating_sub(started),
                state: StepState::Complete,
                cache: crate::evidence::CacheDisposition::Miss,
                validation: ValidationState::NotRequested,
                fidelity: if changed {
                    FidelityDeclaration::Partial
                } else {
                    FidelityDeclaration::Lossless
                },
                skip_reason: None,
                diagnostics: Vec::new(),
            });
            hygiene_evidence.insert(outcome.artifact.id().to_string(), outcome.evidence.clone());
            report.artifacts.insert(format, outcome.artifact);
        }
    }
    let mut output_locators = HashMap::<String, String>::new();
    let mut validation_outcomes = HashMap::<String, ArtifactValidationOutcome>::new();
    let mut blocked_artifacts = hygiene_blocked;
    let mut actual_outputs = Vec::new();
    let mut target_failures = hygiene_failures;
    let mut fatal_validation_failure = false;

    if let Err(error) = validate_post_execution_budgets(&resolved, &report.artifacts) {
        target_failures = true;
        diagnostics.push(ExecutionDiagnostic {
            severity: DiagnosticSeverity::FatalFailure,
            code: "execution.post_budget_failed".to_string(),
            message: redact_sensitive_text(&error.to_string()),
            step_id: None,
        });
    }

    let validation_registry = ValidationRegistry::builtins();
    for target in &resolved.targets {
        let Some(artifact) = report.artifacts.get(&target.format) else {
            target_failures = true;
            diagnostics.push(ExecutionDiagnostic {
                severity: DiagnosticSeverity::RecoverableFailure,
                code: "execution.target_missing".to_string(),
                message: format!(
                    "Execution did not produce selected target '{}'",
                    target.format
                ),
                step_id: None,
            });
            continue;
        };

        let outcome = if resolved.spec.execution.validation.required {
            validation_registry.validate_in_store(
                artifact,
                target.format,
                &resolved.spec.execution.validation.validators,
                &store,
            )
        } else {
            ArtifactValidationOutcome {
                state: ValidationState::Skipped,
                validators: Vec::new(),
            }
        };
        let step_id = producing_step_id(artifact, &report.steps);
        if let Some(step) = report.steps.iter_mut().find(|step| {
            step.output_artifacts
                .iter()
                .any(|artifact_id| artifact_id == artifact.id().as_str())
        }) {
            step.validation = outcome.state;
        }
        for validator in &outcome.validators {
            for validator_diagnostic in &validator.diagnostics {
                let blocking = validator.state == ValidationState::Invalid
                    || (validator.state == ValidationState::Unavailable
                        && !resolved.spec.execution.validation.allow_unavailable);
                diagnostics.push(ExecutionDiagnostic {
                    severity: if blocking {
                        match resolved.spec.execution.validation.failure_mode {
                            ValidationFailureMode::Fatal => DiagnosticSeverity::FatalFailure,
                            ValidationFailureMode::BranchLocal => {
                                DiagnosticSeverity::RecoverableFailure
                            }
                        }
                    } else {
                        DiagnosticSeverity::Warning
                    },
                    code: validator_diagnostic.code.clone(),
                    message: format!(
                        "{} (validator {}@{}, provider {})",
                        validator_diagnostic.message,
                        validator.validator_id,
                        validator.validator_version,
                        validator.provider
                    ),
                    step_id: step_id.clone(),
                });
            }
        }
        let validation_blocked = outcome.state == ValidationState::Invalid
            || (outcome.state == ValidationState::Unavailable
                && !resolved.spec.execution.validation.allow_unavailable);
        let fidelity = producing_fidelity(artifact, &report.steps);
        let fidelity_blocked = resolved
            .spec
            .execution
            .reject_loss_classes
            .iter()
            .any(|class| rejects_fidelity(*class, fidelity));
        if fidelity_blocked {
            diagnostics.push(ExecutionDiagnostic {
                severity: match resolved.spec.execution.validation.failure_mode {
                    ValidationFailureMode::Fatal => DiagnosticSeverity::FatalFailure,
                    ValidationFailureMode::BranchLocal => DiagnosticSeverity::RecoverableFailure,
                },
                code: "fidelity.rejected_loss_class".to_string(),
                message: format!(
                    "Target '{}' has rejected fidelity class '{:?}'",
                    target.format, fidelity
                )
                .to_lowercase(),
                step_id,
            });
        }
        if validation_blocked || fidelity_blocked {
            target_failures = true;
            blocked_artifacts.insert(artifact.id().to_string());
            fatal_validation_failure |=
                resolved.spec.execution.validation.failure_mode == ValidationFailureMode::Fatal;
        }
        validation_outcomes.insert(artifact.id().to_string(), outcome);
    }

    for (target, destination) in resolved.targets.iter().zip(predicted.iter()) {
        let Some(artifact) = report.artifacts.get(&target.format) else {
            continue;
        };
        if fatal_validation_failure || blocked_artifacts.contains(artifact.id().as_str()) {
            continue;
        }
        if target_failures
            && diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "execution.post_budget_failed")
        {
            continue;
        }
        if let Err(error) = store.materialize(artifact, destination) {
            target_failures = true;
            diagnostics.push(ExecutionDiagnostic {
                severity: DiagnosticSeverity::RecoverableFailure,
                code: "execution.materialization_failed".to_string(),
                message: redact_sensitive_text(&error.to_string()),
                step_id: producing_step_id(artifact, &report.steps),
            });
            continue;
        }
        let locator = bundle_locator(&output_root, destination);
        output_locators.insert(artifact.id().to_string(), locator);
        actual_outputs.push(destination.display().to_string());
    }

    let artifacts = artifact_evidence(
        &resolved,
        &source_artifact,
        &report,
        &output_locators,
        &diagnostics,
        &validation_outcomes,
        &hygiene_evidence,
    );
    let has_failed_step = report
        .steps
        .iter()
        .any(|step| step.state == StepState::Failed);
    let has_cancelled_step = report
        .steps
        .iter()
        .any(|step| step.state == StepState::Cancelled);
    let has_failure = target_failures || has_failed_step;
    let state = if has_cancelled_step {
        RunState::Cancelled
    } else if has_failure && actual_outputs.is_empty() {
        RunState::Failed
    } else if has_failure {
        RunState::Partial
    } else {
        RunState::Complete
    };
    let run_manifest = build_run_manifest(
        &resolved,
        started_at_unix_ms,
        state,
        actual_outputs.clone(),
        artifacts,
        report.steps,
        diagnostics,
    )?;
    let manifest_path = persist_run_manifest(&output_root, &run_manifest)?;

    Ok(CanonicalExecutionResult {
        plan: resolved.plan.clone(),
        output_dir: resolved.spec.output.bundle_root.clone(),
        outputs: actual_outputs,
        diagnostics: run_manifest
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
        toolchain: resolved.plan.toolchain.clone(),
        run_manifest,
        manifest_path: Some(manifest_path.display().to_string()),
    })
}

/// Record a cancellation that occurs after planning but before transform execution.
pub fn cancelled(resolved: ResolvedExecution) -> Result<CanonicalExecutionResult> {
    let started_at_unix_ms = unix_time_ms();
    let output_root = PathBuf::from(&resolved.spec.output.bundle_root);
    fs::create_dir_all(&output_root).with_context(|| {
        format!(
            "failed to create output directory '{}' for cancellation evidence",
            output_root.display()
        )
    })?;
    let mut diagnostics = plan_diagnostics(&resolved);
    diagnostics.push(ExecutionDiagnostic {
        severity: DiagnosticSeverity::Cancelled,
        code: "execution.cancelled".to_string(),
        message: "Execution was cancelled before transforms started".to_string(),
        step_id: None,
    });
    let mut cancelled_steps = resolved
        .plan
        .edges
        .iter()
        .map(|edge| {
            Ok(StepEvidence {
                step_id: format!("step:{}-to-{}", edge.from, edge.to),
                transform: edge
                    .evidence
                    .get("transform_id")
                    .cloned()
                    .unwrap_or_else(|| format!("{}-to-{}", edge.from, edge.to)),
                transform_version: "unknown".to_string(),
                capability: edge.capability_id.clone(),
                provider: edge.provider_id.clone(),
                input_artifacts: Vec::new(),
                output_artifacts: Vec::new(),
                configuration_digest: sha256_serialized(edge)?,
                started_at_unix_ms,
                completed_at_unix_ms: started_at_unix_ms,
                duration_ms: 0,
                state: StepState::Cancelled,
                cache: crate::evidence::CacheDisposition::NotApplicable,
                validation: ValidationState::Skipped,
                fidelity: FidelityDeclaration::Unknown,
                skip_reason: Some("execution cancelled before transform started".to_string()),
                diagnostics: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    enrich_step_versions(&mut cancelled_steps, resolved.plan.toolchain.as_ref());
    let run_manifest = build_run_manifest(
        &resolved,
        started_at_unix_ms,
        RunState::Cancelled,
        Vec::new(),
        Vec::new(),
        cancelled_steps,
        diagnostics,
    )?;
    let manifest_path = persist_run_manifest(&output_root, &run_manifest)?;
    Ok(canonical_result(
        &resolved,
        run_manifest,
        Some(manifest_path.display().to_string()),
        Vec::new(),
    ))
}

fn failed_execution_result(
    resolved: &ResolvedExecution,
    output_root: &Path,
    started_at_unix_ms: u64,
    artifacts: Vec<ArtifactEvidence>,
    steps: Vec<StepEvidence>,
    code: &str,
    error: anyhow::Error,
) -> Result<CanonicalExecutionResult> {
    let mut diagnostics = plan_diagnostics(resolved);
    diagnostics.push(ExecutionDiagnostic {
        severity: DiagnosticSeverity::FatalFailure,
        code: code.to_string(),
        message: redact_sensitive_text(&error.to_string()),
        step_id: None,
    });
    let run_manifest = build_run_manifest(
        resolved,
        started_at_unix_ms,
        RunState::Failed,
        Vec::new(),
        artifacts,
        steps,
        diagnostics,
    )?;
    let manifest_path = persist_run_manifest(output_root, &run_manifest)?;
    Ok(canonical_result(
        resolved,
        run_manifest,
        Some(manifest_path.display().to_string()),
        Vec::new(),
    ))
}

fn canonical_result(
    resolved: &ResolvedExecution,
    run_manifest: RunManifest,
    manifest_path: Option<String>,
    outputs: Vec<String>,
) -> CanonicalExecutionResult {
    CanonicalExecutionResult {
        plan: resolved.plan.clone(),
        output_dir: resolved.spec.output.bundle_root.clone(),
        outputs,
        diagnostics: run_manifest
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
        toolchain: resolved.plan.toolchain.clone(),
        run_manifest,
        manifest_path,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_run_manifest(
    resolved: &ResolvedExecution,
    started_at_unix_ms: u64,
    state: RunState,
    outputs: Vec<String>,
    artifacts: Vec<ArtifactEvidence>,
    steps: Vec<StepEvidence>,
    diagnostics: Vec<ExecutionDiagnostic>,
) -> Result<RunManifest> {
    let execution_plan_digest = sha256_serialized(&resolved.plan)?;
    let source_spec_digest = sha256_serialized(&resolved.spec)?;
    let run_id = run_id(&execution_plan_digest, started_at_unix_ms);
    let mut artifact_forest = resolved.plan.artifact_forest.clone();
    if let Some(forest) = &mut artifact_forest {
        forest.produced_artifacts = artifacts
            .iter()
            .map(|artifact| artifact.artifact_id.clone())
            .collect();
        forest.produced_artifacts.sort();
        forest.produced_artifacts.dedup();
    }
    Ok(RunManifest {
        schema_version: RUN_MANIFEST_SCHEMA_V1.to_string(),
        run_id: run_id.clone(),
        execution_plan_digest,
        source_spec_digest,
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        started_at_unix_ms,
        completed_at_unix_ms: unix_time_ms(),
        state,
        artifact_manifest: ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_V1.to_string(),
            run_id,
            output_dir: resolved.spec.output.bundle_root.clone(),
            outputs,
            artifacts,
        },
        steps,
        diagnostics,
        toolchain: resolved.plan.toolchain.clone(),
        artifact_forest,
    })
}

fn persist_run_manifest(output_root: &Path, manifest: &RunManifest) -> Result<PathBuf> {
    let destination = output_root.join("renderflow-run.json");
    let mut temporary = tempfile::NamedTempFile::new_in(output_root).with_context(|| {
        format!(
            "failed to create temporary run manifest in '{}'",
            output_root.display()
        )
    })?;
    temporary
        .write_all(&serde_json::to_vec_pretty(manifest)?)
        .context("failed to write run manifest")?;
    temporary.flush().context("failed to flush run manifest")?;
    temporary
        .as_file()
        .sync_all()
        .context("failed to sync run manifest")?;
    temporary
        .persist(&destination)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "failed to atomically persist run manifest '{}'",
                destination.display()
            )
        })?;
    Ok(destination)
}

fn plan_diagnostics(resolved: &ResolvedExecution) -> Vec<ExecutionDiagnostic> {
    resolved
        .plan
        .diagnostics
        .iter()
        .map(|diagnostic| ExecutionDiagnostic {
            severity: match &diagnostic.level {
                DiagnosticLevel::Info => DiagnosticSeverity::Info,
                DiagnosticLevel::Warning => DiagnosticSeverity::Warning,
                DiagnosticLevel::Error => DiagnosticSeverity::FatalFailure,
            },
            code: "planning.diagnostic".to_string(),
            message: diagnostic.message.clone(),
            step_id: None,
        })
        .collect()
}

fn source_artifact_evidence(resolved: &ResolvedExecution, source: &Artifact) -> ArtifactEvidence {
    ArtifactEvidence::from_artifact(
        source,
        resolved
            .source
            .role
            .clone()
            .unwrap_or_else(|| resolved.source.id.clone()),
        ArtifactRole::Source,
        artifact_store_locator(source),
        ProducerEvidence::source(),
        ValidationState::NotRequested,
        FidelityDeclaration::Lossless,
    )
}

fn artifact_evidence(
    resolved: &ResolvedExecution,
    source: &Artifact,
    report: &DagExecutionReport,
    output_locators: &HashMap<String, String>,
    diagnostics: &[ExecutionDiagnostic],
    validation_outcomes: &HashMap<String, ArtifactValidationOutcome>,
    hygiene_evidence: &HashMap<String, HygieneEvidence>,
) -> Vec<ArtifactEvidence> {
    let mut evidence = vec![source_artifact_evidence(resolved, source)];
    let mut artifacts = report.artifacts.iter().collect::<Vec<_>>();
    artifacts.sort_by(|(left_format, left), (right_format, right)| {
        left_format
            .to_string()
            .cmp(&right_format.to_string())
            .then_with(|| left.id().as_str().cmp(right.id().as_str()))
    });
    for (format, artifact) in artifacts {
        if artifact.id() == source.id() {
            continue;
        }
        let target = resolved
            .targets
            .iter()
            .find(|target| target.format == *format);
        let lifecycle = if target.is_some() {
            ArtifactRole::Terminal
        } else {
            ArtifactRole::Intermediate
        };
        let role = target
            .and_then(|target| target.role.clone().or_else(|| target.id.clone()))
            .unwrap_or_else(|| artifact.format().to_string());
        let producing_step = report.steps.iter().find(|step| {
            step.output_artifacts
                .iter()
                .any(|artifact_id| artifact_id == artifact.id().as_str())
        });
        let outcome = validation_outcomes.get(artifact.id().as_str());
        let validation = outcome
            .map(|outcome| outcome.state)
            .unwrap_or(ValidationState::NotRequested);
        let producer = producing_step
            .map(|step| ProducerEvidence {
                system: "renderflow".to_string(),
                transform: Some(step.transform.clone()),
                capability: step.capability.clone(),
                provider: step.provider.clone(),
                version: Some(step.transform_version.clone()),
            })
            .unwrap_or_else(ProducerEvidence::source);
        let fidelity = producing_step
            .map(|step| step.fidelity)
            .unwrap_or(FidelityDeclaration::Unknown);
        let locator = output_locators
            .get(artifact.id().as_str())
            .cloned()
            .unwrap_or_else(|| artifact_store_locator(artifact));
        let mut artifact_evidence = ArtifactEvidence::from_artifact(
            artifact, role, lifecycle, locator, producer, validation, fidelity,
        );
        artifact_evidence.validation_evidence = outcome
            .map(|outcome| outcome.validators.clone())
            .unwrap_or_default();
        artifact_evidence.hygiene = hygiene_evidence.get(artifact.id().as_str()).cloned();
        if let Some(step) = producing_step {
            artifact_evidence.warnings = diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.severity == DiagnosticSeverity::Warning
                        && diagnostic.step_id.as_deref() == Some(step.step_id.as_str())
                })
                .map(|diagnostic| diagnostic.message.clone())
                .collect();
        }
        evidence.push(artifact_evidence);
    }
    evidence
}

fn artifact_store_locator(artifact: &Artifact) -> String {
    format!(
        "artifact-store:{}",
        artifact
            .payload()
            .relative_path()
            .to_string_lossy()
            .replace('\\', "/")
    )
}

fn bundle_locator(output_root: &Path, destination: &Path) -> String {
    let relative = destination.strip_prefix(output_root).unwrap_or(destination);
    format!("bundle:{}", relative.to_string_lossy().replace('\\', "/"))
}

fn producing_step_id(artifact: &Artifact, steps: &[StepEvidence]) -> Option<String> {
    steps
        .iter()
        .find(|step| {
            step.output_artifacts
                .iter()
                .any(|artifact_id| artifact_id == artifact.id().as_str())
        })
        .map(|step| step.step_id.clone())
}

fn producing_fidelity(artifact: &Artifact, steps: &[StepEvidence]) -> FidelityDeclaration {
    steps
        .iter()
        .find(|step| {
            step.output_artifacts
                .iter()
                .any(|artifact_id| artifact_id == artifact.id().as_str())
        })
        .map(|step| step.fidelity)
        .unwrap_or(FidelityDeclaration::Unknown)
}

fn rejects_fidelity(class: RejectedLossClass, fidelity: FidelityDeclaration) -> bool {
    matches!(
        (class, fidelity),
        (RejectedLossClass::Lossless, FidelityDeclaration::Lossless)
            | (RejectedLossClass::Partial, FidelityDeclaration::Partial)
            | (RejectedLossClass::Lossy, FidelityDeclaration::Lossy)
            | (
                RejectedLossClass::PathDependent,
                FidelityDeclaration::PathDependent
            )
            | (RejectedLossClass::Unknown, FidelityDeclaration::Unknown)
    )
}

fn enrich_step_versions(steps: &mut [StepEvidence], toolchain: Option<&ToolchainSnapshot>) {
    for step in steps {
        let version = step.provider.as_deref().and_then(|provider| {
            toolchain.and_then(|snapshot| {
                snapshot
                    .selected_tools
                    .iter()
                    .find(|tool| tool.id.as_str() == provider)
                    .and_then(|tool| tool.version.clone())
            })
        });
        step.transform_version = version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    }
}

fn apply_request_overrides(spec: &mut SpecV2, request: &PlanningRequest) -> Result<()> {
    if let Some(optimization) = request.optimization {
        spec.execution.optimization = optimization;
    }
    if let Some(target) = &request.target {
        let format: Format = target
            .parse()
            .with_context(|| format!("Unknown target format '{target}'"))?;
        spec.targets = TargetSelection {
            exact: vec![TargetSpec {
                id: Some(format!("cli.target.{}", format)),
                role: Some(format.to_string()),
                format: Some(format.to_string()),
                family: None,
                capability: None,
                transform: None,
                variant: None,
                preset: None,
                template: None,
                requirement: TargetRequirement::Required,
                options: Default::default(),
            }],
            intermediates: spec.targets.intermediates,
            ..TargetSelection::default()
        };
    } else if let Some(profile) = &request.profile {
        if profile == "everything" && !spec.profiles.contains_key(profile) {
            let bundled: DerivativeProfile =
                serde_yaml_ng::from_str(include_str!("../data/profiles/everything-v1.yaml"))
                    .context("bundled everything profile is invalid")?;
            spec.profiles.insert(profile.clone(), bundled);
        }
        if !spec.profiles.contains_key(profile) {
            anyhow::bail!("target profile '{profile}' is not defined");
        }
        spec.targets = TargetSelection {
            profiles: vec![profile.clone()],
            intermediates: spec.targets.intermediates,
            ..TargetSelection::default()
        };
    } else if request.all_reachable {
        let include = spec.targets.include.clone();
        let exclude = spec.targets.exclude.clone();
        spec.targets = TargetSelection {
            all_reachable: true,
            include,
            exclude,
            intermediates: spec.targets.intermediates,
            ..TargetSelection::default()
        };
    }
    merge_selector_set(&mut spec.targets.exclude, &request.exclude);
    Ok(())
}

fn compose_selected_profiles(spec: &mut SpecV2) -> Result<()> {
    let selected = spec.targets.profiles.clone();
    let mut cache = BTreeMap::new();
    for name in selected {
        let profile = resolve_profile(spec, &name, &mut Vec::new(), &mut cache)?;
        spec.targets.all_reachable |= profile.all_reachable;
        if let Some(intermediates) = profile.intermediates {
            spec.targets.intermediates = intermediates;
        }
        apply_profile_policy(&mut spec.execution, &profile.policy);
        spec.profiles.insert(name, profile);
    }
    Ok(())
}

fn resolve_profile(
    spec: &SpecV2,
    name: &str,
    stack: &mut Vec<String>,
    cache: &mut BTreeMap<String, DerivativeProfile>,
) -> Result<DerivativeProfile> {
    if let Some(profile) = cache.get(name) {
        return Ok(profile.clone());
    }
    if let Some(position) = stack.iter().position(|item| item == name) {
        let mut cycle = stack[position..].to_vec();
        cycle.push(name.to_string());
        anyhow::bail!(
            "derivative profile inheritance cycle: {}",
            cycle.join(" -> ")
        );
    }
    let declared = spec
        .profiles
        .get(name)
        .with_context(|| format!("target profile '{name}' is not defined"))?;
    if declared.schema != "renderflow.profile/v1" {
        anyhow::bail!(
            "profile '{name}' uses unsupported schema '{}'; expected renderflow.profile/v1",
            declared.schema
        );
    }
    stack.push(name.to_string());
    let mut result = DerivativeProfile::default();
    let mut inherited_hygiene = BTreeSet::new();
    for parent_name in &declared.extends {
        let parent = resolve_profile(spec, parent_name, stack, cache)?;
        if let Some(policy) = &parent.hygiene_policy {
            inherited_hygiene.insert(policy.clone());
        }
        merge_profile(&mut result, &parent);
    }
    stack.pop();
    if declared.hygiene_policy.is_none() && inherited_hygiene.len() > 1 {
        anyhow::bail!(
            "profile '{name}' inherits conflicting hygiene policies {}; declare hygiene_policy to resolve the conflict",
            inherited_hygiene.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    merge_profile(&mut result, declared);
    result.extends = declared.extends.clone();
    cache.insert(name.to_string(), result.clone());
    Ok(result)
}

fn merge_profile(destination: &mut DerivativeProfile, source: &DerivativeProfile) {
    destination.schema = source.schema.clone();
    if source.description.is_some() {
        destination.description = source.description.clone();
    }
    for target in &source.targets {
        if let Some(id) = &target.id {
            if let Some(existing) = destination
                .targets
                .iter_mut()
                .find(|candidate| candidate.id.as_ref() == Some(id))
            {
                *existing = target.clone();
                continue;
            }
        }
        if !destination.targets.contains(target) {
            destination.targets.push(target.clone());
        }
    }
    merge_selector_set(&mut destination.include, &source.include);
    merge_selector_set(&mut destination.exclude, &source.exclude);
    destination.all_reachable |= source.all_reachable;
    if source.intermediates.is_some() {
        destination.intermediates = source.intermediates;
    }
    if source.hygiene_policy.is_some() {
        destination.hygiene_policy = source.hygiene_policy.clone();
    }
    macro_rules! overlay {
        ($field:ident) => {
            if source.policy.$field.is_some() {
                destination.policy.$field = source.policy.$field.clone();
            }
        };
    }
    overlay!(validation);
    overlay!(minimum_fidelity);
    overlay!(requirements);
    overlay!(network);
    overlay!(ai);
    overlay!(budgets);
    overlay!(publication_policy);
    overlay!(redaction_policy);
}

fn apply_profile_policy(
    execution: &mut crate::spec::ExecutionPolicy,
    policy: &crate::spec::ProfilePolicy,
) {
    if let Some(value) = &policy.validation {
        execution.validation = value.clone();
    }
    if let Some(value) = policy.minimum_fidelity {
        execution.minimum_fidelity = Some(value);
    }
    if let Some(value) = &policy.requirements {
        execution.requirements = value.clone();
    }
    if let Some(value) = policy.network {
        execution.network = value;
    }
    if let Some(value) = policy.ai {
        execution.ai = value;
    }
    if let Some(value) = &policy.budgets {
        execution.budgets = value.clone();
    }
    if let Some(value) = &policy.publication_policy {
        execution.publication_policy = Some(value.clone());
    }
    if let Some(value) = &policy.redaction_policy {
        execution.redaction_policy = Some(value.clone());
    }
}

fn merge_selector_set(destination: &mut SelectorSet, source: &SelectorSet) {
    macro_rules! merge {
        ($field:ident) => {{
            destination.$field.extend(source.$field.iter().cloned());
            destination.$field.sort();
            destination.$field.dedup();
        }};
    }
    merge!(formats);
    merge!(families);
    merge!(capabilities);
    merge!(transforms);
    merge!(providers);
    merge!(roles);
    merge!(profiles);
    merge!(variants);
}

fn select_primary_source(spec: &SpecV2) -> Result<SourceSpec> {
    let artifacts: Vec<&SourceSpec> = spec
        .sources
        .iter()
        .filter(|source| source.kind == SourceKind::Artifact)
        .collect();
    if artifacts.len() != 1 {
        anyhow::bail!(
            "canonical format-DAG execution currently requires exactly one artifact source; found {}. Multi-root/collection source identity is reserved for the Transform v2 execution graph (#357).",
            artifacts.len()
        );
    }
    Ok(artifacts[0].clone())
}

fn resolve_path_relative_to_config(config_path: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        config_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

fn resolve_source_format(
    source: &SourceSpec,
    path: &Path,
    profile: &ResolvedArtifactProfile,
) -> Result<Format> {
    if let Some(format) = &source.format {
        return format
            .parse()
            .with_context(|| format!("unknown source format '{format}' for '{}'", source.id));
    }
    profile
        .format
        .as_deref()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "source '{}' is inspectable but its format is unknown; declare a format or install an intake provider for '{}'",
                source.id,
                path.display()
            )
        })?
        .parse()
        .with_context(|| "intake selected a format unavailable to the built-in graph")
}

fn register_builtin_strategy_edges(
    graph: &mut TransformGraph,
    tools: &mut ToolRegistry,
) -> Result<()> {
    let document_inputs = [
        Format::Markdown,
        Format::Html,
        Format::Docx,
        Format::Epub,
        Format::Rst,
        Format::Latex,
    ];
    let document_outputs = [Format::Html, Format::Pdf, Format::Docx];
    for from in document_inputs {
        for to in document_outputs {
            if from == to {
                continue;
            }
            let capability = transform_capability_id(from, to);
            let provider = ToolId::new("tool.pandoc")?;
            tools.add_capability(&provider, capability.clone())?;
            let mut edge = TransformEdge::new(from, to, 1.0, 0.97)
                .with_provider(provider.to_string(), capability.to_string())
                .with_evidence("adapter", BUILTIN_ADAPTER_EVIDENCE)
                .with_evidence("transform_id", format!("builtin.{from}.{to}"));
            if to == Format::Pdf {
                let tectonic = ToolId::new("tool.tectonic")?;
                tools.add_capability(&tectonic, capability)?;
                edge = edge.with_required_provider(tectonic.to_string());
            }
            graph.add_transform(edge);
        }
    }

    let image_formats = [
        Format::Jpeg,
        Format::Png,
        Format::Tiff,
        Format::Webp,
        Format::Gif,
        Format::Bmp,
        Format::Avif,
    ];
    for from in image_formats {
        for to in image_formats {
            if from == to || output_type_for_format(to).is_none() {
                continue;
            }
            add_ffmpeg_edge(graph, tools, from, to, "image")?;
        }
    }

    let audio_formats = [
        Format::Wav,
        Format::Aiff,
        Format::Bwf,
        Format::Pcm,
        Format::Flac,
        Format::M4aAlac,
        Format::Wv,
        Format::Ape,
        Format::Tta,
        Format::Dsf,
        Format::Dff,
        Format::Shn,
        Format::Mp3,
        Format::M4aAac,
        Format::Aac,
        Format::Ogg,
        Format::Opus,
        Format::Wma,
        Format::Amr,
        Format::Mp2,
        Format::Ra,
        Format::Oma,
        Format::Ac3,
        Format::Ec3,
        Format::Thd,
        Format::Dts,
        Format::DtsHd,
        Format::Midi,
        Format::Mod,
    ];
    for from in audio_formats {
        for to in audio_formats {
            if from == to || output_type_for_format(to).is_none() {
                continue;
            }
            add_ffmpeg_edge(graph, tools, from, to, "audio")?;
        }
    }
    Ok(())
}

fn add_ffmpeg_edge(
    graph: &mut TransformGraph,
    tools: &mut ToolRegistry,
    from: Format,
    to: Format,
    family: &str,
) -> Result<()> {
    let capability = transform_capability_id(from, to);
    let provider = ToolId::new("tool.ffmpeg")?;
    tools.add_capability(&provider, capability.clone())?;
    graph.add_transform(
        TransformEdge::new(from, to, 1.0, 0.92)
            .with_provider(provider.to_string(), capability.to_string())
            .with_evidence("adapter", BUILTIN_ADAPTER_EVIDENCE)
            .with_evidence("family", family)
            .with_evidence("transform_id", format!("builtin.{from}.{to}")),
    );
    Ok(())
}

fn apply_execution_policy(
    graph: &TransformGraph,
    tools: &ToolRegistry,
    spec: &SpecV2,
) -> TransformGraph {
    graph.filtered_by(|edge| edge_allowed(edge, tools, spec))
}

fn edge_allowed(edge: &TransformEdge, tools: &ToolRegistry, spec: &SpecV2) -> bool {
    if spec
        .execution
        .minimum_fidelity
        .is_some_and(|minimum| edge.quality < minimum)
    {
        return false;
    }
    let transform_id = edge.evidence.get("transform_id");
    if let Some(transform_id) = transform_id {
        if spec
            .execution
            .transforms
            .deny
            .iter()
            .any(|denied| denied == transform_id)
        {
            return false;
        }
        if !spec.execution.transforms.allow.is_empty()
            && !spec
                .execution
                .transforms
                .allow
                .iter()
                .any(|allowed| allowed == transform_id)
        {
            return false;
        }
    } else if !spec.execution.transforms.allow.is_empty() {
        return false;
    }

    for provider in edge_provider_ids(edge) {
        if spec
            .execution
            .tools
            .deny
            .iter()
            .any(|denied| denied == provider)
        {
            return false;
        }
        if !spec.execution.tools.allow.is_empty()
            && !spec
                .execution
                .tools
                .allow
                .iter()
                .any(|allowed| allowed == provider)
        {
            return false;
        }
        let Some(descriptor) = tools.get(provider) else {
            return false;
        };
        if spec.execution.requirements.deterministic
            && descriptor.determinism != ToolDeterminism::Deterministic
        {
            return false;
        }
        if spec.execution.requirements.local_only
            && !matches!(
                descriptor.locality,
                ToolLocality::Local | ToolLocality::LocalService
            )
        {
            return false;
        }
        if spec.execution.requirements.offline
            && !matches!(
                descriptor.locality,
                ToolLocality::Local | ToolLocality::LocalService
            )
        {
            return false;
        }
        if matches!(spec.execution.network, crate::spec::NetworkPolicy::Deny)
            && descriptor.locality == ToolLocality::NetworkRequired
        {
            return false;
        }
        if provider.starts_with("tool.ai.") {
            match spec.execution.ai {
                AiPolicy::Deny => return false,
                AiPolicy::LocalOnly
                    if !matches!(
                        descriptor.locality,
                        ToolLocality::Local | ToolLocality::LocalService
                    ) =>
                {
                    return false;
                }
                AiPolicy::LocalOnly | AiPolicy::Allow => {}
            }
        }
    }
    true
}

fn edge_provider_ids(edge: &TransformEdge) -> Vec<&str> {
    edge.provider_id
        .iter()
        .map(String::as_str)
        .chain(edge.required_provider_ids.iter().map(String::as_str))
        .collect()
}

fn resolve_target_intent(
    spec: &SpecV2,
    graph: &TransformGraph,
    source: Format,
) -> Result<Vec<ResolvedTarget>> {
    let reachable = graph.reachable_from(source);
    let mut selected = Vec::new();
    let mut profile_exclusions = SelectorSet::default();

    for target in &spec.targets.exact {
        extend_target_spec(&mut selected, target, graph, source, &reachable)?;
    }
    for profile_name in &spec.targets.profiles {
        let profile = spec
            .profiles
            .get(profile_name)
            .ok_or_else(|| anyhow::anyhow!("target profile '{profile_name}' is not defined"))?;
        for target in &profile.targets {
            extend_target_spec(&mut selected, target, graph, source, &reachable)?;
        }
        extend_selector(&mut selected, &profile.include, graph, source, &reachable)?;
        merge_selector_set(&mut profile_exclusions, &profile.exclude);
    }
    if spec.targets.all_reachable {
        for format in &reachable {
            insert_target(&mut selected, ResolvedTarget::generated(*format))?;
        }
    }
    if !spec.targets.include.is_empty() {
        selected.retain(|target| selector_matches(target, &spec.targets.include, graph));
    }
    apply_exclusions(&mut selected, &profile_exclusions, graph);
    apply_exclusions(&mut selected, &spec.targets.exclude, graph);
    selected.sort_by(|left, right| left.format.to_string().cmp(&right.format.to_string()));
    Ok(selected)
}

fn build_artifact_forest(
    spec: &SpecV2,
    graph: &TransformGraph,
    source: Format,
    selected: &[ResolvedTarget],
    unavailable: &[ResolvedTarget],
    budget_pruned: &[ResolvedTarget],
) -> ArtifactForest {
    let selected_formats = selected
        .iter()
        .map(|target| target.format)
        .collect::<HashSet<_>>();
    let unavailable_formats = unavailable
        .iter()
        .map(|target| target.format)
        .collect::<HashSet<_>>();
    let budget_pruned_formats = budget_pruned
        .iter()
        .map(|target| target.format)
        .collect::<HashSet<_>>();
    let mut exclusions = spec.targets.exclude.clone();
    for profile_name in &spec.targets.profiles {
        if let Some(profile) = spec.profiles.get(profile_name) {
            merge_selector_set(&mut exclusions, &profile.exclude);
        }
    }
    let mut formats = graph.reachable_from(source);
    formats.sort_by_key(ToString::to_string);
    let branches = formats
        .into_iter()
        .filter_map(|format| {
            let target = ResolvedTarget::generated(format);
            let (state, reason_code, reason) = if selected_formats.contains(&format) {
                (
                    ForestBranchState::Selected,
                    "branch.selected",
                    "reachable, policy-allowed, and provider-available",
                )
            } else if unavailable_formats.contains(&format) {
                (
                    ForestBranchState::Unavailable,
                    "branch.provider_unavailable",
                    "a required provider is unavailable on this host",
                )
            } else if budget_pruned_formats.contains(&format) {
                (
                    ForestBranchState::BudgetPruned,
                    "branch.max_artifacts",
                    "pruned by execution.budgets.max_artifacts",
                )
            } else if selector_matches(&target, &exclusions, graph) {
                (
                    ForestBranchState::Excluded,
                    "branch.selector_excluded",
                    "matched a profile, spec, or CLI exclusion selector",
                )
            } else if spec.targets.all_reachable {
                return None;
            } else {
                return None;
            };
            let resolved_target = selected
                .iter()
                .chain(unavailable.iter())
                .chain(budget_pruned.iter())
                .find(|candidate| candidate.format == format)
                .unwrap_or(&target);
            Some(ForestBranch {
                format: format.to_string(),
                role: resolved_target.role.clone(),
                requirement: match resolved_target.requirement {
                    TargetRequirement::Required => "required",
                    TargetRequirement::Optional => "optional",
                }
                .to_string(),
                options: resolved_target.options.clone(),
                state,
                reason_code: reason_code.to_string(),
                reason: reason.to_string(),
            })
        })
        .collect();
    ArtifactForest {
        profiles: spec
            .targets
            .profiles
            .iter()
            .map(|name| {
                let version = spec
                    .profiles
                    .get(name)
                    .map(|profile| profile.schema.as_str())
                    .unwrap_or("unknown");
                format!("{name}@{version}")
            })
            .collect(),
        intermediates: match spec.targets.intermediates {
            IntermediatePolicy::CacheOnly => "cache_only",
            IntermediatePolicy::Retain => "retain",
        }
        .to_string(),
        branches,
        produced_artifacts: Vec::new(),
    }
}

fn extend_target_spec(
    selected: &mut Vec<ResolvedTarget>,
    target: &TargetSpec,
    graph: &TransformGraph,
    source: Format,
    reachable: &[Format],
) -> Result<()> {
    let mut candidates: Vec<Format> = if let Some(format) = &target.format {
        vec![format
            .parse()
            .with_context(|| format!("unknown target format '{format}'"))?]
    } else {
        reachable.to_vec()
    };
    if let Some(family) = &target.family {
        candidates.retain(|format| format_in_family(*format, family));
    }
    if let Some(capability) = &target.capability {
        candidates.retain(|format| {
            graph
                .transforms_to(*format)
                .iter()
                .any(|edge| edge.capability_id.as_deref() == Some(capability.as_str()))
        });
        if candidates.is_empty()
            && capability == crate::super_resolution::SUPER_RESOLUTION_CAPABILITY_ID
        {
            candidates.push(source);
        }
    }
    if let Some(transform) = &target.transform {
        candidates.retain(|format| {
            graph.transforms_to(*format).iter().any(|edge| {
                edge.evidence.get("transform_id").map(String::as_str) == Some(transform.as_str())
            })
        });
    }
    for format in candidates {
        insert_target(selected, ResolvedTarget::from_spec(format, target))?;
    }
    Ok(())
}

fn extend_selector(
    selected: &mut Vec<ResolvedTarget>,
    selector: &SelectorSet,
    graph: &TransformGraph,
    _source: Format,
    reachable: &[Format],
) -> Result<()> {
    for format in reachable {
        let target = ResolvedTarget::generated(*format);
        if selector_matches(&target, selector, graph) {
            insert_target(selected, target)?;
        }
    }
    Ok(())
}

fn insert_target(selected: &mut Vec<ResolvedTarget>, candidate: ResolvedTarget) -> Result<()> {
    if let Some(existing) = selected
        .iter_mut()
        .find(|target| target.format == candidate.format)
    {
        if *existing == candidate || is_generated(existing) {
            if is_generated(existing) {
                *existing = candidate;
            }
            return Ok(());
        }
        if is_generated(&candidate) {
            return Ok(());
        }
        anyhow::bail!(
            "multiple distinct target configurations resolve to format '{}'; format-only DAG nodes cannot represent parallel same-format variants until Transform v2 (#357)",
            candidate.format
        );
    }
    selected.push(candidate);
    Ok(())
}

fn is_generated(target: &ResolvedTarget) -> bool {
    target.id.is_none()
        && target.preset.is_none()
        && target.template.is_none()
        && target.variant.is_none()
}

fn apply_exclusions(
    selected: &mut Vec<ResolvedTarget>,
    selector: &SelectorSet,
    graph: &TransformGraph,
) {
    if selector.is_empty() {
        return;
    }
    selected.retain(|target| !selector_matches(target, selector, graph));
}

fn selector_matches(
    target: &ResolvedTarget,
    selector: &SelectorSet,
    graph: &TransformGraph,
) -> bool {
    let format = target.format;
    let mut has_format_selector = false;
    let mut matches = false;
    if !selector.formats.is_empty() {
        has_format_selector = true;
        matches |= selector
            .formats
            .iter()
            .any(|value| value.parse::<Format>().ok() == Some(format));
    }
    if !selector.families.is_empty() {
        has_format_selector = true;
        matches |= selector
            .families
            .iter()
            .any(|family| format_in_family(format, family));
    }
    if !selector.capabilities.is_empty() {
        has_format_selector = true;
        matches |= graph.transforms_to(format).iter().any(|edge| {
            edge.capability_id
                .as_ref()
                .is_some_and(|capability| selector.capabilities.contains(capability))
        });
    }
    if !selector.transforms.is_empty() {
        has_format_selector = true;
        matches |= graph.transforms_to(format).iter().any(|edge| {
            edge.evidence
                .get("transform_id")
                .is_some_and(|transform| selector.transforms.contains(transform))
        });
    }
    if !selector.providers.is_empty() {
        has_format_selector = true;
        matches |= graph.transforms_to(format).iter().any(|edge| {
            edge.provider_id
                .as_ref()
                .is_some_and(|provider| selector.providers.contains(provider))
                || edge
                    .required_provider_ids
                    .iter()
                    .any(|provider| selector.providers.contains(provider))
        });
    }
    if !selector.roles.is_empty() {
        has_format_selector = true;
        matches |= target
            .role
            .as_ref()
            .is_some_and(|role| selector.roles.contains(role));
    }
    if !selector.profiles.is_empty() {
        has_format_selector = true;
    }
    if !has_format_selector && !selector.variants.is_empty() {
        return true;
    }
    matches
}

fn format_in_family(format: Format, family: &str) -> bool {
    let family = match family.to_ascii_lowercase().as_str() {
        "document" => FormatFamily::Document,
        "image" => FormatFamily::Image,
        "audio" => FormatFamily::Audio,
        "video" => FormatFamily::Video,
        "archive" => FormatFamily::Archive,
        "data" => FormatFamily::Data,
        "subtitle" => FormatFamily::Subtitle,
        "presentation" => FormatFamily::Presentation,
        "spreadsheet" => FormatFamily::Spreadsheet,
        _ => return false,
    };
    FormatCapabilityRegistry::global()
        .get(format)
        .is_some_and(|descriptor| descriptor.is_in_family(family))
}

fn unsupported_targets_error(
    graph: &TransformGraph,
    source: Format,
    targets: &[Format],
) -> anyhow::Error {
    let unreachable = targets
        .iter()
        .filter(|target| **target != source && graph.find_path(source, **target).is_none())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::anyhow!(
        "no policy-allowed transformation path from '{}' to requested target(s): {}",
        source,
        unreachable
    )
}

fn register_builtin_strategy_executors(
    executor: &mut DagExecutor,
    dag: &MultiTargetDag,
    spec: &SpecV2,
    targets: &[ResolvedTarget],
    source_format: Format,
    source_path: &Path,
) -> Result<()> {
    let source_root = source_path.parent().map(Path::to_path_buf);
    for edge in dag.all_edges() {
        if edge.evidence.get("adapter").map(String::as_str) != Some(BUILTIN_ADAPTER_EVIDENCE) {
            continue;
        }
        let target = targets.iter().find(|target| target.format == edge.to);
        let template = target.and_then(|target| target.template.clone());
        let profile = target.and_then(|target| target.preset.clone());
        let asset_root = if edge.from == source_format && document_input_format(edge.from).is_some()
        {
            source_root.clone()
        } else {
            None
        };
        let transform = StrategyArtifactTransform::new(
            edge.from,
            edge.to,
            template,
            profile,
            spec.variables.clone(),
            asset_root,
        )?;
        executor.register_artifact(edge.from, edge.to, Arc::new(transform));
    }
    Ok(())
}

fn selected_provider_ids(dag: &MultiTargetDag) -> BTreeSet<String> {
    dag.all_edges()
        .iter()
        .flat_map(|edge| {
            edge.provider_id
                .iter()
                .cloned()
                .chain(edge.required_provider_ids.iter().cloned())
        })
        .collect()
}

fn preflight_selected_providers(resolved: &ResolvedExecution) -> Result<()> {
    let ids = selected_provider_ids(&resolved.dag);
    let inventory = resolved.tool_registry.assess_ids_current(ids.iter());
    let blocked: Vec<String> = inventory
        .tools
        .iter()
        .filter(|tool| !tool.is_available())
        .map(|tool| tool.summary())
        .collect();
    if !blocked.is_empty() {
        anyhow::bail!(
            "execution preflight failed before any transform ran:\n{}",
            blocked.join("\n")
        );
    }
    Ok(())
}

fn validate_pre_execution_budgets(resolved: &ResolvedExecution) -> Result<()> {
    let budgets = &resolved.spec.execution.budgets;
    if let Some(max_depth) = budgets.max_depth {
        if resolved.plan.metadata.execution_depth as u32 > max_depth {
            anyhow::bail!(
                "execution plan depth {} exceeds max_depth budget {}",
                resolved.plan.metadata.execution_depth,
                max_depth
            );
        }
    }
    if let Some(max_artifacts) = budgets.max_artifacts {
        if resolved.plan.metadata.total_nodes as u64 > max_artifacts {
            anyhow::bail!(
                "execution plan artifact estimate {} exceeds max_artifacts budget {}",
                resolved.plan.metadata.total_nodes,
                max_artifacts
            );
        }
    }
    Ok(())
}

fn validate_post_execution_budgets(
    resolved: &ResolvedExecution,
    artifacts: &HashMap<Format, crate::artifact::Artifact>,
) -> Result<()> {
    let budgets = &resolved.spec.execution.budgets;
    let target_formats: HashSet<Format> = resolved
        .targets
        .iter()
        .map(|target| target.format)
        .collect();
    if let Some(max_output) = budgets.max_output_bytes {
        let total: u64 = artifacts
            .iter()
            .filter(|(format, _)| target_formats.contains(format))
            .map(|(_, artifact)| artifact.size_bytes())
            .sum();
        if total > max_output {
            anyhow::bail!(
                "produced output bytes {total} exceed max_output_bytes budget {max_output}"
            );
        }
    }
    if let Some(max_storage) = budgets.max_storage_bytes {
        let total: u64 = artifacts
            .values()
            .map(|artifact| artifact.size_bytes())
            .sum();
        if total > max_storage {
            anyhow::bail!(
                "execution artifact bytes {total} exceed max_storage_bytes budget {max_storage}"
            );
        }
    }
    Ok(())
}

fn render_output_paths(resolved: &ResolvedExecution) -> Result<Vec<PathBuf>> {
    let root = PathBuf::from(&resolved.spec.output.bundle_root);
    let source_stem = resolved
        .source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    let mut paths = Vec::new();
    let mut seen = HashMap::<PathBuf, usize>::new();
    for target in &resolved.targets {
        let relative = if resolved.source_version == SourceSpecVersion::V1 {
            PathBuf::from(format!("{source_stem}.{}", target.format))
        } else {
            let format_string = target.format.to_string();
            let target_role = target.role.as_deref().unwrap_or(format_string.as_str());
            let target_id = target.id.as_deref().unwrap_or(target_role);
            let source_role = resolved
                .source
                .role
                .as_deref()
                .unwrap_or(resolved.source.id.as_str());
            let mut rendered = resolved.spec.output.naming_template.clone();
            rendered = rendered.replace("{source.id}", &resolved.source.id);
            rendered = rendered.replace("{source.role}", source_role);
            rendered = rendered.replace("{target.id}", target_id);
            rendered = rendered.replace("{target.role}", target_role);
            rendered = rendered.replace("{target.format}", &format_string);
            rendered = rendered.replace("{ext}", &format_string);
            let path = PathBuf::from(rendered);
            validate_relative_output_path(&path)?;
            path
        };
        let mut destination = root.join(&relative);
        let count = seen.entry(destination.clone()).or_insert(0);
        if *count > 0 {
            match resolved.spec.output.collision {
                CollisionPolicy::Error => anyhow::bail!(
                    "multiple selected targets resolve to output path '{}'",
                    destination.display()
                ),
                CollisionPolicy::Replace => {}
                CollisionPolicy::Dedupe => {
                    destination = dedupe_path(&destination, *count + 1);
                }
            }
        }
        *count += 1;
        paths.push(destination);
    }
    Ok(paths)
}

fn validate_relative_output_path(path: &Path) -> Result<()> {
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        anyhow::bail!(
            "output naming template resolved outside bundle root: '{}'",
            path.display()
        );
    }
    Ok(())
}

fn dedupe_path(path: &Path, index: usize) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    let extension = path.extension().and_then(|value| value.to_str());
    let name = match extension {
        Some(extension) => format!("{stem}-{index}.{extension}"),
        None => format!("{stem}-{index}"),
    };
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::spec::{ExecutionPolicy, OutputLayout, SPEC_V2_ID};

    fn minimal_spec(source_format: &str) -> SpecV2 {
        SpecV2 {
            schema: SPEC_V2_ID.to_string(),
            sources: vec![SourceSpec {
                id: "source.main".to_string(),
                role: None,
                kind: SourceKind::Artifact,
                path: Some(format!("input.{source_format}")),
                uri: None,
                members: Vec::new(),
                media_type: None,
                format: Some(source_format.to_string()),
                detect: false,
                immutable: true,
            }],
            profiles: BTreeMap::new(),
            hygiene: BTreeMap::new(),
            targets: TargetSelection::default(),
            execution: ExecutionPolicy::default(),
            output: OutputLayout::default(),
            variables: BTreeMap::new(),
            transforms: None,
        }
    }

    #[test]
    fn exact_and_profile_targets_share_resolver() {
        let mut spec = minimal_spec("markdown");
        spec.targets.exact.push(TargetSpec {
            id: Some("target.html".to_string()),
            role: Some("web".to_string()),
            format: Some("html".to_string()),
            family: None,
            capability: None,
            transform: None,
            variant: None,
            preset: None,
            template: None,
            ..TargetSpec::default()
        });
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        let targets = resolve_target_intent(&spec, &graph, Format::Markdown).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].format, Format::Html);
        assert_eq!(targets[0].role.as_deref(), Some("web"));
    }

    #[test]
    fn all_reachable_include_and_exclude_use_same_selector_model() {
        let mut spec = minimal_spec("markdown");
        spec.targets.all_reachable = true;
        spec.targets.include.families.push("document".to_string());
        spec.targets.exclude.formats.push("pdf".to_string());
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 0.9));
        let targets = resolve_target_intent(&spec, &graph, Format::Markdown).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].format, Format::Html);
    }

    #[test]
    fn policy_filters_denied_provider_before_pathfinding() {
        let spec = minimal_spec("markdown");
        let mut graph = TransformGraph::new();
        graph.add_transform(
            TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0)
                .with_provider("tool.pandoc", "document.convert"),
        );
        let mut denied = spec.clone();
        denied.execution.tools.deny.push("tool.pandoc".to_string());
        let filtered = apply_execution_policy(&graph, &ToolRegistry::builtins(), &denied);
        assert!(filtered.transforms_from(Format::Markdown).is_empty());
    }

    #[test]
    fn output_template_rejects_parent_traversal() {
        assert!(validate_relative_output_path(Path::new("../escape.pdf")).is_err());
        assert!(validate_relative_output_path(Path::new("safe/output.pdf")).is_ok());
    }
}
