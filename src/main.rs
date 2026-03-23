mod adapters;
mod assets;
mod cli;
mod commands;
mod config;
mod files;
mod pipeline;
mod strategies;
mod template;
mod transforms;

use anyhow::{bail, Result};
use clap::Parser;
use cli::{Cli, Commands};
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Build { config }) => commands::build::run(&config)?,
        None => {
            info!("No subcommand provided, defaulting to build");
            match cli.input {
                Some(ref input) => commands::build::run(input)?,
                None => bail!("No input provided. Usage: renderflow <config> or renderflow build --config <config>"),
            }
        }
    }

    Ok(())
}
