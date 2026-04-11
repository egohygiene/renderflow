use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::config::load_config;
use crate::incremental::{hash_file, load_dependency_map};

use super::build;

pub fn run(config_path: &str, debounce_ms: u64) -> Result<()> {
    info!("Starting watch mode for: {}", config_path);

    // Perform an initial build before entering the watch loop.
    if let Err(e) = build::run(config_path, false, None) {
        error!("Initial build failed: {:#}", e);
    }

    let (tx, rx) = mpsc::channel();

    let mut debouncer = new_debouncer(Duration::from_millis(debounce_ms), move |result| {
        tx.send(result).ok();
    })?;

    // Always watch the config file itself.
    debouncer
        .watcher()
        .watch(Path::new(config_path), RecursiveMode::NonRecursive)?;

    // Derive additional paths to watch from the config (best-effort).
    for path in extra_watch_paths(config_path) {
        let mode = if path.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        if let Err(e) = debouncer.watcher().watch(&path, mode) {
            warn!("Could not watch path {}: {}", path.display(), e);
        }
    }

    info!("Watching for file changes. Press Ctrl+C to stop.");

    for result in rx {
        match result {
            Ok(events) => {
                for event in &events {
                    info!("File changed → rebuilding... ({})", event.path.display());

                    // Use the dependency map to log which outputs are affected by
                    // this specific file change.  This gives the user (and
                    // developers) visibility into the incremental build decisions
                    // without changing the current full-rebuild strategy.
                    log_affected_outputs(config_path, &event.path);
                }
                if let Err(e) = build::run_resilient(config_path) {
                    error!("Build failed: {:#}", e);
                }
            }
            Err(e) => {
                error!("Watch error: {}", e);
            }
        }
    }

    Ok(())
}

/// Log which outputs in the dependency map are affected by a change to `changed_path`.
///
/// This is best-effort: if the config cannot be loaded or the dependency map
/// cannot be found, the function silently returns.
fn log_affected_outputs(config_path: &str, changed_path: &Path) {
    let Ok(config) = load_config(config_path) else { return };
    let output_dir = PathBuf::from(&config.output_dir);
    let dep_map_path = output_dir.join(".renderflow-deps.json");
    let dep_map = load_dependency_map(&dep_map_path);

    let changed_str = changed_path.to_string_lossy();
    // Hash the changed file to compare with recorded hashes.  If the file
    // cannot be read (e.g. it was deleted) we use an empty string so that
    // every recorded hash will differ, correctly marking all dependents stale.
    let current_hash = hash_file(changed_path).unwrap_or_default();
    let affected = dep_map.outputs_affected_by(&changed_str, &current_hash);

    if affected.is_empty() {
        debug!(
            changed = %changed_str,
            "No tracked outputs depend on this file (or dependency map is empty)"
        );
    } else {
        debug!(
            changed = %changed_str,
            affected_outputs = ?affected,
            "Outputs affected by this file change (per dependency map)"
        );
    }
}

/// Collect extra paths to watch beyond the config file itself.
///
/// Tries to load the config so that the actual input file is watched.
/// Always includes the `templates` directory when it exists.
fn extra_watch_paths(config_path: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();

    if let Ok(config) = load_config(config_path) {
        let input = PathBuf::from(&config.input);
        if input.exists() {
            paths.push(input);
        }
    }

    let templates_dir = PathBuf::from("templates");
    if templates_dir.exists() {
        paths.push(templates_dir);
    }

    paths
}
