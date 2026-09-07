//! Canonical application-layer planning and execution lifecycle.
//!
//! All CLI and SDK build modes normalize v1/v2 intent here before execution.
//! Execution consumes a previously resolved [`ResolvedExecution`] and never
//! performs implicit target/path re-planning.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::adapters::strategy::{
    document_input_format, output_type_for_format, StrategyArtifactTransform,
};
use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass, ArtifactStore};
use crate::graph::capability::{FormatCapabilityRegistry, FormatFamily};
use crate::graph::{
    DagExecutor, ExecutionPlan, Format, MultiTargetDag, TransformEdge, TransformGraph,
};
use crate::optimization::OptimizationMode;
use crate::spec::{
    load_spec, AiPolicy, CollisionPolicy, SelectorSet, SourceKind, SourceSpec, SourceSpecVersion,
    SpecV2, TargetSelection, TargetSpec,
};
use crate::super_resolution::{select_upscayl_variants, UpscaylModelCatalog};
use crate::toolchain::{
    transform_capability_id, ToolDeterminism, ToolId, ToolLocality, ToolRegistry,
    ToolRuntimeContext, ToolchainSnapshot,
};
use crate::transforms::yaml_loader::build_graph_executor_and_tools_from_yaml;

const BUILTIN_ADAPTER_EVIDENCE: &str = "builtin.strategy";

#[derive(Debug, Clone)]
pub struct PlanningRequest {
    pub config_path: PathBuf,
    pub target: Option<String>,
    pub all_reachable: bool,
    pub optimization: Option<OptimizationMode>,
}

impl PlanningRequest {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
            target: None,
            all_reachable: false,
            optimization: None,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self.all_reachable = false;
        self
    }

    pub fn with_all_reachable(mut self) -> Self {
        self.target = None;
        self.all_reachable = true;
        self
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
    targets: Vec<ResolvedTarget>,
    dag: MultiTargetDag,
    executor: DagExecutor,
    tool_registry: ToolRegistry,
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
}

#[derive(Debug, Clone)]
pub struct CanonicalExecutionResult {
    /// Exact frozen plan resolved before execution.
    pub plan: ExecutionPlan,
    pub output_dir: String,
    pub outputs: Vec<String>,
    pub diagnostics: Vec<String>,
    pub toolchain: Option<ToolchainSnapshot>,
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
    let source_format = resolve_source_format(&source, &source_path)?;

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
    let targets = resolve_target_intent(&spec, &policy_graph, source_format)?;
    if targets.is_empty() {
        anyhow::bail!("target selection resolved to no executable artifact formats");
    }
    let target_formats: Vec<Format> = targets.iter().map(|target| target.format).collect();

    let provider_inventory = tool_registry.assess_ids_current(policy_graph.provider_ids());
    let available_graph =
        policy_graph.filtered_by_available_providers(&provider_inventory.available_ids());
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
        targets,
        dag,
        executor,
        tool_registry,
    })
}

pub fn execute(mut resolved: ResolvedExecution, dry_run: bool) -> Result<CanonicalExecutionResult> {
    let predicted = resolved.predicted_output_paths()?;
    if dry_run {
        return Ok(CanonicalExecutionResult {
            plan: resolved.plan.clone(),
            output_dir: resolved.spec.output.bundle_root.clone(),
            outputs: predicted
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            diagnostics: resolved
                .plan
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect(),
            toolchain: resolved.plan.toolchain.clone(),
        });
    }

    preflight_selected_providers(&resolved)?;
    validate_pre_execution_budgets(&resolved)?;

    let output_root = PathBuf::from(&resolved.spec.output.bundle_root);
    fs::create_dir_all(&output_root).with_context(|| {
        format!(
            "failed to create output directory '{}'",
            output_root.display()
        )
    })?;
    let state_parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let state_dir = state_parent.join(".renderflow");
    fs::create_dir_all(&state_dir)
        .with_context(|| format!("failed to create state directory '{}'", state_dir.display()))?;
    let store = ArtifactStore::new(state_dir.join("artifacts"))?;
    let source_artifact = store.import_path(
        &resolved.source_path,
        ArtifactDescriptor::for_format(resolved.source_format, ArtifactStorageClass::Source)
            .with_metadata("renderflow.source_id", resolved.source.id.clone()),
    )?;

    let executor = std::mem::take(&mut resolved.executor);
    let mut executor = executor
        .with_cache(state_dir.join("canonical-cache.json"))
        .with_max_parallel(resolved.spec.execution.max_parallel);
    if let Some(snapshot) = &resolved.plan.toolchain {
        executor = executor.with_toolchain_fingerprint(snapshot.fingerprint.clone());
        fs::write(
            state_dir.join("toolchain.json"),
            serde_json::to_vec_pretty(snapshot)?,
        )?;
    }
    let artifacts = executor.execute_artifact(
        &resolved.dag,
        resolved.source_format,
        source_artifact,
        &store,
    )?;

    validate_post_execution_budgets(&resolved, &artifacts)?;
    for (target, destination) in resolved.targets.iter().zip(predicted.iter()) {
        let artifact = artifacts.get(&target.format).ok_or_else(|| {
            anyhow::anyhow!(
                "execution plan completed without producing selected target '{}'",
                target.format
            )
        })?;
        if resolved.spec.execution.validation.required && artifact.size_bytes() == 0 {
            anyhow::bail!(
                "validation failed: target '{}' produced an empty artifact",
                target.format
            );
        }
        store.materialize(artifact, destination)?;
    }

    Ok(CanonicalExecutionResult {
        plan: resolved.plan.clone(),
        output_dir: resolved.spec.output.bundle_root.clone(),
        outputs: predicted
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        diagnostics: resolved
            .plan
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
        toolchain: resolved.plan.toolchain.clone(),
    })
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
            }],
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
    Ok(())
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

fn resolve_source_format(source: &SourceSpec, path: &Path) -> Result<Format> {
    if let Some(format) = &source.format {
        return format
            .parse()
            .with_context(|| format!("unknown source format '{format}' for '{}'", source.id));
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "source '{}' has no format and no detectable extension",
                source.id
            )
        })?;
    extension
        .parse()
        .with_context(|| format!("cannot infer a Renderflow format from extension '.{extension}'"))
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
        apply_exclusions(&mut selected, &profile.exclude, graph);
    }
    if spec.targets.all_reachable {
        for format in &reachable {
            insert_target(&mut selected, ResolvedTarget::generated(*format))?;
        }
    }
    if !spec.targets.include.is_empty() {
        selected.retain(|target| selector_matches(target.format, &spec.targets.include, graph));
    }
    apply_exclusions(&mut selected, &spec.targets.exclude, graph);
    selected.sort_by(|left, right| left.format.to_string().cmp(&right.format.to_string()));
    Ok(selected)
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
        if selector_matches(*format, selector, graph) {
            insert_target(selected, ResolvedTarget::generated(*format))?;
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
    selected.retain(|target| !selector_matches(target.format, selector, graph));
}

fn selector_matches(format: Format, selector: &SelectorSet, graph: &TransformGraph) -> bool {
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
