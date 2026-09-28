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
use crate::artifact::{
    Artifact, ArtifactCollection, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore,
};
use crate::checkpoint::CheckpointContext;
use crate::evidence::{
    redact_sensitive_text, run_id, sha256_serialized, unix_time_ms, ArtifactEvidence,
    ArtifactManifest, ArtifactRole, DiagnosticSeverity, ExecutionDiagnostic, FidelityDeclaration,
    ProducerEvidence, RunManifest, RunState, StepEvidence, StepState, ValidationDiagnostic,
    ValidationState, ValidatorEvidence, ARTIFACT_MANIFEST_SCHEMA_V1, RUN_MANIFEST_SCHEMA_V1,
};
use crate::fixed_layout_epub::{
    validate_publication as validate_fixed_epub_publication, FixedEpubPage,
    FixedLayoutEpubTransform, FIXED_EPUB_CAPABILITY, FIXED_EPUB_PROVIDER,
};
use crate::graph::capability::{FormatCapabilityRegistry, FormatFamily};
use crate::graph::{
    ArtifactForest, DagExecutionReport, DagExecutor, DiagnosticLevel, ExecutionPlan, ForestBranch,
    ForestBranchState, Format, InputKind, MultiTargetDag, PlanCollectionMember, PlanSourceArtifact,
    PlanSourceCollection, TransformEdge, TransformGraph,
};
use crate::hygiene::{HygieneEngine, HygieneEvidence};
use crate::intake::{IntakeEngine, IntakeReport, IntakeRequest, ResolvedArtifactProfile};
use crate::optimization::OptimizationMode;
use crate::print_pdf::{PrintInteriorPdfTransform, PRINT_PDF_CAPABILITY, PRINT_PDF_PROVIDER};
use crate::print_pdf_image::{inspect_print_image, PrintImageColorSpace};
use crate::print_pdf_inspect::ExpectedPrintPage;
use crate::publication::write_release_metadata;
use crate::spec::{
    load_spec, AiPolicy, CollisionPolicy, DerivativeProfile, FixedLayoutEpubPolicy, HygienePolicy,
    IntermediatePolicy, NetworkPolicy, PrintPdfInteriorPolicy, RejectedLossClass, SelectorSet,
    SourceKind, SourceSpec, SourceSpecVersion, SpecV2, TargetRequirement, TargetSelection,
    TargetSpec, ValidationFailureMode,
};
use crate::super_resolution::{select_upscayl_variants, UpscaylModelCatalog};
use crate::toolchain::{
    transform_capability_id, CapabilityId, ToolDeterminism, ToolDiscovery, ToolFidelity, ToolId,
    ToolLocality, ToolRegistry, ToolRuntimeContext, ToolVersionRequirement, ToolchainSnapshot,
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
    config_path: PathBuf,
    spec: SpecV2,
    source_version: SourceSpecVersion,
    source: SourceSpec,
    source_path: PathBuf,
    source_format: Format,
    source_members: Vec<ResolvedSourceMember>,
    print_pages: Option<Vec<ExpectedPrintPage>>,
    fixed_epub_pages: Option<Vec<FixedEpubPage>>,
    targets: Vec<ResolvedTarget>,
    dag: MultiTargetDag,
    executor: DagExecutor,
    tool_registry: ToolRegistry,
    resume_checkpoints: bool,
    cancellation: Option<Arc<AtomicBool>>,
}

#[derive(Debug, Clone)]
struct ResolvedSourceMember {
    spec: SourceSpec,
    path: PathBuf,
    intake: IntakeReport,
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

    /// Attach a cooperative cancellation flag to this resolved execution.
    /// Providers that support cancellation receive the same shared flag.
    pub fn with_cancellation_flag(mut self, cancellation: Arc<AtomicBool>) -> Self {
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
    let collection = source.kind == SourceKind::Collection;
    let config_path = if collection {
        fs::canonicalize(&request.config_path)
            .context("collection.source.config: unable to resolve spec path")?
    } else {
        request.config_path.clone()
    };
    let member_specs = if collection {
        source
            .members
            .iter()
            .map(|id| {
                spec.sources
                    .iter()
                    .find(|item| &item.id == id)
                    .cloned()
                    .with_context(|| format!("collection.member.unknown: '{id}'"))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        vec![source.clone()]
    };
    if let Some(publication) = &spec.publication {
        for artwork in &publication.artwork {
            let path = resolve_path_relative_to_config(&request.config_path, &artwork.path);
            if !path.is_file() {
                anyhow::bail!(
                    "publication artwork role '{}' does not resolve to a file: '{}'",
                    artwork.role,
                    path.display()
                );
            }
        }
    }
    // Universal intake establishes immutable byte identity and multi-signal
    // evidence before the graph is selected. The temporary CAS is only a
    // planning staging area; execution imports the same bytes into its durable
    // run store and verifies the digest through normal artifact handling.
    let intake_staging = tempfile::tempdir().context("failed to create intake staging store")?;
    let intake_store = ArtifactStore::new(intake_staging.path())?;
    let mut source_members = Vec::new();
    let mut source_format = None;
    for member in member_specs {
        let path = if collection {
            collection_member_path(&config_path, &member)?
        } else {
            let path = resolve_path_relative_to_config(
                &request.config_path,
                member.path.as_deref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "canonical execution requires a local source path for '{}'",
                        member.id
                    )
                })?,
            );
            if !path.is_file() {
                anyhow::bail!(
                    "source artifact '{}' does not exist or is not a file",
                    path.display()
                );
            }
            path
        };
        let intake = intake_source(&member, &path, &intake_store)?;
        let format = resolve_source_format(&member, &path, &intake.profile)?;
        if collection {
            let declared_media = member
                .media_type
                .as_deref()
                .context("collection.member.media: missing declared media type")?;
            let supported = FormatCapabilityRegistry::global()
                .get(format)
                .is_some_and(|descriptor| descriptor.media_types.contains(&declared_media));
            if !supported {
                anyhow::bail!(
                    "collection.member.unsupported_media: '{}' declares '{}' for format '{}'",
                    member.id,
                    declared_media,
                    format
                );
            }
            if !intake.profile.conflicts.is_empty() {
                anyhow::bail!(
                    "collection.member.format_conflict: '{}' has conflicting format signals",
                    member.id
                );
            }
            if member.sha256.as_deref() != Some(intake.source.digest().value()) {
                anyhow::bail!(
                    "collection.member.digest_mismatch: '{}' differs from declared sha256",
                    member.id
                );
            }
            if member.media_type.as_deref() != Some(intake.profile.media_type.as_str()) {
                anyhow::bail!(
                    "collection.member.media_mismatch: '{}' has media type '{}', expected {:?}",
                    member.id,
                    intake.profile.media_type,
                    member.media_type
                );
            }
            if source_format.is_some_and(|selected| selected != format) {
                anyhow::bail!("collection.member.unsupported_media: '{}' has format '{}'; collection members must share one format", member.id, format);
            }
        }
        source_format = Some(format);
        source_members.push(ResolvedSourceMember {
            spec: member,
            path,
            intake,
        });
    }
    let source_format = source_format.context("source collection has no members")?;
    let source_path = source_members[0].path.clone();
    let source_intake = &source_members[0].intake;
    let print_pages = resolve_print_pages(&spec, collection, source_format, &source_members)?;
    let fixed_epub_pages =
        resolve_fixed_epub_pages(&spec, collection, source_format, &source_members)?;

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
    if let Some(policy) = &spec.execution.print_pdf_interior {
        register_print_pdf_edge(&mut graph, &mut tool_registry, source_format, policy)?;
    }
    if let (Some(policy), Some(publication)) =
        (&spec.execution.fixed_layout_epub, &spec.publication)
    {
        register_fixed_epub_edge(
            &mut graph,
            &tool_registry,
            source_format,
            policy,
            publication,
        )?;
    }
    let policy_graph = apply_execution_policy(&graph, &tool_registry, &spec);
    let policy_graph = if collection {
        policy_graph
            .filtered_by(|edge| edge.from != source_format || edge.input_kind.is_collection())
    } else {
        policy_graph
    };
    let requested_targets = resolve_target_intent(&spec, &policy_graph, source_format)?;
    if requested_targets.is_empty() {
        anyhow::bail!("target selection resolved to no executable artifact formats");
    }
    validate_publication_target_roles(&spec, &requested_targets)?;
    if print_pages.is_some()
        && (requested_targets.len() != 1
            || requested_targets[0].format != Format::Pdf
            || requested_targets[0].role.as_deref() != Some("interior")
            || requested_targets[0].requirement != TargetRequirement::Required)
    {
        anyhow::bail!("print_pdf.target: exactly one required PDF target with role 'interior' must be selected");
    }
    if fixed_epub_pages.is_some()
        && (requested_targets.len() != 1
            || requested_targets[0].format != Format::Epub
            || requested_targets[0].role.as_deref() != Some("ebook")
            || requested_targets[0].requirement != TargetRequirement::Required)
    {
        anyhow::bail!("fixed_epub.target: exactly one required EPUB target with role 'ebook' must be selected");
    }

