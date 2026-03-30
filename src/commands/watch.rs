use anyhow::Result;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::config::load_config;

use super::build;

pub fn run(config_path: &str, debounce_ms: u64) -> Result<()> {
    info!("Starting watch mode for: {}", config_path);

    // Perform an initial build before entering the watch loop.
    if let Err(e) = build::run(config_path, false) {
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
