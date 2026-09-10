use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::evidence::{ArtifactManifest, DiagnosticSeverity, RunManifest};
use crate::graph::ExecutionPlan;
use crate::optimization::OptimizationMode;
use crate::planning::{
    cancelled as cancelled_execution, execute as execute_resolved_plan,
    resolve as resolve_planning_request, PlanningRequest, ResolvedExecution,
};
use crate::toolchain::ToolchainSnapshot;

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
    Executing,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgressEvent {
    pub stage: ProgressStage,
    pub message: String,
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

#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub config_path: PathBuf,
    pub dry_run: bool,
    pub target: Option<String>,
    pub all_targets: bool,
    pub optimization: Option<OptimizationMode>,
}

impl ExecutionRequest {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            config_path: path.as_ref().to_path_buf(),
            dry_run: false,
            target: None,
            all_targets: false,
            optimization: None,
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
                stage,
                message: message.into(),
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
        resolve_planning_request(planning).map_err(RenderflowError::Planning)
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
        self.emit(ProgressStage::Completed, "Execution complete");
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
