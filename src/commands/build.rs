use anyhow::Result;
use tracing::info;

use crate::config::load_config;

pub fn run(config_path: &str) -> Result<()> {
    info!("Executing build command");

    let config = load_config(config_path)?;
    info!(?config, "Loaded config successfully");
    println!("Loaded config successfully");
    println!("Running build pipeline");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn valid_config_file() -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(
            b"outputs:\n  - pdf\n  - html\ninput: \"input.md\"\noutput_dir: \"dist\"\n",
        )
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
}
