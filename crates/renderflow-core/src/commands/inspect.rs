use std::fs;

use anyhow::{Context, Result};
use tracing::info;

use crate::artifact::ArtifactStore;
use crate::optimization::OptimizationMode;
use crate::planning::build::resolve;
use crate::planning::PlanningRequest;
use crate::{IntakeBudgets, IntakeEngine, IntakeRequest};

/// Inspect the same resolved build used by execution.
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
    let output = if resolved.is_composition() {
        let format = match output_format.to_lowercase().as_str() {
            "dot" | "graphviz" => "dot",
            _ => "text",
        };
        crate::graph::renderers::renderer_for(format)
            .context("missing built-in plan renderer")?.render(resolved.plan())
    } else {
        let graph = resolved.graph()?;
        match output_format.to_lowercase().as_str() {
            "dot" | "graphviz" => graph.dag().to_dot(graph.source_format()),
            _ => graph.dag().to_tree(graph.source_format()),
        }
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

/// Inspect arbitrary bytes through the stable universal-intake contract.
pub fn run_artifact(
    input: &str,
    media_type: Option<&str>,
    extract: bool,
    recursive: bool,
    store_root: &str,
    budgets: IntakeBudgets,
    export: Option<&str>,
) -> Result<()> {
    let store = ArtifactStore::new(store_root)?;
    let mut request = IntakeRequest::from_path(input).with_budgets(budgets);
    if let Some(media_type) = media_type {
        request = request.with_media_type(media_type);
    }
    if extract {
        request = request.with_extraction(recursive);
    }
    let report = IntakeEngine::new().intake(&request, &store)?;
    let output = format!("{}\n", serde_json::to_string_pretty(&report)?);
    if let Some(path) = export {
        fs::write(path, &output)
            .with_context(|| format!("Failed to write intake report to '{path}'"))?;
        info!("Artifact inspection written to '{path}'");
    } else {
        print!("{output}");
    }
    Ok(())
}
