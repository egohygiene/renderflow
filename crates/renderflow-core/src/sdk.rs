use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::graph::ExecutionPlan;
use crate::optimization::OptimizationMode;
use crate::planning::{
    execute as execute_resolved_plan, resolve as resolve_planning_request, PlanningRequest,
    ResolvedExecution,
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
pub struct ArtifactManifest {
    pub output_dir: String,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticReport {
    pub warnings: Vec<String>,
    pub recoverable_failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionResult {
    pub manifest: ArtifactManifest,
    pub reused_cached_outputs: Vec<String>,
    pub skipped_transforms: Vec<String>,
    pub diagnostics: DiagnosticReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<ToolchainSnapshot>,
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
        self.ensure_not_cancelled()?;
        self.emit(
            ProgressStage::Executing,
            "Executing resolved renderflow plan",
        );
        let result =
            execute_resolved_plan(resolved, dry_run).map_err(RenderflowError::Execution)?;
        self.emit(ProgressStage::Completed, "Execution complete");
        Ok(ExecutionResult {
            manifest: ArtifactManifest {
                output_dir: result.output_dir,
                outputs: result.outputs,
            },
            reused_cached_outputs: Vec::new(),
            skipped_transforms: Vec::new(),
            diagnostics: DiagnosticReport {
                warnings: result.diagnostics,
                recoverable_failures: Vec::new(),
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
}
