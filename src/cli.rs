use clap::{Parser, Subcommand};

/// Spec-driven document rendering engine
#[derive(Parser)]
#[command(name = "renderflow", version, about)]
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
    /// Build the renderflow pipeline
    Build {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml")]
        config: String,
    },
}
