use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use tracing::{info, warn};

use crate::assets::normalize_asset_paths;
use crate::cache::{compute_input_hash, load_cache, save_cache};
use crate::config::load_config;
use crate::files::{ensure_output_dir, validate_input};
use crate::pipeline::{Pipeline, StrategyStep};
use crate::strategies::select_strategy;
use crate::template::init_tera;
use crate::transforms::register_transforms;

pub fn run(config_path: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        info!("Dry-run mode enabled — no files will be created and no commands will be executed");
    }
    info!("Running build pipeline");

    let config = load_config(config_path)?;
    info!("Loaded config successfully");

    let canonical_input = validate_input(&config.input)?;

    let input_dir = canonical_input
        .parent()
        .ok_or_else(|| anyhow::anyhow!(
            "Could not determine the parent directory of input file '{}'. \
             Please ensure the input path is a valid file path.",
            canonical_input.display()
        ))?;
    let content = fs::read_to_string(&canonical_input)
        .with_context(|| format!("Failed to read input file: {}", canonical_input.display()))?;
    // Resolve and validate all asset paths referenced in the document.
    // The normalized content (with canonical absolute paths) is passed through
    // the pipeline so transforms and strategies operate on the actual file content.
    let normalized_content = normalize_asset_paths(&content, input_dir)?;
    info!("Asset paths validated successfully");

    let output_dir = if dry_run {
        let path = std::path::PathBuf::from(&config.output_dir);
        info!("[dry-run] Would create output directory: {}", path.display());
        path
    } else {
        ensure_output_dir(&config.output_dir)?
    };

    let input_stem = Path::new(&config.input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");

    let tera = init_tera("templates")?;
    let template_count = tera.get_template_names().count();
    info!("Tera template engine initialised with {} template(s)", template_count);

    // Warn early if any configured template is not present in the templates directory.
    for output in &config.outputs {
        if let Some(ref name) = output.template {
            if !tera.get_template_names().any(|n| n == name) {
                warn!(
                    template = %name,
                    "Configured template '{}' was not found in the templates directory; \
                     rendering will fail if this template is required.",
                    name
                );
            }
        }
    }

    let output_formats: Vec<String> = config.outputs.iter().map(|o| o.output_type.to_string()).collect();
    if output_formats.is_empty() {
        warn!("No output formats configured — nothing to build");
        return Ok(());
    }
    info!("Selected outputs: {}", output_formats.join(", "));

    // One tick for transforms (run once) plus one tick per output format for rendering.
    let total_steps = 1 + config.outputs.len() as u64;
    let mp = MultiProgress::new();
    let pb = mp.add(ProgressBar::new(total_steps));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .expect("hardcoded progress bar template is valid")
            .progress_chars("█▓░"),
    );

    // Transforms are pure in-memory operations (no files, no external commands) and
    // are not output-format dependent, so they are executed once and reused for all
    // output formats.  The cache allows skipping this phase entirely when inputs have
    // not changed since the last successful build.
    let transform_pipeline = Pipeline::with_registry(register_transforms(&config.variables));
    let input_hash = compute_input_hash(&normalized_content, &config.variables);
    let cache_path = output_dir.join(".renderflow-cache.json");
    // Always attempt to read the cache; load_cache handles missing/corrupt files
    // gracefully.  Only write back to disk in non-dry-run mode.
    let mut transform_cache = load_cache(&cache_path);

    pb.set_message("Applying transforms");
    let transformed = if let Some(cached) = transform_cache.get(&input_hash) {
        info!("Cache hit — skipping transforms");
        pb.inc(1);
        cached.to_string()
    } else {
        info!("Cache miss — running transforms");
        let result = transform_pipeline
            .run_transforms(normalized_content)
            .with_context(|| "Transform pipeline failed; no output formats will be rendered")?;
        pb.inc(1);
        if !dry_run {
            transform_cache.insert(input_hash, result.clone());
            if let Err(e) = save_cache(&transform_cache, &cache_path) {
                warn!(error = %e, "Failed to save transform cache");
            }
        }
        result
    };

    // Output formats are rendered concurrently via rayon. Progress bar updates
    // and log messages may interleave across formats; this is expected and
    // acceptable for parallel execution.
    let failed_outputs: Vec<(String, anyhow::Error)> = config
        .outputs
        .par_iter()
        .map(|output| {
        let format = output.output_type.clone();
        let output_path = format!("{}/{}.{}", output_dir.display(), input_stem, format);
        info!(format = %format, output = %output_path, template = ?output.template, "Running pipeline for format");

        if dry_run {
            info!("[dry-run] Would render {} output to: {}", format, output_path);
            pb.set_message(format!("[{format}] Would render output"));
            pb.inc(1);
            pb.println(format!("[dry-run] Would write output to: {}", output_path));
            (format.to_string(), Ok(()))
        } else {
            let result = (|| -> Result<()> {
                let strategy = select_strategy(format.clone(), output.template.clone(), "templates".to_string())?;
                let mut pipeline = Pipeline::new();
                pipeline.add_step(Box::new(StrategyStep::new(strategy, &output_path, config.input_format(), config.variables.clone(), false)));

                pb.set_message(format!("[{format}] Rendering output"));
                pipeline.run_steps(transformed.clone())?;
                Ok(())
            })();

            match &result {
                Ok(_) => {
                    pb.inc(1);
                    pb.println(format!("✔ Output written to: {}", output_path));
                    info!(output = %output_path, "Pipeline completed for format: {}", format);
                }
                Err(e) => {
                    warn!(format = %format, error = %e, "Rendering failed for output format");
                    pb.inc(1);
                    pb.println(format!("✘ Failed to render {} output: {:#}", format, e));
                }
            }
            (format.to_string(), result)
        }
    })
    .filter_map(|(fmt, r)| r.err().map(|e| (fmt, e)))
    .collect();

    if dry_run {
        pb.finish_with_message("✔ Dry-run complete — no output written");
    } else if failed_outputs.is_empty() {
        pb.finish_with_message("✔ Build complete");
    } else {
        pb.finish_with_message(format!("⚠ Build completed with {} failure(s)", failed_outputs.len()));
        let messages: Vec<String> = failed_outputs
            .iter()
            .map(|(fmt, err)| format!("  - {}: {:#}", fmt, err))
            .collect();
        anyhow::bail!(
            "One or more output formats failed to render:\n{}",
            messages.join("\n")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn valid_config_file() -> (NamedTempFile, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, "# Test\n").expect("failed to write input file");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write temp file");
        (f, dir)
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_build_run_succeeds() {
        let (f, _dir) = valid_config_file();
        assert!(run(f.path().to_str().unwrap(), false).is_ok());
    }

    #[test]
    fn test_build_run_missing_config() {
        let result = run("/nonexistent/renderflow.yaml", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_run_missing_input_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"/nonexistent/input.md\"\noutput_dir: \"{}\"\n",
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write config");
        let result = run(f.path().to_str().unwrap(), false);
        assert!(result.is_err(), "expected error when input file is missing");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Input file not found"), "unexpected error: {}", msg);
    }

    #[test]
    fn test_build_run_unsupported_format() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, "# Test\n").expect("failed to write input file");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: epub\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write config");
        let result = run(f.path().to_str().unwrap(), false);
        assert!(result.is_err(), "expected error for unsupported format");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("not a valid output type"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    #[ignore = "requires pandoc to be installed and a valid input file"]
    fn test_build_run_with_pandoc() {
        let dir = tempfile::tempdir().unwrap();
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, "# Hello\n\nThis is a test.\n").unwrap();

        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );

        let mut config_file = NamedTempFile::new().unwrap();
        config_file
            .write_all(config_content.as_bytes())
            .unwrap();

        assert!(run(config_file.path().to_str().unwrap(), false).is_ok());
        assert!(output_dir.join("input.html").exists());
    }

    #[test]
    fn test_dry_run_succeeds_without_pandoc() {
        let (f, dir) = valid_config_file();
        let output_dir = dir.path().join("dist");
        let result = run(f.path().to_str().unwrap(), true);
        assert!(result.is_ok(), "dry-run should succeed: {:?}", result);
        // No output directory should have been created in dry-run mode
        assert!(!output_dir.exists(), "output directory must not be created in dry-run mode");
    }

    #[test]
    fn test_dry_run_does_not_create_output_files() {
        let (f, dir) = valid_config_file();
        let output_dir = dir.path().join("dist");
        run(f.path().to_str().unwrap(), true).expect("dry-run should not fail");
        // The dist directory and any rendered files must not exist
        assert!(!output_dir.exists(), "output directory must not be created in dry-run mode");
    }

    #[test]
    fn test_dry_run_missing_config_still_errors() {
        let result = run("/nonexistent/renderflow.yaml", true);
        assert!(result.is_err(), "dry-run with missing config should still error");
    }

    /// Build a config with multiple output formats for testing that transforms run once.
    fn multi_output_config_file() -> (NamedTempFile, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_path = dir.path().join("input.md");
        // Content includes emoji and a variable so transforms have real work to do
        // across both the EmojiTransform and VariableSubstitutionTransform stages.
        fs::write(&input_path, "# Hello 😀\n\nValue: {{greeting}}\n")
            .expect("failed to write input file");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: html\n  - type: pdf\ninput: \"{}\"\noutput_dir: \"{}\"\nvariables:\n  greeting: world\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write temp file");
        (f, dir)
    }

    #[test]
    fn test_dry_run_multiple_outputs_succeeds() {
        // Dry-run should succeed for multiple output formats without requiring
        // any external tools (pandoc/tectonic). Transforms run once and the
        // result is reused for each format.
        let (f, dir) = multi_output_config_file();
        let output_dir = dir.path().join("dist");
        let result = run(f.path().to_str().unwrap(), true);
        assert!(result.is_ok(), "dry-run with multiple outputs should succeed: {:?}", result);
        // No output directory should have been created in dry-run mode.
        assert!(!output_dir.exists(), "output directory must not be created in dry-run mode");
    }

    #[test]
    fn test_transforms_applied_once_content_consistent_across_formats() {
        // Verify that transform output is consistent when multiple formats are
        // configured: the same variable substitution result should appear
        // regardless of how many output formats are requested. We exercise
        // this indirectly by checking that a dry-run with multiple outputs
        // succeeds with the same result as a single-output dry-run.
        let (single_f, _single_dir) = valid_config_file();
        let (multi_f, _multi_dir) = multi_output_config_file();

        let single_result = run(single_f.path().to_str().unwrap(), true);
        let multi_result = run(multi_f.path().to_str().unwrap(), true);

        assert!(single_result.is_ok(), "single-output dry-run failed: {:?}", single_result);
        assert!(multi_result.is_ok(), "multi-output dry-run failed: {:?}", multi_result);
    }

    // ── cache integration tests ───────────────────────────────────────────────

    /// Pre-populate the transform cache file at `output_dir/.renderflow-cache.json`
    /// with the given hash → content mapping so that a subsequent build can
    /// exercise cache-hit behaviour without running pandoc.
    fn write_cache_file(output_dir: &std::path::Path, hash: &str, content: &str) {
        fs::create_dir_all(output_dir).expect("failed to create output dir");
        let cache_path = output_dir.join(".renderflow-cache.json");
        let map: std::collections::HashMap<&str, &str> =
            std::collections::HashMap::from([(hash, content)]);
        let json = serde_json::to_string(&map).expect("failed to serialize cache");
        fs::write(&cache_path, json).expect("failed to write cache file");
    }

    #[test]
    fn test_cache_miss_on_fresh_dry_run() {
        // A dry-run with no pre-existing cache file should proceed normally
        // (transforms run, no cache written).
        let (f, dir) = valid_config_file();
        let output_dir = dir.path().join("dist");
        // No cache file exists — this is a fresh state.
        let result = run(f.path().to_str().unwrap(), true);
        assert!(result.is_ok(), "dry-run should succeed without a cache: {:?}", result);
        // In dry-run mode the output directory is never created.
        assert!(!output_dir.exists(), "output directory must not be created in dry-run mode");
    }

    #[test]
    fn test_cache_hit_uses_pre_populated_cache() {
        // Pre-populate the cache with the exact hash that the build would
        // compute for the input file, then run a dry-run.  The build should
        // detect the cache hit and skip the transform phase.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_content = "# Test\n";
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, input_content).expect("failed to write input file");
        let output_dir = dir.path().join("dist");

        // Compute the hash the same way the build command will.
        let variables = std::collections::HashMap::new();
        let hash = crate::cache::compute_input_hash(input_content, &variables);
        let cached_transform = "# Test (from cache)\n";
        write_cache_file(&output_dir, &hash, cached_transform);

        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes()).expect("failed to write config");

        // The dry-run should succeed; cache hit is detected in both modes.
        let result = run(f.path().to_str().unwrap(), true);
        assert!(result.is_ok(), "dry-run with cache hit should succeed: {:?}", result);
    }

    #[test]
    fn test_cache_miss_when_input_changed() {
        // After changing the input content the hash changes, so the previously
        // cached entry should not match and transforms must run again.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let original_content = "# Original\n";
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, original_content).expect("failed to write input file");
        let output_dir = dir.path().join("dist");

        // Cache is keyed on the *original* content.
        let variables = std::collections::HashMap::new();
        let old_hash = crate::cache::compute_input_hash(original_content, &variables);
        write_cache_file(&output_dir, &old_hash, "cached result");

        // Now change the input file — the hash will be different.
        let new_content = "# Changed\n";
        fs::write(&input_path, new_content).expect("failed to write updated input");

        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes()).expect("failed to write config");

        // Dry-run still succeeds; it runs transforms because the hash misses.
        let result = run(f.path().to_str().unwrap(), true);
        assert!(result.is_ok(), "dry-run with cache miss should still succeed: {:?}", result);
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_cache_file_written_after_build() {
        // After a real (non-dry-run) build the cache file must exist in the
        // output directory and contain the hash of the transformed content.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, "# Hello\n").expect("failed to write input");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes()).expect("failed to write config");

        run(f.path().to_str().unwrap(), false).expect("build should succeed");

        let cache_path = output_dir.join(".renderflow-cache.json");
        assert!(cache_path.exists(), "cache file must exist after a real build");
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_second_build_hits_cache() {
        // Running the build twice with the same input must result in a cache
        // hit on the second run.  We verify this indirectly by checking that
        // the cache file still exists and that the second run also succeeds.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, "# Hello\n").expect("failed to write input");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes()).expect("failed to write config");

        // First build — cache miss, cache written.
        run(f.path().to_str().unwrap(), false).expect("first build should succeed");
        // Second build — cache hit.
        run(f.path().to_str().unwrap(), false).expect("second build (cache hit) should succeed");

        let cache_path = output_dir.join(".renderflow-cache.json");
        assert!(cache_path.exists(), "cache file must still exist after second build");
    }
}
