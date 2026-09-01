//! Renderflow library crate.
//!
//! Exposes the core subsystems for use by benchmarks, tests, and external
//! integrations.  The binary entrypoint lives in `main.rs`.

mod adapters;
pub mod ai;
pub mod app;
pub mod artifact;
mod assets;
mod audio;
pub mod cache;
pub mod cli;
mod commands;
mod compat;
mod config;
pub mod detect;
pub mod error;
pub mod graph;
mod image;
mod input_format;
pub mod optimization;
mod pipeline;
pub mod planning;
pub mod process;
mod sdk;
pub mod spec;
pub mod strategies;
pub mod super_resolution;
pub mod toolchain;
pub mod transforms;

pub use sdk::{
    ArtifactManifest, ArtifactProfile, CancellationToken, DiagnosticReport, Engine, EngineBuilder,
    ExecutionRequest, ExecutionResult, InspectionRequest, PlanRequest, ProgressEvent,
    ProgressReporter, ProgressStage, RenderflowError,
};
