use std::{fs, path::Path};

use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass, ArtifactStore};
use crate::config::load_config_for_graph;
use crate::files::ensure_output_dir;
use crate::graph::Format;
use crate::optimization::OptimizationMode;
use crate::toolchain::filter_graph_for_current_toolchain;
use crate::transforms::yaml_loader::build_graph_executor_and_tools_from_yaml;

/// Run graph-based execution targeting a single output format.
pub fn run_target(
    config_path: &str,
    target: &str,
    dry_run: bool,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    let target_format = target
        .parse::<Format>()
        .with_context(|| format!("'{}' is not a valid target format", target))?;

    run_impl(
        config_path,
        Some(vec![target_format]),
        dry_run,
        optimization,
    )
}

/// Run graph-based execution targeting all formats reachable from the source.
pub fn run_all(
    config_path: &str,
    dry_run: bool,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    run_impl(config_path, None, dry_run, optimization)
}

/// Shared implementation for `run_target` and `run_all`.
fn run_impl(
    config_path: &str,
    explicit_targets: Option<Vec<Format>>,
    dry_run: bool,
    optimization: Option<OptimizationMode>,
) -> Result<()> {
    if dry_run {
        info!("Dry-run mode enabled — no files or transform commands will be produced; bounded tool probes may run for planning");
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

    let (raw_graph, executor, tool_registry) =
        build_graph_executor_and_tools_from_yaml(transforms_path)?;
    let (graph, tool_inventory, tool_context) =
        filter_graph_for_current_toolchain(&raw_graph, &tool_registry);
    info!(
        "Loaded tool-aware transform graph from '{}'",
        transforms_path
    );

    let opt_mode = optimization.unwrap_or(config.optimization);
    info!(optimization = %opt_mode, "Using optimization mode");

    let source_format: Format = config.input_format().to_string().parse().with_context(|| {
        format!(
            "Could not map input format '{}' to a known graph format",
            config.input_format()
        )
    })?;

    let targets: Vec<Format> = match explicit_targets {
        Some(targets) => targets,
        None => {
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
                    .map(|format| format.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            reachable
        }
    };

    let dag = graph
        .build_multi_target_dag_with_mode(source_format, &targets, opt_mode)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Could not build an execution plan: one or more target formats \
                 are not reachable from '{}' after provider availability filtering. Blocked providers: {}",
                source_format,
                tool_inventory.blocked_summaries().join("; ")
            )
        })?;

    let toolchain = tool_registry.fingerprint_for_dag(&tool_inventory, &dag, &tool_context)?;
    info!(fingerprint = %toolchain.fingerprint, providers = toolchain.selected_tools.len(), "Resolved execution toolchain");

    debug!("Execution plan (DAG tree):\n{}", dag.to_tree(source_format));

    let input_stem = Path::new(&config.input)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("document");

    let output_dir = if dry_run {
        let path = std::path::PathBuf::from(&config.output_dir);
        info!(
            "[DRY RUN] Would create output directory: {}",
            path.display()
        );
        for target in &targets {
            let output_path = path.join(format!("{}.{}", input_stem, target));
            info!(
                "[DRY RUN] Would write '{}' output to: {}",
                target,
                output_path.display()
            );
        }
        return Ok(());
    } else {
        ensure_output_dir(&config.output_dir)?
    };

    // Keep intermediate/cache state outside the final output directory itself.
    let state_parent = output_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let state_dir = state_parent.join(".renderflow");
    let artifact_store = ArtifactStore::new(state_dir.join("artifacts"))?;
    fs::create_dir_all(&state_dir)?;
    fs::write(
        state_dir.join("toolchain.json"),
        serde_json::to_vec_pretty(&toolchain)?,
    )?;
    let executor = executor
        .with_cache(state_dir.join("dag-cache.json"))
        .with_toolchain_fingerprint(toolchain.fingerprint.clone());

    let source_artifact = artifact_store.import_path(
        &config.input,
        ArtifactDescriptor::for_format(source_format, ArtifactStorageClass::Source),
    )?;

    info!(
        artifact = %source_artifact.id(),
        digest = %source_artifact.digest(),
        bytes = source_artifact.size_bytes(),
        "Executing graph-based pipeline from binary-safe source artifact"
    );
    let results = executor
        .execute_artifact(&dag, source_format, source_artifact, &artifact_store)
        .context("Graph execution failed")?;

    for (format, artifact) in &results {
        if *format == source_format {
            continue;
        }
        let output_path = output_dir.join(format!("{}.{}", input_stem, format));
        let terminal_artifact = artifact
            .clone()
            .with_storage_class(ArtifactStorageClass::Terminal);
        artifact_store
            .materialize(&terminal_artifact, &output_path)
            .with_context(|| {
                format!(
                    "Failed to materialize '{}' output to '{}'",
                    format,
                    output_path.display()
                )
            })?;
        info!(
            artifact = %terminal_artifact.id(),
            digest = %terminal_artifact.digest(),
            bytes = terminal_artifact.size_bytes(),
            "✔ Output written to: {}",
            output_path.display()
        );
    }

    Ok(())
}
