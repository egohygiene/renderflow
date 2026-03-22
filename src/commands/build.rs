use anyhow::Result;
use std::path::Path;
use tracing::info;

use crate::config::load_config;
use crate::pipeline::{PandocStep, Pipeline};

pub fn run(config_path: &str) -> Result<()> {
    info!("Executing build command");

    let config = load_config(config_path)?;
    info!(?config, "Loaded config successfully");
    println!("Loaded config successfully");

    std::fs::create_dir_all(&config.output_dir)?;
    info!(output_dir = %config.output_dir, "Ensured output directory exists");

    let input_stem = Path::new(&config.input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");

    println!("Running build pipeline");

    for format in &config.outputs {
        let output_path = format!("{}/{}.{}", config.output_dir, input_stem, format);
        info!(format = %format, output = %output_path, "Running pipeline for format");
        println!("Running build pipeline for format: {}", format);

        let mut pipeline = Pipeline::new();
        pipeline.add_step(Box::new(PandocStep::new(&output_path)));
        pipeline.run(config.input.clone())?;

        info!(output = %output_path, "Pipeline completed for format: {}", format);
        println!("Output written to: {}", output_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn valid_config_file() -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(b"outputs: []\ninput: \"input.md\"\noutput_dir: \"dist\"\n")
            .expect("failed to write temp file");
        f
    }

    #[test]
    fn test_build_run_succeeds() {
        let f = valid_config_file();
        assert!(run(f.path().to_str().unwrap()).is_ok());
    }

    #[test]
    fn test_build_run_missing_config() {
        let result = run("/nonexistent/renderflow.yaml");
        assert!(result.is_err());
    }

    #[test]
    #[ignore = "requires pandoc to be installed and a valid input file"]
    fn test_build_run_with_pandoc() {
        use std::fs;

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
