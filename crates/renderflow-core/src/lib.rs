//! Renderflow library crate.
//!
//! Exposes the core subsystems for use by benchmarks, tests, and external
//! integrations.  The binary entrypoint lives in `main.rs`.

#![recursion_limit = "256"]

pub mod adapters;
pub mod ai;
pub mod app;
pub mod artifact;
mod assets;
mod audio;
pub mod cache;
pub mod checkpoint;
pub mod cli;
mod commands;
mod compat;
mod config;
pub mod detect;
pub mod ebook;
pub mod error;
pub mod evidence;
pub mod graph;
pub mod hygiene;
mod image;
mod input_format;
pub mod intake;
pub mod optimization;
mod pipeline;
pub mod planning;
pub mod process;
pub mod publication;
mod sdk;
pub mod spec;
pub mod strategies;
pub mod super_resolution;
pub mod toolchain;
pub mod transforms;
pub mod validation;

pub use evidence::{ArtifactManifest, RunManifest};
pub use hygiene::{
    ContentRedactionProvider, HygieneEngine, HygieneEvidence, HygieneFinding, HygieneFindingKind,
    HygieneOutcome, HygieneStatus, RedactionEvidence, RedactionRequest, RedactionResult,
    HYGIENE_EVIDENCE_SCHEMA_V1,
};
pub use intake::{
    ArtifactIntakeProvider, DetectionConfidence, DiscoveredArtifact, EvidenceOrigin,
    InspectionContext, IntakeBudgetUsage, IntakeBudgets, IntakeConflict, IntakeDiagnostic,
    IntakeDiagnosticSeverity, IntakeEngine, IntakeReport, IntakeRequest, IntakeSignal,
    IntakeSignalKind, ProvenanceValue, ProviderInspection, ResolvedArtifactProfile,
    INTAKE_SCHEMA_V1,
};
pub use sdk::{
    ArtifactProfile, CancellationToken, DiagnosticReport, Engine, EngineBuilder, ExecutionRequest,
    ExecutionResult, InspectionRequest, PlanRequest, ProgressEvent, ProgressReporter,
    ProgressStage, ProviderCapabilities, ProviderPlan, RenderflowError, RenderflowProvider,
    SavedRunAssessment,
};
