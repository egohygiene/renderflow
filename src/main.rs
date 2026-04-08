mod adapters;
mod assets;
mod cache;
mod cli;
mod commands;
mod compat;
mod config;
mod deps;
pub mod error;
mod files;
pub mod graph;
mod input_format;
mod optimization;
mod pipeline;
mod strategies;
mod template;
mod transforms;

use anyhow::{bail, Result};
use clap::Parser;
use cli::{Cli, Commands};
use tracing::info;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.debug {
        tracing::Level::TRACE
    } else if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .init();

    match cli.command {
        Some(Commands::Build { config, dry_run, optimization }) => {
            commands::build::run(&config, dry_run, optimization)?
        }
        Some(Commands::Watch { config, debounce }) => commands::watch::run(&config, debounce)?,
        Some(Commands::Audit) => commands::audit::run()?,
        None => {
            info!("No subcommand provided, defaulting to build");
            match cli.input {
                Some(ref input) => commands::build::run(input, false, None)?,
                None => bail!("No input provided. Usage: renderflow <config> or renderflow build --config <config>"),
            }
        }
    }

    Ok(())
}
