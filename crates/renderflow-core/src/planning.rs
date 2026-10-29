//! Canonical application-layer planning and execution lifecycle.
//!
//! [`build`] is the shared CLI/SDK entry point for graph and composition
//! sources. This module retains the graph-specific [`ResolvedExecution`] API;
//! both engines execute frozen state without implicit target/path re-planning.

pub mod build;
mod build_composition;
pub mod composition;
pub(crate) mod composition_evidence;
pub use composition::{
    execute_composition, resolve_composition, CompositionExecutionPlan,
    CompositionPlanningRequest, CompositionStagePlan, ResolvedCompositionExecution,
};

mod forest;
use forest::{plan_forest_routes, group_forest_routes, finalize_forest_identity, attach_forest_execution,
    hygiene_candidate_key, record_target_hygiene};
mod screenplay_recovery;
use screenplay_recovery::{recovery_provider_ids, register_recovery_edges,
    register_recovery_executors, validate_recovery_source, validate_recovery_targets};
mod screenplay_body;
use screenplay_body::{register_body_edges, register_body_executors,
    validate_body_source, validate_body_targets, SCREENPLAY_BODY_ADAPTER_EVIDENCE};

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};

use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc, Mutex};

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
use crate::hygiene::{HygieneEngine, HygieneEvidence, HygieneOutcome};
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
    SubtitlePolicy, TargetSpec, ValidationFailureMode,
};
use crate::speech::graph::{
    retained_artifacts, retained_artifact_name, SpeechArtifactTransform, SpeechProgressObserver,
    SpeechProgressPhase,
};
use crate::speech::policy::SpeechPolicy;
use crate::speech::recovery::{receipt_bytes, SAVED_RESULT_FILENAME, SAVED_RESULT_SCHEMA};
use crate::speech::WHISPER_PROVIDER_ID;
use crate::screenplay::graph::{native_format, ScreenplayArtifactTransform};
use crate::screenplay::body_graph::{
    retained_canonical as retained_body_canonical, validate_artifact as validate_screenplay_body_artifact,
    SCREENPLAY_BODY_CAPABILITY,
};
use crate::screenplay::afterwriting::{self, AfterwritingConfig, AFTERWRITING_PROVIDER_ID, NODE_PROVIDER_ID};
use crate::screenplay::afterwriting_graph::{
    validate_published_artifact, AfterwritingArtifactTransform, SCREENPLAY_PDF_CAPABILITY,
};
use crate::process::ProcessCancellationToken;
use crate::screenplay::provider::{supported_route, supported_version_line, ScripttoolConfig};
use crate::super_resolution::{select_upscayl_variants, UpscaylModelCatalog};
use crate::timed_text::graph::{
    supports_media_format, validate_text_export_from_canonical, SubtitleExtractionTransform,
    TimedTextArtifactTransform, SUBTITLE_ARTIFACT_METADATA_KEY,
};
use crate::toolchain::{
    transform_capability_id, CapabilityId, ToolDeterminism, ToolDiscovery, ToolFidelity, ToolId,
    ToolLocality, ToolRegistry, ToolRuntimeContext, ToolVersionRequirement, ToolchainSnapshot,
    ToolAvailability, ToolAvailabilityStatus, ToolInventory, ToolSupportTier,
};
use crate::transforms::yaml_loader::build_graph_executor_and_tools_from_yaml;
use crate::validation::{ArtifactValidationOutcome, ValidationRegistry};

/// Record actual execution separately from the plan's unobserved declarations.
struct SpeechRunObserver {
    downstream: Option<Arc<dyn SpeechProgressObserver>>,
    diagnostics: Mutex<Vec<ExecutionDiagnostic>>,
}

impl SpeechRunObserver {
    fn record(&self, code: &str, message: String) {
        self.diagnostics.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(ExecutionDiagnostic {
            severity: DiagnosticSeverity::Info, code: code.to_owned(), message, step_id: None,
        });
    }
}

impl SpeechProgressObserver for SpeechRunObserver {
    fn phase(&self, phase: SpeechProgressPhase) {
        match phase {
            SpeechProgressPhase::SavedResultHit => self.record("speech.saved_results.hit",
                "A verified historical result was selected from the local index; no current native tool or model was observed".to_owned()),
            SpeechProgressPhase::SavedResultMiss => self.record("speech.saved_results.miss",
                "No saved-result index entry exists for this source and recognition policy; native recognition may proceed".to_owned()),
            SpeechProgressPhase::Transcribe => self.record("speech.native.started",
                "Execution entered native speech recognition".to_owned()),
            _ => {}
        }
        if let Some(observer) = &self.downstream { observer.phase(phase); }
    }

    fn saved_result(&self, receipt_sha256: &str) {
        self.record("speech.saved_result.selected", format!(
            "Selected historical speech receipt SHA-256 {receipt_sha256}; execution revalidates the current source before restoring artifacts"));
        if let Some(observer) = &self.downstream { observer.saved_result(receipt_sha256); }
    }
}

const BUILTIN_ADAPTER_EVIDENCE: &str = "builtin.strategy";
const SUBTITLE_ADAPTER_EVIDENCE: &str = "builtin.timed_text";
const SUBTITLE_EXTRACTION_EVIDENCE: &str = "builtin.subtitle_extraction";
const SPEECH_ADAPTER_EVIDENCE: &str = "builtin.speech";
const SPEECH_CAPABILITY: &str = "speech.transcribe";
const TIMED_TEXT_PROVIDER: &str = "tool.renderflow-timed-text";
const SCREENPLAY_PROVIDER: &str = "tool.scripttool";
const SCREENPLAY_ADAPTER_EVIDENCE: &str = "builtin.screenplay.scripttool";
const SCREENPLAY_PDF_ADAPTER_EVIDENCE: &str = "builtin.screenplay.afterwriting";
const SCREENPLAY_RECOVERY_ADAPTER_EVIDENCE: &str = "builtin.screenplay.recovery";

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

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTarget {
    pub format: Format,
    pub id: Option<String>,
    pub role: Option<String>,
    pub preset: Option<String>,
    pub template: Option<String>,
    pub variant: Option<String>,
    pub capability: Option<String>,
    pub transform: Option<String>,
    pub provider: Option<String>,
    pub include_selectors: Vec<SelectorSet>,
    pub exclude_selectors: Vec<SelectorSet>,
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
            capability: None,
            transform: None,
            provider: None,
            include_selectors: Vec::new(),
            exclude_selectors: Vec::new(),
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
            capability: target.capability.clone(),
            transform: target.transform.clone(),
            provider: target.provider.clone(),
            include_selectors: Vec::new(),
            exclude_selectors: Vec::new(),
            requirement: target.requirement,
            options: target.options.clone(),
        }
    }

    /// Stable output identity. Required/optional selection does not change bytes.
    /// Explicit roles and ids are distinct publication destinations.
    pub fn identity(&self) -> String {
        let material = serde_json::json!({
            "schema": "renderflow/target-identity-v1", "format": self.format.to_string(),
            "id": self.id, "role": self.role, "preset": self.preset,
            "template": self.template, "variant": self.variant, "options": self.options,
            "capability": self.capability, "transform": self.transform, "provider": self.provider,
            "include": self.include_selectors, "exclude": self.exclude_selectors,
        });
        format!("target-{:x}", Sha256::digest(serde_json::to_vec(&material)
            .expect("target identity is JSON serializable")))
    }
}

struct ResolvedExecutionGroup {
    dag: MultiTargetDag,
    targets: Vec<ResolvedTarget>,
    executor: DagExecutor,
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
    execution_groups: Vec<ResolvedExecutionGroup>,
    tool_registry: ToolRegistry,
    resume_checkpoints: bool,
    cancellation: Option<Arc<AtomicBool>>,
    speech_observer: Option<Arc<dyn SpeechProgressObserver>>,
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

    /// Compatibility projection of the first execution group; use dags() or
    /// plan().artifact_forest for a complete multi-rendition plan.
    pub fn dag(&self) -> &MultiTargetDag {
        &self.dag
    }

    /// Exact executable groups; distinct renditions never share a format-keyed DAG.
    pub fn dags(&self) -> impl Iterator<Item = &MultiTargetDag> {
        self.execution_groups.iter().map(|group| &group.dag)
    }

    pub fn predicted_output_paths(&self) -> Result<Vec<PathBuf>> {
        render_output_paths(self)
    }

    pub(crate) fn with_resume(mut self, resume: bool) -> Self {
        self.resume_checkpoints = resume;
        self
    }

