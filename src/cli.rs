use clap::{Parser, Subcommand};

use crate::optimization::OptimizationMode;

/// Spec-driven document rendering engine
#[derive(Parser)]
#[command(
    name = "renderflow",
    version,
    about = "Spec-driven document rendering engine",
    long_about = "renderflow — Spec-driven document rendering engine\n\n\
        Transform structured YAML configurations into rendered documents\n\
        (PDF, HTML, LaTeX) using Pandoc, Tectonic, and Jinja2 templates.",
    after_help = "Examples:\n  \
        renderflow build                        Build using renderflow.yaml\n  \
        renderflow build --config custom.yaml   Build with a custom config file\n  \
        renderflow build --dry-run              Preview what would be built\n  \
        renderflow watch                        Watch using renderflow.yaml\n  \
        renderflow watch --config custom.yaml   Watch with a custom config file\n  \
        renderflow audit                        Generate an optimization audit report\n  \
        renderflow my-project.yaml              Shorthand: run build on the given config"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to the renderflow configuration file (used when no subcommand is provided)
    pub input: Option<String>,

    /// Enable verbose logging (DEBUG level)
    #[arg(long, global = true)]
    pub verbose: bool,

    /// Enable debug logging (TRACE level); takes precedence over --verbose
    #[arg(long, global = true)]
    pub debug: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build rendered documents from a renderflow configuration file
    #[command(
        after_help = "Examples:\n  \
            renderflow build                        Build using renderflow.yaml\n  \
            renderflow build --config custom.yaml   Build with a custom config file\n  \
            renderflow build --dry-run              Preview what would be built\n  \
            renderflow build --optimization speed   Build using speed optimization mode\n  \
            renderflow build --optimization pareto  Build with Pareto-optimal path selection\n  \
            renderflow build --target pdf           Build only the PDF output via graph resolution\n  \
            renderflow build --all                  Build all reachable outputs via graph resolution"
    )]
    Build {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Simulate execution: log intended actions without creating files or running commands
        #[arg(long)]
        dry_run: bool,

        /// Optimization mode: controls how transformation paths are selected.
        /// Overrides the value set in the config file when provided.
        /// Choices: speed (minimise cost), quality (maximise quality), balanced (default),
        /// pareto (return Pareto-optimal frontier of non-dominated paths).
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,

        /// Build only the specified output format using graph-based path resolution.
        /// The format must be reachable from the input format via the configured transforms.
        /// Requires a 'transforms' key in the config file.
        /// Cannot be combined with --all.
        #[arg(long, value_name = "FORMAT", conflicts_with = "all")]
        target: Option<String>,

        /// Build all reachable output formats using graph-based path resolution.
        /// Requires a 'transforms' key in the config file.
        /// Cannot be combined with --target.
        #[arg(long, conflicts_with = "target")]
        all: bool,
    },

    /// Watch for file changes and automatically rebuild
    #[command(
        after_help = "Examples:\n  \
            renderflow watch                                    Watch using renderflow.yaml\n  \
            renderflow watch --config custom.yaml               Watch with a custom config file\n  \
            renderflow watch --config custom.yaml --debounce 300   Watch with a 300 ms debounce delay"
    )]
    Watch {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Debounce delay in milliseconds: wait this long after the last change before rebuilding
        #[arg(long, default_value = "500", value_name = "MS")]
        debounce: u64,
    },

    /// Generate an optimization audit report covering performance, memory, and Rust best practices
    #[command(
        after_help = "Examples:\n  \
            renderflow audit   Generate an audit report in the audits/ directory"
    )]
    Audit,
}
