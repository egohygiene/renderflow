use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use tracing::{info, warn};

use crate::assets::normalize_asset_paths;
use crate::config::load_config;
use crate::files::{ensure_output_dir, validate_input};
use crate::pipeline::{Pipeline, StrategyStep};
use crate::strategies::select_strategy;
use crate::template::init_tera;
use crate::transforms::{EmojiTransform, VariableSubstitutionTransform};

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

    // Two ticks per output format: one for transforms, one for rendering.
    let total_steps = config.outputs.len() as u64 * 2;
    let pb = ProgressBar::new(total_steps);
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .expect("hardcoded progress bar template is valid")
            .progress_chars("█▓░"),
    );

    let mut failed_outputs: Vec<(String, anyhow::Error)> = Vec::new();

    for output in &config.outputs {
        let format = output.output_type.clone();
        let output_path = format!("{}/{}.{}", output_dir.display(), input_stem, format);
        info!(format = %format, output = %output_path, template = ?output.template, "Running pipeline for format");

        let mut pipeline = Pipeline::new();
        pipeline.add_transform(Box::new(EmojiTransform::new()));
        if !config.variables.is_empty() {
            pipeline.add_transform(Box::new(VariableSubstitutionTransform::new(
                config.variables.clone(),
            )));
        }

        // Transforms are pure in-memory operations (no files, no external commands),
        // so they run in both normal and dry-run mode to give an accurate preview.
        pb.set_message(format!("[{format}] Applying transforms"));
        let transformed = match pipeline.run_transforms(normalized_content.clone()) {
            Ok(t) => t,
            Err(e) => {
                warn!(format = %format, error = %e, "Transform failed for output format");
                failed_outputs.push((format.to_string(), e));
                // Consume both the transform tick and the render tick we're skipping.
                pb.inc(2);
                continue;
            }
        };
        pb.inc(1);

        if dry_run {
            info!("[dry-run] Would render {} output to: {}", format, output_path);
            pb.set_message(format!("[{format}] Would render output"));
            pb.inc(1);
            pb.println(format!("[dry-run] Would write output to: {}", output_path));
        } else {
            let strategy = select_strategy(format.clone(), output.template.clone(), "templates".to_string())?;
            pipeline.add_step(Box::new(StrategyStep::new(strategy, &output_path)));

            pb.set_message(format!("[{format}] Rendering output"));
            match pipeline.run_steps(transformed) {
                Ok(_) => {
                    pb.inc(1);
                    pb.println(format!("✔ Output written to: {}", output_path));
                    info!(output = %output_path, "Pipeline completed for format: {}", format);
                }
                Err(e) => {
                    warn!(format = %format, error = %e, "Rendering failed for output format");
                    pb.inc(1);
                    pb.println(format!("✘ Failed to render {} output: {:#}", format, e));
                    failed_outputs.push((format.to_string(), e));
                }
            }
        }
    }

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
            "outputs:\n  - type: docx\ninput: \"{}\"\noutput_dir: \"{}\"\n",
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
            msg.contains("DOCX output is not yet supported"),
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
}