    let provider_inventory = tool_registry.assess_ids_current(policy_graph.provider_ids());
    let available_graph =
        policy_graph.filtered_by_available_providers(&provider_inventory.available_ids());
    let available_formats = available_graph
        .reachable_from(source_format)
        .into_iter()
        .collect::<HashSet<_>>();
    let (mut available_targets, pruned_unavailable) = partition_targets_by_availability(
        requested_targets,
        &available_formats,
        spec.targets.all_reachable,
    );
    let mut pruned_budget = Vec::new();
    if let Some(max_artifacts) = spec.execution.budgets.max_artifacts {
        let limit = usize::try_from(max_artifacts).unwrap_or(usize::MAX);
        if available_targets.len() > limit {
            pruned_budget.extend(available_targets[limit..].iter().cloned());
            available_targets.truncate(limit);
        }
    }
    if available_targets.is_empty() && pruned_unavailable.is_empty() {
        anyhow::bail!(
            "target selection resolved to no available branches; inspect the artifact-forest plan or relax provider/policy constraints"
        );
    }
    let (targets, used_unavailable_only_fallback) =
        planning_targets(&available_targets, &pruned_unavailable);
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
    if collection
        && (dag.all_edges().is_empty()
            || dag
                .all_edges()
                .iter()
                .any(|edge| edge.from == source_format && !edge.input_kind.is_collection())
            || targets.iter().any(|target| target.format == source_format))
    {
        anyhow::bail!("collection.transform.unsupported: a collection requires a registered collection-input transform and a distinct output format");
    }
    if print_pages.is_some()
        && (dag.all_edges().len() != 1
            || dag.all_edges()[0].capability_id.as_deref() != Some(PRINT_PDF_CAPABILITY))
    {
        anyhow::bail!(
            "print_pdf.route: selected plan must contain only the exact print-interior capability"
        );
    }
    if fixed_epub_pages.is_some()
        && (dag.all_edges().len() != 1
            || dag.all_edges()[0].capability_id.as_deref() != Some(FIXED_EPUB_CAPABILITY))
    {
        anyhow::bail!("fixed_epub.route: selected plan must contain only the exact fixed-layout EPUB capability");
    }

    register_builtin_strategy_executors(
        &mut executor,
        &dag,
        &spec,
        &targets,
        source_format,
        &source_path,
        &request.config_path,
    )?;

    let mut plan = ExecutionPlan::from_dag(&dag, source_format, &target_formats, optimization);
    plan.attach_artifact_forest(build_artifact_forest(
        &spec,
        &policy_graph,
        source_format,
        &available_targets,
        &pruned_unavailable,
        &pruned_budget,
    ));
    if collection {
        plan.source_collection = Some(PlanSourceCollection {
            source_id: source.id.clone(),
            members: source_members
                .iter()
                .map(|member| PlanCollectionMember {
                    source_id: member.spec.id.clone(),
                    locator: member.spec.path.clone().unwrap_or_default(),
                    media_type: member.intake.profile.media_type.clone(),
                    format: source_format.to_string(),
                    geometry: member.spec.geometry.clone(),
                    artifact: PlanSourceArtifact::from(&member.intake),
                })
                .collect(),
        });
    } else {
        plan.attach_source_artifact(source_intake);
    }
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
    if used_unavailable_only_fallback {
        plan.add_tool_diagnostic(
            "no provider-available branches were found; policy-allowed unavailable branches were retained so the plan remains inspectable, while execution stays blocked by provider preflight",
        );
    }
    if used_blocked_provider_fallback {
        plan.add_tool_diagnostic(
            "one or more planned paths require providers unavailable on this host; dry-run remains inspectable but execution preflight will fail until dependencies are available",
        );
    }

