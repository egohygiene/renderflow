//! Renderflow library crate.
//!
//! Exposes the core subsystems for use by benchmarks, tests, and external
//! integrations.  The binary entrypoint lives in `main.rs`.

pub mod ai;
mod adapters;
mod assets;
mod audio;
pub mod cache;
mod compat;
mod config;
mod deps;
pub mod error;
mod files;
pub mod graph;
mod image;
mod incremental;
mod input_format;
pub mod optimization;
mod pipeline;
mod strategies;
mod template;
pub mod transforms;
