use std::fs;

use anyhow::{Context, Result};
use tracing::info;

use crate::config::load_config_for_graph;
use crate::graph::execution_plan::ExecutionPlan;
use crate::graph::renderers::renderer_for;
use crate::graph::{Format, MultiTargetDag};
use crate::optimization::OptimizationMode;
use crate::transforms::yaml_loader::build_graph_and_executor_from_yaml;

// ── helpers ─────────────────────────────────────────────────────────────────

/// Load the config, build the transform graph, compute the DAG, and return an
/// [`ExecutionPlan`].
fn load_plan(
    config_path: &str,
    target: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<(ExecutionPlan, Vec<Format>)> {
    let config = load_config_for_graph(config_path)?;
    info!("Loaded config from '{}'", config_path);

    let transforms_path = config.transforms.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "This subcommand requires a 'transforms' key in the config file \
             pointing to a YAML transform configuration"
        )
    })?;

    let (graph, _executor) = build_graph_and_executor_from_yaml(transforms_path)?;
    info!("Loaded transform graph from '{}'", transforms_path);

    let opt_mode = optimization.unwrap_or(config.optimization);

    let source_format: Format = config.input_format().to_string().parse().with_context(|| {
        format!(
            "Could not map input format '{}' to a known graph format",
            config.input_format()
        )
    })?;

    let targets: Vec<Format> = if let Some(t) = target {
        vec![t
            .parse::<Format>()
            .with_context(|| format!("'{}' is not a valid target format", t))?]
    } else {
        let reachable = graph.reachable_from(source_format);
        if reachable.is_empty() {
            anyhow::bail!(
                "No output formats are reachable from '{}' in the transform graph",
                source_format
            );
        }
        reachable
    };

    let dag: MultiTargetDag = graph
        .build_multi_target_dag_with_mode(source_format, &targets, opt_mode)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Could not build an execution plan: one or more target formats \
                 are not reachable from '{}' in the transform graph",
                source_format
            )
        })?;

    let plan = ExecutionPlan::from_dag(&dag, source_format, &targets, opt_mode);
    Ok((plan, targets))
}

/// Emit `output` to `export` path (if provided) or to stdout.
fn emit(output: &str, export: Option<&str>) -> Result<()> {
    if let Some(path) = export {
        fs::write(path, output)
            .with_context(|| format!("Failed to write output to '{}'", path))?;
        info!("Output written to '{}'", path);
    } else {
        print!("{}", output);
    }
    Ok(())
}

// ── plan ─────────────────────────────────────────────────────────────────────

/// `renderflow graph plan` – display the canonical execution plan.
///
/// Supported `format` values (case-insensitive):
/// `text` (default), `dot`, `mermaid`, `json`, `yaml`, `markdown`.
pub fn run_plan(
    config_path: &str,
    format: &str,
    target: Option<&str>,
    export: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    let (plan, _) = load_plan(config_path, target, optimization)?;

    let renderer = renderer_for(format)
        .ok_or_else(|| anyhow::anyhow!("Unknown output format '{}'. Supported: text, dot, mermaid, json, yaml, markdown", format))?;

    let output = renderer.render(&plan);
    emit(&output, export)
}

// ── render ───────────────────────────────────────────────────────────────────

/// `renderflow graph render` – render the graph in a specific visual format.
///
/// Alias for `plan` with a default of `mermaid`.
pub fn run_render(
    config_path: &str,
    format: &str,
    target: Option<&str>,
    export: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    run_plan(config_path, format, target, export, optimization)
}

// ── explain ──────────────────────────────────────────────────────────────────

/// `renderflow graph explain` – print planning diagnostics.
///
/// Explains why a particular path was selected, lists warnings for lossy
/// transforms, and describes optimization trade-offs.
pub fn run_explain(
    config_path: &str,
    target: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    let (plan, _) = load_plan(config_path, target, optimization)?;

    println!("Planning Diagnostics");
    println!("====================");
    println!("Source:       {}", plan.source);
    println!("Targets:      {}", plan.targets.join(", "));
    println!("Optimization: {}", plan.optimization);
    println!();

    if plan.diagnostics.is_empty() {
        println!("  No diagnostics — the plan is clean.");
    } else {
        for diag in &plan.diagnostics {
            let level_tag = format!("[{:?}]", diag.level).to_lowercase();
            println!("  {} {}", level_tag, diag.message);
        }
    }

    Ok(())
}

// ── export ───────────────────────────────────────────────────────────────────

/// `renderflow graph export` – export the execution plan to a file.
///
/// `format` determines the serialization format; `output` is the file path.
pub fn run_export(
    config_path: &str,
    format: &str,
    output_path: &str,
    target: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    run_plan(config_path, format, target, Some(output_path), optimization)
}

// ── doctor ───────────────────────────────────────────────────────────────────

/// `renderflow graph doctor` – diagnose the execution plan for issues.
///
/// Prints all planning diagnostics and exits with a non-zero status when any
/// error-level diagnostics are present.
pub fn run_doctor(
    config_path: &str,
    target: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    use crate::graph::execution_plan::DiagnosticLevel;

    let (plan, _) = load_plan(config_path, target, optimization)?;

    let mut has_errors = false;

    println!("Graph Doctor");
    println!("============");
    println!();

    if plan.diagnostics.is_empty() {
        println!("✓ No issues found.");
    } else {
        for diag in &plan.diagnostics {
            let prefix = match diag.level {
                DiagnosticLevel::Info => "  ℹ",
                DiagnosticLevel::Warning => "  ⚠",
                DiagnosticLevel::Error => {
                    has_errors = true;
                    "  ✗"
                }
            };
            println!("{} {}", prefix, diag.message);
        }
    }

    if has_errors {
        anyhow::bail!("Graph doctor found error-level diagnostics.");
    }

    println!();
    println!("✓ Graph doctor passed.");
    Ok(())
}

// ── stats ────────────────────────────────────────────────────────────────────

/// `renderflow graph stats` – print graph statistics.
pub fn run_stats(
    config_path: &str,
    target: Option<&str>,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    let (plan, _) = load_plan(config_path, target, optimization)?;

    println!("Graph Statistics");
    println!("================");
    println!("  nodes:                {}", plan.metadata.total_nodes);
    println!("  edges:                {}", plan.metadata.total_edges);
    println!("  depth:                {}", plan.metadata.execution_depth);
    println!("  waves:                {}", plan.metadata.execution_waves);
    println!("  estimated cost:       {:.2}", plan.metadata.estimated_cost);
    println!("  estimated quality:    {:.2}", plan.metadata.estimated_quality);
    println!("  reused intermediates: {}", plan.metadata.reused_intermediates);
    println!("  output count:         {}", plan.metadata.output_count);

    Ok(())
}
