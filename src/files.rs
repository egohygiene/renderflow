use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Validates that the input file exists and returns its canonical path.
pub fn validate_input(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    if !p.exists() {
        anyhow::bail!("Input file not found: {}", path);
    }
    let canonical = fs::canonicalize(p)
        .with_context(|| format!("Failed to resolve input path: {}", path))?;
    info!(input = %canonical.display(), "Validated input file");
    Ok(canonical)
}

/// Ensures the output directory exists, creating it if necessary, and returns its canonical path.
pub fn ensure_output_dir(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    fs::create_dir_all(p)
        .with_context(|| format!("Failed to create output directory: {}", path))?;
    let canonical = fs::canonicalize(p)
        .with_context(|| format!("Failed to resolve output directory path: {}", path))?;
    info!(output_dir = %canonical.display(), "Ensured output directory exists");
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_validate_input_success() {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(b"hello").expect("failed to write");
        let result = validate_input(f.path().to_str().unwrap());
        assert!(result.is_ok(), "expected Ok for existing file");
        let path = result.unwrap();
        assert!(path.is_absolute(), "canonical path should be absolute");
        assert!(path.exists(), "canonical path should point to an existing file");
    }

    #[test]
    fn test_validate_input_missing_file() {
        let result = validate_input("/nonexistent/path/input.md");
        assert!(result.is_err(), "expected error for missing file");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Input file not found"), "unexpected error: {}", msg);
    }

    #[test]
    fn test_ensure_output_dir_creates_directory() {
        let base = TempDir::new().expect("failed to create temp dir");
        let output = base.path().join("nested").join("output");
        let result = ensure_output_dir(output.to_str().unwrap());
        assert!(result.is_ok(), "expected Ok when creating nested directory");
        assert!(output.exists(), "output directory should have been created");
        assert!(output.is_dir(), "output path should be a directory");
    }

    #[test]
    fn test_ensure_output_dir_existing_directory() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let result = ensure_output_dir(dir.path().to_str().unwrap());
        assert!(result.is_ok(), "expected Ok for already-existing directory");
    }

    #[test]
    fn test_ensure_output_dir_returns_canonical_path() {
        let base = TempDir::new().expect("failed to create temp dir");
        let output = base.path().join("dist");
        let result = ensure_output_dir(output.to_str().unwrap());
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert!(canonical.is_absolute(), "returned path should be absolute");
    }
}