    let selected_ids = selected_provider_ids(&dag);
    let selected_inventory = tool_registry.assess_ids_current(selected_ids.iter());
    if let Some(policy) = &spec.execution.print_pdf_interior {
        validate_print_provider_version(&selected_inventory, policy)?;
    }
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
        config_path,
        spec,
        source_version,
        source,
        source_path,
        source_format,
        source_members,
        print_pages,
        fixed_epub_pages,
        targets,
        dag,
        executor,
        tool_registry,
        resume_checkpoints: false,
        cancellation: None,
    })
}

/// Separate provider-available targets from optional or all-reachable targets
/// whose registered paths cannot execute on the current host.
///
/// Required exact targets remain selected so the canonical blocked-provider
/// fallback and execution preflight can report their failure explicitly.
fn partition_targets_by_availability(
    targets: Vec<ResolvedTarget>,
    available_formats: &HashSet<Format>,
    all_reachable: bool,
) -> (Vec<ResolvedTarget>, Vec<ResolvedTarget>) {
    targets.into_iter().partition(|target| {
        available_formats.contains(&target.format)
            || (!all_reachable && target.requirement == TargetRequirement::Required)
    })
}

/// Keep an all-reachable plan inspectable when the current host provides none
/// of its policy-allowed branches.
///
/// The unavailable targets remain classified as unavailable in the artifact
/// forest. They are used here only to construct a truthful blocked plan; a
/// real execution still fails before any transform runs during provider
/// preflight.
fn planning_targets(
    available: &[ResolvedTarget],
    unavailable: &[ResolvedTarget],
) -> (Vec<ResolvedTarget>, bool) {
    if available.is_empty() && !unavailable.is_empty() {
        (unavailable.to_vec(), true)
    } else {
        (available.to_vec(), false)
    }
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
    let mut source_artifacts = Vec::new();
    for (index, member) in resolved.source_members.iter().enumerate() {
        let imported = (|| -> Result<Artifact> {
            if resolved.source.kind == SourceKind::Collection {
                collection_member_path(&resolved.config_path, &member.spec)?;
            }
            let descriptor = ArtifactDescriptor::for_format(
                resolved.source_format,
                ArtifactStorageClass::Source,
            )
            .with_metadata("renderflow.source_id", member.spec.id.clone())
            .with_metadata("renderflow.intake.schema", crate::intake::INTAKE_SCHEMA_V1)
            .with_metadata(
                "renderflow.intake.profile",
                serde_json::to_value(&member.intake.profile)?,
            );
            let descriptor = if resolved.source.kind == SourceKind::Collection {
                descriptor
                    .with_metadata("renderflow.collection.id", resolved.source.id.clone())
                    .with_metadata("renderflow.collection.index", index)
                    .with_metadata("renderflow.collection.locator", member.spec.path.clone())
                    .with_metadata(
                        "renderflow.collection.geometry",
                        serde_json::to_value(&member.spec.geometry)?,
                    )
            } else {
                descriptor
            };
            let artifact = store
                .import_path(&member.path, descriptor)
                .with_context(|| format!("collection.member.unreadable: '{}'", member.spec.id))?;
            if resolved.source.kind == SourceKind::Collection {
                collection_member_path(&resolved.config_path, &member.spec)?;
            }
            let planned_digest = if let Some(collection) = &resolved.plan.source_collection {
                &collection.members[index].artifact.digest
            } else {
                &resolved
                    .plan
                    .source_artifact
                    .as_ref()
                    .context("missing source artifact plan")?
                    .digest
            };
            if artifact.digest().to_string() != *planned_digest {
                anyhow::bail!(
                    "collection.member.changed: '{}' differs from planned bytes; re-plan",
                    member.spec.id
                );
            }
            Ok(artifact)
        })();
        match imported {
            Ok(artifact) => source_artifacts.push(artifact),
            Err(error) => {
                return failed_execution_result(
                    &resolved,
                    &output_root,
                    started_at_unix_ms,
                    source_evidence(&resolved, &source_artifacts),
                    Vec::new(),
                    "execution.source_changed_after_intake",
                    error,
                )
            }
        }
    }
    let source_artifact = source_artifacts[0].clone();

    let mut executor = std::mem::take(&mut resolved.executor);
    if let Some(pages) = &resolved.print_pages {
        let policy = resolved
            .spec
            .execution
            .print_pdf_interior
            .clone()
            .context("print_pdf.policy: policy missing from planned print execution")?;
        let transform = PrintInteriorPdfTransform::new(
            policy,
            pages.clone(),
            source_artifacts
                .iter()
                .map(|artifact| artifact.digest().to_string())
                .collect(),
            resolved.cancellation.clone(),
        )?;
        executor.register_collection_artifact(
            resolved.source_format,
            Format::Pdf,
            Arc::new(transform),
        );
    }
    if let Some(pages) = &resolved.fixed_epub_pages {
        let policy = resolved
            .spec
            .execution
            .fixed_layout_epub
            .clone()
            .context("fixed_epub.policy: policy missing from planned EPUB execution")?;
        let publication = resolved
            .spec
            .publication
            .clone()
            .context("fixed_epub.metadata.missing")?;
        let transform = FixedLayoutEpubTransform::new(
            policy,
            publication,
            pages.clone(),
            source_artifacts
                .iter()
                .map(|artifact| artifact.digest().to_string())
                .collect(),
            resolved.cancellation.clone(),
        )?;
        executor.register_collection_artifact(
            resolved.source_format,
            Format::Epub,
            Arc::new(transform),
        );
    }
    let checkpoint_context = CheckpointContext {
        execution_plan_digest: sha256_serialized(&resolved.plan)?,
        source_spec_digest: sha256_serialized(&resolved.spec)?,
        toolchain_fingerprint: resolved
            .plan
            .toolchain
            .as_ref()
            .map(|snapshot| snapshot.fingerprint.clone()),
    };
    let cache_name = if resolved.plan.source_collection.is_some() {
        format!(
            "canonical-cache-{}.json",
            checkpoint_context.execution_plan_digest.value
        )
    } else {
        "canonical-cache.json".to_string()
    };
    let mut executor = executor
        .with_cache(state_dir.join(cache_name))
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
                source_evidence(&resolved, &source_artifacts),
                Vec::new(),
                "execution.toolchain_evidence_failed",
                error.into(),
            );
        }
    }
    let execution = if resolved.source.kind == SourceKind::Collection {
        executor.execute_collection_with_evidence(
            &resolved.dag,
            resolved.source_format,
            ArtifactCollection::new(source_artifacts.clone()),
            &store,
        )
    } else {
        executor.execute_artifact_with_evidence(
            &resolved.dag,
            resolved.source_format,
            source_artifact.clone(),
            &store,
        )
    };
    let mut report = match execution {
        Ok(report) => report,
        Err(error) => {
            let source_evidence = source_evidence(&resolved, &source_artifacts);
            return failed_execution_result(
                &resolved,
                &output_root,
                started_at_unix_ms,
                source_evidence,
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

        let mut outcome = if resolved.print_pages.is_some() {
            if resolved.spec.execution.validation.validators.is_empty() {
                ArtifactValidationOutcome {
                    state: ValidationState::Valid,
                    validators: Vec::new(),
                }
            } else {
                validation_registry.validate_in_store(
                    artifact,
                    target.format,
                    &resolved.spec.execution.validation.validators,
                    &store,
                )
            }
        } else if resolved.spec.execution.validation.required {
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
        if resolved.print_pages.is_some() {
            let expected_sources = source_artifacts
                .iter()
                .map(|source| source.id())
                .collect::<Vec<_>>();
            let valid = artifact
                .metadata()
                .contains_key("renderflow.print_pdf.inspection")
                && artifact.sources().iter().collect::<Vec<_>>() == expected_sources;
            let state = if valid {
                ValidationState::Valid
            } else {
                ValidationState::Invalid
            };
            outcome.validators.push(ValidatorEvidence {
                validator_id: "validator.print_pdf.interior".to_string(),
                validator_version: env!("CARGO_PKG_VERSION").to_string(),
                provider: "renderflow.core".to_string(),
                state,
                diagnostics: if valid {
                    Vec::new()
                } else {
                    vec![ValidationDiagnostic {
                        code: "print_pdf.validation.evidence".to_string(),
                        message: "PDF inspection or ordered source lineage is missing".to_string(),
                    }]
                },
            });
            if !valid {
                outcome.state = ValidationState::Invalid;
            }
        }
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
        output_locators.insert(target_evidence_key(target), locator);
        actual_outputs.push(destination.display().to_string());
    }

    let artifacts = artifact_evidence(
        &resolved,
        &source_artifacts,
        &report,
        &output_locators,
        &diagnostics,
        &validation_outcomes,
        &hygiene_evidence,
    );
    let materialized_artifact_count = actual_outputs.len();
    if let Some(publication) = &resolved.spec.publication {
        let source_spec_digest = sha256_serialized(&resolved.spec)?;
        let execution_plan_digest = sha256_serialized(&resolved.plan)?;
        let sidecars = write_release_metadata(
            &output_root,
            publication,
            &artifacts,
            &actual_outputs,
            &source_spec_digest,
            &execution_plan_digest,
            resolved.plan.toolchain.as_ref(),
        )?;
        actual_outputs.extend(sidecars);
    }
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
    } else if has_failure && materialized_artifact_count == 0 {
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

fn source_evidence(resolved: &ResolvedExecution, sources: &[Artifact]) -> Vec<ArtifactEvidence> {
    resolved
        .source_members
        .iter()
        .zip(sources)
        .map(|(member, source)| {
            ArtifactEvidence::from_artifact(
                source,
                member
                    .spec
                    .role
                    .clone()
                    .unwrap_or_else(|| member.spec.id.clone()),
                ArtifactRole::Source,
                artifact_store_locator(source),
                ProducerEvidence::source(),
                ValidationState::NotRequested,
                FidelityDeclaration::Lossless,
            )
        })
        .collect()
}

fn artifact_evidence(
    resolved: &ResolvedExecution,
    sources: &[Artifact],
    report: &DagExecutionReport,
    output_locators: &HashMap<String, String>,
    diagnostics: &[ExecutionDiagnostic],
    validation_outcomes: &HashMap<String, ArtifactValidationOutcome>,
    hygiene_evidence: &HashMap<String, HygieneEvidence>,
) -> Vec<ArtifactEvidence> {
    let mut evidence = source_evidence(resolved, sources);
    let mut artifacts = report.artifacts.iter().collect::<Vec<_>>();
    artifacts.sort_by(|(left_format, left), (right_format, right)| {
        left_format
            .to_string()
            .cmp(&right_format.to_string())
            .then_with(|| left.id().as_str().cmp(right.id().as_str()))
    });
    for (format, artifact) in artifacts {
        if sources.iter().any(|source| artifact.id() == source.id())
            && !resolved
                .targets
                .iter()
                .any(|target| target.format == *format)
        {
            continue;
        }
        let targets = resolved
            .targets
            .iter()
            .filter(|target| target.format == *format)
            .collect::<Vec<_>>();
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
        let target_instances = if targets.is_empty() {
            vec![None]
        } else {
            targets.into_iter().map(Some).collect()
        };
        for target in target_instances {
            let lifecycle = if target.is_some() {
                ArtifactRole::Terminal
            } else {
                ArtifactRole::Intermediate
            };
            let role = target
                .and_then(|target| target.role.clone().or_else(|| target.id.clone()))
                .unwrap_or_else(|| artifact.format().to_string());
            let locator = target
                .and_then(|target| output_locators.get(&target_evidence_key(target)).cloned())
                .unwrap_or_else(|| artifact_store_locator(artifact));
            let mut item = ArtifactEvidence::from_artifact(
                artifact,
                role,
                lifecycle,
                locator,
                producer.clone(),
                validation,
                fidelity,
            );
            item.validation_evidence = outcome
                .map(|outcome| outcome.validators.clone())
                .unwrap_or_default();
            item.hygiene = hygiene_evidence.get(artifact.id().as_str()).cloned();
            if let Some(target) = target {
                item.metadata.insert(
                    "renderflow.target.id".to_string(),
                    serde_json::json!(target.id),
                );
                item.metadata.insert(
                    "renderflow.target.options".to_string(),
                    serde_json::json!(target.options),
                );
            }
            if let Some(step) = producing_step {
                item.warnings = diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic.severity == DiagnosticSeverity::Warning
                            && diagnostic.step_id.as_deref() == Some(step.step_id.as_str())
                    })
                    .map(|diagnostic| diagnostic.message.clone())
                    .collect();
            }
            evidence.push(item);
        }
    }
    evidence
}

fn target_evidence_key(target: &ResolvedTarget) -> String {
    target
        .id
        .clone()
        .or_else(|| target.role.clone())
        .unwrap_or_else(|| target.format.to_string())
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
        if matches!(
            profile.as_str(),
            "everything" | "magazine" | "coloring-book"
        ) && !spec.profiles.contains_key(profile)
        {
            let (source, label) = match profile.as_str() {
                "magazine" => (
                    include_str!("../data/profiles/magazine-v1.yaml"),
                    "magazine",
                ),
                "coloring-book" => (
                    include_str!("../data/profiles/coloring-book-v1.yaml"),
                    "coloring-book",
                ),
                _ => (
                    include_str!("../data/profiles/everything-v1.yaml"),
                    "everything",
                ),
            };
            let bundled: DerivativeProfile = serde_yaml_ng::from_str(source)
                .with_context(|| format!("bundled {label} profile is invalid"))?;
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
    let collections: Vec<&SourceSpec> = spec
        .sources
        .iter()
        .filter(|source| source.kind == SourceKind::Collection)
        .collect();
    if let [collection] = collections.as_slice() {
        let declared = spec
            .sources
            .iter()
            .filter(|source| source.kind == SourceKind::Artifact)
            .map(|source| source.id.as_str())
            .collect::<BTreeSet<_>>();
        let selected = collection
            .members
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if declared != selected {
            anyhow::bail!("collection.source.ambiguous: every declared artifact must occur exactly once in the selected collection");
        }
        return Ok((*collection).clone());
    }
    if collections.len() > 1 {
        anyhow::bail!("collection.source.ambiguous: canonical execution requires exactly one collection source");
    }
    let artifacts: Vec<&SourceSpec> = spec
        .sources
        .iter()
        .filter(|source| source.kind == SourceKind::Artifact)
        .collect();
    if artifacts.len() != 1 {
        anyhow::bail!(
            "canonical execution requires one artifact source or one explicitly ordered collection; found {} artifacts",
            artifacts.len()
        );
    }
    Ok(artifacts[0].clone())
}

fn collection_member_path(config_path: &Path, source: &SourceSpec) -> Result<PathBuf> {
    let locator = source.path.as_deref().ok_or_else(|| {
        anyhow::anyhow!("collection.member.locator: '{}' requires a path", source.id)
    })?;
    let relative = Path::new(locator);
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        anyhow::bail!(
            "collection.member.locator: '{}' must use a root-relative path without traversal",
            source.id
        );
    }
    let root = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        let metadata = fs::symlink_metadata(&path).with_context(|| {
            format!(
                "collection.member.missing: '{}' at '{}'",
                source.id,
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!(
                "collection.member.symlink: '{}' has a symlink component at '{}'",
                source.id,
                path.display()
            );
        }
    }
    if !path.is_file() {
        anyhow::bail!(
            "collection.member.not_file: '{}' at '{}'",
            source.id,
            path.display()
        );
    }
    Ok(path)
}

fn intake_source(source: &SourceSpec, path: &Path, store: &ArtifactStore) -> Result<IntakeReport> {
    let mut request = IntakeRequest::from_path(path);
    if let Some(format) = &source.format {
        request = request.with_format(format.clone());
    }
    if let Some(media_type) = &source.media_type {
        request = request.with_media_type(media_type.clone());
    }
    IntakeEngine::new()
        .intake(&request, store)
        .with_context(|| format!("source.intake.failed: '{}'", source.id))
}

fn resolve_print_pages(
    spec: &SpecV2,
    collection: bool,
    source_format: Format,
    members: &[ResolvedSourceMember],
) -> Result<Option<Vec<ExpectedPrintPage>>> {
    let Some(policy) = &spec.execution.print_pdf_interior else {
        return Ok(None);
    };
    policy.validate()?;
    if !collection || !matches!(source_format, Format::Png | Format::Jpeg) {
        anyhow::bail!("print_pdf.input.format: print interior requires a homogeneous ordered PNG or JPEG collection");
    }
    if spec.transforms.is_some() || spec.execution.hygiene_policy.is_some() {
        anyhow::bail!("print_pdf.policy: custom transforms and post-render hygiene are unsupported for this exact route");
    }
    if members.len() > policy.max_pages {
        anyhow::bail!("print_pdf.bounds: collection exceeds max_pages");
    }
    let input_bytes = members.iter().try_fold(0_u64, |sum, member| {
        sum.checked_add(member.intake.source.size_bytes())
            .context("print_pdf.bounds: input byte count overflow")
    })?;
    if input_bytes > policy.max_input_bytes {
        anyhow::bail!("print_pdf.bounds: collection exceeds max_input_bytes");
    }
    let mut pages = Vec::with_capacity(members.len());
    let mut common_geometry = None;
    for member in members {
        let geometry = member.spec.geometry.as_ref().with_context(|| {
            format!(
                "print_pdf.geometry.missing: '{}' requires explicit trim size and bleed",
                member.spec.id
            )
        })?;
        let bleed = geometry.bleed.with_context(|| {
            format!(
                "print_pdf.geometry.bleed: '{}' requires explicit bleed, including zero",
                member.spec.id
            )
        })?;
        if geometry.unit != "mm"
            || !geometry.width.is_finite()
            || !geometry.height.is_finite()
            || !bleed.is_finite()
            || geometry.width <= 0.0
            || geometry.height <= 0.0
            || bleed < 0.0
            || geometry.margin.is_some()
            || geometry.safe_area.is_some()
        {
            anyhow::bail!("print_pdf.geometry.unsupported: '{}' requires positive trim width/height in mm, nonnegative bleed, and no undeclared margin/safe-area policy", member.spec.id);
        }
        if let Some(common) = &common_geometry {
            if common != geometry {
                anyhow::bail!("print_pdf.geometry.mixed: current img2pdf route requires uniform page geometry");
            }
        } else {
            common_geometry = Some(geometry.clone());
        }
        let image = inspect_print_image(&member.path, source_format).with_context(|| {
            format!(
                "print_pdf.image.unreadable: '{}' did not pass image preflight",
                member.spec.id
            )
        })?;
        let media_width_pt = (geometry.width + 2.0 * bleed) * 72.0 / 25.4;
        let media_height_pt = (geometry.height + 2.0 * bleed) * 72.0 / 25.4;
        if !media_width_pt.is_finite()
            || !media_height_pt.is_finite()
            || media_width_pt > 14_400.0
            || media_height_pt > 14_400.0
        {
            anyhow::bail!("print_pdf.geometry.bounds: page exceeds PDF media bounds");
        }
        let scaled_width_pt =
            media_height_pt * f64::from(image.width_px) / f64::from(image.height_px);
        if (media_width_pt - scaled_width_pt).abs() > 0.02 {
            anyhow::bail!("print_pdf.scaling.aspect: '{}' aspect ratio would leave a border or require crop/stretch", member.spec.id);
        }
        pages.push(ExpectedPrintPage {
            media_width_pt,
            media_height_pt,
            trim_inset_pt: bleed * 72.0 / 25.4,
            pixel_width: image.width_px,
            pixel_height: image.height_px,
            rotation: 0,
            color_space: match image.color_space {
                PrintImageColorSpace::Rgb => "DeviceRGB".to_string(),
                PrintImageColorSpace::Gray => "DeviceGray".to_string(),
            },
            image_filter: match source_format {
                Format::Png => "FlateDecode".to_string(),
                Format::Jpeg => "DCTDecode".to_string(),
                _ => unreachable!("source format checked above"),
            },
            image_stream_sha256: image.image_stream_sha256,
        });
    }
    if let Some(publication) = &spec.publication {
        let geometry = common_geometry
            .as_ref()
            .context("print_pdf.geometry.missing")?;
        if &publication.geometry != geometry
            || publication
                .color_policy
                .as_deref()
                .is_some_and(|color| color != policy.color_policy)
        {
            anyhow::bail!("print_pdf.publication.constraints: publication geometry or color policy differs from the selected interior");
        }
        if let Some(role) = publication.output_roles.get("interior") {
            if role.format != "pdf"
                || role
                    .geometry
                    .as_ref()
                    .is_some_and(|declared| declared != geometry)
                || role
                    .color_policy
                    .as_deref()
                    .is_some_and(|color| color != policy.color_policy)
                || role.require_embedded_fonts
                || !role.validators.is_empty()
            {
                anyhow::bail!("print_pdf.publication.constraints: unsupported or inconsistent interior role constraint");
            }
            if let Some(dpi) = role.minimum_image_dpi {
                let media_width_in = pages[0].media_width_pt / 72.0;
                let media_height_in = pages[0].media_height_pt / 72.0;
                if pages.iter().any(|page| {
                    f64::from(page.pixel_width) / media_width_in < f64::from(dpi)
                        || f64::from(page.pixel_height) / media_height_in < f64::from(dpi)
                }) {
                    anyhow::bail!("print_pdf.publication.dpi: page is below minimum_image_dpi");
                }
            }
        }
    }
    Ok(Some(pages))
}

fn resolve_fixed_epub_pages(
    spec: &SpecV2,
    collection: bool,
    source_format: Format,
    members: &[ResolvedSourceMember],
) -> Result<Option<Vec<FixedEpubPage>>> {
    let Some(policy) = &spec.execution.fixed_layout_epub else {
        return Ok(None);
    };
    policy.validate()?;
    if spec.execution.print_pdf_interior.is_some() {
        anyhow::bail!("fixed_epub.policy: print PDF and fixed EPUB policies cannot be combined in one exact route");
    }
    if !collection || !matches!(source_format, Format::Png | Format::Jpeg) {
        anyhow::bail!("fixed_epub.input.format: only a homogeneous ordered PNG or JPEG collection is proven; SVG requires separately reviewed safe XML handling");
    }
    if spec.transforms.is_some()
        || spec.execution.hygiene_policy.is_some()
        || spec.execution.network != NetworkPolicy::Deny
        || spec.execution.ai != AiPolicy::Deny
    {
        anyhow::bail!("fixed_epub.policy: custom transforms, post-render hygiene, network, and AI execution are unsupported on this route");
    }
    if members.is_empty() || members.len() > policy.max_pages {
        anyhow::bail!("fixed_epub.bounds: collection has no pages or exceeds max_pages");
    }
    if policy.cover_member_id != members[0].spec.id {
        anyhow::bail!("fixed_epub.cover: cover_member_id must name the first ordered page");
    }
    let input_bytes = members.iter().try_fold(0_u64, |sum, member| {
        sum.checked_add(member.intake.source.size_bytes())
            .context("fixed_epub.bounds: input byte count overflow")
    })?;
    if input_bytes > policy.max_input_bytes {
        anyhow::bail!("fixed_epub.bounds: collection exceeds max_input_bytes");
    }
    let publication = spec
        .publication
        .as_ref()
        .context("fixed_epub.metadata.missing: publication contract is required")?;
    if serde_json::to_vec(publication)?.len() > 1024 * 1024 {
        anyhow::bail!("fixed_epub.bounds: publication metadata exceeds 1 MiB");
    }
    validate_fixed_epub_publication(publication)?;
    if let Some(role) = publication.output_roles.get("ebook") {
        if role.format != "epub"
            || role
                .geometry
                .as_ref()
                .is_some_and(|geometry| geometry != &publication.geometry)
        {
            anyhow::bail!("fixed_epub.publication.constraints: ebook role format or geometry differs from the EPUB route");
        }
    }
    let mut pages = Vec::with_capacity(members.len());
    for member in members {
        let geometry = member.spec.geometry.as_ref().with_context(|| {
            format!(
                "fixed_epub.geometry.missing: '{}' requires explicit page geometry",
                member.spec.id
            )
        })?;
        if geometry != &publication.geometry
            || geometry.unit != "mm"
            || !geometry.width.is_finite()
            || !geometry.height.is_finite()
            || geometry.width <= 0.0
            || geometry.height <= 0.0
            || geometry.width > 10_000.0
            || geometry.height > 10_000.0
            || geometry.bleed.is_some_and(|bleed| bleed != 0.0)
            || geometry.margin.is_some()
            || geometry.safe_area.is_some()
        {
            anyhow::bail!("fixed_epub.geometry.unsupported: '{}' requires matching positive millimeter publication geometry without bleed, margin, or safe area", member.spec.id);
        }
        let image = inspect_print_image(&member.path, source_format).with_context(|| {
            format!(
                "fixed_epub.image.unreadable: '{}' did not pass bounded image preflight",
                member.spec.id
            )
        })?;
        let pixel_ratio = f64::from(image.width_px) / f64::from(image.height_px);
        let page_ratio = geometry.width / geometry.height;
        if (pixel_ratio - page_ratio).abs() > 0.01 {
            anyhow::bail!(
                "fixed_epub.geometry.aspect: '{}' page and image aspect ratios differ",
                member.spec.id
            );
        }
        let source_path = member
            .spec
            .path
            .as_deref()
            .context("fixed_epub.metadata.alt_text")?;
        let mut matching = publication
            .artwork
            .iter()
            .filter(|artwork| artwork.role == "page" && artwork.path == source_path);
        let artwork = matching.next().with_context(|| {
            format!("fixed_epub.metadata.alt_text: '{}' requires artwork with role=page and matching source path", member.spec.id)
        })?;
        let alt_text = artwork.alt_text.as_deref().unwrap_or_default().trim();
        if matching.next().is_some() || alt_text.is_empty() || alt_text.len() > 4096 {
            anyhow::bail!("fixed_epub.metadata.alt_text: '{}' needs exactly one bounded nonempty page description", member.spec.id);
        }
        pages.push(FixedEpubPage {
            source_id: member.spec.id.clone(),
            format: source_format,
            width_px: image.width_px,
            height_px: image.height_px,
            alt_text: alt_text.to_string(),
            digest: member.intake.source.digest().value().to_string(),
        });
    }
    Ok(Some(pages))
}

fn register_fixed_epub_edge(
    graph: &mut TransformGraph,
    tools: &ToolRegistry,
    source_format: Format,
    policy: &FixedLayoutEpubPolicy,
    publication: &crate::publication::PublicationContract,
) -> Result<()> {
    tools
        .get(FIXED_EPUB_PROVIDER)
        .context("fixed_epub.provider: native provider descriptor missing")?;
    graph.add_transform(
        TransformEdge::with_input_kind(
            source_format,
            Format::Epub,
            0.1,
            1.0,
            InputKind::Collection,
        )
        .with_provider(FIXED_EPUB_PROVIDER, FIXED_EPUB_CAPABILITY)
        .with_variant(env!("CARGO_PKG_VERSION"))
        .with_evidence("transform_id", FIXED_EPUB_CAPABILITY)
        .with_evidence("fixed_epub_policy_sha256", sha256_serialized(policy)?.value)
        .with_evidence(
            "fixed_epub_publication_sha256",
            sha256_serialized(publication)?.value,
        ),
    );
    Ok(())
}

fn register_print_pdf_edge(
    graph: &mut TransformGraph,
    tools: &mut ToolRegistry,
    source_format: Format,
    policy: &PrintPdfInteriorPolicy,
) -> Result<()> {
    let mut descriptor = tools
        .get(PRINT_PDF_PROVIDER)
        .context("print_pdf.provider: img2pdf tool descriptor missing")?
        .clone();
    let patch = policy
        .provider_version
        .rsplit('.')
        .next()
        .context("print_pdf.provider: invalid version")?
        .parse::<u64>()?
        .checked_add(1)
        .context("print_pdf.provider: patch version overflow")?;
    let prefix = policy.provider_version.rsplit_once('.').unwrap().0;
    descriptor.discovery = ToolDiscovery::Executable {
        candidates: vec![policy.executable.clone()],
        version_args: vec!["--version".to_string()],
    };
    descriptor.version = ToolVersionRequirement {
        min_inclusive: Some(policy.provider_version.clone()),
        max_exclusive: Some(format!("{prefix}.{patch}")),
    };
    descriptor.determinism = ToolDeterminism::Deterministic;
    descriptor.locality = ToolLocality::Local;
    descriptor.fidelity = ToolFidelity::Lossless;
    tools.register(descriptor)?;
    let capability = CapabilityId::new(PRINT_PDF_CAPABILITY)?;
    tools.add_capability(&ToolId::new(PRINT_PDF_PROVIDER)?, capability)?;
    graph.add_transform(
        TransformEdge::with_input_kind(source_format, Format::Pdf, 0.1, 1.0, InputKind::Collection)
            .with_provider(PRINT_PDF_PROVIDER, PRINT_PDF_CAPABILITY)
            .with_evidence("transform_id", PRINT_PDF_CAPABILITY)
            .with_evidence("print_policy_sha256", sha256_serialized(policy)?.value),
    );
    Ok(())
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
        Format::Kepub,
        Format::Rst,
        Format::Latex,
    ];
    let document_outputs = [Format::Html, Format::Pdf, Format::Docx, Format::Epub];
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
    let kepub_capability = CapabilityId::new("ebook.convert.kepub")?;
    let kepubify = ToolId::new("tool.kepubify")?;
    tools.add_capability(&kepubify, kepub_capability.clone())?;
    graph.add_transform(
        TransformEdge::new(Format::Epub, Format::Kepub, 0.25, 0.98)
            .with_provider(kepubify.to_string(), kepub_capability.to_string())
            .with_evidence("adapter", BUILTIN_ADAPTER_EVIDENCE)
            .with_evidence("family", "ebooks")
            .with_evidence("transform_id", "builtin.epub.kepub"),
    );

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
    selected.sort_by_key(|item| item.format.to_string());
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
    let mut exclusions = spec.targets.exclude.clone();
    for profile_name in &spec.targets.profiles {
        if let Some(profile) = spec.profiles.get(profile_name) {
            merge_selector_set(&mut exclusions, &profile.exclude);
        }
    }
    let branch = |target: &ResolvedTarget,
                  state: ForestBranchState,
                  reason_code: &str,
                  reason: &str| ForestBranch {
        format: target.format.to_string(),
        role: target.role.clone(),
        requirement: match target.requirement {
            TargetRequirement::Required => "required",
            TargetRequirement::Optional => "optional",
        }
        .to_string(),
        options: target.options.clone(),
        state,
        reason_code: reason_code.to_string(),
        reason: reason.to_string(),
    };
    let mut branches = selected
        .iter()
        .map(|target| {
            branch(
                target,
                ForestBranchState::Selected,
                "branch.selected",
                "reachable, policy-allowed, and provider-available",
            )
        })
        .chain(unavailable.iter().map(|target| {
            branch(
                target,
                ForestBranchState::Unavailable,
                "branch.provider_unavailable",
                "a required provider is unavailable on this host",
            )
        }))
        .chain(budget_pruned.iter().map(|target| {
            branch(
                target,
                ForestBranchState::BudgetPruned,
                "branch.max_artifacts",
                "pruned by execution.budgets.max_artifacts",
            )
        }))
        .collect::<Vec<_>>();
    let represented = selected
        .iter()
        .chain(unavailable.iter())
        .chain(budget_pruned.iter())
        .map(|target| target.format)
        .collect::<HashSet<_>>();
    for format in graph.reachable_from(source) {
        let target = ResolvedTarget::generated(format);
        if !represented.contains(&format) && selector_matches(&target, &exclusions, graph) {
            branches.push(branch(
                &target,
                ForestBranchState::Excluded,
                "branch.selector_excluded",
                "matched a profile, spec, or CLI exclusion selector",
            ));
        }
    }
    branches.sort_by(|left, right| {
        left.format
            .cmp(&right.format)
            .then_with(|| left.role.cmp(&right.role))
    });
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
        let shares_render = existing.preset == candidate.preset
            && existing.template == candidate.template
            && existing.variant == candidate.variant
            && existing.options == candidate.options;
        if shares_render {
            selected.push(candidate);
            return Ok(());
        }
        anyhow::bail!(
            "multiple target roles resolve to format '{}' with different render configurations; use a shared preset/template/options or Transform v2 target nodes",
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

fn validate_publication_target_roles(spec: &SpecV2, targets: &[ResolvedTarget]) -> Result<()> {
    let Some(publication) = &spec.publication else {
        return Ok(());
    };
    for (role, constraints) in &publication.output_roles {
        let Some(target) = targets
            .iter()
            .find(|target| target.role.as_deref() == Some(role.as_str()))
        else {
            continue;
        };
        if target.format.to_string() != constraints.format {
            anyhow::bail!(
                "publication output role '{role}' expects format '{}' but the selected target resolves to '{}'",
                constraints.format,
                target.format
            );
        }
    }
    Ok(())
}

fn register_builtin_strategy_executors(
    executor: &mut DagExecutor,
    dag: &MultiTargetDag,
    spec: &SpecV2,
    targets: &[ResolvedTarget],
    source_format: Format,
    source_path: &Path,
    config_path: &Path,
) -> Result<()> {
    let source_root = source_path.parent().map(Path::to_path_buf);
    let mut variables = spec.variables.clone();
    if let Some(registry) = variables.get_mut(crate::font::FONT_REGISTRY_VARIABLE) {
        let resolved = resolve_path_relative_to_config(config_path, registry);
        *registry = resolved.to_string_lossy().into_owned();
    }
    for edge in dag.all_edges() {
        if edge.evidence.get("adapter").map(String::as_str) != Some(BUILTIN_ADAPTER_EVIDENCE) {
            continue;
        }
        let target = targets
            .iter()
            .find(|target| target.format == edge.to)
            .or_else(|| {
                (edge.to == Format::Epub
                    && dag.all_edges().iter().any(|candidate| {
                        candidate.from == Format::Epub && candidate.to == Format::Kepub
                    }))
                .then(|| targets.iter().find(|target| target.format == Format::Kepub))
                .flatten()
            });
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
            variables.clone(),
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
    if resolved.print_pages.is_some() && resolved.plan.toolchain.is_none() {
        anyhow::bail!("print_pdf.provider.unavailable: print interior was planned without an observed compatible img2pdf toolchain; re-plan when it is installed");
    }
    let ids = selected_provider_ids(&resolved.dag);
    let inventory = resolved.tool_registry.assess_ids_current(ids.iter());
    if let Some(policy) = &resolved.spec.execution.print_pdf_interior {
        validate_print_provider_version(&inventory, policy)?;
    }
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

fn validate_print_provider_version(
    inventory: &crate::toolchain::ToolInventory,
    policy: &PrintPdfInteriorPolicy,
) -> Result<()> {
    if let Some(provider) = inventory.get(PRINT_PDF_PROVIDER) {
        let expected_line = format!("img2pdf {}", policy.provider_version);
        if provider.is_available()
            && provider.version_line.as_deref() != Some(expected_line.as_str())
        {
            anyhow::bail!("print_pdf.provider.version: observed img2pdf version line does not exactly match the proven 0.6.3 release");
        }
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
            PathBuf::from(format!(
                "{source_stem}.{}",
                artifact_extension(target.format)
            ))
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
            rendered = rendered.replace("{ext}", &artifact_extension(target.format));
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
    if resolved.spec.publication.is_some() {
        paths.extend([
            root.join("metadata/publication.json"),
            root.join("metadata/manifest.json"),
            root.join("metadata/provenance.json"),
            root.join("metadata/preflight.json"),
            root.join("metadata/checksums.sha256"),
        ]);
    }
    Ok(paths)
}

fn artifact_extension(format: Format) -> String {
    match format {
        Format::Kepub => "kepub.epub".to_string(),
        _ => format.to_string(),
    }
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
                sha256: None,
                geometry: None,
                detect: false,
                immutable: true,
            }],
            profiles: BTreeMap::new(),
            hygiene: BTreeMap::new(),
            publication: None,
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
    fn all_reachable_partition_preserves_mixed_availability_states() {
        let requested = vec![
            ResolvedTarget::generated(Format::Html),
            ResolvedTarget::generated(Format::Pdf),
        ];
        let available_formats = HashSet::from([Format::Html]);

        let (available, unavailable) =
            partition_targets_by_availability(requested, &available_formats, true);
        let (planned, used_unavailable_only_fallback) = planning_targets(&available, &unavailable);

        assert_eq!(
            available
                .iter()
                .map(|target| target.format)
                .collect::<Vec<_>>(),
            [Format::Html]
        );
        assert_eq!(
            unavailable
                .iter()
                .map(|target| target.format)
                .collect::<Vec<_>>(),
            [Format::Pdf]
        );
        assert_eq!(
            planned
                .iter()
                .map(|target| target.format)
                .collect::<Vec<_>>(),
            [Format::Html]
        );
        assert!(!used_unavailable_only_fallback);
    }

    #[test]
    fn all_reachable_partition_retains_unavailable_only_plan_for_inspection() {
        let requested = vec![
            ResolvedTarget::generated(Format::Html),
            ResolvedTarget::generated(Format::Pdf),
        ];

        let (available, unavailable) =
            partition_targets_by_availability(requested, &HashSet::new(), true);
        let (planned, used_unavailable_only_fallback) = planning_targets(&available, &unavailable);

        assert!(available.is_empty());
        assert_eq!(
            unavailable
                .iter()
                .map(|target| target.format)
                .collect::<Vec<_>>(),
            [Format::Html, Format::Pdf]
        );
        assert_eq!(
            planned
                .iter()
                .map(|target| target.format)
                .collect::<Vec<_>>(),
            [Format::Html, Format::Pdf]
        );
        assert!(used_unavailable_only_fallback);
    }

    #[test]
    fn required_exact_target_keeps_blocked_provider_fallback() {
        let mut target = ResolvedTarget::generated(Format::Html);
        target.requirement = TargetRequirement::Required;

        let (available, unavailable) =
            partition_targets_by_availability(vec![target], &HashSet::new(), false);

        assert_eq!(available.len(), 1);
        assert_eq!(available[0].format, Format::Html);
        assert!(unavailable.is_empty());
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
