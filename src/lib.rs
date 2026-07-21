//! Renderflow library crate.
//!
//! Exposes the core subsystems for use by benchmarks, tests, and external
//! integrations.  The binary entrypoint lives in `main.rs`.

mod adapters;
pub mod ai;
pub mod app;
mod assets;
mod audio;
pub mod cache;
pub mod cli;
mod commands;
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
pub mod strategies;
mod template;
pub mod transforms;
