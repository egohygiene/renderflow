//! One build lifecycle for ordinary format graphs and layered composition.
//!
//! Each backend retains its own frozen executable state. A composition's
//! locale-aware forest is never coerced into a format-keyed DAG.

use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::{Context, Result};

use super::{build_composition, CanonicalExecutionResult, PlanningRequest, ResolvedExecution};
use crate::graph::ExecutionPlan;
use crate::spec::{load_spec, SourceKind, SourceSpecVersion, SpecV2};
use crate::speech::graph::SpeechProgressObserver;

/// A resolved, immutable build. Execute this value without loading its config
/// again. Use `graph()` only for operations that require a format DAG.
pub struct ResolvedBuild {
    backend: Backend,
}

enum Backend {
    Graph(Box<ResolvedExecution>),
    Composition(Box<build_composition::ResolvedCompositionBuild>),
}

impl ResolvedBuild {
    pub fn plan(&self) -> &ExecutionPlan {
        match &self.backend {
            Backend::Graph(value) => value.plan(),
            Backend::Composition(value) => value.plan(),
        }
    }

    pub fn spec(&self) -> &SpecV2 {
        match &self.backend {
            Backend::Graph(value) => value.spec(),
            Backend::Composition(value) => value.spec(),
        }
    }

    pub fn source_version(&self) -> SourceSpecVersion {
        match &self.backend {
            Backend::Graph(value) => value.source_version(),
            Backend::Composition(value) => value.source_version(),
        }
    }

    pub fn source_path(&self) -> &Path {
        match &self.backend {
            Backend::Graph(value) => value.source_path(),
            Backend::Composition(value) => value.source_path(),
        }
    }

    /// Declared source representation; composition is not ordinary JSON input.
    pub fn source_format(&self) -> &str {
        &self.plan().source
    }

    pub fn target_formats(&self) -> Vec<String> {
        match &self.backend {
            Backend::Graph(value) => value.target_formats().iter().map(ToString::to_string).collect(),
            Backend::Composition(_) => self.spec().targets.exact.iter()
                .filter_map(|target| target.format.clone()).collect(),
        }
    }

    /// Explicit graph-only access prevents fabricated formats and checkpoint
    /// state for composition builds.
    pub fn graph(&self) -> Result<&ResolvedExecution> {
        match &self.backend {
            Backend::Graph(value) => Ok(value),
            Backend::Composition(_) => anyhow::bail!(
                "composition.build.graph_only: generic DAG/checkpoint operations do not apply to composition; use its plan forest and optional composition cache"
            ),
        }
    }

    pub fn is_composition(&self) -> bool {
        matches!(&self.backend, Backend::Composition(_))
    }

    pub fn predicted_output_paths(&self) -> Result<Vec<PathBuf>> {
        match &self.backend {
            Backend::Graph(value) => value.predicted_output_paths(),
            Backend::Composition(value) => value.predicted_output_paths(),
        }
    }

    pub fn with_resume(mut self, resume: bool) -> Result<Self> {
        if resume && self.is_composition() {
            anyhow::bail!("composition.build.resume_unsupported: composition builds require a new output directory; configure execution.composition.cache_directory for verified derivative reuse");
        }
        self.backend = match self.backend {
            Backend::Graph(value) => Backend::Graph(Box::new((*value).with_resume(resume))),
            value @ Backend::Composition(_) => value,
        };
        Ok(self)
    }

    pub fn with_cancellation_flag(mut self, cancellation: Arc<AtomicBool>) -> Self {
        self.backend = match self.backend {
            Backend::Graph(value) => Backend::Graph(Box::new((*value).with_cancellation_flag(cancellation))),
            Backend::Composition(value) => Backend::Composition(Box::new((*value).with_cancellation_flag(cancellation))),
        };
        self
    }

    pub fn with_speech_observer(mut self, observer: Arc<dyn SpeechProgressObserver>) -> Self {
        self.backend = match self.backend {
            Backend::Graph(value) => Backend::Graph(Box::new((*value).with_speech_observer(observer))),
            value @ Backend::Composition(_) => value,
        };
        self
    }
}

/// Load and validate the config once, then capture the selected backend.
pub fn resolve(request: PlanningRequest) -> Result<ResolvedBuild> {
    let path = request.config_path.to_str().context("Config path contains non-UTF8 characters")?;
    let loaded = load_spec(path)?;
    let backend = if loaded.spec.sources.iter().any(|source| source.kind == SourceKind::Composition) {
        Backend::Composition(Box::new(build_composition::resolve(request.config_path.clone(), loaded, &request)?))
    } else {
        Backend::Graph(Box::new(super::resolve_loaded(request, loaded)?))
    };
    Ok(ResolvedBuild { backend })
}

pub fn execute(resolved: ResolvedBuild, dry_run: bool) -> Result<CanonicalExecutionResult> {
    match resolved.backend {
        Backend::Graph(value) => super::execute(*value, dry_run),
        Backend::Composition(value) => build_composition::execute(*value, dry_run),
    }
}

pub fn cancelled(resolved: ResolvedBuild) -> Result<CanonicalExecutionResult> {
    match resolved.backend {
        Backend::Graph(value) => super::cancelled(*value),
        Backend::Composition(value) => build_composition::cancelled(*value),
    }
}
