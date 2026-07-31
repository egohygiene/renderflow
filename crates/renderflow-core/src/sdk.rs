use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::commands;
use crate::config::{load_config, load_config_for_graph};
use crate::graph::{ExecutionPlan, Format};
use crate::optimization::OptimizationMode;
use crate::transforms::yaml_loader::build_graph_and_executor_from_yaml;

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
        self.emit(ProgressStage::Inspecting, "Loading configuration");

        let config = load_config(request.config_path.to_str().ok_or_else(|| {
            RenderflowError::Configuration(anyhow::anyhow!(
                "Config path contains non-UTF8 characters"
            ))
        })?)
        .map_err(RenderflowError::Configuration)?;

        self.emit(ProgressStage::Completed, "Inspection complete");

        Ok(ArtifactProfile {
            input_path: config.input.clone(),
            input_format: config.input_format().to_string(),
            output_dir: config.output_dir.clone(),
            targets: config
                .outputs
                .iter()
                .map(|output| output.output_type.to_string())
                .collect(),
            transforms_path: config.transforms.clone(),
        })
    }

    pub fn plan(&self, request: PlanRequest) -> Result<ExecutionPlan, RenderflowError> {
        self.ensure_not_cancelled()?;
        self.emit(ProgressStage::Planning, "Constructing execution plan");

        let config = load_config_for_graph(request.config_path.to_str().ok_or_else(|| {
            RenderflowError::Planning(anyhow::anyhow!("Config path contains non-UTF8 characters"))
        })?)
        .map_err(RenderflowError::Planning)?;

        let transforms_path = config.transforms.as_deref().ok_or_else(|| {
            RenderflowError::Planning(anyhow::anyhow!(
                "Planning requires a `transforms` key in the config file"
            ))
        })?;

        let (graph, _executor) = build_graph_and_executor_from_yaml(transforms_path)
            .map_err(RenderflowError::Planning)?;

        let opt_mode = request.optimization.unwrap_or(config.optimization);
        let source_format: Format = config
            .input_format()
            .to_string()
            .parse()
            .map_err(|err| RenderflowError::Planning(anyhow::anyhow!("{}", err)))?;

        let targets: Vec<Format> = if let Some(target) = request.target {
            vec![target
                .parse::<Format>()
                .map_err(|err| RenderflowError::Planning(anyhow::anyhow!("{}", err)))?]
        } else {
            let reachable = graph.reachable_from(source_format);
            if reachable.is_empty() {
                return Err(RenderflowError::Planning(anyhow::anyhow!(
                    "No output formats are reachable from '{}'",
                    source_format
                )));
            }
            reachable
        };

        let dag = graph
            .build_multi_target_dag_with_mode(source_format, &targets, opt_mode)
            .ok_or_else(|| {
                RenderflowError::Planning(anyhow::anyhow!(
                    "Could not build an execution plan for one or more targets"
                ))
            })?;

        self.emit(ProgressStage::Completed, "Planning complete");
        Ok(ExecutionPlan::from_dag(
            &dag,
            source_format,
            &targets,
            opt_mode,
        ))
    }

    pub fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, RenderflowError> {
        self.ensure_not_cancelled()?;
        self.emit(ProgressStage::Executing, "Executing renderflow pipeline");

        let config_path = request.config_path.to_str().ok_or_else(|| {
            RenderflowError::Execution(anyhow::anyhow!("Config path contains non-UTF8 characters"))
        })?;

        let config = load_config(config_path).map_err(RenderflowError::Execution)?;

        if let Some(target) = request.target.as_deref() {
            commands::graph_build::run_target(
                config_path,
                target,
                request.dry_run,
                request.optimization,
            )
            .map_err(RenderflowError::Execution)?;
        } else if request.all_targets {
            commands::graph_build::run_all(config_path, request.dry_run, request.optimization)
                .map_err(RenderflowError::Execution)?;
        } else {
            commands::build::run(config_path, request.dry_run, request.optimization)
                .map_err(RenderflowError::Execution)?;
        }

        self.emit(ProgressStage::Completed, "Execution complete");

        let outputs = if let Some(target) = request.target {
            vec![target]
        } else {
            config
                .outputs
                .iter()
                .map(|output| output.output_type.to_string())
                .collect()
        };

        Ok(ExecutionResult {
            manifest: ArtifactManifest {
                output_dir: config.output_dir,
                outputs,
            },
            reused_cached_outputs: Vec::new(),
            skipped_transforms: Vec::new(),
            diagnostics: DiagnosticReport {
                warnings: Vec::new(),
                recoverable_failures: Vec::new(),
            },
        })
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
}
