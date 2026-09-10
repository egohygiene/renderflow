use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass, ArtifactStore};
use crate::checkpoint::{CheckpointContext, CheckpointStore, RecoveryAction, RecoveryDecision};
use crate::evidence::{
    sha256_serialized, ArtifactManifest, DiagnosticSeverity, ExecutionDiagnostic, RunManifest,
};
use crate::graph::ExecutionPlan;
use crate::intake::{IntakeEngine, IntakeReport, IntakeRequest};
use crate::optimization::OptimizationMode;
use crate::planning::{
    cancelled as cancelled_execution, execute as execute_resolved_plan,
    resolve as resolve_planning_request, PlanningRequest, ResolvedExecution,
};
use crate::toolchain::ToolchainSnapshot;

pub const PROVIDER_CONTRACT_V1: &str = "renderflow.provider/v1";
pub const PROGRESS_EVENT_V1: &str = "renderflow.progress/v1";

#[derive(Debug, Error)]
pub enum RenderflowError {
    #[error("configuration error: {0}")]
    Configuration(#[source] anyhow::Error),
    #[error("planning error: {0}")]
    Planning(#[source] anyhow::Error),
    #[error("execution error: {0}")]
    Execution(#[source] anyhow::Error),
    #[error("operation cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProgressStage {
    Inspecting,
    Planning,
    Assessing,
    Resuming,
    Executing,
    Cancelled,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgressEvent {
    pub schema_version: String,
    pub stage: ProgressStage,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExecutionDiagnostic>,
}

pub trait ProgressReporter: Send + Sync {
    fn on_event(&self, event: &ProgressEvent);
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub(crate) fn flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
}

#[derive(Debug, Clone)]
pub struct InspectionRequest {
    pub config_path: PathBuf,
}

impl InspectionRequest {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanRequest {
    pub config_path: PathBuf,
    pub target: Option<String>,
    pub optimization: Option<OptimizationMode>,
}

impl PlanRequest {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
            target: None,
            optimization: None,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    pub fn with_optimization(mut self, optimization: OptimizationMode) -> Self {
        self.optimization = Some(optimization);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub config_path: PathBuf,
    pub dry_run: bool,
    pub target: Option<String>,
    pub all_targets: bool,
    pub optimization: Option<OptimizationMode>,
    #[serde(default)]
    pub resume: bool,
}

impl ExecutionRequest {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
            dry_run: false,
            target: None,
            all_targets: false,
            optimization: None,
            resume: false,
        }
    }

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self.all_targets = false;
        self
    }

    pub fn with_all_targets(mut self) -> Self {
        self.all_targets = true;
        self.target = None;
        self
    }

    pub fn with_optimization(mut self, optimization: OptimizationMode) -> Self {
        self.optimization = Some(optimization);
        self
    }

    pub fn with_resume(mut self, resume: bool) -> Self {
        self.resume = resume;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapabilities {
    pub schema_version: String,
    pub provider_id: String,
    pub provider_version: String,
    pub operations: Vec<String>,
    pub progress_event_schema: String,
    pub checkpoint_schema: String,
    pub artifact_contract: String,
    pub intake_schema: String,
    pub hygiene_schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderPlan {
    pub schema_version: String,
    pub plan: ExecutionPlan,
    pub digest: crate::evidence::DigestEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SavedRunAssessment {
    pub schema_version: String,
    pub checkpoint_path: String,
    pub overall: RecoveryDecision,
    pub decisions: Vec<RecoveryDecision>,
}

pub trait RenderflowProvider {
    fn inspect_capabilities(&self) -> ProviderCapabilities;
    fn plan_request(&self, request: PlanRequest) -> Result<ProviderPlan, RenderflowError>;
    fn run_plan(&self, request: ExecutionRequest) -> Result<ExecutionResult, RenderflowError>;
    fn assess_saved_run(
        &self,
        request: ExecutionRequest,
    ) -> Result<SavedRunAssessment, RenderflowError>;
    fn resume_saved_run(
        &self,
        request: ExecutionRequest,
    ) -> Result<ExecutionResult, RenderflowError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactProfile {
    pub input_path: String,
    pub input_format: String,
    pub output_dir: String,
    pub targets: Vec<String>,
    pub transforms_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticReport {
    #[serde(default)]
    pub info: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub recoverable_failures: Vec<String>,
    #[serde(default)]
    pub fatal_failures: Vec<String>,
    #[serde(default)]
    pub cancellations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionResult {
    pub manifest: ArtifactManifest,
    pub run_manifest: RunManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<String>,
    pub reused_cached_outputs: Vec<String>,
    pub skipped_transforms: Vec<String>,
    pub diagnostics: DiagnosticReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<ToolchainSnapshot>,
}

impl ExecutionResult {
    pub fn is_success(&self) -> bool {
        matches!(
            self.run_manifest.state,
            crate::evidence::RunState::Planned | crate::evidence::RunState::Complete
        )
    }
}

#[derive(Default)]
pub struct EngineBuilder {
    reporter: Option<Arc<dyn ProgressReporter>>,
    cancellation_token: Option<CancellationToken>,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_transforms(self) -> Self {
        self
    }

    pub fn with_progress_reporter(mut self, reporter: Arc<dyn ProgressReporter>) -> Self {
        self.reporter = Some(reporter);
        self
    }

    pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
        self.cancellation_token = Some(cancellation_token);
        self
    }

    pub fn build(self) -> Result<Engine, RenderflowError> {
        Ok(Engine {
            reporter: self.reporter,
            cancellation_token: self.cancellation_token,
        })
    }
}

pub struct Engine {
    reporter: Option<Arc<dyn ProgressReporter>>,
    cancellation_token: Option<CancellationToken>,
}

impl Engine {
    fn emit(&self, stage: ProgressStage, message: impl Into<String>) {
        if let Some(reporter) = &self.reporter {
            reporter.on_event(&ProgressEvent {
                schema_version: PROGRESS_EVENT_V1.to_string(),
                stage,
                message: message.into(),
                run_id: None,
                step_id: None,
                artifact_ids: Vec::new(),
                state: None,
                diagnostics: Vec::new(),
            });
        }
    }

    fn emit_step_events(&self, manifest: &RunManifest) {
        let Some(reporter) = &self.reporter else {
            return;
        };
        for step in &manifest.steps {
            reporter.on_event(&ProgressEvent {
                schema_version: PROGRESS_EVENT_V1.to_string(),
                stage: if step.state == crate::evidence::StepState::Cancelled {
                    ProgressStage::Cancelled
                } else {
                    ProgressStage::Executing
                },
                message: format!("Transform '{}' is {:?}", step.transform, step.state),
                run_id: Some(manifest.run_id.clone()),
                step_id: Some(step.step_id.clone()),
                artifact_ids: step.output_artifacts.clone(),
                state: Some(format!("{:?}", step.state).to_lowercase()),
                diagnostics: step.diagnostics.clone(),
            });
        }
    }

    fn emit_completed(&self, manifest: &RunManifest) {
        if let Some(reporter) = &self.reporter {
            reporter.on_event(&ProgressEvent {
                schema_version: PROGRESS_EVENT_V1.to_string(),
                stage: if manifest.state == crate::evidence::RunState::Cancelled {
                    ProgressStage::Cancelled
                } else {
                    ProgressStage::Completed
                },
                message: format!("Renderflow run is {:?}", manifest.state),
                run_id: Some(manifest.run_id.clone()),
                step_id: None,
                artifact_ids: manifest
                    .artifact_manifest
                    .artifacts
                    .iter()
                    .map(|artifact| artifact.artifact_id.clone())
                    .collect(),
                state: Some(format!("{:?}", manifest.state).to_lowercase()),
                diagnostics: manifest.diagnostics.clone(),
            });
        }
    }

    fn ensure_not_cancelled(&self) -> Result<(), RenderflowError> {
        if self
            .cancellation_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(RenderflowError::Cancelled);
        }
        Ok(())
    }

    pub fn inspect(&self, request: InspectionRequest) -> Result<ArtifactProfile, RenderflowError> {
        self.ensure_not_cancelled()?;
        self.emit(
            ProgressStage::Inspecting,
            "Resolving canonical execution context",
        );
        let resolved = resolve_planning_request(PlanningRequest::from_path(&request.config_path))
            .map_err(RenderflowError::Configuration)?;
        let profile = ArtifactProfile {
            input_path: resolved.source_path().display().to_string(),
            input_format: resolved.source_format().to_string(),
            output_dir: resolved.spec().output.bundle_root.clone(),
            targets: resolved
                .target_formats()
                .iter()
                .map(ToString::to_string)
                .collect(),
            transforms_path: resolved.spec().transforms.clone(),
        };
        self.emit(ProgressStage::Completed, "Inspection complete");
        Ok(profile)
    }

    /// Inspect arbitrary input bytes and optionally extract first-class child artifacts.
    pub fn inspect_artifact(
        &self,
        request: IntakeRequest,
        store_root: impl AsRef<Path>,
    ) -> Result<IntakeReport, RenderflowError> {
        self.ensure_not_cancelled()?;
        self.emit(ProgressStage::Inspecting, "Inspecting source artifact");
        let store = ArtifactStore::new(store_root.as_ref().to_path_buf())
            .map_err(RenderflowError::Execution)?;
        let report = IntakeEngine::new()
            .intake(&request, &store)
            .map_err(RenderflowError::Execution)?;
        if let Some(reporter) = &self.reporter {
            reporter.on_event(&ProgressEvent {
                schema_version: PROGRESS_EVENT_V1.to_string(),
                stage: ProgressStage::Completed,
                message: "Artifact inspection complete".to_string(),
                run_id: None,
                step_id: None,
                artifact_ids: report
                    .artifact_collection()
                    .iter()
                    .map(|artifact| artifact.id().to_string())
                    .collect(),
                state: Some("complete".to_string()),
                diagnostics: Vec::new(),
            });
        }
        Ok(report)
    }

    pub fn plan(&self, request: PlanRequest) -> Result<ExecutionPlan, RenderflowError> {
        self.ensure_not_cancelled()?;
        self.emit(
            ProgressStage::Planning,
            "Constructing canonical execution plan",
        );
        let mut planning = PlanningRequest::from_path(&request.config_path);
        if let Some(target) = request.target {
            planning = planning.with_target(target);
        }
        if let Some(optimization) = request.optimization {
            planning = planning.with_optimization(optimization);
        }
        let resolved = resolve_planning_request(planning).map_err(RenderflowError::Planning)?;
        let plan = resolved.plan().clone();
        self.emit(ProgressStage::Completed, "Planning complete");
        Ok(plan)
    }

    /// Resolve an execution request into the frozen plan/runtime object that
    /// [`Engine::execute_resolved`] consumes without re-planning.
    pub fn resolve_execution(
        &self,
        request: ExecutionRequest,
    ) -> Result<ResolvedExecution, RenderflowError> {
        self.ensure_not_cancelled()?;
        self.emit(ProgressStage::Planning, "Resolving execution request");
        let mut planning = PlanningRequest::from_path(&request.config_path);
        if let Some(target) = request.target {
            planning = planning.with_target(target);
        } else if request.all_targets {
            planning = planning.with_all_reachable();
        }
        if let Some(optimization) = request.optimization {
            planning = planning.with_optimization(optimization);
        }
        let mut resolved = resolve_planning_request(planning).map_err(RenderflowError::Planning)?;
        resolved = resolved.with_resume(request.resume);
        if let Some(cancellation) = &self.cancellation_token {
            resolved = resolved.with_cancellation_flag(cancellation.flag());
        }
        Ok(resolved)
    }

    /// Execute an already-resolved plan without implicit re-planning.
    pub fn execute_resolved(
        &self,
        resolved: ResolvedExecution,
        dry_run: bool,
    ) -> Result<ExecutionResult, RenderflowError> {
        self.emit(
            ProgressStage::Executing,
            "Executing resolved renderflow plan",
        );
        let result = if self
            .cancellation_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            cancelled_execution(resolved).map_err(RenderflowError::Execution)?
        } else {
            execute_resolved_plan(resolved, dry_run).map_err(RenderflowError::Execution)?
        };
        self.emit_step_events(&result.run_manifest);
        self.emit_completed(&result.run_manifest);
        let info = result
            .run_manifest
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Info)
            .map(|diagnostic| diagnostic.message.clone())
            .collect();
        let warnings = result
            .run_manifest
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
            .map(|diagnostic| diagnostic.message.clone())
            .collect();
        let recoverable_failures = result
            .run_manifest
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::RecoverableFailure)
            .map(|diagnostic| diagnostic.message.clone())
            .collect();
        let fatal_failures = result
            .run_manifest
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::FatalFailure)
            .map(|diagnostic| diagnostic.message.clone())
            .collect();
        let cancellations = result
            .run_manifest
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Cancelled)
            .map(|diagnostic| diagnostic.message.clone())
            .collect();
        let reused_cached_outputs = result.run_manifest.cache_hits();
        let skipped_transforms = result.run_manifest.skipped_transforms();
        Ok(ExecutionResult {
            manifest: result.run_manifest.artifact_manifest.clone(),
            run_manifest: result.run_manifest,
            manifest_path: result.manifest_path,
            reused_cached_outputs,
            skipped_transforms,
            diagnostics: DiagnosticReport {
                info,
                warnings,
                recoverable_failures,
                fatal_failures,
                cancellations,
            },
            toolchain: result.toolchain,
        })
    }

    pub fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, RenderflowError> {
        let dry_run = request.dry_run;
        let resolved = self.resolve_execution(request)?;
        self.execute_resolved(resolved, dry_run)
    }

    pub fn provider_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            schema_version: PROVIDER_CONTRACT_V1.to_string(),
            provider_id: "provider.renderflow".to_string(),
            provider_version: env!("CARGO_PKG_VERSION").to_string(),
            operations: vec![
                "inspect_capabilities".to_string(),
                "inspect_artifact".to_string(),
                "extract_artifacts".to_string(),
                "publication_hygiene".to_string(),
                "plan".to_string(),
                "run".to_string(),
                "assess".to_string(),
                "resume".to_string(),
                "invalidate".to_string(),
            ],
            progress_event_schema: PROGRESS_EVENT_V1.to_string(),
            checkpoint_schema: crate::checkpoint::CHECKPOINT_SCHEMA_V1.to_string(),
            artifact_contract: crate::evidence::FLOW_ARTIFACT_SCHEMA_V1.to_string(),
            intake_schema: crate::intake::INTAKE_SCHEMA_V1.to_string(),
            hygiene_schema: crate::hygiene::HYGIENE_EVIDENCE_SCHEMA_V1.to_string(),
        }
    }

    pub fn provider_plan(&self, request: PlanRequest) -> Result<ProviderPlan, RenderflowError> {
        let plan = self.plan(request)?;
        let digest = sha256_serialized(&plan).map_err(RenderflowError::Planning)?;
        Ok(ProviderPlan {
            schema_version: PROVIDER_CONTRACT_V1.to_string(),
            plan,
            digest,
        })
    }

    pub fn assess_resume(
        &self,
        request: ExecutionRequest,
    ) -> Result<SavedRunAssessment, RenderflowError> {
        self.emit(
            ProgressStage::Assessing,
            "Assessing saved Renderflow checkpoints",
        );
        let resolved = self.resolve_execution(request)?;
        let (checkpoint_path, artifact_root, context) =
            checkpoint_state(&resolved).map_err(RenderflowError::Execution)?;
        if !checkpoint_path.exists() {
            let overall = RecoveryDecision {
                action: RecoveryAction::Recompute,
                reason_code: "checkpoint.file_missing".to_string(),
                message: "No saved checkpoint file exists".to_string(),
                checkpoint_key: None,
                artifact_ids: Vec::new(),
            };
            return Ok(SavedRunAssessment {
                schema_version: PROVIDER_CONTRACT_V1.to_string(),
                checkpoint_path: checkpoint_path.display().to_string(),
                overall: overall.clone(),
                decisions: vec![overall],
            });
        }
        let checkpoints = match CheckpointStore::open(&checkpoint_path, context.clone()) {
            Ok(checkpoints) => checkpoints,
            Err(error) => {
                let overall = CheckpointStore::corruption_decision(&error);
                return Ok(SavedRunAssessment {
                    schema_version: PROVIDER_CONTRACT_V1.to_string(),
                    checkpoint_path: checkpoint_path.display().to_string(),
                    overall: overall.clone(),
                    decisions: vec![overall],
                });
            }
        };
        let context_decision = checkpoints.context_decision(&context);
        let store = ArtifactStore::new(artifact_root).map_err(RenderflowError::Execution)?;
        let current_source = store
            .import_path(
                resolved.source_path(),
                ArtifactDescriptor::for_format(
                    resolved.source_format(),
                    ArtifactStorageClass::Source,
                ),
            )
            .map_err(RenderflowError::Execution)?;
        let mut decisions = checkpoints.decisions_for_source(current_source.id().as_str(), &store);
        if decisions.is_empty() {
            decisions.push(RecoveryDecision {
                action: RecoveryAction::Recompute,
                reason_code: "checkpoint.empty".to_string(),
                message: "Checkpoint file contains no completed nodes".to_string(),
                checkpoint_key: None,
                artifact_ids: Vec::new(),
            });
        }
        let overall = if context_decision.action != RecoveryAction::Reuse {
            context_decision
        } else if decisions
            .iter()
            .all(|decision| decision.action == RecoveryAction::Reuse)
        {
            RecoveryDecision {
                action: RecoveryAction::Reuse,
                reason_code: "checkpoint.run_compatible".to_string(),
                message: "Saved run is compatible and resumable".to_string(),
                checkpoint_key: None,
                artifact_ids: decisions
                    .iter()
                    .flat_map(|decision| decision.artifact_ids.iter().cloned())
                    .collect(),
            }
        } else {
            RecoveryDecision {
                action: RecoveryAction::Recompute,
                reason_code: "checkpoint.partial_recompute".to_string(),
                message: "Some saved work must be recomputed".to_string(),
                checkpoint_key: None,
                artifact_ids: Vec::new(),
            }
        };
        Ok(SavedRunAssessment {
            schema_version: PROVIDER_CONTRACT_V1.to_string(),
            checkpoint_path: checkpoint_path.display().to_string(),
            overall,
            decisions,
        })
    }

    pub fn resume(&self, request: ExecutionRequest) -> Result<ExecutionResult, RenderflowError> {
        self.emit(
            ProgressStage::Resuming,
            "Resuming compatible Renderflow work",
        );
        self.execute(request.with_resume(true))
    }

    pub fn invalidate_checkpoints(
        &self,
        request: ExecutionRequest,
        step_id: Option<&str>,
    ) -> Result<usize, RenderflowError> {
        let resolved = self.resolve_execution(request)?;
        let (checkpoint_path, _, context) =
            checkpoint_state(&resolved).map_err(RenderflowError::Execution)?;
        match step_id {
            Some(step_id) => {
                let mut checkpoints = CheckpointStore::open(checkpoint_path, context)
                    .map_err(RenderflowError::Execution)?;
                checkpoints
                    .invalidate_step(step_id)
                    .map_err(RenderflowError::Execution)
            }
            None => {
                let removed = CheckpointStore::open(&checkpoint_path, context.clone())
                    .map(|checkpoints| checkpoints.len())
                    .unwrap_or(0);
                CheckpointStore::reset(checkpoint_path, context)
                    .map(|_| removed)
                    .map_err(RenderflowError::Execution)
            }
        }
    }
}

fn checkpoint_state(
    resolved: &ResolvedExecution,
) -> anyhow::Result<(PathBuf, PathBuf, CheckpointContext)> {
    let output_root = PathBuf::from(&resolved.spec().output.bundle_root);
    let state_parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let state_dir = state_parent.join(".renderflow");
    Ok((
        state_dir.join("checkpoints.json"),
        state_dir.join("artifacts"),
        CheckpointContext {
            execution_plan_digest: sha256_serialized(resolved.plan())?,
            source_spec_digest: sha256_serialized(resolved.spec())?,
            toolchain_fingerprint: resolved
                .plan()
                .toolchain
                .as_ref()
                .map(|snapshot| snapshot.fingerprint.clone()),
        },
    ))
}

impl RenderflowProvider for Engine {
    fn inspect_capabilities(&self) -> ProviderCapabilities {
        self.provider_capabilities()
    }