    /// Receive bounded speech phases during native execution.
    pub fn with_speech_observer(mut self, observer: Arc<dyn SpeechProgressObserver>) -> Self {
        self.speech_observer = Some(observer);
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
    if loaded.spec.sources.iter().any(|source| source.kind == SourceKind::Composition) {
        anyhow::bail!("composition.build.entrypoint: use planning::build::resolve for composition sources");
    }
    resolve_loaded(request, loaded)
}

fn resolve_loaded(request: PlanningRequest, loaded: crate::spec::LoadedSpec) -> Result<ResolvedExecution> {
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
    if let Some(policy) = &spec.execution.screenplay_body {
        validate_body_source(source_format, collection, policy, &spec)?;
        if !source_intake.profile.conflicts.is_empty() {
            anyhow::bail!("screenplay_body.source.format_conflict: original Fountain source has conflicting format signals");
        }
    }
    if let Some(policy) = &spec.execution.screenplay_pdf {
        validate_screenplay_pdf_policy(source_format, collection, policy)?;
        validate_screenplay_pdf_hygiene(&spec)?;
        if !source_intake.profile.conflicts.is_empty() {
            anyhow::bail!("screenplay_pdf.source.format_conflict: original Fountain source has conflicting format signals");
        }
    }
    if let Some(policy) = &spec.execution.screenplay_recovery {
        validate_recovery_source(source_format, collection, policy, &spec)?;
        if !source_intake.profile.conflicts.is_empty() {
            anyhow::bail!("screenplay_recovery.source.format_conflict: original source has conflicting format signals");
        }
    }
    if let Some(policy) = &spec.execution.screenplay {
        validate_screenplay_policy(source_format, collection, policy)?;
        validate_screenplay_hygiene(&spec)?;
        if source_intake.profile.conflicts.iter().any(|conflict| {
            !screenplay_wrapper_conflict(source_format, source.format.as_deref(), &conflict.formats)
        }) {
            anyhow::bail!("screenplay.source.format_conflict: original source has conflicting format signals");
        }
    }
    if let Some(policy) = &spec.execution.speech {
        policy.validate_source(source_format, collection)?;
        if policy.source_sha256 != source_intake.source.digest().value() {
            anyhow::bail!("speech.source.digest_mismatch: original source differs from execution.speech.source_sha256");
        }
        if !source_intake.profile.conflicts.is_empty() {
            anyhow::bail!("speech.source.format_conflict: original source has conflicting format signals");
        }
        if spec.transforms.is_some() || spec.execution.hygiene_policy.is_some() {
            anyhow::bail!("speech.policy.route: custom transforms and post-transcription hygiene are unsupported for this bounded route");
        }
    }
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
    if let Some(policy) = &spec.execution.screenplay_body {
        register_body_edges(&mut graph, &mut tool_registry, source_format, policy)?;
        graph = graph.filtered_by(|edge| edge.evidence.get("adapter").map(String::as_str)
            == Some(SCREENPLAY_BODY_ADAPTER_EVIDENCE));
    }
    if let Some(policy) = &spec.execution.screenplay_recovery {
        register_recovery_edges(&mut graph, &mut tool_registry, source_format, policy)?;
        graph = graph.filtered_by(|edge| edge.evidence.get("adapter").map(String::as_str)
            == Some(SCREENPLAY_RECOVERY_ADAPTER_EVIDENCE));
    }
    if let Some(policy) = &spec.execution.screenplay_pdf {
        register_screenplay_pdf_edge(&mut graph, &mut tool_registry, source_format, policy)?;
        graph = graph.filtered_by(|edge| edge.evidence.get("adapter").map(String::as_str)
            == Some(SCREENPLAY_PDF_ADAPTER_EVIDENCE));
    }
    if let Some(policy) = &spec.execution.screenplay {
        register_screenplay_edges(&mut graph, &mut tool_registry, source_format, policy)?;
    }
    if let Some(policy) = &spec.execution.subtitles {
        register_subtitle_edges(
            &mut graph,
            &mut tool_registry,
            source_format,
            collection,
            policy,
        )?;
        // Plain transcript text deliberately loses timing and cue identity.
        // A custom registry must not turn this terminal into an inferred input.
        graph = graph.filtered_by(|edge| edge.from != Format::Text);
    }
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
    if let Some(policy) = &spec.execution.speech {
        register_speech_edges(&mut graph, &mut tool_registry, source_format, policy)?;
        // Recognition consumes the original artifact directly. Do not let an
        // unrelated media conversion silently change the waveform or identity.
        graph = graph.filtered_by(|edge| {
            edge.evidence.get("adapter").map(String::as_str) == Some(SPEECH_ADAPTER_EVIDENCE)
                || (edge.evidence.get("adapter").map(String::as_str) == Some(SUBTITLE_ADAPTER_EVIDENCE)
                    && edge.from == Format::TimedTranscriptJson
                    && matches!(edge.to, Format::Srt | Format::WebVtt | Format::Text))
        });
    }
    let policy_graph = apply_execution_policy(&graph, &tool_registry, &spec);
    let policy_graph = if collection {
        policy_graph
            .filtered_by(|edge| edge.from != source_format || edge.input_kind.is_collection())
    } else {
        policy_graph
    };
    let requested_targets = resolve_target_intent(&spec, &graph, source_format)?;
    if requested_targets.iter().any(|target| target.preset.as_deref() == Some(crate::strategies::editorial::EDITORIAL_PRESET)) {
        anyhow::ensure!(!collection && source_format == Format::Markdown,
            "editorial.source: select one canonical Markdown article");
        let mut pandoc = tool_registry.get("tool.pandoc").context("editorial.provider.missing")?.clone();
        pandoc.version.min_inclusive = Some(crate::strategies::editorial::MIN_PANDOC_VERSION.to_string());
        tool_registry.register(pandoc)?;
    }
    if spec.execution.screenplay_body.is_some() {
        validate_body_targets(&requested_targets)?;
    }
    if spec.execution.screenplay_recovery.is_some() {
        validate_recovery_targets(&requested_targets)?;
    }
    if spec.execution.speech.is_some() && requested_targets.iter().any(|target| {
        !matches!(target.format, Format::TimedTranscriptJson | Format::Srt | Format::WebVtt | Format::Text)
    }) {
        anyhow::bail!("speech.target.unsupported: speech targets are timed_transcript_json, srt, vtt, or text");
    }
    if spec.execution.screenplay.is_some() && requested_targets.iter().any(|target| {
        !matches!((native_format(source_format), native_format(target.format)),
            (Some(from), Some(to)) if supported_route(from, to))
    }) {
        anyhow::bail!("screenplay.target.unsupported: choose a directly supported scripttool conversion; same-format, transitive, PDF, text, and canonical screenplay JSON routes are not supplied by this policy");
    }
    if spec.execution.screenplay_pdf.is_some() {
        validate_screenplay_pdf_targets(&requested_targets)?;
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

    // Speech paths/hashes are declarations until their bounded native adapter
    // verifies them at execution. Planning never launches their version probes.
    let deferred_speech_ids = speech_provider_ids(spec.execution.speech.as_ref());
    let deferred_recovery_ids = recovery_provider_ids(source_format, spec.execution.screenplay_recovery.as_ref());
    let mut provider_inventory = tool_registry.assess_ids_current(
        policy_graph.provider_ids().into_iter().filter(|id|
            !deferred_speech_ids.contains(id) && !deferred_recovery_ids.contains(id)
                && !is_screenplay_pdf_provider(id, spec.execution.screenplay_pdf.as_ref())),
    );
    apply_screenplay_version_requirement(&mut provider_inventory, spec.execution.screenplay.as_ref());
    assess_screenplay_pdf_availability(
        &mut provider_inventory, &spec, &tool_registry, &policy_graph.all_edges(),
        |policy| afterwriting::preflight(policy, ProcessCancellationToken::default())
            .map(|observation| (observation.node_version, observation.provider_version))
            .map_err(|error| error.to_string()),
    );
    let mut planning_provider_ids = provider_inventory.available_ids();
    planning_provider_ids.extend(deferred_speech_ids.iter().cloned());
    planning_provider_ids.extend(deferred_recovery_ids.iter().cloned());
    let available_graph = policy_graph.filtered_by_available_providers(&planning_provider_ids);
    let planned_forest = plan_forest_routes(
        &spec, &graph, &policy_graph, &available_graph, source_format, requested_targets,
    )?;
    let targets = planned_forest.routes.iter().map(|(target, _)| target.clone()).collect::<Vec<_>>();
    let optimization = spec.execution.optimization;
    let grouped_routes = group_forest_routes(source_format, &planned_forest.routes)?;
    if grouped_routes.len() > 1 && (collection || spec.execution.speech.is_some()
        || spec.execution.screenplay.is_some() || spec.execution.screenplay_pdf.is_some()
        || spec.execution.screenplay_recovery.is_some() || spec.execution.screenplay_body.is_some()
        || spec.execution.subtitles.is_some() || print_pages.is_some() || fixed_epub_pages.is_some()) {
        anyhow::bail!("forest.native_group_conflict: this bounded native/collection policy requires one compatible execution group; use separate requests for distinct render settings");
    }
    let mut execution_groups = Vec::new();
    for (group_dag, group_targets) in grouped_routes {
        let mut group_executor = executor.clone();
        register_builtin_strategy_executors(
            &mut group_executor, &group_dag, &spec, &group_targets,
            source_format, &source_path, &request.config_path,
        )?;
        execution_groups.push(ResolvedExecutionGroup {
            dag: group_dag, targets: group_targets, executor: group_executor,
        });
    }
    // Format-only compatibility APIs expose the first group; forest routes and
    // dags() expose the complete plan without collapsing rendition identities.
    let dag = execution_groups[0].dag.clone();
    executor = execution_groups[0].executor.clone();
    if collection && !targets.is_empty()
        && (dag.all_edges().is_empty()
            || dag
                .all_edges()
                .iter()
                .any(|edge| edge.from == source_format && !edge.input_kind.is_collection())
            || targets.iter().any(|target| target.format == source_format))
    {
        anyhow::bail!("collection.transform.unsupported: a collection requires a registered collection-input transform and a distinct output format");
    }
    if print_pages.is_some() && !targets.is_empty()
        && (dag.all_edges().len() != 1
            || dag.all_edges()[0].capability_id.as_deref() != Some(PRINT_PDF_CAPABILITY))
    {
        anyhow::bail!(
            "print_pdf.route: selected plan must contain only the exact print-interior capability"
        );
    }
    if fixed_epub_pages.is_some() && !targets.is_empty()
        && (dag.all_edges().len() != 1
            || dag.all_edges()[0].capability_id.as_deref() != Some(FIXED_EPUB_CAPABILITY))
    {
        anyhow::bail!("fixed_epub.route: selected plan must contain only the exact fixed-layout EPUB capability");
    }

    if spec.execution.screenplay_pdf.is_some() && !targets.is_empty()
        && (dag.all_edges().len() != 1
            || dag.all_edges()[0].capability_id.as_deref() != Some(SCREENPLAY_PDF_CAPABILITY))
    {
        anyhow::bail!("screenplay_pdf.route: selected plan must contain only the original Fountain to PDF edge");
    }

    let branch_plans = execution_groups.iter().map(|group| ExecutionPlan::from_dag(
        &group.dag, source_format, &group.targets.iter().map(|target| target.format).collect::<Vec<_>>(), optimization,
    )).collect::<Vec<_>>();
    let mut plan = if branch_plans.len() == 1 { branch_plans[0].clone() } else {
        ExecutionPlan::from_branch_plans(&branch_plans, &targets.iter().map(|target| target.format.to_string()).collect::<Vec<_>>())
    };
    plan.metadata.output_count = targets.len();
    plan.attach_artifact_forest(planned_forest.evidence);
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
    let selected_edges = execution_groups.iter().flat_map(|group| group.dag.all_edges()).collect::<Vec<_>>();
    let selected_ids = selected_edges.iter().flat_map(|edge| edge_provider_ids(edge).into_iter().map(str::to_owned)).collect::<BTreeSet<_>>();
    let mut selected_inventory = tool_registry.assess_ids_current(
        selected_ids.iter().filter(|id|
            !deferred_speech_ids.contains(*id) && !deferred_recovery_ids.contains(*id)
                && !is_screenplay_pdf_provider(id, spec.execution.screenplay_pdf.as_ref())),
    );
    apply_screenplay_version_requirement(&mut selected_inventory, spec.execution.screenplay.as_ref());
    selected_inventory.tools.extend(provider_inventory.tools.iter().filter(|tool|
        is_screenplay_pdf_provider(tool.id.as_str(), spec.execution.screenplay_pdf.as_ref())
            && selected_ids.contains(tool.id.as_str())).cloned());
    if spec.execution.screenplay_recovery.is_some() {
        plan.add_tool_diagnostic("screenplay_recovery.provider.unobserved: recovery and its exporters are authored and unqualified. Native paths, versions, and digests are declarations checked only at execution; planning launches no probes. Every result remains lossy and review-required. No AI fallback, source PDF regeneration, cache, checkpoint, or observed-toolchain claim is supplied.");
    }
    if spec.execution.screenplay_body.is_some() {
        plan.add_tool_diagnostic("screenplay_body.provider.unqualified: direct original-Fountain body exports use the local built-in codec without native executables. The bounded semantic profile and retained canonical reference do not qualify native application opening, pagination, layout, or creative approval. Publication independently revalidates the original, output, policy and canonical evidence; cache and checkpoint reuse remain disabled.");
    }
    if spec.execution.screenplay_pdf.is_some() {
        plan.add_tool_diagnostic("screenplay_pdf.provider.unqualified: bounded preflight verifies configured Node/package pins, required package assets, and exact Node/package versions. This is availability evidence only; no PDF rendering or visual qualification occurs during planning. Execution independently inspects the emitted PDF; cache and checkpoint reuse remain disabled.");
    }
    if spec.execution.screenplay.is_some() {
        plan.add_tool_diagnostic("screenplay.provider.unqualified: availability reflects only the configured executable version probe. Execution verifies its SHA-256 pin and validates source/output semantic subsets. These authored routes are unqualified; no full-fidelity, toolchain-attestation, or reusable-cache claim is made.");
    }
    if let Some(policy) = &spec.execution.speech {
        if policy.recovery.is_some() {
            plan.add_tool_diagnostic("speech.saved_result.selected: execution will verify the explicitly pinned historical receipt and complete retained bundle before replay. Planning does not read the receipt or observe current native tools/models. Missing or incompatible evidence fails without native fallback; no current toolchain, recognition accuracy, or video-alignment claim is made.");
        } else if policy.saved_results.is_enabled() {
            plan.add_tool_diagnostic("speech.saved_results.unobserved: execution will inspect the local saved-result index after verifying the source. A verified historical match is replayed; a missing entry permits native recognition; a present invalid entry fails without fallback. Planning does not read the index or observe current native tools/models.");
        } else {
            plan.add_tool_diagnostic("speech.provider.unobserved: explicit provider/model/audio paths and digests are planned intent; native identity and availability are checked only during execution. No observed toolchain, automatically reusable result, calibrated confidence, or video-alignment claim is made.");
        }
    }
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
    if spec.execution.speech.is_none() && spec.execution.screenplay.is_none()
        && spec.execution.screenplay_pdf.is_none() && spec.execution.screenplay_recovery.is_none()
        && spec.execution.screenplay_body.is_none() && selected_inventory
        .tools
        .iter()
        .all(|tool| tool.is_available())
    {
        let context = ToolRuntimeContext::current();
        let snapshot = tool_registry.fingerprint_for_edges(&selected_inventory, &selected_edges, &context)?;
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

    finalize_forest_identity(&mut plan, &spec)?;

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
        execution_groups,
        tool_registry,
        resume_checkpoints: false,
        cancellation: None,
        speech_observer: None,
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
    register_subtitle_executors(
        &mut executor,
        &resolved.dag,
        resolved.spec.execution.subtitles.as_ref(),
        resolved.cancellation.clone(),
        resolved.spec.execution.speech.is_some(),
    )?;
    let speech_run_observer = Arc::new(SpeechRunObserver {
        downstream: resolved.speech_observer.clone(), diagnostics: Mutex::new(Vec::new()),
    });
    register_speech_executors(
        &mut executor, &resolved.dag, resolved.spec.execution.speech.as_ref(),
        resolved.cancellation.clone(), Some(speech_run_observer.clone()),
    )?;
    register_screenplay_pdf_executors(
        &mut executor, &resolved.dag, resolved.spec.execution.screenplay_pdf.as_ref(),
        resolved.cancellation.clone(),
    )?;
    register_screenplay_executors(
        &mut executor, &resolved.dag, resolved.spec.execution.screenplay.as_ref(),
        resolved.cancellation.clone(),
    )?;
    register_recovery_executors(
        &mut executor, &resolved.dag, resolved.spec.execution.screenplay_recovery.as_ref(),
        resolved.cancellation.clone(),
    )?;
    register_body_executors(
        &mut executor, &resolved.dag, resolved.spec.execution.screenplay_body.as_ref(),
        resolved.cancellation.clone(),
    )?;
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
        executor.register_collection_artifact_for_edge(
            resolved.dag.all_edges()[0],
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
        executor.register_collection_artifact_for_edge(
            resolved.dag.all_edges()[0],
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
    let multiple_groups = resolved.execution_groups.len() > 1;
    let executor = if multiple_groups || resolved.spec.execution.speech.is_some() || resolved.spec.execution.screenplay.is_some()
        || resolved.spec.execution.screenplay_pdf.is_some() || resolved.spec.execution.screenplay_recovery.is_some()
        || resolved.spec.execution.screenplay_body.is_some() {
        // Unqualified native routes and isolated rendition groups never read
        // or amend a shared cache/checkpoint file.
        executor
    } else {
        executor.with_cache(state_dir.join(cache_name)).with_checkpoints(
            state_dir.join("checkpoints.json"), checkpoint_context, resolved.resume_checkpoints,
        )
    };
    let mut executor = executor.with_max_parallel(resolved.spec.execution.max_parallel);
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
    // Format-keyed execution remains local to a compatible route group. Never
    // merge distinct terminal configurations back into the representative map.
    let mut target_artifacts = HashMap::<String, Artifact>::new();
    let mut target_steps = HashMap::<String, String>::new();
    let mut all_artifacts = HashMap::<String, Artifact>::new();
    let first_targets = resolved.execution_groups.first()
        .map(|group| group.targets.as_slice()).unwrap_or(&resolved.targets);
    if multiple_groups {
        namespace_forest_steps(&mut report, first_targets)?;
    }
    record_forest_group(&report, first_targets, &mut target_artifacts, &mut target_steps, &mut all_artifacts);
    for index in 1..resolved.execution_groups.len() {
        let mut group_executor = std::mem::take(&mut resolved.execution_groups[index].executor)
            .with_max_parallel(resolved.spec.execution.max_parallel);
        if let Some(cancellation) = &resolved.cancellation {
            group_executor = group_executor.with_cancellation_flag(cancellation.clone());
        }
        if let Some(snapshot) = &resolved.plan.toolchain {
            group_executor = group_executor.with_toolchain_fingerprint(snapshot.fingerprint.clone());
        }
        // Each group starts from the original source. Persistent reuse stays
        // disabled until the executor can key checkpoints by branch identity.
        let group = &resolved.execution_groups[index];
        match group_executor.execute_artifact_with_evidence(
            &group.dag, resolved.source_format, source_artifact.clone(), &store,
        ) {
            Ok(mut branch_report) => {
                namespace_forest_steps(&mut branch_report, &group.targets)?;
                record_forest_group(&branch_report, &group.targets, &mut target_artifacts, &mut target_steps, &mut all_artifacts);
                report.steps.extend(branch_report.steps);
                report.diagnostics.extend(branch_report.diagnostics);
            }
            Err(error) => report.diagnostics.push(ExecutionDiagnostic {
                severity: DiagnosticSeverity::RecoverableFailure,
                code: "execution.forest_group_failed".to_owned(),
                message: redact_sensitive_text(&error.to_string()),
                step_id: None,
            }),
        }
    }
    enrich_step_versions(&mut report.steps, resolved.plan.toolchain.as_ref());

    let mut diagnostics = plan_diagnostics(&resolved);
    diagnostics.append(&mut report.diagnostics);
    diagnostics.append(&mut speech_run_observer.diagnostics.lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()));
    let mut hygiene_evidence = HashMap::<String, HygieneEvidence>::new();
    let mut hygiene_blocked = HashSet::<String>::new();
    let mut hygiene_failures = false;
    if let Some((policy_id, policy)) = effective_hygiene_policy(&resolved.spec)? {
        let engine = HygieneEngine::new();
        let mut target_identities = target_artifacts.keys().cloned().collect::<Vec<_>>();
        target_identities.sort();
        let mut hygienic_artifacts = HashMap::<String, (HygieneOutcome, Option<String>)>::new();
        for identity in target_identities {
            let Some(candidate) = target_artifacts.get(&identity).cloned() else {
                continue;
            };
            let candidate_key = hygiene_candidate_key(&candidate)?;
            if let Some((outcome, producer)) = hygienic_artifacts.get(&candidate_key) {
                hygiene_failures |= record_target_hygiene(&identity, outcome,
                    &mut hygiene_evidence, &mut hygiene_blocked);
                target_artifacts.insert(identity.clone(), outcome.artifact.clone());
                if let Some(producer) = producer {
                    target_steps.insert(identity, producer.clone());
                }
                continue;
            }
            let started = unix_time_ms();
            let outcome = engine.apply(&policy_id, &policy, &candidate, &store)?;
            let completed = unix_time_ms();
            let step_id = format!("hygiene:{policy_id}:{candidate_key}");
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
            hygiene_failures |= record_target_hygiene(&identity, &outcome,
                &mut hygiene_evidence, &mut hygiene_blocked);
            let changed = !outcome.evidence.changed_field_classes.is_empty();
            report.steps.push(StepEvidence {
                step_id: step_id.clone(),
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
            let producer = (outcome.artifact.id() != candidate.id()).then_some(step_id);
            if let Some(producer) = &producer {
                target_steps.insert(identity.clone(), producer.clone());
            }
            for representative in report.artifacts.values_mut() {
                if &*representative == &candidate {
                    *representative = outcome.artifact.clone();
                }
            }
            all_artifacts.insert(outcome.artifact.id().to_string(), outcome.artifact.clone());
            hygienic_artifacts.insert(candidate_key, (outcome.clone(), producer));
            target_artifacts.insert(identity, outcome.artifact);
        }
    }
    let mut output_locators = HashMap::<String, String>::new();
    let mut validation_outcomes = HashMap::<String, ArtifactValidationOutcome>::new();
    let mut blocked_targets = hygiene_blocked;
    let mut actual_outputs = Vec::new();
    let mut target_failures = hygiene_failures;
    let mut fatal_validation_failure = false;
    let mut speech_publication_failed = false;
    let mut speech_publication_cancelled = false;
    let retained_speech = match collect_speech_artifacts(&resolved, &report, &store) {
        Ok(retained) => retained,
        Err(error) => {
            target_failures = true;
            speech_publication_failed = true;
            speech_publication_cancelled = matches!(error.downcast_ref::<crate::speech::SpeechError>(),
                Some(crate::speech::SpeechError::Cancelled));
            diagnostics.push(ExecutionDiagnostic {
                severity: if speech_publication_cancelled { DiagnosticSeverity::Cancelled } else { DiagnosticSeverity::FatalFailure },
                code: if speech_publication_cancelled { "execution.cancelled" } else { "speech.evidence.invalid" }.to_owned(),
                message: redact_sensitive_text(&error.to_string()),
                step_id: None,
            });
            Vec::new()
        }
    };

    if let Some(policy) = &resolved.spec.execution.screenplay_body {
        // Direct native exports also create a real canonical payload. Keep it
        // in the run's intermediate inventory and storage/artifact budgets even
        // when the operator did not select it as a published terminal target.
        for artifact in target_artifacts.values() {
            match retained_body_canonical(artifact, &source_artifact, &store, policy) {
                Ok(canonical) => {
                    all_artifacts.entry(canonical.id().to_string()).or_insert(canonical);
                }
                Err(error) => {
                    target_failures = true;
                    fatal_validation_failure = true;
                    diagnostics.push(ExecutionDiagnostic {
                        severity: DiagnosticSeverity::FatalFailure,
                        code: "screenplay_body.evidence.invalid".to_owned(),
                        message: redact_sensitive_text(&error.to_string()), step_id: None,
                    });
                }
            }
        }
    }

    if let Err(error) = validate_post_execution_budgets(&resolved, &target_artifacts, &all_artifacts, &retained_speech) {
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
        let identity = target.identity();
        let Some(artifact) = target_artifacts.get(&identity) else {
            target_failures = true;
            diagnostics.push(ExecutionDiagnostic {
                severity: DiagnosticSeverity::RecoverableFailure,
                code: "execution.target_missing".to_string(),
                message: format!(
                    "Execution did not produce selected target '{}'",
                    identity
                ),
                step_id: None,
            });
            continue;
        };

        let mut outcome = if resolved.print_pages.is_some() || resolved.spec.execution.screenplay_pdf.is_some()
            || resolved.spec.execution.screenplay_recovery.is_some() || resolved.spec.execution.screenplay_body.is_some() {
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
        if let Some(policy) = &resolved.spec.execution.screenplay_pdf {
            let check = validate_published_artifact(artifact, &source_artifact, &store, policy);
            let state = if check.is_ok() { ValidationState::Valid } else { ValidationState::Invalid };
            let validation_diagnostics = check.err().map(|error| vec![ValidationDiagnostic {
                code: "screenplay_pdf.validation.evidence".to_owned(), message: error.to_string(),
            }]).unwrap_or_default();
            outcome.validators.push(ValidatorEvidence {
                validator_id: "validator.screenplay_pdf.afterwriting".to_owned(),
                validator_version: env!("CARGO_PKG_VERSION").to_owned(),
                provider: "renderflow.core".to_owned(), state,
                diagnostics: validation_diagnostics,
            });
            if state == ValidationState::Invalid { outcome.state = state; }
        }
        if let Some(policy) = &resolved.spec.execution.screenplay_recovery {
            let check = crate::screenplay::recovery::graph::validate_published_artifact(
                artifact, &source_artifact, &store, policy,
            );
            let state = if check.is_ok() { ValidationState::Valid } else { ValidationState::Invalid };
            outcome.validators.push(ValidatorEvidence {
                validator_id: "validator.screenplay_recovery.evidence".to_owned(),
                validator_version: env!("CARGO_PKG_VERSION").to_owned(),
                provider: "renderflow.core".to_owned(), state,
                diagnostics: check.err().map(|error| vec![ValidationDiagnostic {
                    code: "screenplay_recovery.validation.evidence".to_owned(), message: error.to_string(),
                }]).unwrap_or_default(),
            });
            if state == ValidationState::Invalid { outcome.state = state; }
        }
        if let Some(policy) = &resolved.spec.execution.screenplay_body {
            // This source-bound admission check is mandatory even when the
            // caller disables the optional generic validator registry.
            let check = validate_screenplay_body_artifact(artifact, &source_artifact, &store, policy);
            let state = if check.is_ok() { ValidationState::Valid } else { ValidationState::Invalid };
            outcome.validators.push(ValidatorEvidence {
                validator_id: "validator.screenplay_body.evidence".to_owned(),
                validator_version: env!("CARGO_PKG_VERSION").to_owned(),
                provider: "renderflow.core".to_owned(), state,
                diagnostics: check.err().map(|error| vec![ValidationDiagnostic {
                    code: "screenplay_body.validation.evidence".to_owned(), message: error.to_string(),
                }]).unwrap_or_default(),
            });
            if state == ValidationState::Invalid { outcome.state = state; }
        }
        if target.format == Format::Text && (resolved.spec.execution.subtitles.is_some()
            || resolved.spec.execution.speech.is_some()
            || artifact.metadata().contains_key(SUBTITLE_ARTIFACT_METADATA_KEY)) {
            // Persistent subtitle records can bypass transform application.
            // Never publish a cached text export with detached loss/lineage evidence.
            let check = report.artifacts.get(&Format::TimedTranscriptJson)
                .context("subtitle.text.validation.canonical: the actual retained canonical transcript is required")
                .and_then(|canonical| validate_text_export_from_canonical(artifact, canonical, &store));
            let state = if check.is_ok() { ValidationState::Valid } else { ValidationState::Invalid };
            outcome.validators.push(ValidatorEvidence {
                validator_id: "validator.subtitle.text_evidence".to_owned(),
                validator_version: env!("CARGO_PKG_VERSION").to_owned(),
                provider: "renderflow.core".to_owned(), state,
                diagnostics: check.err().map(|error| vec![ValidationDiagnostic {
                    code: "subtitle.text.validation.evidence".to_owned(), message: error.to_string(),
                }]).unwrap_or_default(),
            });
            if state == ValidationState::Invalid { outcome.state = state; }
        }
        let step_id = target_steps.get(&identity).cloned();
        if let Some(step) = report.steps.iter_mut().find(|step| Some(step.step_id.as_str()) == step_id.as_deref()) {
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
        let fidelity = report.steps.iter()
            .find(|step| Some(step.step_id.as_str()) == step_id.as_deref())
            .map(|step| step.fidelity).unwrap_or_else(|| if artifact.id() == source_artifact.id() { FidelityDeclaration::Lossless } else { FidelityDeclaration::Unknown });
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
            blocked_targets.insert(identity);
            fatal_validation_failure |=
                resolved.spec.execution.validation.failure_mode == ValidationFailureMode::Fatal;
        }
        validation_outcomes.insert(artifact.id().to_string(), outcome);
    }

    let mut retained_locators = HashMap::<String, String>::new();
    if resolved.spec.execution.speech.is_some() {
        speech_publication_failed |= fatal_validation_failure || !blocked_targets.is_empty()
            || diagnostics.iter().any(|diagnostic| diagnostic.code == "execution.post_budget_failed");
        if !speech_publication_failed {
            match publish_speech_artifacts(&resolved, &retained_speech, &store, &output_root, &predicted) {
                Ok(published) => {
                    for (artifact_id, destination) in published {
                        retained_locators.insert(artifact_id, bundle_locator(&output_root, &destination));
                        actual_outputs.push(destination.display().to_string());
                    }
                }
                Err(error) => {
                    target_failures = true;
                    speech_publication_failed = true;
                    speech_publication_cancelled = matches!(error.downcast_ref::<crate::speech::SpeechError>(),
                        Some(crate::speech::SpeechError::Cancelled));
                    diagnostics.push(ExecutionDiagnostic {
                        severity: if speech_publication_cancelled { DiagnosticSeverity::Cancelled } else { DiagnosticSeverity::FatalFailure },
                        code: if speech_publication_cancelled { "execution.cancelled" } else { "speech.evidence.publication_failed" }.to_owned(),
                        message: redact_sensitive_text(&error.to_string()),
                        step_id: None,
                    });
                }
            }
        }
    }

    for (target, destination) in resolved.targets.iter().zip(predicted.iter()) {
        if resolved.spec.execution.speech.is_some() && speech_not_cancelled(&resolved).is_err() {
            speech_publication_cancelled = true;
            diagnostics.push(ExecutionDiagnostic {
                severity: DiagnosticSeverity::Cancelled,
                code: "execution.cancelled".to_owned(),
                message: "speech publication was cancelled before selected target materialization".to_owned(),
                step_id: None,
            });
            break;
        }
        let Some(artifact) = target_artifacts.get(&target.identity()) else {
            continue;
        };
        if fatal_validation_failure || speech_publication_failed || blocked_targets.contains(&target.identity()) {
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
        output_locators.insert(target.identity(), locator);
        actual_outputs.push(destination.display().to_string());
    }

    let artifacts = artifact_evidence(
        &resolved,
        &source_artifacts,
        &report,
        &target_artifacts,
        &target_steps,
        &all_artifacts,
        &output_locators,
        &diagnostics,
        &validation_outcomes,
        &hygiene_evidence,
        &retained_speech,
        &retained_locators,
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
    let state = if has_cancelled_step || speech_publication_cancelled {
        RunState::Cancelled
    } else if has_failure && materialized_artifact_count == 0 {
        RunState::Failed
    } else if has_failure {
        RunState::Partial
    } else {
        RunState::Complete
    };
    let mut run_manifest = build_run_manifest(
        &resolved,
        started_at_unix_ms,
        state,
        actual_outputs.clone(),
        artifacts,
        report.steps,
        diagnostics,
    )?;
    let manifest_path = persist_run_manifest(&output_root, &run_manifest)?;
    if run_manifest.state == RunState::Complete {
        if let Some(policy) = resolved.spec.execution.speech.as_ref()
            .filter(|policy| policy.saved_results.is_enabled())
        {
            // Only successful exports with a durable run manifest enter index
            // promotion. Cancellation after atomic promotion can leave a valid
            // pointer; never remove a concurrently selected verified result.
            let cancellation = resolved.cancellation.clone()
                .map(crate::process::ProcessCancellationToken::from_shared).unwrap_or_default();
            let remembered = report.artifacts.get(&Format::TimedTranscriptJson)
                .context("speech saved-result indexing requires a canonical transcript")
                .and_then(|canonical| crate::speech::cache::remember_saved_result(
                    &store, policy, &source_artifact, canonical, &cancellation));
            let diagnostic = match remembered {
                Ok(saved) => ExecutionDiagnostic {
                    severity: DiagnosticSeverity::Info,
                    code: "speech.saved_results.recorded".to_owned(),
                    message: format!("Verified historical result index points to receipt SHA-256 {}; an existing verified first-writer entry is retained even if this run produced another result", saved.receipt_sha256()),
                    step_id: None,
                },
                Err(error) => {
                    let cancelled = matches!(error.downcast_ref::<crate::speech::SpeechError>(),
                        Some(crate::speech::SpeechError::Cancelled));
                    run_manifest.state = if cancelled { RunState::Cancelled } else { RunState::Partial };
                    ExecutionDiagnostic {
                        severity: if cancelled { DiagnosticSeverity::Cancelled } else { DiagnosticSeverity::RecoverableFailure },
                        code: if cancelled { "speech.cancelled" } else { "speech.saved_results.record_failed" }.to_owned(),
                        message: redact_sensitive_text(&error.to_string()), step_id: None,
                    }
                }
            };
            run_manifest.diagnostics.push(diagnostic);
            run_manifest.completed_at_unix_ms = unix_time_ms();
            persist_run_manifest(&output_root, &run_manifest)?;
        }
    }

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

#[cfg(test)]
mod forest_execution_tests;
#[cfg(test)]
mod forest_tests;

fn namespace_forest_steps(report: &mut DagExecutionReport, targets: &[ResolvedTarget]) -> Result<()> {
    let mut identities = targets.iter().map(ResolvedTarget::identity).collect::<Vec<_>>();
    identities.sort();
    identities.dedup();
    let prefix = format!("forest:{}:", sha256_serialized(&identities)?.value);
    for step in &mut report.steps {
        step.step_id = format!("{prefix}{}", step.step_id);
        for diagnostic in &mut step.diagnostics {
            if let Some(step_id) = &mut diagnostic.step_id {
                *step_id = format!("{prefix}{step_id}");
            }
        }
    }
    for diagnostic in &mut report.diagnostics {
        if let Some(step_id) = &mut diagnostic.step_id {
            *step_id = format!("{prefix}{step_id}");
        }
    }
    Ok(())
}

fn record_forest_group(
    report: &DagExecutionReport,
    targets: &[ResolvedTarget],
    target_artifacts: &mut HashMap<String, Artifact>,
    target_steps: &mut HashMap<String, String>,
    all_artifacts: &mut HashMap<String, Artifact>,
) {
    for artifact in report.artifacts.values() {
        all_artifacts.insert(artifact.id().to_string(), artifact.clone());
    }
    for target in targets {
        if let Some(artifact) = report.artifacts.get(&target.format) {
            let identity = target.identity();
            if let Some(step_id) = producing_step_id(artifact, &report.steps) {
                target_steps.insert(identity.clone(), step_id);
            }
            target_artifacts.insert(identity, artifact.clone());
        }
    }
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
                step_id: if let Some(group) = edge.evidence.get("forest_execution_group") {
                    format!("forest-group:{group}:step:{}-to-{}", edge.from, edge.to)
                } else { format!("step:{}-to-{}", edge.from, edge.to) },
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
        attach_forest_execution(forest, state, &resolved.spec.output.bundle_root,
            &outputs, &artifacts, &steps);
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
    target_artifacts: &HashMap<String, Artifact>,
    target_steps: &HashMap<String, String>,
    all_artifacts: &HashMap<String, Artifact>,
    output_locators: &HashMap<String, String>,
    diagnostics: &[ExecutionDiagnostic],
    validation_outcomes: &HashMap<String, ArtifactValidationOutcome>,
    hygiene_evidence: &HashMap<String, HygieneEvidence>,
    retained_speech: &[Artifact],
    retained_locators: &HashMap<String, String>,
) -> Vec<ArtifactEvidence> {
    let mut evidence = source_evidence(resolved, sources);
    let mut artifacts = all_artifacts.values().collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left.format()
            .to_string()
            .cmp(&right.format().to_string())
            .then_with(|| left.id().as_str().cmp(right.id().as_str()))
    });
    for artifact in artifacts {
        if sources.iter().any(|source| artifact.id() == source.id())
            && !target_artifacts.values().any(|target| target.id() == artifact.id())
        {
            continue;
        }
        let targets = resolved
            .targets
            .iter()
            .filter(|target| target_artifacts.get(&target.identity())
                .is_some_and(|terminal| terminal.id() == artifact.id()))
            .collect::<Vec<_>>();
        let outcome = validation_outcomes.get(artifact.id().as_str());
        let validation = outcome
            .map(|outcome| outcome.state)
            .unwrap_or(ValidationState::NotRequested);
        let target_instances = if targets.is_empty() {
            vec![None]
        } else {
            targets.into_iter().map(Some).collect()
        };
        for target in target_instances {
            // Record IDs commit bytes, format and lineage, but not metadata.
            // Equal-byte renditions must retain their own native evidence instead
            // of borrowing the deduplicated storage representative's metadata.
            let artifact = target.and_then(|target| target_artifacts.get(&target.identity()))
                .unwrap_or(artifact);
            let producing_step = if let Some(target) = target {
                target_steps.get(&target.identity()).and_then(|step_id|
                    report.steps.iter().find(|step| &step.step_id == step_id))
            } else {
                report.steps.iter().find(|step| step.output_artifacts.iter()
                    .any(|artifact_id| artifact_id == artifact.id().as_str()))
                    .or_else(|| if resolved.spec.execution.screenplay_body.is_some()
                        && artifact.format().as_str() == "screenplay_json" {
                        // Auxiliary canonical bytes are produced by the direct
                        // body step, without introducing a canonical graph hop.
                        report.steps.iter().find(|step| step.capability.as_deref()
                            == Some(SCREENPLAY_BODY_CAPABILITY) && step.state == StepState::Complete)
                    } else { None })
            };
            let producer = producing_step.map(|step| ProducerEvidence {
                system: "renderflow".to_string(),
                transform: Some(step.transform.clone()),
                capability: step.capability.clone(),
                provider: step.provider.clone(),
                version: Some(step.transform_version.clone()),
            }).unwrap_or_else(ProducerEvidence::source);
            let fidelity = producing_step.map(|step| step.fidelity)
                .unwrap_or_else(|| if sources.iter().any(|source| source.id() == artifact.id()) { FidelityDeclaration::Lossless } else { FidelityDeclaration::Unknown });
            let lifecycle = if target.is_some() {
                ArtifactRole::Terminal
            } else {
                ArtifactRole::Intermediate
            };
            let role = target
                .and_then(|target| target.role.clone().or_else(|| target.id.clone()))
                .unwrap_or_else(|| artifact.format().to_string());
            let locator = target
                .and_then(|target| output_locators.get(&target.identity()).cloned())
                .or_else(|| retained_locators.get(artifact.id().as_str()).cloned())
                .unwrap_or_else(|| artifact_store_locator(artifact));
            let mut item = ArtifactEvidence::from_artifact(
                artifact,
                role,
                lifecycle,
                locator,
                producer,
                validation,
                fidelity,
            );
            item.validation_evidence = outcome
                .map(|outcome| outcome.validators.clone())
                .unwrap_or_default();
            item.hygiene = target.and_then(|target| hygiene_evidence.get(&target.identity())).cloned();
            if let Some(target) = target {
                item.metadata.insert(
                    "renderflow.target.identity".to_string(),
                    serde_json::json!(target.identity()),
                );
                item.metadata.insert(
                    "renderflow.target.id".to_string(),
                    serde_json::json!(target.id),
                );
                item.metadata.insert(
                    "renderflow.target.options".to_string(),
                    serde_json::json!(target.options),
                );
                // A retained canonical artifact can already have a bundle locator
                // even when this target's separate destination was not published.
                item.metadata.insert("renderflow.target.published".to_owned(),
                    serde_json::json!(output_locators.contains_key(&target.identity())));
                if let Some(step) = producing_step {
                    item.metadata.insert("renderflow.target.producing_step_id".to_owned(),
                        serde_json::json!(step.step_id));
                }
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
    for artifact in retained_speech {
        if evidence.iter().any(|item| item.artifact_id == artifact.id().as_str()) {
            continue;
        }
        let producing_step = report.steps.iter().find(|step| step.capability.as_deref() == Some(SPEECH_CAPABILITY));
        let producer = producing_step.map(|step| ProducerEvidence {
            system: "renderflow".to_owned(), transform: Some(step.transform.clone()),
            capability: step.capability.clone(), provider: step.provider.clone(),
            version: Some(step.transform_version.clone()),
        }).unwrap_or_else(ProducerEvidence::source);
        let role = artifact.metadata().get("renderflow.speech.role")
            .and_then(serde_json::Value::as_str).unwrap_or("speech_evidence");
        evidence.push(ArtifactEvidence::from_artifact(
            artifact, format!("speech.{role}"), ArtifactRole::Intermediate,
            retained_locators.get(artifact.id().as_str()).cloned()
                .unwrap_or_else(|| artifact_store_locator(artifact)),
            producer, ValidationState::NotRequested, FidelityDeclaration::Unknown,
        ));
    }
    evidence
}

/// Validate native speech record links and retained bytes before publication.
/// Include the canonical transcript even for an SRT-only request, so the output
/// bundle carries the complete recognition chain without the private store.
fn collect_speech_artifacts(
    resolved: &ResolvedExecution,
    report: &DagExecutionReport,
    store: &ArtifactStore,
) -> Result<Vec<Artifact>> {
    let Some(policy) = &resolved.spec.execution.speech else { return Ok(Vec::new()); };
    let Some(canonical) = report.artifacts.get(&Format::TimedTranscriptJson) else {
        if report.steps.iter().any(|step| step.capability.as_deref() == Some(SPEECH_CAPABILITY)
            && step.state == StepState::Complete)
        {
            anyhow::bail!("completed speech step did not retain its canonical transcript");
        }
        return Ok(Vec::new());
    };
    let mut retained = retained_artifacts(canonical)?;
    let expected_count = if policy.audio.is_some() { 4 } else { 2 };
    if retained.len() != expected_count {
        anyhow::bail!("speech canonical transcript is missing required retained artifacts");
    }
    let source = report.artifacts.get(&resolved.source_format)
        .context("speech execution did not retain its original source record")?;
    let metadata = canonical.metadata().get("renderflow.speech")
        .context("speech canonical metadata is missing")?;
    if metadata["input_sha256"].as_str() != Some(policy.source_sha256.as_str())
        || metadata["input_artifact_id"].as_str() != Some(source.id().as_str())
        || source.digest().value() != policy.source_sha256
    {
        anyhow::bail!("speech evidence does not reference the pinned original source");
    }
    for (format, artifact) in &report.artifacts {
        if *format == resolved.source_format { continue; }
        if retained_artifacts(artifact)? != retained
            || artifact.metadata().get("renderflow.speech") != Some(metadata)
        {
            anyhow::bail!("speech derivative has missing or inconsistent retained evidence");
        }
    }
    retained.push(canonical.clone());
    for artifact in &retained {
        speech_not_cancelled(resolved)?;
        verify_speech_file(resolved, &store.payload_path(artifact)?, artifact)
            .context("retained speech payload is missing or changed")?;
    }
    // The receipt commits the canonical record and its supporting records;
    // canonical lineage never points back to the receipt. Keep it last so a
    // portable completion marker is published only after every required file.
    let cancellation = resolved.cancellation.clone()
        .map(crate::process::ProcessCancellationToken::from_shared).unwrap_or_default();
    let bytes = receipt_bytes(source, policy, canonical, store, &cancellation)?;
    speech_not_cancelled(resolved)?;
    retained.push(store.put_bytes(&bytes,
        ArtifactDescriptor::for_format(Format::Json, ArtifactStorageClass::Intermediate)
            .with_source(canonical.id().clone())
            .with_metadata("renderflow.speech.role", "saved_result")
            .with_metadata("renderflow.speech.saved_result_schema", SAVED_RESULT_SCHEMA))?);
    Ok(retained)
}

/// Publish evidence under a content-addressed directory. Existing identical
/// files are acceptable; an existing different file or symlink is refused.
/// A partial directory may remain after an I/O failure, but no selected target
/// is published unless every required supporting artifact was retained.
fn publish_speech_artifacts(
    resolved: &ResolvedExecution,
    artifacts: &[Artifact],
    store: &ArtifactStore,
    output_root: &Path,
    target_paths: &[PathBuf],
) -> Result<Vec<(String, PathBuf)>> {
    if artifacts.is_empty() { return Ok(Vec::new()); }
    // A selected target alias must not overwrite a retained file through an
    // existing directory symlink after the lexical collision check below.
    for target in target_paths {
        let relative = target.strip_prefix(output_root)
            .context("speech target must remain inside its output bundle")?;
        let mut prefix = output_root.to_path_buf();
        for component in std::iter::once(None).chain(relative.components().map(Some)) {
            if let Some(component) = component {
                if !matches!(component, Component::Normal(_)) {
                    anyhow::bail!("speech target paths must contain only normal relative components");
                }
                prefix.push(component.as_os_str());
            }
            match fs::symlink_metadata(&prefix) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    anyhow::bail!("speech target paths must not traverse existing symlinks");
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    let envelope = artifacts.iter().find(|artifact| {
        artifact.metadata().get("renderflow.speech.role").and_then(serde_json::Value::as_str)
            == Some("transcription")
    }).context("retained speech transcription envelope is missing")?;
    let parent = output_root.join("speech");
    let directory = parent.join(envelope.digest().value());
    let mut destinations = Vec::new();
    for artifact in artifacts {
        let name = if artifact.metadata().get("renderflow.speech.role")
            .and_then(serde_json::Value::as_str) == Some("saved_result")
        {
            SAVED_RESULT_FILENAME
        } else if artifact.format().as_str() == Format::TimedTranscriptJson.to_string() {
            "transcript.timed-transcript.json"
        } else {
            retained_artifact_name(artifact)?
        };
        let destination = directory.join(name);
        if target_paths.iter().any(|target| target == &destination) {
            anyhow::bail!("selected target path collides with reserved speech evidence path");
        }
        destinations.push((artifact, destination));
    }
    speech_not_cancelled(resolved)?;
    ensure_speech_directory(&parent)?;
    ensure_speech_directory(&directory)?;
    let mut published = Vec::new();
    for (artifact, destination) in destinations {
        speech_not_cancelled(resolved)?;
        match fs::symlink_metadata(&destination) {
            Ok(_) => verify_speech_file(resolved, &destination, artifact)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut input = store.open(artifact)?;
                let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
                let mut buffer = [0_u8; 64 * 1024];
                let mut size = 0_u64;
                let mut hasher = Sha256::new();
                loop {
                    speech_not_cancelled(resolved)?;
                    let count = input.read(&mut buffer)?;
                    if count == 0 { break; }
                    size = size.checked_add(count as u64).context("speech evidence size overflow")?;
                    if size > artifact.size_bytes() { anyhow::bail!("speech payload changed during publication"); }
                    hasher.update(&buffer[..count]);
                    temporary.write_all(&buffer[..count])?;
                }
                if size != artifact.size_bytes() || format!("{:x}", hasher.finalize()) != artifact.digest().value() {
                    anyhow::bail!("speech payload changed during publication");
                }
                temporary.as_file().sync_all()?;
                speech_not_cancelled(resolved)?;
                match temporary.persist_noclobber(&destination) {
                    Ok(_) => {}
                    Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                        verify_speech_file(resolved, &destination, artifact)?;
                    }
                    Err(error) => return Err(error.error.into()),
                }
            }
            Err(error) => return Err(error.into()),
        }
        published.push((artifact.id().to_string(), destination));
    }
    Ok(published)
}

fn ensure_speech_directory(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        anyhow::bail!("speech evidence directory must be a real directory");
    }
    Ok(())
}

fn verify_speech_file(resolved: &ResolvedExecution, path: &Path, artifact: &Artifact) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != artifact.size_bytes() {
        anyhow::bail!("existing speech evidence path is not the required immutable payload");
    }
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        speech_not_cancelled(resolved)?;
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        size = size.checked_add(count as u64).context("speech evidence size overflow")?;
        if size > artifact.size_bytes() { anyhow::bail!("existing speech evidence grew while read"); }
        hasher.update(&buffer[..count]);
    }
    if size != artifact.size_bytes() || format!("{:x}", hasher.finalize()) != artifact.digest().value() {
        anyhow::bail!("existing speech evidence differs from the required immutable payload");
    }
    Ok(())
}

fn speech_not_cancelled(resolved: &ResolvedExecution) -> Result<()> {
    if resolved.cancellation.as_ref().is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst)) {
        return Err(crate::speech::SpeechError::Cancelled.into());
    }
    Ok(())
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
                provider: None,
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
        if !spec.profiles.contains_key(profile) {
            if let Some(bundled) = crate::spec::bundled_profile(profile)? {
                spec.profiles.insert(profile.clone(), bundled);
            }
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

fn validate_subtitle_policy(
    source_format: Format,
    collection: bool,
    policy: &SubtitlePolicy,
) -> Result<()> {
    if collection {
        anyhow::bail!("subtitle.policy.collection: subtitle steps require one source artifact, not a collection");
    }
    if supports_media_format(source_format) {
        if policy.stream_index.is_none() {
            anyhow::bail!("subtitle.stream_index.required: media extraction requires execution.subtitles.stream_index from subtitles inspect; the first stream is never selected implicitly");
        }
    } else if matches!(source_format, Format::Srt | Format::WebVtt | Format::TimedTranscriptJson) {
        if policy.stream_index.is_some() {
            anyhow::bail!("subtitle.stream_index.unexpected: standalone subtitle conversion does not accept an embedded stream index");
        }
    } else {
        anyhow::bail!("subtitle.policy.source: subtitle steps support SRT, WebVTT, timed-transcript JSON, MKV, WebM, MP4, or MOV; source format is '{source_format}'");
    }
    Ok(())
}

/// Subtitle forest edges are deliberately opt-in. Recognition-only formats and
/// normal document/media planning do not acquire implicit extraction routes.
fn register_subtitle_edges(
    graph: &mut TransformGraph,
    tools: &mut ToolRegistry,
    source_format: Format,
    collection: bool,
    policy: &SubtitlePolicy,
) -> Result<()> {
    validate_subtitle_policy(source_format, collection, policy)?;
    let provider = ToolId::new(TIMED_TEXT_PROVIDER)?;
    for (from, to) in [
        (Format::Srt, Format::TimedTranscriptJson),
        (Format::WebVtt, Format::TimedTranscriptJson),
        (Format::TimedTranscriptJson, Format::Srt),
        (Format::TimedTranscriptJson, Format::WebVtt),
        (Format::TimedTranscriptJson, Format::Text),
    ] {
        let capability = CapabilityId::new(format!("subtitle.convert.{from}.{to}"))?;
        tools.add_capability(&provider, capability.clone())?;
        graph.add_transform(
            TransformEdge::new(from, to, 0.1, if to == Format::Text { 0.5 } else { 0.95 })
                .with_provider(provider.to_string(), capability.to_string())
                .with_evidence("adapter", SUBTITLE_ADAPTER_EVIDENCE)
                .with_evidence("family", "subtitle")
                .with_evidence("transform_id", format!("builtin.subtitle.{from}.{to}")),
        );
    }
    if let Some(stream_index) = policy.stream_index {
        let capability = CapabilityId::new("subtitle.extract.selected")?;
        for provider in ["tool.ffmpeg", "tool.ffprobe"] {
            tools.add_capability(&ToolId::new(provider)?, capability.clone())?;
        }
        graph.add_transform(
            TransformEdge::new(source_format, Format::TimedTranscriptJson, 1.0, 0.95)
                .with_provider("tool.ffmpeg", capability.to_string())
                .with_required_provider("tool.ffprobe")
                .with_evidence("adapter", SUBTITLE_EXTRACTION_EVIDENCE)
                .with_evidence("family", "subtitle")
                .with_evidence("transform_id", format!("builtin.subtitle.extract.{source_format}"))
                .with_evidence("subtitle_stream_index", stream_index.to_string()),
        );
    }
    Ok(())
}

/// Providers are bound after the resolved execution receives SDK cancellation.
fn register_subtitle_executors(
    executor: &mut DagExecutor,
    dag: &MultiTargetDag,
    policy: Option<&SubtitlePolicy>,
    cancellation: Option<Arc<AtomicBool>>,
    speech: bool,
) -> Result<()> {
    for edge in dag.all_edges() {
        match edge.evidence.get("adapter").map(String::as_str) {
            Some(SUBTITLE_ADAPTER_EVIDENCE) => {
                let transform = TimedTextArtifactTransform::new(edge.from, edge.to)?;
                let transform = if speech { transform.without_cache() } else { transform };
                executor.register_artifact_for_edge(edge, Arc::new(transform));
            }
            Some(SUBTITLE_EXTRACTION_EVIDENCE) => {
                let stream_index = policy.and_then(|policy| policy.stream_index)
                    .context("subtitle.stream_index.required: selected extraction edge has no explicit stream index")?;
                if edge.to != Format::TimedTranscriptJson
                    || edge.evidence.get("subtitle_stream_index") != Some(&stream_index.to_string())
                {
                    anyhow::bail!("subtitle.plan.inconsistent: extraction edge differs from the frozen stream selection");
                }
                executor.register_artifact_for_edge(
                    edge,
                    Arc::new(SubtitleExtractionTransform::new(
                        edge.from,
                        stream_index,
                        cancellation.clone(),
                    )?),
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_screenplay_pdf_policy(source: Format, collection: bool, policy: &AfterwritingConfig) -> Result<()> {
    policy.validate()?;
    if collection || source != Format::Fountain {
        anyhow::bail!("screenplay_pdf.policy.source: exactly one original Fountain source is required; native normalization and collections are separate work");
    }
    Ok(())
}

fn validate_screenplay_pdf_hygiene(spec: &SpecV2) -> Result<()> {
    if effective_hygiene_policy(spec)?.is_some() {
        anyhow::bail!("screenplay_pdf.policy.hygiene: inherited or direct post-render hygiene would detach the PDF from its independently inspected evidence");
    }
    Ok(())
}

fn validate_screenplay_pdf_targets(targets: &[ResolvedTarget]) -> Result<()> {
    if targets.len() != 1 || targets[0].format != Format::Pdf {
        anyhow::bail!("screenplay_pdf.target.unsupported: choose exactly one PDF target; same-format, text, canonical JSON, native screenplay formats, and transitive routes are unavailable");
    }
    let target = &targets[0];
    if target.preset.is_some() || target.template.is_some() || target.variant.is_some() || !target.options.is_empty() {
        anyhow::bail!("screenplay_pdf.target.options: formatting is selected only by execution.screenplay_pdf.profile; target presets, templates, variants, and custom options are unsupported");
    }
    Ok(())
}

fn register_screenplay_pdf_edge(
    graph: &mut TransformGraph,
    tools: &mut ToolRegistry,
    source: Format,
    policy: &AfterwritingConfig,
) -> Result<()> {
    validate_screenplay_pdf_policy(source, false, policy)?;
    for id in [AFTERWRITING_PROVIDER_ID, NODE_PROVIDER_ID] {
        let mut descriptor = tools.get(id)
            .with_context(|| format!("screenplay_pdf.provider.registry: {id} descriptor missing"))?.clone();
        // These providers deliberately retain explicit-configuration discovery.
        // Only the bounded, environment-clearing adapter may probe Node.
        descriptor.discovery = ToolDiscovery::RuntimeService {
            service: format!("renderflow.configured.{}", id.trim_start_matches("tool.")),
        };
        descriptor.version = ToolVersionRequirement::default();
        descriptor.determinism = ToolDeterminism::ConfigurationDependent;
        descriptor.locality = ToolLocality::Local;
        descriptor.fidelity = ToolFidelity::PartialLoss;
        if id == AFTERWRITING_PROVIDER_ID {
            descriptor.capabilities = vec![CapabilityId::new(SCREENPLAY_PDF_CAPABILITY)?];
        }
        tools.register(descriptor)?;
    }
    graph.add_transform(TransformEdge::new(Format::Fountain, Format::Pdf, 1.0, 0.0)
        .with_provider(AFTERWRITING_PROVIDER_ID, SCREENPLAY_PDF_CAPABILITY)
        .with_required_provider(NODE_PROVIDER_ID)
        .with_evidence("adapter", SCREENPLAY_PDF_ADAPTER_EVIDENCE)
        .with_evidence("family", "screenplay")
        .with_evidence("transform_id", SCREENPLAY_PDF_CAPABILITY)
        .with_evidence("screenplay_pdf_policy_sha256", sha256_serialized(policy)?.value)
        .with_evidence("qualification", "authored_unqualified")
        .with_evidence("quality_interpretation", "unqualified_not_fidelity_score")
        .with_evidence("cache", "disabled"));
    Ok(())
}

fn is_screenplay_pdf_provider(id: &str, policy: Option<&AfterwritingConfig>) -> bool {
    policy.is_some() && matches!(id, AFTERWRITING_PROVIDER_ID | NODE_PROVIDER_ID)
}

/// A configured installation is not permission to probe a denied route. Use the
/// surviving planning edges or selected execution edges and reapply the policy
/// before invoking the callback, including at execution-time refresh.
fn assess_screenplay_pdf_availability(
    inventory: &mut ToolInventory,
    spec: &SpecV2,
    tools: &ToolRegistry,
    edges: &[&TransformEdge],
    probe: impl FnOnce(&AfterwritingConfig) -> std::result::Result<(String, String), String>,
) -> bool {
    let Some(policy) = &spec.execution.screenplay_pdf else { return false; };
    let allowed = edges.iter().any(|edge| {
        edge.from == Format::Fountain && edge.to == Format::Pdf
            && edge.evidence.get("adapter").map(String::as_str) == Some(SCREENPLAY_PDF_ADAPTER_EVIDENCE)
            && edge.provider_id.as_deref() == Some(AFTERWRITING_PROVIDER_ID)
            && edge.required_provider_ids == vec![NODE_PROVIDER_ID.to_owned()]
            && edge.capability_id.as_deref() == Some(SCREENPLAY_PDF_CAPABILITY)
            && edge_allowed(edge, tools, spec)
    });
    if !allowed { return false; }
    append_screenplay_pdf_availability(inventory, policy, probe(policy));
    true
}

/// Keep this observation separate from generic executable discovery. The package
/// has no version command, and Node must never inherit NODE_OPTIONS in a probe.
fn append_screenplay_pdf_availability(
    inventory: &mut ToolInventory,
    policy: &AfterwritingConfig,
    observed: std::result::Result<(String, String), String>,
) {
    for (id, name) in [(NODE_PROVIDER_ID, "Configured Node.js"), (AFTERWRITING_PROVIDER_ID, "Configured afterwriting")] {
        let (status, version_line, diagnostic) = match &observed {
            Ok((node_version, provider_version)) => (
                ToolAvailabilityStatus::Available,
                Some(if id == NODE_PROVIDER_ID { node_version.clone() } else { provider_version.clone() }),
                Some("Pinned installation and runtime preflight passed; native rendering remains unqualified".to_owned()),
            ),
            Err(error) => (
                ToolAvailabilityStatus::ProbeFailed, None,
                Some(format!("{error}; configure execution.screenplay_pdf with an installed compatible Node executable, complete afterwriting package including required assets, and verified SHA-256 pins; no PATH fallback or installation is performed")),
            ),
        };
        inventory.tools.push(ToolAvailability {
            id: ToolId::new(id).expect("static provider ID"), name: name.to_owned(), status,
            support_tier: ToolSupportTier::Experimental,
            selected_executable: if id == NODE_PROVIDER_ID { Some(policy.node_executable.to_string_lossy().into_owned()) } else { None },
            normalized_version: version_line.clone(), version_line, diagnostic,
        });
    }
}

fn register_screenplay_pdf_executors(
    executor: &mut DagExecutor,
    dag: &MultiTargetDag,
    policy: Option<&AfterwritingConfig>,
    cancellation: Option<Arc<AtomicBool>>,
) -> Result<()> {
    for edge in dag.all_edges() {
        if edge.evidence.get("adapter").map(String::as_str) != Some(SCREENPLAY_PDF_ADAPTER_EVIDENCE) { continue; }
        let policy = policy.context("screenplay_pdf.policy.missing: explicit afterwriting configuration is required")?;
        if edge.from != Format::Fountain || edge.to != Format::Pdf
            || edge.provider_id.as_deref() != Some(AFTERWRITING_PROVIDER_ID)
            || edge.required_provider_ids != vec![NODE_PROVIDER_ID.to_owned()]
            || edge.capability_id.as_deref() != Some(SCREENPLAY_PDF_CAPABILITY)
            || edge.evidence.get("screenplay_pdf_policy_sha256") != Some(&sha256_serialized(policy)?.value)
        {
            anyhow::bail!("screenplay_pdf.plan.inconsistent: selected edge differs from the frozen direct rendering policy");
        }
        executor.register_artifact_for_edge(edge,
            Arc::new(AfterwritingArtifactTransform::new(policy.clone(), cancellation.clone())?));
    }
    Ok(())
}

fn validate_screenplay_hygiene(spec: &SpecV2) -> Result<()> {
    if effective_hygiene_policy(spec)?.is_some() {
        anyhow::bail!("screenplay.policy.hygiene: post-conversion hygiene, including inherited profile hygiene, is unsupported because it would detach output bytes from scripttool evidence");
    }
    Ok(())
}

/// Generic XML/ZIP observations identify the wrapper, not the screenplay
/// dialect. Only an explicit dialect declaration may resolve this exact pair;
/// the native adapter still validates the envelope and its bounded semantics.
fn screenplay_wrapper_conflict(source: Format, declared: Option<&str>, formats: &[String]) -> bool {
    if declared.and_then(|value| value.parse::<Format>().ok()) != Some(source) { return false; }
    let wrapper = match source {
        Format::Fdx | Format::Osf => "xml",
        Format::FadeIn => "zip",
        _ => return false,
    };
    formats.len() == 2 && formats.iter().any(|format| format == wrapper)
        && formats.iter().any(|format| format == &source.to_string())
}

fn validate_screenplay_policy(source: Format, collection: bool, policy: &ScripttoolConfig) -> Result<()> {
    policy.validate()?;
    if collection {
        anyhow::bail!("screenplay.policy.collection: scripttool requires one source artifact");
    }
    let from = native_format(source)
        .context("screenplay.policy.source: supported sources are FDX, OSF, or Fade In")?;
    if !supported_route(from, crate::screenplay::native::NativeScreenplayFormat::Fountain) {
        anyhow::bail!("screenplay.policy.source: only the FDX, OSF, and Fade In imports to Fountain are adopted; reverse writers and provider JSON are unavailable");
    }
    Ok(())
}

/// Register only direct edges from the selected original source. In particular,
/// the graph cannot manufacture a reverse writer via a screenplay intermediate.
fn register_screenplay_edges(
    graph: &mut TransformGraph,
    tools: &mut ToolRegistry,
    source: Format,
    policy: &ScripttoolConfig,
) -> Result<()> {
    validate_screenplay_policy(source, false, policy)?;
    let mut descriptor = tools.get(SCREENPLAY_PROVIDER)
        .context("screenplay.provider.registry: scripttool descriptor missing")?.clone();
    let executable = policy.executable.to_str()
        .context("screenplay.provider.path: executable must be valid UTF-8")?;
    descriptor.discovery = ToolDiscovery::Executable {
        candidates: vec![executable.to_owned()], version_args: vec!["-version".to_owned()],
    };
    // Exact version-line compatibility is applied to observations below. A
    // numeric min/max range would accept builds outside this bounded adapter.
    descriptor.version = ToolVersionRequirement::default();
    descriptor.determinism = ToolDeterminism::ConfigurationDependent;
    descriptor.locality = ToolLocality::Local;
    descriptor.fidelity = ToolFidelity::PartialLoss;
    let capability = CapabilityId::new(format!("screenplay.import.{source}.fountain"))?;
    descriptor.capabilities = vec![capability.clone()];
    tools.register(descriptor)?;
    graph.add_transform(TransformEdge::new(source, Format::Fountain, 1.0, 0.0)
        .with_provider(SCREENPLAY_PROVIDER, capability.to_string())
        .with_evidence("adapter", SCREENPLAY_ADAPTER_EVIDENCE)
        .with_evidence("family", "screenplay")
        .with_evidence("transform_id", format!("builtin.screenplay.{source}.fountain"))
        .with_evidence("screenplay_policy_sha256", sha256_serialized(policy)?.value)
        .with_evidence("qualification", "authored_unqualified")
        .with_evidence("quality_interpretation", "unqualified_not_fidelity_score")
        .with_evidence("cache", "disabled"));
    Ok(())
}

/// Keep missing and incompatible configured providers in the ordinary inventory
/// so only their own graph branches are pruned. A successful version probe does
/// not verify the executable SHA-256; the native adapter verifies that pin.
fn apply_screenplay_version_requirement(inventory: &mut ToolInventory, policy: Option<&ScripttoolConfig>) {
    let Some(policy) = policy else { return; };
    for tool in &mut inventory.tools {
        if tool.id.as_str() != SCREENPLAY_PROVIDER { continue; }
        if tool.is_available() && !tool.version_line.as_deref().is_some_and(|line| {
            supported_version_line(line, &policy.expected_version, &policy.executable)
        }) {
            tool.status = ToolAvailabilityStatus::UnsupportedVersion;
            tool.diagnostic = Some(format!(
                "configured scripttool version probe did not match the supported {} version line; select the supported executable and update execution.screenplay.executable_sha256 to its verified digest",
                policy.expected_version,
            ));
        } else if !tool.is_available() {
            let original = tool.diagnostic.as_deref().unwrap_or("configured executable unavailable");
            tool.diagnostic = Some(format!(
                "{original}; configure execution.screenplay.executable as an installed compatible scripttool path with an explicit SHA-256 pin; no installation or PATH fallback is performed"
            ));
        }
    }
}

fn register_screenplay_executors(
    executor: &mut DagExecutor,
    dag: &MultiTargetDag,
    policy: Option<&ScripttoolConfig>,
    cancellation: Option<Arc<AtomicBool>>,
) -> Result<()> {
    for edge in dag.all_edges() {
        if edge.evidence.get("adapter").map(String::as_str) != Some(SCREENPLAY_ADAPTER_EVIDENCE) {
            continue;
        }
        let policy = policy.context("screenplay.policy.missing: selected edge requires explicit scripttool configuration")?;
        if edge.provider_id.as_deref() != Some(SCREENPLAY_PROVIDER)
            || edge.capability_id.as_deref() != Some(format!("screenplay.import.{}.{}", edge.from, edge.to).as_str())
            || edge.evidence.get("screenplay_policy_sha256") != Some(&sha256_serialized(policy)?.value)
        {
            anyhow::bail!("screenplay.plan.inconsistent: edge differs from the frozen scripttool policy");
        }
        executor.register_artifact_for_edge(edge, Arc::new(ScreenplayArtifactTransform::new(
            edge.from, edge.to, policy.clone(), cancellation.clone(),
        )?));
    }
    Ok(())
}

fn speech_provider_ids(policy: Option<&SpeechPolicy>) -> HashSet<String> {
    let mut ids = HashSet::new();
    if let Some(policy) = policy {
        ids.insert(WHISPER_PROVIDER_ID.to_owned());
        if policy.audio.is_some() {
            ids.extend(["tool.ffmpeg".to_owned(), "tool.ffprobe".to_owned()]);
        }
    }
    ids
}

fn register_speech_edges(
    graph: &mut TransformGraph,
    tools: &mut ToolRegistry,
    source_format: Format,
    policy: &SpeechPolicy,
) -> Result<()> {
    policy.validate_source(source_format, false)?;
    let mut descriptor = tools.get("tool.whisper_cpp")
        .context("speech.provider.registry: whisper.cpp descriptor missing")?.clone();
    descriptor.id = ToolId::new(WHISPER_PROVIDER_ID)?;
    descriptor.discovery = ToolDiscovery::Executable {
        candidates: vec![policy.provider.path.clone()], version_args: vec!["--version".to_owned()],
    };
    descriptor.capabilities = vec![CapabilityId::new(SPEECH_CAPABILITY)?];
    descriptor.determinism = ToolDeterminism::ConfigurationDependent;
    descriptor.locality = ToolLocality::Local;
    descriptor.fidelity = ToolFidelity::PathDependent;
    tools.register(descriptor)?;
    let mut edge = TransformEdge::new(source_format, Format::TimedTranscriptJson, 1.0, 0.0)
        .with_provider(WHISPER_PROVIDER_ID, SPEECH_CAPABILITY)
        .with_variant(policy.model.id.clone())
        .with_evidence("adapter", SPEECH_ADAPTER_EVIDENCE)
        .with_evidence("transform_id", SPEECH_CAPABILITY)
        .with_evidence("family", "speech")
        .with_evidence("speech_policy_sha256", sha256_serialized(policy)?.value)
        .with_evidence("provider_identity", if policy.recovery.is_some() {
            "historical_receipt_unverified"
        } else if policy.saved_results.is_enabled() {
            "historical_index_unverified"
        } else { "declared_unobserved" })
        .with_evidence("speech_execution", if policy.recovery.is_some() {
            "explicit_saved_result"
        } else if policy.saved_results.is_enabled() {
            "reuse_saved_result_if_available"
        } else { "native" })
        .with_evidence("quality_interpretation", "unqualified_not_accuracy")
        .with_evidence("cache", "disabled");
    if let Some(audio) = &policy.audio {
        for (id, pinned) in [("tool.ffmpeg", &audio.ffmpeg), ("tool.ffprobe", &audio.ffprobe)] {
            let mut descriptor = tools.get(id)
                .with_context(|| format!("speech.provider.registry: {id} descriptor missing"))?.clone();
            descriptor.discovery = ToolDiscovery::Executable {
                candidates: vec![pinned.path.clone()], version_args: vec!["-version".to_owned()],
            };
            descriptor.locality = ToolLocality::Local;
            descriptor.determinism = ToolDeterminism::ConfigurationDependent;
            tools.register(descriptor)?;
            tools.add_capability(&ToolId::new(id)?, CapabilityId::new(SPEECH_CAPABILITY)?)?;
            edge = edge.with_required_provider(id);
        }
        edge = edge.with_evidence("audio_stream_index", audio.stream_index.to_string());
    }
    graph.add_transform(edge);
    // Conversion exports reuse the same canonical transcript from the one ASR
    // edge. No native subtitle extraction is registered under speech intent.
    for to in [Format::Srt, Format::WebVtt, Format::Text] {
        let from = Format::TimedTranscriptJson;
        let capability = CapabilityId::new(format!("subtitle.convert.{from}.{to}"))?;
        tools.add_capability(&ToolId::new(TIMED_TEXT_PROVIDER)?, capability.clone())?;
        graph.add_transform(TransformEdge::new(from, to, 0.1, if to == Format::Text { 0.5 } else { 0.95 })
            .with_provider(TIMED_TEXT_PROVIDER, capability.to_string())
            .with_evidence("adapter", SUBTITLE_ADAPTER_EVIDENCE)
            .with_evidence("family", "subtitle")
            .with_evidence("transform_id", format!("builtin.subtitle.{from}.{to}"))
            .with_evidence("cache", "disabled"));
    }
    Ok(())
}

fn register_speech_executors(
    executor: &mut DagExecutor,
    dag: &MultiTargetDag,
    policy: Option<&SpeechPolicy>,
    cancellation: Option<Arc<AtomicBool>>,
    observer: Option<Arc<dyn SpeechProgressObserver>>,
) -> Result<()> {
    for edge in dag.all_edges() {
        if edge.evidence.get("adapter").map(String::as_str) != Some(SPEECH_ADAPTER_EVIDENCE) {
            continue;
        }
        let policy = policy.context("speech.policy.missing: selected ASR edge has no explicit policy")?;
        if edge.to != Format::TimedTranscriptJson
            || edge.provider_id.as_deref() != Some(WHISPER_PROVIDER_ID)
            || edge.capability_id.as_deref() != Some(SPEECH_CAPABILITY)
            || edge.evidence.get("speech_policy_sha256") != Some(&sha256_serialized(policy)?.value)
        {
            anyhow::bail!("speech.plan.inconsistent: selected ASR edge differs from frozen speech policy");
        }
        let transform = SpeechArtifactTransform::new(edge.from, policy.clone(), cancellation.clone())?;
        let transform = match &observer {
            Some(observer) => transform.with_observer(observer.clone()),
            None => transform,
        };
        executor.register_artifact_for_edge(edge, Arc::new(transform));
    }
    Ok(())
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
    if edge.from == Format::Text && (spec.execution.subtitles.is_some()
        || spec.execution.speech.is_some()) {
        return false;
    }
    if edge.evidence.get("adapter").map(String::as_str) == Some(SCREENPLAY_BODY_ADAPTER_EVIDENCE)
        && spec.execution.screenplay_body.as_ref().is_none_or(|policy| !policy.allow_unqualified)
    {
        return false;
    }
    if edge.evidence.get("adapter").map(String::as_str) == Some(SCREENPLAY_RECOVERY_ADAPTER_EVIDENCE)
        && spec.execution.screenplay_recovery.is_none()
    {
        return false;
    }
    if edge.evidence.get("adapter").map(String::as_str) == Some(SCREENPLAY_PDF_ADAPTER_EVIDENCE)
        && spec.execution.screenplay_pdf.is_none()
    {
        return false;
    }
    if edge.evidence.get("adapter").map(String::as_str) == Some(SCREENPLAY_ADAPTER_EVIDENCE)
        && spec.execution.screenplay.is_none()
    {
        return false;
    }
    if edge.evidence.get("adapter").map(String::as_str) == Some(SPEECH_ADAPTER_EVIDENCE)
        && (spec.execution.speech.is_none() || spec.execution.ai == AiPolicy::Deny)
    {
        return false;
    }
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
        for target in &mut selected { target.include_selectors.push(spec.targets.include.clone()); }
    }
    apply_exclusions(&mut selected, &profile_exclusions, graph);
    apply_exclusions(&mut selected, &spec.targets.exclude, graph);
    selected.sort_by(|a, b| a.format.to_string().cmp(&b.format.to_string()).then_with(|| a.identity().cmp(&b.identity())));
    Ok(selected)
}

fn extend_target_spec(
    selected: &mut Vec<ResolvedTarget>, target: &TargetSpec, graph: &TransformGraph,
    source: Format, reachable: &[Format],
) -> Result<()> {
    let candidates = if let Some(format) = &target.format {
        // Preserve an explicit unsupported request as a diagnostic branch.
        vec![format.parse().with_context(|| format!("unknown target format '{format}'"))?]
    } else {
        let mut candidates = reachable.iter().copied().filter(|format| {
            target.family.as_ref().is_none_or(|family| format_in_family(*format, family))
                && graph.transforms_to(*format).iter().any(|edge| {
                    target.capability.as_ref().is_none_or(|id| edge.capability_id.as_ref() == Some(id))
                        && target.transform.as_ref().is_none_or(|id| edge.evidence.get("transform_id") == Some(id))
                        && target.provider.as_ref().is_none_or(|id| edge.provider_id.as_ref() == Some(id))
                        && target.variant.as_ref().is_none_or(|id| edge.variant_id.as_ref() == Some(id))
                })
        }).collect::<Vec<_>>();
        if candidates.is_empty() && target.capability.as_deref() == Some(crate::super_resolution::SUPER_RESOLUTION_CAPABILITY_ID) {
            candidates.push(source);
        }
        if candidates.is_empty() {
            anyhow::bail!("target.selector_unsupported: no registered artifact format matches the target selector; specify format to retain an explicit unsupported branch in the plan");
        }
        candidates
    };
    for format in candidates { insert_target(selected, ResolvedTarget::from_spec(format, target))?; }
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
        let mut target = ResolvedTarget::generated(*format);
        if selector_matches(&target, selector, graph) {
            target.include_selectors.push(selector.clone());
            insert_target(selected, target)?;
        }
    }
    Ok(())
}

fn insert_target(selected: &mut Vec<ResolvedTarget>, candidate: ResolvedTarget) -> Result<()> {
    if let Some(existing) = selected.iter_mut().find(|target| target.identity() == candidate.identity()) {
        if candidate.requirement == TargetRequirement::Required { existing.requirement = TargetRequirement::Required; }
        return Ok(());
    }
    // Explicit declarations replace format-only expansion. Two explicit render
    // identities remain distinct even when their extension is the same.
    if is_generated(&candidate) && selected.iter().any(|target| target.format == candidate.format) {
        return Ok(());
    }
    if !is_generated(&candidate) {
        selected.retain(|target| target.format != candidate.format || !is_generated(target));
    }
    selected.push(candidate);
    Ok(())
}

fn is_generated(target: &ResolvedTarget) -> bool {
    target.id.is_none() && target.role.as_deref() == Some(target.format.to_string().as_str())
        && target.preset.is_none() && target.template.is_none() && target.variant.is_none()
        && target.capability.is_none() && target.transform.is_none() && target.provider.is_none()
        && target.options.is_empty() && target.requirement == TargetRequirement::Optional
}

fn apply_exclusions(selected: &mut Vec<ResolvedTarget>, selector: &SelectorSet, _graph: &TransformGraph) {
    if selector.is_empty() { return; }
    selected.retain(|target| !forest::selector_matches_identity(target, selector));
    for target in selected { target.exclude_selectors.push(selector.clone()); }
}

fn selector_matches(target: &ResolvedTarget, selector: &SelectorSet, graph: &TransformGraph) -> bool {
    forest::selector_matches_identity(target, selector) || graph.transforms_to(target.format).iter().any(|edge|
        forest::target_edge_matches(target, edge) && forest::selector_matches_edge(selector, edge))
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
        executor.register_artifact_for_edge(edge, Arc::new(transform));
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
    if let Some(forest) = &resolved.plan.artifact_forest {
        let blocked = forest.branches.iter().filter(|branch| branch.requirement == "required"
            && matches!(branch.state, ForestBranchState::Unavailable | ForestBranchState::Unsupported | ForestBranchState::BudgetPruned))
            .map(|branch| format!("{} ({}): {}", branch.target_id, branch.reason_code, branch.reason)).collect::<Vec<_>>();
        if !blocked.is_empty() { anyhow::bail!("forest.required_targets_blocked: {}", blocked.join("; ")); }
    }
    if resolved.targets.is_empty() { anyhow::bail!("forest.no_selected_targets: the forest contains diagnostics only; no transform or output was scheduled"); }
    if resolved.print_pages.is_some() && resolved.plan.toolchain.is_none() {
        anyhow::bail!("print_pdf.provider.unavailable: print interior was planned without an observed compatible img2pdf toolchain; re-plan when it is installed");
    }
    let ids = resolved.execution_groups.iter().flat_map(|group| selected_provider_ids(&group.dag)).collect::<BTreeSet<_>>();
    let deferred_speech_ids = speech_provider_ids(resolved.spec.execution.speech.as_ref());
    let deferred_recovery_ids = recovery_provider_ids(resolved.source_format, resolved.spec.execution.screenplay_recovery.as_ref());
    let mut inventory = resolved.tool_registry.assess_ids_current(
        ids.iter().filter(|id|
            !deferred_speech_ids.contains(*id)
                && !deferred_recovery_ids.contains(*id)
                && !is_screenplay_pdf_provider(id, resolved.spec.execution.screenplay_pdf.as_ref())),
    );
    apply_screenplay_version_requirement(&mut inventory, resolved.spec.execution.screenplay.as_ref());
    let screenplay_pdf_probed = assess_screenplay_pdf_availability(
        &mut inventory, &resolved.spec, &resolved.tool_registry, &resolved.dag.all_edges(),
        |policy| {
            let cancellation = resolved.cancellation.clone().map(ProcessCancellationToken::from_shared).unwrap_or_default();
            afterwriting::preflight(policy, cancellation)
            .map(|observation| (observation.node_version, observation.provider_version))
            .map_err(|error| error.to_string())
        },
    );
    if resolved.spec.execution.screenplay_pdf.is_some() && !screenplay_pdf_probed {
        anyhow::bail!("screenplay_pdf.provider.denied: selected route is missing or excluded by execution policy; configured providers were not probed");
    }
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
        let deepest = resolved.execution_groups.iter().map(|group| ExecutionPlan::from_dag(&group.dag, resolved.source_format, &[], resolved.spec.execution.optimization).metadata.execution_depth).max().unwrap_or(0);
        if deepest as u32 > max_depth {
            anyhow::bail!(
                "execution plan depth {} exceeds max_depth budget {}",
                deepest,
                max_depth
            );
        }
    }
    if let Some(max_artifacts) = budgets.max_artifacts {
        let retained_count = resolved.spec.execution.speech.as_ref()
            .map(|policy| if policy.audio.is_some() { 5 } else { 3 }).unwrap_or(0)
            + u64::from(resolved.spec.execution.screenplay_body.is_some()
                && !resolved.targets.iter().any(|target| target.format == Format::ScreenplayJson));
        let nodes = resolved.source_members.len() as u64 + resolved.execution_groups.iter().map(|group| group.dag.node_count().saturating_sub(1) as u64).sum::<u64>();
        let estimate = nodes.checked_add(retained_count)
            .context("execution artifact estimate overflow")?;
        if estimate > max_artifacts {
            anyhow::bail!("execution plan artifact estimate {estimate} exceeds max_artifacts budget {max_artifacts}");
        }
    }
    Ok(())
}

fn validate_post_execution_budgets(
    resolved: &ResolvedExecution,
    target_artifacts: &HashMap<String, Artifact>,
    artifacts: &HashMap<String, Artifact>,
    retained_speech: &[Artifact],
) -> Result<()> {
    let budgets = &resolved.spec.execution.budgets;
    if let Some(max_output) = budgets.max_output_bytes {
        // Count every selected publication, even when two identities happen to
        // render identical bytes. Storage accounting below deduplicates IDs.
        let total: u64 = resolved.targets.iter()
            .filter_map(|target| target_artifacts.get(&target.identity()))
            .map(Artifact::size_bytes)
            .chain(retained_speech.iter().map(Artifact::size_bytes))
            .try_fold(0_u64, |total, size| total.checked_add(size).context("artifact byte count overflow"))?;
        if total > max_output {
            anyhow::bail!(
                "produced output bytes {total} exceed max_output_bytes budget {max_output}"
            );
        }
    }
    if let Some(max_storage) = budgets.max_storage_bytes {
        let mut seen = HashSet::new();
        let total: u64 = artifacts.values().chain(retained_speech.iter())
            .filter(|artifact| seen.insert(artifact.id().to_string()))
            .map(Artifact::size_bytes)
            .try_fold(0_u64, |total, size| total.checked_add(size).context("artifact byte count overflow"))?;
        let total = if resolved.spec.execution.speech.as_ref().is_some_and(|policy| policy.saved_results.is_enabled()) {
            total.checked_add(crate::speech::cache::MAX_INDEX_BYTES as u64)
                .context("speech index storage reservation overflow")?
        } else { total };
        if total > max_storage {
            anyhow::bail!(
                "execution artifact bytes {total} exceed max_storage_bytes budget {max_storage}"
            );
        }
    }
    if let Some(max_artifacts) = budgets.max_artifacts {
        let count = artifacts.values().chain(retained_speech.iter())
            .map(|artifact| artifact.id().to_string()).collect::<HashSet<_>>().len() as u64;
        if count > max_artifacts {
            anyhow::bail!("execution artifact count {count} exceeds max_artifacts budget {max_artifacts}");
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
    let mut seen = HashSet::<PathBuf>::new();
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
            rendered = rendered.replace("{target.identity}", &target.identity());
            rendered = rendered.replace("{target.role}", target_role);
            rendered = rendered.replace("{target.format}", &format_string);
            rendered = rendered.replace("{target.variant}", target.variant.as_deref().unwrap_or("default"));
            rendered = rendered.replace("{target.preset}", target.preset.as_deref().unwrap_or("default"));
            rendered = rendered.replace("{ext}", &artifact_extension(target.format));
            let path = PathBuf::from(rendered);
            validate_relative_output_path(&path)?;
            path
        };
        let mut destination = root.join(&relative);
        if seen.contains(&destination) {
            match resolved.spec.output.collision {
                CollisionPolicy::Error | CollisionPolicy::Replace => anyhow::bail!(
                    "multiple selected targets resolve to output path '{}'; include {{target.identity}}, {{target.id}}, or a distinct {{target.role}} in the naming template, or choose collision: dedupe; replace cannot overwrite another target in the same run",
                    destination.display()
                ),
                CollisionPolicy::Dedupe => {
                    let original = destination.clone();
                    let mut suffix = 2;
                    loop {
                        destination = dedupe_path(&original, suffix);
                        if !seen.contains(&destination) { break; }
                        suffix += 1;
                    }
                }
            }
        }
        seen.insert(destination.clone());
        paths.push(destination);
    }
    if resolved.spec.publication.is_some() && !resolved.targets.is_empty() {
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
        Format::Text => "txt".to_string(),
        Format::Kepub => "kepub.epub".to_string(),
        Format::ScreenplayJson => "screenplay.json".to_string(),
        Format::TimedTranscriptJson => "timed-transcript.json".to_string(),
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

    #[test]
    fn subtitle_policy_requires_explicit_media_stream_and_rejects_other_inputs() {
        let conversion = SubtitlePolicy::default();
        let extraction = SubtitlePolicy { stream_index: Some(0) };
        for media in [Format::Mkv, Format::WebM, Format::Mp4, Format::Mov] {
            assert!(validate_subtitle_policy(media, false, &conversion).is_err());
            assert!(validate_subtitle_policy(media, false, &extraction).is_ok());
        }
        for subtitle in [Format::Srt, Format::WebVtt, Format::TimedTranscriptJson] {
            assert!(validate_subtitle_policy(subtitle, false, &conversion).is_ok());
            assert!(validate_subtitle_policy(subtitle, false, &extraction).is_err());
            assert!(validate_subtitle_policy(subtitle, true, &conversion).is_err());
        }
        for unsupported in [Format::Json, Format::Ass, Format::Ssa, Format::Ttml, Format::Avi] {
            assert!(validate_subtitle_policy(unsupported, false, &conversion).is_err());
            assert!(validate_subtitle_policy(unsupported, false, &extraction).is_err());
        }
    }

    #[test]
    fn subtitle_edges_preserve_required_providers_and_selected_stream_identity() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_subtitle_edges(
            &mut graph,
            &mut tools,
            Format::Mkv,
            false,
            &SubtitlePolicy { stream_index: Some(3) },
        ).unwrap();
        let extraction = graph.transforms_from(Format::Mkv);
        assert_eq!(extraction.len(), 1);
        let edge = &extraction[0];
        assert_eq!(edge.to, Format::TimedTranscriptJson);
        assert_eq!(edge.provider_id.as_deref(), Some("tool.ffmpeg"));
        assert_eq!(edge.required_provider_ids, vec!["tool.ffprobe".to_string()]);
        assert_eq!(edge.evidence.get("subtitle_stream_index").map(String::as_str), Some("3"));
        let mut spec = minimal_spec("mkv");
        spec.execution.tools.deny.push("tool.ffprobe".to_string());
        let filtered = apply_execution_policy(&graph, &tools, &spec);
        assert!(filtered.transforms_from(Format::Mkv).is_empty());
        assert!(filtered.find_path(Format::Srt, Format::WebVtt).is_some());
    }

    #[test]
    fn subtitle_registration_keeps_unrequested_media_and_recognition_only_formats_unreachable() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_subtitle_edges(
            &mut graph,
            &mut tools,
            Format::Srt,
            false,
            &SubtitlePolicy::default(),
        ).unwrap();
        assert!(graph.find_path(Format::Srt, Format::WebVtt).is_some());
        assert!(graph.find_path(Format::Mkv, Format::TimedTranscriptJson).is_none());
        for unsupported in [Format::Ass, Format::Ssa, Format::Ttml, Format::Json] {
            assert!(graph.find_path(unsupported, Format::TimedTranscriptJson).is_none());
        }
        assert_eq!(artifact_extension(Format::TimedTranscriptJson), "timed-transcript.json");
    }

    #[test]
    fn transcript_text_is_a_lossy_terminal_even_with_custom_outgoing_text_edges() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_subtitle_edges(&mut graph, &mut tools, Format::Srt, false,
            &SubtitlePolicy::default()).unwrap();
        graph.add_transform(TransformEdge::new(Format::Text, Format::Pdf, 0.1, 1.0));
        graph.add_transform(TransformEdge::new(Format::Text, Format::TimedTranscriptJson, 0.1, 1.0));
        let mut spec = minimal_spec("srt");
        spec.execution.subtitles = Some(SubtitlePolicy::default());
        let filtered = apply_execution_policy(&graph, &tools, &spec);
        assert!(filtered.find_path(Format::Srt, Format::Text).is_some());
        assert!(filtered.transforms_from(Format::Text).is_empty());
        assert!(filtered.find_path(Format::Srt, Format::Pdf).is_none());
        assert!(validate_subtitle_policy(Format::Text, false, &SubtitlePolicy::default()).is_err());
        let exports = filtered.transforms_from(Format::TimedTranscriptJson);
        let text = exports.iter().find(|edge| edge.to == Format::Text).unwrap();
        let timed = exports.iter().find(|edge| edge.to == Format::Srt).unwrap();
        assert!(text.quality < timed.quality);
        spec.execution.minimum_fidelity = Some(0.9);
        let preserved = apply_execution_policy(&graph, &tools, &spec);
        assert!(preserved.find_path(Format::Srt, Format::Text).is_none());
        assert!(preserved.find_path(Format::Srt, Format::WebVtt).is_some());
    }

    #[test]
    fn explicit_speech_recipe_exports_text_from_its_one_canonical_transcript() {
        let policy: SpeechPolicy = serde_json::from_value(serde_json::json!({
            "source_sha256": "a".repeat(64),
            "provider": {"path": std::env::temp_dir().join("unobserved-whisper"), "sha256": "b".repeat(64)},
            "model": {"path": std::env::temp_dir().join("unobserved-model.bin"),
                "sha256": "c".repeat(64), "id": "synthetic-model"}
        })).unwrap();
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_speech_edges(&mut graph, &mut tools, Format::Wav, &policy).unwrap();
        let recognition = graph.transforms_from(Format::Wav);
        assert_eq!(recognition.len(), 1);
        assert_eq!(recognition[0].to, Format::TimedTranscriptJson);
        assert!(graph.find_path(Format::Wav, Format::Text).is_some());
        assert!(graph.transforms_from(Format::Text).is_empty());
        let exports = graph.transforms_from(Format::TimedTranscriptJson);
        let text = exports.iter().find(|edge| edge.to == Format::Text).unwrap();
        assert_eq!(text.evidence.get("cache").map(String::as_str), Some("disabled"));
        assert!(text.quality < exports.iter().find(|edge| edge.to == Format::Srt).unwrap().quality);
        let mut spec = minimal_spec("wav");
        spec.execution.ai = AiPolicy::LocalOnly;
        spec.execution.speech = Some(policy);
        spec.targets.exact.push(TargetSpec { format: Some("text".to_owned()), ..Default::default() });
        let targets = resolve_target_intent(&spec, &graph, Format::Wav).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].format, Format::Text);
    }

    #[test]
    fn builtin_strategies_do_not_claim_screenplay_conversion_routes() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_builtin_strategy_edges(&mut graph, &mut tools).unwrap();
        assert!(graph.find_path(Format::Markdown, Format::Pdf).is_some());
        for format in [
            Format::Fountain,
            Format::Fdx,
            Format::FadeIn,
            Format::Osf,
            Format::ScreenplayJson,
        ] {
            assert!(graph.transforms_from(format).is_empty());
            assert!(graph.transforms_to(format).is_empty());
            assert!(graph.find_path(format, Format::Pdf).is_none());
            assert!(document_input_format(format).is_none());
        }
        assert_eq!(artifact_extension(Format::ScreenplayJson), "screenplay.json");
    }

    fn screenplay_pdf_policy() -> AfterwritingConfig {
        AfterwritingConfig {
            node_executable: std::env::temp_dir().join("node"), node_sha256: "a".repeat(64),
            expected_node_version: "v24.0.0".to_owned(), package_root: std::env::temp_dir().join("afterwriting"),
            package_tree_sha256: "b".repeat(64), expected_provider_version: "1.17.3".to_owned(),
            profile: crate::screenplay::afterwriting::ScreenplayPdfProfile::A4,
            timeout_seconds: 30, allow_unqualified: true,
        }
    }

    #[test]
    fn screenplay_pdf_edge_requires_original_fountain_and_both_configured_providers() {
        let policy = screenplay_pdf_policy();
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_screenplay_pdf_edge(&mut graph, &mut tools, Format::Fountain, &policy).unwrap();
        let edges = graph.all_edges();
        assert_eq!(edges.len(), 1);
        assert_eq!((edges[0].from, edges[0].to), (Format::Fountain, Format::Pdf));
        assert_eq!(edges[0].provider_id.as_deref(), Some(AFTERWRITING_PROVIDER_ID));
        assert_eq!(edges[0].required_provider_ids, vec![NODE_PROVIDER_ID.to_owned()]);
        assert_eq!(edges[0].capability_id.as_deref(), Some(SCREENPLAY_PDF_CAPABILITY));
        assert_eq!(edges[0].evidence["cache"], "disabled");
        for source in [Format::Fdx, Format::FadeIn, Format::Osf, Format::ScreenplayJson, Format::Markdown] {
            assert!(validate_screenplay_pdf_policy(source, false, &policy).is_err());
            assert!(graph.find_path(source, Format::Pdf).is_none());
        }
        assert!(validate_screenplay_pdf_policy(Format::Fountain, true, &policy).is_err());
    }

    #[test]
    fn screenplay_pdf_policy_and_provider_denials_remove_the_exact_edge() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_screenplay_pdf_edge(&mut graph, &mut tools, Format::Fountain, &screenplay_pdf_policy()).unwrap();
        let mut spec = minimal_spec("fountain");
        assert!(apply_execution_policy(&graph, &tools, &spec).all_edges().is_empty());
        spec.execution.screenplay_pdf = Some(screenplay_pdf_policy());
        assert_eq!(apply_execution_policy(&graph, &tools, &spec).all_edges().len(), 1);
        for denied in [NODE_PROVIDER_ID, AFTERWRITING_PROVIDER_ID] {
            spec.execution.tools.deny = vec![denied.to_owned()];
            assert!(apply_execution_policy(&graph, &tools, &spec).all_edges().is_empty());
        }
    }

    #[test]
    fn screenplay_pdf_denied_routes_never_invoke_the_probe_callback() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_screenplay_pdf_edge(&mut graph, &mut tools, Format::Fountain, &screenplay_pdf_policy()).unwrap();
        let mut base = minimal_spec("fountain");
        base.execution.screenplay_pdf = Some(screenplay_pdf_policy());
        base.targets.exact = vec![TargetSpec { format: Some("pdf".to_owned()), ..Default::default() }];
        let mut denied_specs = Vec::new();
        for provider in [NODE_PROVIDER_ID, AFTERWRITING_PROVIDER_ID] {
            let mut denied = base.clone();
            denied.execution.tools.deny.push(provider.to_owned());
            denied_specs.push(denied);
            let mut excluded = base.clone();
            excluded.execution.tools.allow.push(provider.to_owned());
            denied_specs.push(excluded);
        }
        let mut denied_transform = base.clone();
        denied_transform.execution.transforms.deny.push(SCREENPLAY_PDF_CAPABILITY.to_owned());
        denied_specs.push(denied_transform);
        let mut deterministic = base;
        deterministic.execution.requirements.deterministic = true;
        denied_specs.push(deterministic);

        for spec in denied_specs {
            let filtered = apply_execution_policy(&graph, &tools, &spec);
            // Explicit targets survive intent resolution even when policy
            // removes their edge, which previously still launched Node.
            let targets = resolve_target_intent(&spec, &filtered, Format::Fountain).unwrap();
            assert_eq!(targets.len(), 1);
            assert_eq!(targets[0].format, Format::Pdf);
            // Planning sees filtered edges; execution refresh must also refuse
            // an otherwise selected edge when its current policy denies it.
            for edges in [filtered.all_edges(), graph.all_edges()] {
                let mut inventory = ToolInventory { tools: Vec::new() };
                let mut calls = 0;
                let probed = assess_screenplay_pdf_availability(&mut inventory, &spec, &tools, &edges, |_| {
                    calls += 1;
                    Ok(("v24.0.0".to_owned(), "1.17.3".to_owned()))
                });
                assert!(!probed);
                assert_eq!(calls, 0);
                assert!(inventory.tools.is_empty());
            }
        }
    }

    #[test]
    fn screenplay_pdf_probe_requires_its_exact_selected_edge() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_screenplay_pdf_edge(&mut graph, &mut tools, Format::Fountain, &screenplay_pdf_policy()).unwrap();
        let mut spec = minimal_spec("fountain");
        spec.execution.screenplay_pdf = Some(screenplay_pdf_policy());
        let mut inventory = ToolInventory { tools: Vec::new() };
        let mut calls = 0;
        assert!(assess_screenplay_pdf_availability(&mut inventory, &spec, &tools, &graph.all_edges(), |_| {
            calls += 1;
            Ok(("v24.0.0".to_owned(), "1.17.3".to_owned()))
        }));
        assert_eq!(calls, 1);
        assert_eq!(inventory.tools.len(), 2);
        assert!(inventory.tools.iter().all(ToolAvailability::is_available));

        let mut unrelated = graph.all_edges()[0].clone();
        unrelated.capability_id = Some("screenplay.other".to_owned());
        let mut unselected = ToolInventory { tools: Vec::new() };
        for edges in [Vec::new(), vec![&unrelated]] {
            assert!(!assess_screenplay_pdf_availability(&mut unselected, &spec, &tools, &edges, |_| {
                calls += 1;
                Ok(("v24.0.0".to_owned(), "1.17.3".to_owned()))
            }));
        }
        assert_eq!(calls, 1);
        assert!(unselected.tools.is_empty());
    }

    #[test]
    fn screenplay_pdf_unavailable_pin_or_runtime_prunes_only_its_edge() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        let policy = screenplay_pdf_policy();
        register_screenplay_pdf_edge(&mut graph, &mut tools, Format::Fountain, &policy).unwrap();
        graph.add_transform(TransformEdge::new(Format::Srt, Format::TimedTranscriptJson, 0.1, 1.0));
        for failure in ["missing package font assets", "node version mismatch", "package SHA-256 mismatch", "cancelled probe"] {
            let mut inventory = ToolInventory { tools: Vec::new() };
            append_screenplay_pdf_availability(&mut inventory, &policy, Err(failure.to_owned()));
            assert!(inventory.tools.iter().all(|tool| !tool.is_available()));
            assert!(inventory.tools.iter().all(|tool| tool.diagnostic.as_deref().unwrap().contains("execution.screenplay_pdf")));
            let available = graph.filtered_by_available_providers(&inventory.available_ids());
            assert!(available.find_path(Format::Fountain, Format::Pdf).is_none());
            assert!(available.find_path(Format::Srt, Format::TimedTranscriptJson).is_some());
        }
    }

    #[test]
    fn screenplay_pdf_availability_uses_safe_preflight_observations_without_generic_probe() {
        let mut inventory = ToolInventory { tools: Vec::new() };
        let policy = screenplay_pdf_policy();
        append_screenplay_pdf_availability(&mut inventory, &policy, Ok(("v24.0.0".to_owned(), "1.17.3".to_owned())));
        assert!(inventory.tools.iter().all(ToolAvailability::is_available));
        assert_eq!(inventory.get(NODE_PROVIDER_ID).unwrap().version_line.as_deref(), Some("v24.0.0"));
        assert_eq!(inventory.get(AFTERWRITING_PROVIDER_ID).unwrap().version_line.as_deref(), Some("1.17.3"));
        assert!(is_screenplay_pdf_provider(NODE_PROVIDER_ID, Some(&policy)));
        assert!(is_screenplay_pdf_provider(AFTERWRITING_PROVIDER_ID, Some(&policy)));
        assert!(!is_screenplay_pdf_provider("tool.ffmpeg", Some(&policy)));
        assert!(!is_screenplay_pdf_provider(NODE_PROVIDER_ID, None));
    }

    #[test]
    fn screenplay_pdf_targets_refuse_same_format_and_ignored_formatting_options() {
        assert!(validate_screenplay_pdf_targets(&[ResolvedTarget::generated(Format::Pdf)]).is_ok());
        assert!(validate_screenplay_pdf_targets(&[]).is_err());
        for format in [Format::Fountain, Format::ScreenplayJson, Format::Json, Format::Fdx] {
            assert!(validate_screenplay_pdf_targets(&[ResolvedTarget::generated(format)]).is_err());
        }
        let mut target = ResolvedTarget::generated(Format::Pdf);
        target.options.insert("font_family".to_owned(), serde_json::json!("Custom"));
        assert!(validate_screenplay_pdf_targets(&[target]).is_err());
        let mut target = ResolvedTarget::generated(Format::Pdf);
        target.template = Some("custom.html".to_owned());
        assert!(validate_screenplay_pdf_targets(&[target]).is_err());
    }

    #[test]
    fn screenplay_pdf_refuses_hygiene_inherited_through_profiles() {
        let mut spec = minimal_spec("fountain");
        spec.execution.screenplay_pdf = Some(screenplay_pdf_policy());
        spec.hygiene.insert("hygiene.screenplay".to_owned(), HygienePolicy::default());
        spec.profiles.insert("screenplay.parent".to_owned(), DerivativeProfile {
            hygiene_policy: Some("hygiene.screenplay".to_owned()), ..Default::default()
        });
        spec.profiles.insert("screenplay.child".to_owned(), DerivativeProfile {
            extends: vec!["screenplay.parent".to_owned()], ..Default::default()
        });
        spec.targets.profiles.push("screenplay.child".to_owned());
        assert!(validate_screenplay_pdf_hygiene(&spec).unwrap_err().to_string().contains("screenplay_pdf.policy.hygiene"));
    }

    fn screenplay_policy() -> ScripttoolConfig {
        ScripttoolConfig {
            executable: std::env::temp_dir().join("scripttool"),
            executable_sha256: "a".repeat(64),
            expected_version: "0.0.10".to_owned(),
            timeout_seconds: 30,
            allow_unqualified: true,
        }
    }

    struct ScreenplayProbe {
        expected_executable: String,
        status: crate::process::ToolProbeStatus,
        line: Option<String>,
    }

    impl crate::toolchain::ToolProbe for ScreenplayProbe {
        fn probe(&self, executable: &str, args: &[String]) -> crate::process::ToolProbeEvidence {
            assert_eq!(executable, self.expected_executable);
            assert_eq!(args, &["-version".to_owned()]);
            crate::process::ToolProbeEvidence {
                executable: executable.to_owned(),
                status: self.status,
                version_line: self.line.clone(),
                duration_ms: 0,
                platform: crate::process::ProcessPlatform {
                    os: "linux", arch: "x86_64",
                    tree_termination: crate::process::ProcessTreeTermination::DirectChild,
                },
                diagnostic: None,
            }
        }
    }

    #[test]
    fn screenplay_registration_preserves_direct_direction_and_explicit_policy() {
        for source in [Format::Fdx, Format::FadeIn, Format::Osf] {
            let policy = screenplay_policy();
            let mut graph = TransformGraph::new();
            let mut tools = ToolRegistry::builtins();
            register_screenplay_edges(&mut graph, &mut tools, source, &policy).unwrap();
            let edges = graph.all_edges();
            assert_eq!(edges.len(), 1);
            assert_eq!((edges[0].from, edges[0].to), (source, Format::Fountain));
            assert_eq!(edges[0].provider_id.as_deref(), Some(SCREENPLAY_PROVIDER));
            assert_eq!(edges[0].evidence["qualification"], "authored_unqualified");
            assert_eq!(edges[0].evidence["cache"], "disabled");
            for unsupported in [Format::Fdx, Format::Osf, Format::FadeIn, Format::ScreenplayJson, Format::Json] {
                assert!(graph.find_path(Format::Fountain, unsupported).is_none());
            }
            let mut spec = minimal_spec(&source.to_string());
            assert!(apply_execution_policy(&graph, &tools, &spec).all_edges().is_empty());
            spec.execution.screenplay = Some(policy);
            assert_eq!(apply_execution_policy(&graph, &tools, &spec).all_edges().len(), 1);
            spec.execution.tools.deny.push(SCREENPLAY_PROVIDER.to_owned());
            assert!(apply_execution_policy(&graph, &tools, &spec).all_edges().is_empty());
        }
    }

    #[test]
    fn screenplay_missing_or_incompatible_configured_provider_prunes_only_its_routes() {
        for (status, line, expected_status) in [
            (crate::process::ToolProbeStatus::Missing, None, ToolAvailabilityStatus::MissingExecutable),
            (crate::process::ToolProbeStatus::Available, Some("scripttool 0.0.9"), ToolAvailabilityStatus::UnsupportedVersion),
            (crate::process::ToolProbeStatus::Available, Some("unexpected 0.0.10"), ToolAvailabilityStatus::UnsupportedVersion),
        ] {
            let policy = screenplay_policy();
            let mut graph = TransformGraph::new();
            let mut tools = ToolRegistry::builtins();
            register_screenplay_edges(&mut graph, &mut tools, Format::Fdx, &policy).unwrap();
            graph.add_transform(TransformEdge::new(Format::Srt, Format::TimedTranscriptJson, 0.1, 1.0)
                .with_provider(TIMED_TEXT_PROVIDER, "subtitle.convert.srt.timed_transcript_json"));
            let probe = ScreenplayProbe {
                expected_executable: policy.executable.to_str().unwrap().to_owned(),
                status, line: line.map(str::to_owned),
            };
            let mut inventory = tools.assess_ids_with(graph.provider_ids(), &probe,
                &ToolRuntimeContext::for_platform("linux", "x86_64"));
            apply_screenplay_version_requirement(&mut inventory, Some(&policy));
            let blocked = inventory.get(SCREENPLAY_PROVIDER).unwrap();
            assert_eq!(blocked.status, expected_status);
            assert!(blocked.diagnostic.as_deref().unwrap().contains("execution.screenplay.executable"));
            let available = graph.filtered_by_available_providers(&inventory.available_ids());
            assert!(available.find_path(Format::Fdx, Format::Fountain).is_none());
            assert!(available.find_path(Format::Srt, Format::TimedTranscriptJson).is_some());
        }
    }

    #[test]
    fn screenplay_compatible_version_remains_an_observation_not_pin_attestation() {
        let policy = screenplay_policy();
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_screenplay_edges(&mut graph, &mut tools, Format::Osf, &policy).unwrap();
        let probe = ScreenplayProbe {
            expected_executable: policy.executable.to_str().unwrap().to_owned(),
            status: crate::process::ToolProbeStatus::Available,
            line: Some("scripttool 0.0.10".to_owned()),
        };
        let mut inventory = tools.assess_ids_with(graph.provider_ids(), &probe,
            &ToolRuntimeContext::for_platform("linux", "x86_64"));
        apply_screenplay_version_requirement(&mut inventory, Some(&policy));
        assert!(inventory.get(SCREENPLAY_PROVIDER).unwrap().is_available());
        assert!(graph.filtered_by_available_providers(&inventory.available_ids())
            .find_path(Format::Osf, Format::Fountain).is_some());
        // No file is opened to establish this observation; the deliberately
        // synthetic executable hash remains a declaration until execution.
        assert_eq!(policy.executable_sha256, "a".repeat(64));
    }

    #[test]
    fn screenplay_policy_rejects_collections_and_non_adopted_sources() {
        for source in [Format::Fdx, Format::Osf, Format::FadeIn] {
            assert!(validate_screenplay_policy(source, false, &screenplay_policy()).is_ok());
            assert!(validate_screenplay_policy(source, true, &screenplay_policy()).is_err());
        }
        for source in [Format::Fountain, Format::ScreenplayJson, Format::Json, Format::Markdown] {
            assert!(validate_screenplay_policy(source, false, &screenplay_policy()).is_err());
        }
    }

    #[test]
    fn screenplay_policy_rejects_hygiene_inherited_from_selected_profiles() {
        let mut spec = minimal_spec("fdx");
        spec.execution.screenplay = Some(screenplay_policy());
        spec.hygiene.insert("hygiene.screenplay".to_owned(), HygienePolicy::default());
        spec.profiles.insert("screenplay.parent".to_owned(), DerivativeProfile {
            hygiene_policy: Some("hygiene.screenplay".to_owned()),
            ..DerivativeProfile::default()
        });
        spec.profiles.insert("screenplay.child".to_owned(), DerivativeProfile {
            extends: vec!["screenplay.parent".to_owned()],
            ..DerivativeProfile::default()
        });
        spec.targets.profiles.push("screenplay.child".to_owned());
        compose_selected_profiles(&mut spec).unwrap();
        assert!(spec.execution.hygiene_policy.is_none());
        assert!(validate_screenplay_hygiene(&spec).unwrap_err().to_string().contains("screenplay.policy.hygiene"));
    }

    #[test]
    fn screenplay_wrapper_conflicts_require_explicit_dialects_and_exact_container_pairs() {
        for (source, wrapper) in [(Format::Fdx, "xml"), (Format::Osf, "xml"), (Format::FadeIn, "zip")] {
            let declared = source.to_string();
            let pair = vec![declared.clone(), wrapper.to_owned()];
            assert!(screenplay_wrapper_conflict(source, Some(&declared), &pair));
            assert!(!screenplay_wrapper_conflict(source, None, &pair));
            assert!(!screenplay_wrapper_conflict(source, Some("markdown"), &pair));
            assert!(!screenplay_wrapper_conflict(source, Some(&declared), &[declared.clone(), "html".to_owned()]));
            assert!(!screenplay_wrapper_conflict(source, Some(&declared), &[declared.clone(), wrapper.to_owned(), "pdf".to_owned()]));
        }
    }
}
