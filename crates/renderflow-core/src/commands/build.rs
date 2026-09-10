use anyhow::Result;
use tracing::info;

use crate::optimization::OptimizationMode;
use crate::planning::{execute, resolve, PlanningRequest};

/// Run the canonical Renderflow execution lifecycle using the target intent
/// declared in the v1/v2 configuration.
pub fn run(config_path: &str, dry_run: bool, optimization: Option<OptimizationMode>) -> Result<()> {
    run_selection(config_path, dry_run, false, optimization, None, false)
}

/// Compatibility entrypoint for watch mode.
///
/// Watch mode itself owns resilience by keeping the watcher alive after an
/// execution error; individual builds still use the exact same canonical
/// planner/executor and fail atomically.
pub fn run_resilient(config_path: &str) -> Result<()> {
    run(config_path, false, None)
}

/// Run the canonical lifecycle with optional CLI target overrides.
pub(crate) fn run_selection(
    config_path: &str,
    dry_run: bool,
    resume: bool,
    optimization: Option<OptimizationMode>,
    target: Option<&str>,
    all_reachable: bool,
) -> Result<()> {
    if dry_run {
        info!(
            "Dry-run mode enabled — planning and bounded provider probes may run, but transforms and output writes are disabled"
        );
    }

    let mut request = PlanningRequest::from_path(config_path);
    if let Some(optimization) = optimization {
        request = request.with_optimization(optimization);
    }
    if let Some(target) = target {
        request = request.with_target(target);
    } else if all_reachable {
        request = request.with_all_reachable();
    }

    let resolved = resolve(request)?.with_resume(resume);
    info!(
        source = %resolved.source_format(),
        targets = %resolved
            .target_formats()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        depth = resolved.plan().metadata.execution_depth,
        waves = resolved.plan().metadata.execution_waves,
        "Resolved canonical execution plan"
    );

    let result = execute(resolved, dry_run)?;
    if dry_run {
        // stdout is reserved for machine-readable plan evidence; tracing remains on stderr.
        println!("{}", serde_json::to_string_pretty(&result.plan)?);
    }
    for output in &result.run_manifest.artifact_manifest.outputs {
        if dry_run {
            info!("[DRY RUN] Planned output: {}", output);
        } else {
            info!("✔ Output written to: {}", output);
        }
    }
    if let Some(manifest_path) = &result.manifest_path {
        info!(
            run_id = %result.run_manifest.run_id,
            state = ?result.run_manifest.state,
            "Run evidence written to: {}",
            manifest_path
        );
    }
    if !result.is_success() {
        anyhow::bail!(
            "renderflow execution finished with state {:?}; inspect '{}' for structured evidence",
            result.run_manifest.state,
            result
                .manifest_path
                .as_deref()
                .unwrap_or("renderflow-run.json")
        );
    }
    Ok(())
}
