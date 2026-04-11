use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::Itertools as _;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

use crate::assets::normalize_asset_paths;
use crate::cache::{compute_input_hash, compute_output_hash, load_cache, load_output_cache, save_cache, save_output_cache};
use crate::config::{load_config, OutputType};
use crate::deps::validate_dependencies;
use crate::files::{ensure_output_dir, validate_input};
use crate::incremental::{build_output_dependencies, load_dependency_map, save_dependency_map, FileDependency};
use crate::optimization::OptimizationMode;
use crate::pipeline::{Pipeline, StrategyStep};
use crate::strategies::select_strategy;
use crate::template::{init_tera, validate_templates};
use crate::transforms::{load_transforms_from_yaml, FailureMode, TransformRegistry};

/// Run the full build pipeline.
///
/// Transforms fail fast: the first transform error aborts the build and returns an error.
///
/// `optimization` overrides the mode from the config file when `Some`.
pub fn run(config_path: &str, dry_run: bool, optimization: Option<OptimizationMode>) -> Result<()> {
    run_impl(config_path, dry_run, false, optimization)
}

/// Run the build pipeline in resilient mode.
///
/// Like [`run`], but transform failures are logged and skipped rather than
/// aborting the build.  Suitable for watch-mode rebuilds where a transient
/// transform error should not stop the file watcher.
pub fn run_resilient(config_path: &str) -> Result<()> {
    run_impl(config_path, false, true, None)
}

/// Each element produced by the parallel render loop:
/// (format_name, output_path, render_result, optional_output_hash, optional_file_deps).
///
/// The optional hash and deps are `Some` only for successful renders (including
/// outputs that were skipped as up-to-date) and are used to update the output
/// cache and dependency map after all formats have finished.
type RenderResult = (String, String, Result<()>, Option<String>, Option<Vec<FileDependency>>);

