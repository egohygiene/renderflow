use anyhow::Result;
use std::path::Path;
use tracing::info;

use crate::config::load_config;
use crate::files::{ensure_output_dir, validate_input};
use crate::pipeline::{Pipeline, StrategyStep};
use crate::strategies::select_strategy;

pub fn run(config_path: &str) -> Result<()> {
    info!("Executing build command");

    let config = load_config(config_path)?;
    info!(?config, "Loaded config successfully");
    println!("Loaded config successfully");

    validate_input(&config.input)?;

    let output_dir = ensure_output_dir(&config.output_dir)?;

    let input_stem = Path::new(&config.input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");

    println!("Running build pipeline");

    for format in &config.outputs {
        let output_path = format!("{}/{}.{}", output_dir.display(), input_stem, format);
        info!(format = %format, output = %output_path, "Running pipeline for format");
        println!("Running build pipeline for format: {}", format);

        let strategy = select_strategy(format)?;
        let mut pipeline = Pipeline::new();
        pipeline.add_step(Box::new(StrategyStep::new(strategy, &output_path)));
        pipeline.run(config.input.clone())?;

        info!(output = %output_path, "Pipeline completed for format: {}", format);
        println!("Output written to: {}", output_path);
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
            "outputs: []\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write temp file");
        (f, dir)
    }

    #[test]
    fn test_build_run_succeeds() {
        let (f, _dir) = valid_config_file();
        assert!(run(f.path().to_str().unwrap()).is_ok());
    }

    #[test]
    fn test_build_run_missing_config() {
        let result = run("/nonexistent/renderflow.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_build_run_missing_input_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let output_dir = dir.path().join("dist");
        let config_content = format!(
            "outputs: []\ninput: \"/nonexistent/input.md\"\noutput_dir: \"{}\"\n",
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write config");
        let result = run(f.path().to_str().unwrap());
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
            "outputs:\n  - docx\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(config_content.as_bytes())
            .expect("failed to write config");
        let result = run(f.path().to_str().unwrap());
        assert!(result.is_err(), "expected error for unsupported format");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("Unsupported output format"),
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
            "outputs:\n  - html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input_path.display(),
            output_dir.display()
        );

        let mut config_file = NamedTempFile::new().unwrap();
        config_file
            .write_all(config_content.as_bytes())
            .unwrap();

        assert!(run(config_file.path().to_str().unwrap()).is_ok());
        assert!(output_dir.join("input.html").exists());
    }
}
