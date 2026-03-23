use clap::{Parser, Subcommand};

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
            renderflow build --dry-run              Preview what would be built"
    )]
    Build {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Simulate execution: log intended actions without creating files or running commands
        #[arg(long)]
        dry_run: bool,
    },
}