    fn plan_request(&self, request: PlanRequest) -> Result<ProviderPlan, RenderflowError> {
        self.provider_plan(request)
    }

    fn run_plan(&self, request: ExecutionRequest) -> Result<ExecutionResult, RenderflowError> {
        self.execute(request)
    }

    fn assess_saved_run(
        &self,
        request: ExecutionRequest,
    ) -> Result<SavedRunAssessment, RenderflowError> {
        self.assess_resume(request)
    }

    fn resume_saved_run(
        &self,
        request: ExecutionRequest,
    ) -> Result<ExecutionResult, RenderflowError> {
        self.resume(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::RunState;

    #[test]
    fn cancellation_token_reports_cancelled_state() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn execution_request_with_all_targets_clears_explicit_target() {
        let request = ExecutionRequest::from_path("renderflow.yaml")
            .with_target("pdf")
            .with_all_targets();

        assert!(request.target.is_none());
        assert!(request.all_targets);
    }

    #[test]
    fn execution_request_with_target_clears_all_targets() {
        let request = ExecutionRequest::from_path("renderflow.yaml")
            .with_all_targets()
            .with_target("html");

        assert_eq!(request.target.as_deref(), Some("html"));
        assert!(!request.all_targets);
    }

    #[test]
    fn cancellation_after_planning_returns_and_persists_structured_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let source_path = directory.path().join("input.md");
        let output_path = directory.path().join("dist");
        let config_path = directory.path().join("renderflow.yaml");
        std::fs::write(&source_path, "# fixture\n").unwrap();
        std::fs::write(
            &config_path,
            format!(
                "input: \"{}\"\noutput_dir: \"{}\"\noutputs:\n  - type: html\n",
                source_path.display(),
                output_path.display()
            ),
        )
        .unwrap();

        let cancellation = CancellationToken::new();
        let engine = EngineBuilder::new()
            .with_cancellation_token(cancellation.clone())
            .build()
            .unwrap();
        let resolved = engine
            .resolve_execution(ExecutionRequest::from_path(&config_path))
            .unwrap();
        cancellation.cancel();

        let result = engine.execute_resolved(resolved, false).unwrap();
        assert_eq!(result.run_manifest.state, RunState::Cancelled);
        assert!(result.manifest.outputs.is_empty());
        assert!(!result.diagnostics.cancellations.is_empty());
        assert!(output_path.join("renderflow-run.json").is_file());
    }
}
