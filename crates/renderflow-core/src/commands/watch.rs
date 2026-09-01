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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn config_with_input(input_path: &str, output_dir: &str) -> NamedTempFile {
        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path, output_dir
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write config file");
        f
    }

    // ── extra_watch_paths ─────────────────────────────────────────────────────

    #[test]
    fn test_extra_watch_paths_returns_empty_for_missing_config() {
        let paths = extra_watch_paths("/nonexistent/renderflow.yaml");
        // The templates directory may or may not exist; the config is missing so
        // the input file path cannot be resolved — the list must not contain an
        // input-derived path.
        // We can only assert that the function returns without panicking and that
        // every returned path actually exists on disk.
        for p in &paths {
            assert!(
                p.exists(),
                "extra_watch_paths returned a non-existent path: {}",
                p.display()
            );
        }
    }

    #[test]
    fn test_extra_watch_paths_includes_existing_input_file() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let input_path = dir.path().join("doc.md");
        fs::write(&input_path, "# Hello\n").expect("write failed");
        let output_dir = dir.path().join("dist");
        let config =
            config_with_input(&input_path.to_string_lossy(), &output_dir.to_string_lossy());

        let paths = extra_watch_paths(config.path().to_str().unwrap());
        assert!(
            paths.contains(&input_path),
            "extra_watch_paths should include the input file, got: {paths:?}"
        );
    }

    #[test]
    fn test_extra_watch_paths_skips_missing_input_file() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        // The input file does not exist.
        let input_path = dir.path().join("missing.md");
        let output_dir = dir.path().join("dist");
        let config =
            config_with_input(&input_path.to_string_lossy(), &output_dir.to_string_lossy());

        let paths = extra_watch_paths(config.path().to_str().unwrap());
        assert!(
            !paths.contains(&input_path),
            "extra_watch_paths should not include a missing input file, got: {paths:?}"
        );
    }

    #[test]
    fn test_extra_watch_paths_all_returned_paths_exist() {
        let dir = tempfile::tempdir().expect("tempdir failed");
        let input_path = dir.path().join("doc.md");
        fs::write(&input_path, "# Hello\n").expect("write failed");
        let output_dir = dir.path().join("dist");
        let config =
            config_with_input(&input_path.to_string_lossy(), &output_dir.to_string_lossy());

        let paths = extra_watch_paths(config.path().to_str().unwrap());
        for p in &paths {
            assert!(
                p.exists(),
                "extra_watch_paths returned non-existent path: {}",
                p.display()
            );
        }
    }
}
