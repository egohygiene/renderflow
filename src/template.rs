use anyhow::{Context, Result};
use tera::Tera;

/// Initialise a Tera template engine and load all `*.html` files found under
/// `template_dir`.
///
/// If the directory does not exist or contains no matching files, the function
/// still succeeds and returns an empty Tera instance so that the rest of the
/// pipeline can continue without templates.  An error is only returned when
/// Tera encounters an invalid glob pattern or a template that fails to parse.
pub fn init_tera(template_dir: &str) -> Result<Tera> {
    let glob = format!("{}/**/*.html", template_dir);
    let tera = Tera::new(&glob)
        .with_context(|| format!("Failed to initialise Tera from template directory: {}", template_dir))?;
    Ok(tera)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_template(dir: &TempDir, name: &str, content: &str) {
        let path = dir.path().join(name);
        fs::write(path, content).expect("failed to write template");
    }

    #[test]
    fn test_init_tera_with_valid_template_dir() {
        let dir = TempDir::new().unwrap();
        write_template(&dir, "default.html", "<html>{{ body }}</html>");
        let tera = init_tera(dir.path().to_str().unwrap());
        assert!(tera.is_ok(), "expected Tera to initialise successfully");
        let tera = tera.unwrap();
        assert!(
            tera.get_template_names().any(|n| n.contains("default.html")),
            "expected 'default.html' to be loaded"
        );
    }

    #[test]
    fn test_init_tera_with_empty_template_dir() {
        let dir = TempDir::new().unwrap();
        let tera = init_tera(dir.path().to_str().unwrap());
        assert!(tera.is_ok(), "expected Tera to initialise with no templates");
    }

    #[test]
    fn test_init_tera_with_nonexistent_dir() {
        let tera = init_tera("/nonexistent/template/dir");
        assert!(tera.is_ok(), "expected Tera to handle missing directory gracefully");
    }
}
