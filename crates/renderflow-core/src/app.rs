use anyhow::{bail, Result};
use clap::Parser;
use tracing::info;

use crate::cli::{
    AiCommands, AiSkillCommands, Cli, Commands, DnaCommands, EbookCommands, FontCommands,
    GraphCommands, LuluCommands, PluginCommands, PublicationCommands, SpecCommands, ToolCommands,
    VideoCommands,
};
use crate::video::HandBrakeLimits;
use crate::{commands, transforms};

/// Initialize logging for a Renderflow CLI run.
pub fn init_logging(cli: &Cli) {
    let log_level = if cli.debug {
        tracing::Level::TRACE
    } else if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_writer(std::io::stderr)
        .init();
}

/// Dispatch a parsed CLI command.
pub fn run_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Commands::Build {
            config,
            dry_run,
            resume,
            optimization,
            target,
            profile,
            exclude,
            all,
        }) => commands::build::run_selection(
            &config,
            dry_run,
            resume,
            optimization,
            target.as_deref(),
            profile.as_deref(),
            &exclude,
            all,
        )?,
        Some(Commands::Watch { config, debounce }) => commands::watch::run(&config, debounce)?,
        Some(Commands::Audit) => commands::audit::run()?,
        Some(Commands::Inspect {
            input,
            config,
            output_format,
            target,
            all,
            export,
            media_type,
            extract,
            recursive,
            store,
            max_depth,
            max_artifacts,
            max_extracted_bytes,
            max_expansion_ratio,
        }) => {
            if let Some(input) = input {
                commands::inspect::run_artifact(
                    &input,
                    media_type.as_deref(),
                    extract,
                    recursive,
                    &store,
                    crate::intake::IntakeBudgets {
                        max_depth,
                        max_artifacts,
                        max_extracted_bytes,
                        max_expansion_ratio,
                    },
                    export.as_deref(),
                )?;
            } else {
                commands::inspect::run(
                    &config,
                    &output_format,
                    target.as_deref(),
                    all,
                    export.as_deref(),
                    None,
                )?;
            }
        }
        Some(Commands::Plugin { subcommand }) => {
            let registry = transforms::plugin::PluginRegistry::new();
            match subcommand {
                PluginCommands::List => commands::plugin::run_list(&registry)?,
                PluginCommands::Info { name } => commands::plugin::run_info(&registry, &name)?,
                PluginCommands::Validate => commands::plugin::run_validate(&registry)?,
                PluginCommands::Doctor => commands::plugin::run_doctor(&registry)?,
            }
        }
        Some(Commands::Ai { subcommand }) => match subcommand {
            AiCommands::Matrix { format, catalog } => {
                commands::ai::run_matrix(&format, catalog.as_deref())?
            }
            AiCommands::Resolve {
                skill,
                skill_version,
                execution_preference,
                allow_remote,
                allow_unverified,
                format,
                catalog,
            } => commands::ai::run_resolve(
                &skill,
                skill_version.as_deref(),
                &execution_preference,
                allow_remote,
                allow_unverified,
                &format,
                catalog.as_deref(),
            )?,
            AiCommands::Skills { subcommand } => match subcommand {
                AiSkillCommands::List { format } => commands::ai::run_skills_list(&format)?,
                AiSkillCommands::Inspect {
                    id,
                    version,
                    format,
                } => commands::ai::run_skills_inspect(&id, version.as_deref(), &format)?,
                AiSkillCommands::Validate { path, format } => {
                    commands::ai::run_skills_validate(path.as_deref(), &format)?
                }
            },
            AiCommands::Providers => commands::ai::run_providers()?,
            AiCommands::Models => commands::ai::run_models()?,
            AiCommands::Doctor { ollama_endpoint } => commands::ai::run_doctor(&ollama_endpoint)?,
            AiCommands::Cache { path } => commands::ai::run_cache(&path)?,
        },
        Some(Commands::Dna { subcommand }) => match subcommand {
            DnaCommands::Extract {
                input,
                output,
                store,
                media_type,
                format,
                max_source_bytes,
                max_observations,
                allow_ai,
                allow_network,
                allow_remote,
                protected_reference,
            } => commands::dna::run_extract(
                &input,
                output.as_deref(),
                &store,
                media_type.as_deref(),
                &format,
                max_source_bytes,
                max_observations,
                allow_ai,
                allow_network,
                allow_remote,
                &protected_reference,
            )?,
            DnaCommands::Validate { input, format } => {
                commands::dna::run_validate(&input, &format)?
            }
            DnaCommands::Compare {
                left,
                right,
                output,
                format,
            } => commands::dna::run_compare(&left, &right, output.as_deref(), &format)?,
        },
        Some(Commands::Font { subcommand }) => match subcommand {
            FontCommands::Validate { registry, format } => {
                commands::font::run_validate(&registry, &format)?
            }
            FontCommands::Resolve {
                registry,
                target,
                output,
                format,
            } => commands::font::run_resolve(&registry, &target, output.as_deref(), &format)?,
            FontCommands::Css { registry, output } => {
                commands::font::run_css(&registry, output.as_deref())?
            }
        },
        Some(Commands::Graph { subcommand }) => match subcommand {
            GraphCommands::Plan {
                config,
                format,
                target,
                profile,
                exclude,
                export,
                optimization,
            } => commands::graph::run_plan(
                &config,
                &format,
                target.as_deref(),
                profile.as_deref(),
                &exclude,
                export.as_deref(),
                optimization,
            )?,
            GraphCommands::Render {
                config,
                format,
                target,
                export,
                optimization,
            } => commands::graph::run_render(
                &config,
                &format,
                target.as_deref(),
                export.as_deref(),
                optimization,
            )?,
            GraphCommands::Explain {
                config,
                target,
                optimization,
            } => commands::graph::run_explain(&config, target.as_deref(), optimization)?,
            GraphCommands::Export {
                config,
                format,
                output,
                target,
                optimization,
            } => commands::graph::run_export(
                &config,
                &format,
                &output,
                target.as_deref(),
                optimization,
            )?,
            GraphCommands::Doctor {
                config,
                target,
                optimization,
            } => commands::graph::run_doctor(&config, target.as_deref(), optimization)?,
            GraphCommands::Stats {
                config,
                target,
                optimization,
            } => commands::graph::run_stats(&config, target.as_deref(), optimization)?,
        },
        Some(Commands::Tools { subcommand }) => match subcommand {
            ToolCommands::Ecosystem {
                format,
                capability,
                preferred,
                available_only,
            } => commands::tools::run_ecosystem(
                &format,
                capability.as_deref(),
                &preferred,
                available_only,
            )?,
            ToolCommands::List { format, transforms } => {
                commands::tools::run_list(transforms.as_deref(), &format)?
            }
            ToolCommands::Inspect {
                id,
                format,
                transforms,
            } => commands::tools::run_inspect(&id, transforms.as_deref(), &format)?,
            ToolCommands::Variants {
                id,
                models_dir,
                format,
            } => commands::tools::run_variants(&id, models_dir.as_deref(), &format)?,
        },
        Some(Commands::Ebook { subcommand }) => match subcommand {
            EbookCommands::Inspect {
                input,
                format,
                epubcheck,
            } => commands::ebook::run_inspect(&input, &format, epubcheck)?,
            EbookCommands::Capabilities { format } => commands::ebook::run_capabilities(&format)?,
        },
        Some(Commands::Publication { subcommand }) => match subcommand {
            PublicationCommands::ColoringBookPreflight {
                contract,
                output,
                format,
                allow_remote,
            } => commands::publication::run_coloring_book_preflight(
                &contract,
                output.as_deref(),
                &format,
                allow_remote,
            )?,
            PublicationCommands::MagazineCandidates {
                config,
                asset_role,
                output,
                format,
                ai,
                ai_catalog,
                ai_preference,
                allow_remote,
                allow_unverified,
                source_approved_for_ai,
                privacy_approved_for_remote,
                openai_endpoint,
                openai_api_key_env,
            } => commands::publication::run_magazine_candidates(
                &config,
                &asset_role,
                output.as_deref(),
                &format,
                ai,
                ai_catalog.as_deref(),
                &ai_preference,
                allow_remote,
                allow_unverified,
                source_approved_for_ai,
                privacy_approved_for_remote,
                openai_endpoint.as_deref(),
                &openai_api_key_env,
            )?,
            PublicationCommands::Lulu { subcommand } => match subcommand {
                LuluCommands::Rules { format, output } => {
                    commands::publication::run_lulu_rules(&format, output.as_deref())?
                }
                LuluCommands::Preflight {
                    request,
                    format,
                    output,
                    epubcheck,
                } => commands::publication::run_lulu_preflight(
                    &request,
                    &format,
                    output.as_deref(),
                    epubcheck,
                )?,
            },
        },
        Some(Commands::Video { subcommand }) => match subcommand {
            VideoCommands::Capabilities { format } => commands::video::run_capabilities(&format)?,
            VideoCommands::Plan {
                input,
                output,
                preset,
                timeout_seconds,
                capture_limit_bytes,
                progress_interval_ms,
                maximum_output_bytes,
                format,
            } => commands::video::run_plan(
                &input,
                &output,
                preset.into(),
                HandBrakeLimits {
                    timeout_seconds,
                    capture_limit_bytes,
                    progress_interval_ms,
                    maximum_output_bytes,
                },
                &format,
            )?,
            VideoCommands::Transcode {
                input,
                output,
                preset,
                timeout_seconds,
                capture_limit_bytes,
                progress_interval_ms,
                maximum_output_bytes,
                format,
            } => commands::video::run_transcode(
                &input,
                &output,
                preset.into(),
                HandBrakeLimits {
                    timeout_seconds,
                    capture_limit_bytes,
                    progress_interval_ms,
                    maximum_output_bytes,
                },
                &format,
            )?,
        },
        Some(Commands::Capabilities {
            format,
            transforms,
            matrix,
        }) => commands::tools::run_capabilities(transforms.as_deref(), &format, matrix)?,
        Some(Commands::Spec { subcommand }) => match subcommand {
            SpecCommands::Validate { config, format } => {
                commands::spec::run_validate(&config, &format)?
            }
            SpecCommands::Migrate { config, output } => {
                commands::spec::run_migrate(&config, output.as_deref())?
            }
            SpecCommands::Schema { format, output } => {
                commands::spec::run_schema(&format, output.as_deref())?
            }
        },
        Some(Commands::Version) => commands::system::run_version(),
        Some(Commands::Env) => commands::system::run_env(),
        Some(Commands::Doctor { strict }) => commands::system::run_doctor(strict)?,
        None => {
            info!("No subcommand provided, defaulting to build");
            match cli.input {
                Some(ref input) => commands::build::run(input, false, None)?,
                None => bail!(
                    "No input provided. Usage: renderflow <config> or renderflow build --config <config>"
                ),
            }
        }
    }

    Ok(())
}

/// Parse CLI arguments from the process environment, initialize logging,
/// and run the command dispatch.
pub fn run_cli_from_env() -> Result<()> {
    let cli = Cli::parse();
    init_logging(&cli);
    run_cli(cli)
}

#[cfg(test)]
mod tests {
    use super::run_cli;
    use crate::cli::{Cli, Commands};

    #[test]
    fn run_cli_supports_version_subcommand() {
        let cli = Cli {
            command: Some(Commands::Version),
            input: None,
            verbose: false,
            debug: false,
        };
        assert!(run_cli(cli).is_ok());
    }
}
