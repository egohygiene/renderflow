use clap::{Parser, Subcommand};

/// Spec-driven document rendering engine
#[derive(Parser)]
#[command(name = "renderflow", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
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