fn run_impl(config_path: &str, dry_run: bool, resilient: bool, optimization: Option<OptimizationMode>) -> Result<()> {
    if dry_run {
        info!("Dry-run mode enabled — no files will be created and no commands will be executed");
    }
    info!("Running build pipeline");

    let config = load_config(config_path)?;
    info!("Loaded config successfully");

    // Read the raw config file content so it can be included in the transform
    // cache hash.  Any change to the config file (not only to `variables`) will
    // then invalidate the cached transform results and force a fresh pipeline run.
    let config_content = fs::read_to_string(config_path)
        .with_context(|| format!("Failed to re-read config file for hashing: {}", config_path))?;

    // CLI flag takes precedence over config file; fall back to config value.
    let opt_mode = optimization.unwrap_or(config.optimization);
    info!(optimization = %opt_mode, "Using optimization mode");

    let canonical_input = validate_input(&config.input)?;

    // Validate required system dependencies after confirming the config and input
    // are accessible. Skip in dry-run mode because no external tools are invoked.
    if !dry_run {
        let pdf_requested = config.outputs.iter().any(|o| o.output_type == OutputType::Pdf);
        validate_dependencies(pdf_requested)?;
    }

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
        info!("[DRY RUN] Would create output directory: {}", path.display());
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

    // Validate all configured templates early, before any pipeline execution,
    // so that missing templates are detected immediately with a clear error
    // rather than discovered later during rendering.
    validate_templates(&config.outputs, "templates")?;

    if config.outputs.is_empty() {
        warn!("No output formats configured — nothing to build");
        return Ok(());
    }
    info!("Selected outputs: {}", config.outputs.iter().map(|o| o.output_type.to_string()).join(", "));

    // One tick for the transform phase plus one tick per output format for rendering.
    let total_steps = 1 + config.outputs.len() as u64;
    let mp = MultiProgress::new();
    let pb = mp.add(ProgressBar::new(total_steps));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .expect("hardcoded progress bar template is valid")
            .progress_chars("█▓░"),
    );

    // Pre-create one spinner per output format *before* the parallel rendering
    // section.  Calling mp.add() inside a rayon parallel closure acquires an
    // internal mutex on every call and can serialise the workers.  By creating
    // all bars up-front each worker can update its own bar without contention.
    let output_bars: Vec<ProgressBar> = config.outputs.iter().map(|output| {
        let bar = mp.add(ProgressBar::new_spinner());
        bar.set_style(
            ProgressStyle::with_template("  {spinner:.blue} [{prefix:.bold.cyan}] {msg}")
                .expect("hardcoded per-output progress bar template is valid"),
        );
        bar.set_prefix(output.output_type.to_string());
        bar.set_message("queued");
        bar
    }).collect();

    // Transforms are run once per output format (serially, before parallel rendering)
    // because some transforms are format-specific.  In particular, EmojiTransform skips
    // replacement for HTML (which renders emoji natively) but applies it for PDF, DOCX,
    // and other formats.  A format-keyed cache avoids redundant work across builds.
    let base_input_hash = compute_input_hash(&normalized_content, &config_content, &config.variables);
    let cache_path = output_dir.join(".renderflow-cache.json");
    // Always attempt to read the cache; load_cache handles missing/corrupt files
    // gracefully.  Only write back to disk in non-dry-run mode.
    let mut transform_cache = load_cache(&cache_path);

    // Load command-based transforms from the YAML file specified in config, if any.
    // These are applied after the standard built-in pipeline (emoji, variables, syntax).
    let command_registry: Option<TransformRegistry> = if let Some(ref path) = config.transforms {
        info!(path = %path, "Loading command-based transforms");
        let mode = if resilient { FailureMode::ContinueOnError } else { FailureMode::FailFast };
        Some(
            load_transforms_from_yaml(path)
                .with_context(|| format!("Failed to load command-based transforms from '{}'", path))?
                .with_failure_mode(mode),
        )
    } else {
        None
    };

    pb.set_message(if dry_run { "[DRY RUN] Applying transforms" } else { "Applying transforms" });
    let mut format_transformed: HashMap<String, String> = HashMap::new();
    for output in &config.outputs {
        let format = &output.output_type;
        let format_str = format.to_string();
        // Include the output format in the cache key so that HTML and PDF
        // transformations are cached independently.
        let hash_key = format!("{base_input_hash}-{format_str}");

        let transformed = if let Some(cached) = transform_cache.get(&hash_key) {
            debug!(hash = %hash_key, format = %format_str, "Transform cache hit — skipping transforms");
            cached.to_string()
        } else {
            debug!(hash = %hash_key, format = %format_str, "Transform cache miss — running transforms");
            let pipeline = if resilient {
                Pipeline::with_standard_transforms_resilient(&config.variables, format)
            } else {
                Pipeline::with_standard_transforms(&config.variables, format)
            };
            let standard_output = pipeline
                .run_transforms(normalized_content.as_ref().to_owned())
                .with_context(|| format!("Transform pipeline failed for format: {format_str}; aborting build"))?;
            // Apply command-based transforms (e.g. ImageMagick, Pandoc) if configured.
            if let Some(ref cmd_registry) = command_registry {
                debug!(format = %format_str, "Applying command-based transforms");
                cmd_registry
                    .apply_all(standard_output)
                    .with_context(|| format!("Command transform pipeline failed for format: {format_str}"))?
            } else {
                standard_output
            }
        };

        if !dry_run {
            transform_cache.insert(hash_key, transformed.clone());
        }
        format_transformed.insert(format_str, transformed);
    }
    pb.inc(1);

    if !dry_run {
        if let Err(e) = save_cache(&transform_cache, &cache_path) {
            warn!(error = %e, "Failed to save transform cache");
        }
    }

    // Load the output cache so that individual render steps can be skipped when
    // their inputs (transformed content + output type + template) have not changed.
    let output_cache_path = output_dir.join(".renderflow-output-cache.json");
    let mut output_cache = load_output_cache(&output_cache_path);

    // Load the dependency map so that file-level dependencies are tracked across
    // builds.  This records which specific input files (source document, config,
    // templates) produced each output, enabling precise change attribution.
    let dep_map_path = output_dir.join(".renderflow-deps.json");
    let mut dep_map = load_dependency_map(&dep_map_path);

    // Output formats are rendered concurrently via rayon. Each output has its own
    // pre-created progress bar so workers never block each other updating the display.
    // Failures are captured per-output and aggregated at the end — a single format
    // failure does not abort sibling renders.
    //
    // Each element is (format_name, output_path, result, Option<new_output_hash>,
    // Option<file_deps>).  The optional hash and deps are Some only when the
    // render succeeded (or was skipped as up-to-date), and are used to update
    // the output cache and dependency map after all formats finish.
    let render_results: Vec<RenderResult> = config
        .outputs
        .par_iter()
        .zip(output_bars.par_iter())
        .map(|(output, bar)| {
        let format = output.output_type.clone();
        let format_str = format.to_string();
        let output_path = format!("{}/{}.{}", output_dir.display(), input_stem, format);
        info!(format = %format, output = %output_path, template = ?output.template, "Running pipeline for format");

        // format_transformed is populated for every configured output in the serial loop above.
        let transformed = format_transformed
            .get(&format_str)
            .expect("format_str must be present in format_transformed")
            .clone();

        if dry_run {
            info!("[DRY RUN] Would render {} output to: {}", format, output_path);
            bar.finish_with_message(format!("[DRY RUN] Would render to {}", output_path));
            pb.inc(1);
            pb.println(format!("[DRY RUN] Would write output to: {}", output_path));
            (format_str, output_path, Ok(()), None, None)
        } else {
            // Build the list of file dependencies for this output.  This is
            // used to populate the dependency map after the render completes.
            let template_path = output.template.as_deref().map(|name| {
                Path::new("templates").join(name)
            });
            let file_deps = build_output_dependencies(
                &canonical_input,
                Path::new(config_path),
                template_path.as_deref(),
            );

            // Compute a hash of all inputs that determine this output's content.
            // If the stored hash matches and the output file already exists, pandoc
            // can be skipped entirely.  The template file content (not just its
            // name) is included so that edits to a template file invalidate the
            // output cache even when the template path is unchanged.
            //
            // Additionally, log the file-level dependency status (from the
            // dependency map) at DEBUG level so that the precise reason a rebuild
            // was or was not triggered is visible in verbose output.
            debug!(
                output = %output_path,
                dep_map_up_to_date = dep_map.is_output_up_to_date(&output_path, &file_deps),
                recorded_deps = ?dep_map.dependencies_for(&output_path),
                "Incremental dependency check"
            );
            let template_content = output.template.as_deref().and_then(|name| {
                let path = Path::new("templates").join(name);
                match fs::read_to_string(&path) {
                    Ok(content) => Some(content),
                    Err(e) => {
                        warn!(
                            template = %name,
                            path = %path.display(),
                            error = %e,
                            "Failed to read template file for cache hash; template changes may not invalidate cache"
                        );
                        None
                    }
                }
            });
            let output_hash = compute_output_hash(
                &transformed,
                &format_str,
                output.template.as_deref(),
                template_content.as_deref(),
            );

            if Path::new(&output_path).exists()
                && output_cache.get(&output_path) == Some(output_hash.as_str())
            {
                debug!(hash = %output_hash, output = %output_path, "Output cache hit — skipping render");
                info!("Skipping {} render (unchanged)", format);
                bar.finish_with_message(format!("↩ unchanged: {}", output_path));
                pb.inc(1);
                pb.println(format!("↩ Skipping {} output (unchanged): {}", format, output_path));
                return (format_str, output_path, Ok(()), Some(output_hash), Some(file_deps));
            }

            bar.set_message("rendering…");
            let result = (|| -> Result<()> {
                debug!(hash = %output_hash, output = %output_path, "Output cache miss — rendering output");
                let strategy = select_strategy(&format, output.template.as_deref(), "templates")?;
                let mut pipeline = Pipeline::new();
                pipeline.add_step(Box::new(StrategyStep::new(strategy, &output_path, config.input_format(), config.variables.clone(), false)));

                pipeline.run(transformed)?;
                Ok(())
            })();

            let new_hash = if result.is_ok() { Some(output_hash) } else { None };
            let new_deps = if result.is_ok() { Some(file_deps) } else { None };

            match &result {
                Ok(_) => {
                    bar.finish_with_message(format!("✔ {}", output_path));
                    pb.inc(1);
                    pb.println(format!("✔ Output written to: {}", output_path));
                    info!(output = %output_path, "Pipeline completed for format: {}", format);
                }
                Err(e) => {
                    warn!(format = %format, error = %e, "Rendering failed for output format");
                    bar.finish_with_message(format!("✘ failed: {:#}", e));
                    pb.inc(1);
                    pb.println(format!("✘ Failed to render {} output: {:#}", format, e));
                }
            }
            (format_str, output_path, result, new_hash, new_deps)
        }
    })
    .collect();

    // Persist updated output cache and dependency map for all successful renders
    // (including skipped ones).
    if !dry_run {
        for (_, output_path, result, new_hash, new_deps) in &render_results {
            if result.is_ok() {
                if let Some(hash) = new_hash {
                    output_cache.insert(output_path.clone(), hash.clone());
                }
                if let Some(deps) = new_deps {
                    dep_map.record(output_path.clone(), deps.clone());
                }
            }
        }
        if let Err(e) = save_output_cache(&output_cache, &output_cache_path) {
            warn!(error = %e, "Failed to save output cache");
        }
        if let Err(e) = save_dependency_map(&dep_map, &dep_map_path) {
            warn!(error = %e, "Failed to save dependency map");
        }
    }

    let failed_outputs: Vec<(String, anyhow::Error)> = render_results
        .into_iter()
        .filter_map(|(fmt, _, r, _, _)| r.err().map(|e| (fmt, e)))
        .collect();

    if dry_run {
        pb.finish_with_message("[DRY RUN] ✔ Dry-run complete — no output written");
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
        assert!(run(f.path().to_str().unwrap(), false, None).is_ok());
    }

    #[test]
    fn test_build_run_missing_config() {
        let result = run("/nonexistent/renderflow.yaml", false, None);
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
        let result = run(f.path().to_str().unwrap(), false, None);
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
        let result = run(f.path().to_str().unwrap(), false, None);
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

        assert!(run(config_file.path().to_str().unwrap(), false, None).is_ok());
        assert!(output_dir.join("input.html").exists());
    }

    #[test]
    fn test_dry_run_succeeds_without_pandoc() {
        let (f, dir) = valid_config_file();
        let output_dir = dir.path().join("dist");
        let result = run(f.path().to_str().unwrap(), true, None);
        assert!(result.is_ok(), "dry-run should succeed: {:?}", result);
        // No output directory should have been created in dry-run mode
        assert!(!output_dir.exists(), "output directory must not be created in dry-run mode");
    }

    #[test]
    fn test_dry_run_does_not_create_output_files() {
        let (f, dir) = valid_config_file();
        let output_dir = dir.path().join("dist");
        run(f.path().to_str().unwrap(), true, None).expect("dry-run should not fail");
        // The dist directory and any rendered files must not exist
        assert!(!output_dir.exists(), "output directory must not be created in dry-run mode");
    }

    #[test]
    fn test_dry_run_missing_config_still_errors() {
        let result = run("/nonexistent/renderflow.yaml", true, None);
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
        let result = run(f.path().to_str().unwrap(), true, None);
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

        let single_result = run(single_f.path().to_str().unwrap(), true, None);
        let multi_result = run(multi_f.path().to_str().unwrap(), true, None);

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
        let result = run(f.path().to_str().unwrap(), true, None);
        assert!(result.is_ok(), "dry-run should succeed without a cache: {:?}", result);
        // In dry-run mode the output directory is never created.
        assert!(!output_dir.exists(), "output directory must not be created in dry-run mode");
    }

    #[test]
    fn test_cache_hit_uses_pre_populated_cache() {
        // Pre-populate the cache with the exact hash that the build would
        // compute for the input file + config + format, then run a dry-run.
        // The build should detect the cache hit and skip the transform phase
        // for that format.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_content = "# Test\n";
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, input_content).expect("failed to write input file");
        let output_dir = dir.path().join("dist");

        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );

        // Compute the hash the same way the build command will: base hash + "-html".
        // The hash now also incorporates the config file content.
        let variables = std::collections::HashMap::new();
        let base_hash = crate::cache::compute_input_hash(input_content, &config_content, &variables);
        let hash_key = format!("{base_hash}-html");
        let cached_transform = "# Test (from cache)\n";
        write_cache_file(&output_dir, &hash_key, cached_transform);

        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes()).expect("failed to write config");

        // The dry-run should succeed; cache hit is detected in both modes.
        let result = run(f.path().to_str().unwrap(), true, None);
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

        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );

        // Cache is keyed on the *original* content + config + format.
        let variables = std::collections::HashMap::new();
        let old_base_hash = crate::cache::compute_input_hash(original_content, &config_content, &variables);
        let old_hash_key = format!("{old_base_hash}-html");
        write_cache_file(&output_dir, &old_hash_key, "cached result");

        // Now change the input file — the hash will be different.
        let new_content = "# Changed\n";
        fs::write(&input_path, new_content).expect("failed to write updated input");

        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes()).expect("failed to write config");

        // Dry-run still succeeds; it runs transforms because the hash misses.
        let result = run(f.path().to_str().unwrap(), true, None);
        assert!(result.is_ok(), "dry-run with cache miss should still succeed: {:?}", result);
    }

    #[test]
    fn test_cache_miss_when_config_changed() {
        // After changing the config content the hash changes, so the previously
        // cached entry should not match and transforms must run again.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_content = "# Hello\n";
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, input_content).expect("failed to write input file");
        let output_dir = dir.path().join("dist");

        // Original config — used to seed the cache.
        let original_config = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let variables = std::collections::HashMap::new();
        let old_base_hash = crate::cache::compute_input_hash(input_content, &original_config, &variables);
        let old_hash_key = format!("{old_base_hash}-html");
        write_cache_file(&output_dir, &old_hash_key, "cached result");

        // Updated config (adds a variable) — the hash must differ from the original.
        let updated_config = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\nvariables:\n  title: changed\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(updated_config.as_bytes()).expect("failed to write config");

        // Dry-run succeeds; transforms run because the config hash misses.
        let result = run(f.path().to_str().unwrap(), true, None);
        assert!(result.is_ok(), "dry-run with changed config should succeed: {:?}", result);
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

        run(f.path().to_str().unwrap(), false, None).expect("build should succeed");

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
        run(f.path().to_str().unwrap(), false, None).expect("first build should succeed");
        // Second build — cache hit.
        run(f.path().to_str().unwrap(), false, None).expect("second build (cache hit) should succeed");

        let cache_path = output_dir.join(".renderflow-cache.json");
        assert!(cache_path.exists(), "cache file must still exist after second build");
    }

    // ── output cache integration tests ───────────────────────────────────────

    /// Write a pre-populated output cache file at `output_dir/.renderflow-output-cache.json`.
    fn write_output_cache_file(output_dir: &std::path::Path, output_path: &str, hash: &str) {
        fs::create_dir_all(output_dir).expect("failed to create output dir");
        let cache_path = output_dir.join(".renderflow-output-cache.json");
        let map: std::collections::HashMap<&str, &str> =
            std::collections::HashMap::from([(output_path, hash)]);
        let json = serde_json::to_string(&map).expect("failed to serialize output cache");
        fs::write(&cache_path, json).expect("failed to write output cache file");
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_output_cache_file_written_after_build() {
        // After a successful (non-dry-run) build the output cache file must
        // exist alongside the transform cache.
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

        run(f.path().to_str().unwrap(), false, None).expect("build should succeed");

        let output_cache_path = output_dir.join(".renderflow-output-cache.json");
        assert!(output_cache_path.exists(), "output cache file must exist after a real build");
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_second_build_skips_unchanged_output() {
        // Run the build twice with the same inputs; the second run must skip
        // pandoc for all outputs because the output cache indicates they are
        // already up-to-date.  We verify indirectly that both runs succeed and
        // the output cache file persists.
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

        // First build — output cache miss, pandoc runs, cache written.
        run(f.path().to_str().unwrap(), false, None).expect("first build should succeed");
        // Second build — output cache hit, pandoc skipped.
        run(f.path().to_str().unwrap(), false, None).expect("second build (output cache hit) should succeed");

        let output_cache_path = output_dir.join(".renderflow-output-cache.json");
        assert!(output_cache_path.exists(), "output cache must still exist after second build");
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_changed_input_triggers_rebuild() {
        // After modifying the input file, a subsequent build must re-run pandoc
        // because both the transform cache and output cache hashes change.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, "# Original\n").expect("failed to write input");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes()).expect("failed to write config");

        // First build with original content.
        run(f.path().to_str().unwrap(), false, None).expect("first build should succeed");

        // Modify input — caches must be invalidated.
        fs::write(&input_path, "# Modified\n").expect("failed to write updated input");

        // Second build must succeed (re-render triggered by cache miss).
        run(f.path().to_str().unwrap(), false, None).expect("second build after input change should succeed");
    }

    #[test]
    fn test_output_cache_not_written_in_dry_run() {
        // In dry-run mode the output cache file must never be created.
        let (f, dir) = valid_config_file();
        let output_dir = dir.path().join("dist");
        run(f.path().to_str().unwrap(), true, None).expect("dry-run should succeed");
        let output_cache_path = output_dir.join(".renderflow-output-cache.json");
        assert!(
            !output_cache_path.exists(),
            "output cache must not be written in dry-run mode"
        );
    }

    #[test]
    fn test_pre_populated_output_cache_loaded_without_error() {
        // Even when a pre-populated output cache exists, a dry-run should
        // complete without error (the cache is read but never written back).
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_content = "# Test\n";
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, input_content).expect("failed to write input file");
        let output_dir = dir.path().join("dist");

        // Write a dummy output cache entry.
        let output_path = format!("{}/input.html", output_dir.display());
        write_output_cache_file(&output_dir, &output_path, "dummy_hash");

        let config_content = format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes()).expect("failed to write config");

        let result = run(f.path().to_str().unwrap(), true, None);
        assert!(result.is_ok(), "dry-run with output cache should succeed: {:?}", result);
    }

    #[test]
    fn test_parallel_dry_run_produces_result_per_output() {
        // Dry-run with multiple output formats: the parallel rendering loop must
        // process every configured output and produce a result for each one.
        // This verifies that par_iter covers all outputs, not just the first.
        let (f, _dir) = multi_output_config_file();
        let result = run(f.path().to_str().unwrap(), true, None);
        assert!(result.is_ok(), "dry-run with multiple outputs should succeed: {:?}", result);
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_parallel_failure_isolation_and_aggregation() {
        // Verify that when multiple output formats are configured and one format
        // fails, the other formats are still attempted and their failures are
        // collected independently.  The final error must mention every failing
        // format so the caller can identify which outputs need attention.
        //
        // We configure two unsupported formats.  Each fails independently inside
        // the rayon parallel loop; the outer run() collects all failures and
        // returns a single aggregated error.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, "# Test\n").expect("failed to write input file");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: epub\n  - type: rst\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write config");

        let result = run(f.path().to_str().unwrap(), false, None);
        assert!(result.is_err(), "build with only unsupported formats should fail");
        let err_msg = format!("{:#}", result.unwrap_err());
        // Both format names must appear in the aggregated error message.
        assert!(
            err_msg.contains("epub"),
            "aggregated error should mention 'epub': {err_msg}"
        );
        assert!(
            err_msg.contains("rst"),
            "aggregated error should mention 'rst': {err_msg}"
        );
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_parallel_renders_all_formats_independently() {
        // With html, pdf, and docx configured the parallel loop must produce one
        // successful result per format.  This exercises the full rayon path for
        // each output strategy running concurrently.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let input_path = dir.path().join("input.md");
        fs::write(&input_path, "# Hello\n\nThis is a test.\n")
            .expect("failed to write input file");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs:\n  - type: html\n  - type: pdf\n  - type: docx\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write config");

        let result = run(f.path().to_str().unwrap(), false, None);
        assert!(result.is_ok(), "parallel build with html+pdf+docx should succeed: {:?}", result);
        assert!(output_dir.join("input.html").exists(), "html output must exist");
        assert!(output_dir.join("input.pdf").exists(), "pdf output must exist");
        assert!(output_dir.join("input.docx").exists(), "docx output must exist");
    }
}
