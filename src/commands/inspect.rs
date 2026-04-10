use std::fs;

use anyhow::{Context, Result};
use tracing::info;

use crate::config::load_config_for_graph;
use crate::graph::Format;
use crate::optimization::OptimizationMode;
use crate::transforms::yaml_loader::build_graph_and_executor_from_yaml;

/// Run the `inspect` subcommand: visualize the transformation DAG.
///
/// Supports two output formats:
/// * `"tree"` – human-readable CLI tree view (default)
/// * `"dot"`  – Graphviz DOT language, suitable for `dot -Tsvg`
///
/// When `export` is `Some(path)` the output is written to that file;
/// otherwise it is printed to stdout.
pub fn run(
    config_path: &str,
    output_format: &str,
    target: Option<&str>,
    all: bool,
    export: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    let config = load_config_for_graph(config_path)?;
    info!("Loaded config from '{}'", config_path);

    let transforms_path = config.transforms.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "DAG inspection requires a 'transforms' key in the config file \
             pointing to a YAML transform configuration"
        )
    })?;

    let (graph, _executor) = build_graph_and_executor_from_yaml(transforms_path)?;
    info!("Loaded transform graph from '{}'", transforms_path);

    let opt_mode = optimization.unwrap_or(config.optimization);

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

    // Determine targets.
    let targets: Vec<Format> = if let Some(t) = target {
        vec![t
            .parse::<Format>()
            .with_context(|| format!("'{}' is not a valid target format", t))?]
    } else if all {
        let reachable = graph.reachable_from(source_format);
        if reachable.is_empty() {
            anyhow::bail!(
                "No output formats are reachable from '{}' in the transform graph",
                source_format
            );
        }
        reachable
    } else {
        // Default: discover all reachable formats.
        let reachable = graph.reachable_from(source_format);
        if reachable.is_empty() {
            anyhow::bail!(
                "No output formats are reachable from '{}' in the transform graph",
                source_format
            );
        }
        reachable
    };

    let dag = graph
        .build_multi_target_dag_with_mode(source_format, &targets, opt_mode)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Could not build an execution plan: one or more target formats \
                 are not reachable from '{}' in the transform graph",
                source_format
            )
        })?;

    let output = match output_format.to_lowercase().as_str() {
        "dot" | "graphviz" => dag.to_dot(source_format),
        _ => dag.to_tree(source_format),
    };

    if let Some(path) = export {
        fs::write(path, &output)
            .with_context(|| format!("Failed to write DAG output to '{}'", path))?;
        info!("DAG visualization written to '{}'", path);
    } else {
        print!("{}", output);
    }

    Ok(())
}
