use std::fs;

use anyhow::{Context, Result};
use tracing::info;

use crate::optimization::OptimizationMode;
use crate::planning::{resolve, PlanningRequest};

/// Run the `inspect` subcommand against the same resolved DAG used by execution.
pub fn run(
    config_path: &str,
    output_format: &str,
    target: Option<&str>,
    all: bool,
    export: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    let mut request = PlanningRequest::from_path(config_path);
    if let Some(optimization) = optimization {
        request = request.with_optimization(optimization);
    }
    if let Some(target) = target {
        request = request.with_target(target);
    } else if all {
        request = request.with_all_reachable();
    }

    let resolved = resolve(request)?;
    let output = match output_format.to_lowercase().as_str() {
        "dot" | "graphviz" => resolved.dag().to_dot(resolved.source_format()),
        _ => resolved.dag().to_tree(resolved.source_format()),
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
