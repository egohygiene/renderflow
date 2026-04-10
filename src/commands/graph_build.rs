use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::config::load_config_for_graph;
use crate::files::ensure_output_dir;
use crate::graph::Format;
use crate::optimization::OptimizationMode;
use crate::transforms::yaml_loader::build_graph_and_executor_from_yaml;

/// Run graph-based execution targeting a single output format.
///
/// The transform graph is resolved automatically from the `transforms` YAML
/// file referenced in `config_path`.  The shortest path (according to
/// `optimization`) from the detected source format to `target` is found,
/// and every intermediate and final format is produced.
///
/// # Errors
///
/// Returns an error when:
/// * the config file cannot be read or parsed,
/// * no `transforms` key is present in the config,
/// * `target` is not a recognised format,
/// * `target` is not reachable from the source format,
/// * any transform in the execution plan fails.
pub fn run_target(
    config_path: &str,
    target: &str,
    dry_run: bool,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    let target_format = target
        .parse::<Format>()
        .with_context(|| format!("'{}' is not a valid target format", target))?;

    run_impl(config_path, Some(vec![target_format]), dry_run, optimization)
}

/// Run graph-based execution targeting all formats reachable from the source.
///
/// The transform graph is resolved automatically from the `transforms` YAML
/// file referenced in `config_path`.  Every format reachable from the source
/// format is produced in dependency order.
///
/// # Errors
///
/// Returns an error when:
/// * the config file cannot be read or parsed,
/// * no `transforms` key is present in the config,
/// * no output formats are reachable from the source format,
/// * any transform in the execution plan fails.
pub fn run_all(
    config_path: &str,
    dry_run: bool,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    run_impl(config_path, None, dry_run, optimization)
}

/// Shared implementation for `run_target` and `run_all`.
///
/// `explicit_targets` is `Some(vec)` for `--target` mode and `None` for
/// `--all` mode (targets are discovered dynamically from the graph).
fn run_impl(
    config_path: &str,
    explicit_targets: Option<Vec<Format>>,
    dry_run: bool,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    if dry_run {
        info!("Dry-run mode enabled — no files will be created and no commands will be executed");
    }
    info!("Running graph-based build pipeline");

    let config = load_config_for_graph(config_path)?;
    info!("Loaded config successfully");

    let transforms_path = config.transforms.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "Graph-based execution requires a 'transforms' key in the config file \
             pointing to a YAML transform configuration"
        )
    })?;

    let (graph, executor) = build_graph_and_executor_from_yaml(transforms_path)?;
    info!("Loaded transform graph from '{}'", transforms_path);

    let opt_mode = optimization.unwrap_or(config.optimization);
    info!(optimization = %opt_mode, "Using optimization mode");

    // Derive the source format from the config's input field.
    let source_format: Format = config
        .input_format()
        .to_string()
        .parse()
        .with_context(|| {
            format!(
                "Could not map input format '{}' to a known graph format",
                config.input_format()
            )
        })?;

    // Determine which formats to build.
    let targets: Vec<Format> = match explicit_targets {
        Some(t) => t,
        None => {
            // --all: discover every format reachable from the source.
            let reachable = graph.reachable_from(source_format);
            if reachable.is_empty() {
                anyhow::bail!(
                    "No output formats are reachable from '{}' in the transform graph",
                    source_format
                );
            }
            info!(
                "Discovered {} reachable output format(s): {}",
                reachable.len(),
                reachable
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            reachable
        }
    };

    // Build the minimal DAG that covers all targets.
    let dag = graph
        .build_multi_target_dag_with_mode(source_format, &targets, opt_mode)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Could not build an execution plan: one or more target formats \
                 are not reachable from '{}' in the transform graph",
                source_format
            )
        })?;

    let input_stem = Path::new(&config.input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");

    let output_dir = if dry_run {
        let path = std::path::PathBuf::from(&config.output_dir);
        info!(
            "[DRY RUN] Would create output directory: {}",
            path.display()
        );
        for target in &targets {
            let output_path =
                format!("{}/{}.{}", path.display(), input_stem, target);
            info!("[DRY RUN] Would write '{}' output to: {}", target, output_path);
        }
        return Ok(());
    } else {
        ensure_output_dir(&config.output_dir)?
    };

    // Read and execute.
    let content = fs::read_to_string(&config.input)
        .with_context(|| format!("Failed to read input file: {}", config.input))?;

    info!("Executing graph-based pipeline");
    let results = executor
        .execute(&dag, source_format, content)
        .context("Graph execution failed")?;

    // Write each produced format to disk (skip the source format).
    for (format, output_content) in &results {
        if *format == source_format {
            continue;
        }
        let output_path = format!("{}/{}.{}", output_dir.display(), input_stem, format);
        fs::write(&output_path, output_content)
            .with_context(|| format!("Failed to write output to '{}'", output_path))?;
        info!("✔ Output written to: {}", output_path);
    }

    Ok(())
}
