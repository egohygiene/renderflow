pub mod ai;
mod adapters;
mod assets;
mod audio;
mod cache;
mod cli;
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
mod optimization;
mod pipeline;
mod strategies;
mod template;
mod transforms;

use anyhow::{bail, Result};
use clap::Parser;
use cli::{AiCommands, Cli, Commands, PluginCommands};
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

    tracing_subscriber::fmt().with_max_level(log_level).init();

    match cli.command {
        Some(Commands::Build {
            config,
            dry_run,
            optimization,
            target,
            all,
        }) => {
            if let Some(ref target_format) = target {
                commands::graph_build::run_target(&config, target_format, dry_run, optimization)?
            } else if all {
                commands::graph_build::run_all(&config, dry_run, optimization)?
            } else {
                commands::build::run(&config, dry_run, optimization)?
            }
        }
        Some(Commands::Watch { config, debounce }) => commands::watch::run(&config, debounce)?,
        Some(Commands::Audit) => commands::audit::run()?,
        Some(Commands::Inspect {
            config,
            output_format,
            target,
            all,
            export,
        }) => {
            commands::inspect::run(
                &config,
                &output_format,
                target.as_deref(),
                all,
                export.as_deref(),
                None, // optimization: use the mode from the config file
            )?
        }
        Some(Commands::Plugin { subcommand }) => {
            // The plugin registry is empty at the top-level CLI entry point.
            // Third-party plugins are registered programmatically before
            // calling renderflow as a library.  The CLI commands are
            // primarily useful when renderflow is embedded in a larger
            // application that populates the registry before dispatching.
            let registry = transforms::plugin::PluginRegistry::new();
            match subcommand {
                PluginCommands::List => commands::plugin::run_list(&registry)?,
                PluginCommands::Info { name } => commands::plugin::run_info(&registry, &name)?,
                PluginCommands::Validate => commands::plugin::run_validate(&registry)?,
                PluginCommands::Doctor => commands::plugin::run_doctor(&registry)?,
            }
        }
        Some(Commands::Ai { subcommand }) => match subcommand {
            AiCommands::Providers => commands::ai::run_providers()?,
            AiCommands::Models => commands::ai::run_models()?,
            AiCommands::Doctor { ollama_endpoint } => {
                commands::ai::run_doctor(&ollama_endpoint)?
            }
            AiCommands::Cache { path } => commands::ai::run_cache(&path)?,
        },
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
