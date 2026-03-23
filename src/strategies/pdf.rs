use anyhow::{Context, Result};
use std::io::ErrorKind;
use tracing::info;

use crate::adapters::command::run_command;
use crate::strategies::OutputStrategy;

/// Renders a document to PDF format using pandoc with the tectonic PDF engine.
pub struct PdfStrategy {
    pub template: Option<String>,
}

impl PdfStrategy {
    pub fn new(template: Option<String>) -> Self {
        Self { template }
    }

    /// Returns an error if the tectonic PDF engine is not installed.
    fn check_tectonic() -> Result<()> {
        match std::process::Command::new("tectonic")
            .arg("--version")
            .output()
        {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                anyhow::bail!(
                    "PDF rendering failed: `tectonic` is not installed.\n\n\
                     Fix:\n\
                     - Install tectonic: https://tectonic-typesetting.github.io/en-US/\n\
                     - Or configure a different PDF engine"
                );
            }
            _ => Ok(()),
        }
    }
}

impl OutputStrategy for PdfStrategy {
    fn render(&self, input: &str, output_path: &str) -> Result<()> {
        info!(input = %input, output = %output_path, template = ?self.template, "Rendering PDF via pandoc");

        Self::check_tectonic()?;

        run_command(
            "pandoc",
            &[
                "--from",
                "markdown",
                input,
                "-o",
                output_path,
                "--pdf-engine=tectonic",
            ],
        )
        .with_context(|| format!(
            "Failed to render PDF output '{}'. \
             Check that pandoc and tectonic are installed (`pandoc --version`, `tectonic --version`) \
             and that the input file '{}' is valid Markdown.",
            output_path, input
        ))?;
        info!(output = %output_path, "PDF rendering completed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns `true` if the `tectonic` binary is available in PATH.
    fn tectonic_available() -> bool {
        std::process::Command::new("tectonic")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_pdf_strategy_errors_on_missing_input() {
        let strategy = PdfStrategy::new(None);
        let result = strategy.render("/nonexistent/input.md", "/tmp/output.pdf");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        // The error is either a missing-tectonic error or a pandoc render error.
        assert!(
            msg.contains("tectonic") || msg.contains("Failed to render PDF output"),
            "error should describe what failed: {}",
            msg
        );
    }

    #[test]
    fn test_check_tectonic_returns_clear_error_when_missing() {
        if tectonic_available() {
            // Nothing to test when tectonic is present.
            return;
        }
        let result = PdfStrategy::check_tectonic();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("tectonic") && msg.contains("not installed"),
            "error should explain that tectonic is not installed: {}",
            msg
        );
    }

    #[test]
    fn test_pdf_strategy_stores_template() {
        let strategy = PdfStrategy::new(Some("default".to_string()));
        assert_eq!(strategy.template, Some("default".to_string()));
    }

    #[test]
    #[ignore = "requires pandoc and tectonic to be installed"]
    fn test_pdf_strategy_produces_output() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, "# Hello\n\nThis is a test.").unwrap();

        let output = NamedTempFile::new().unwrap();
        let output_path = output.path().with_extension("pdf");

        let strategy = PdfStrategy::new(None);
        let result = strategy.render(
            input.path().to_str().unwrap(),
            output_path.to_str().unwrap(),
        );
        assert!(result.is_ok());
        assert!(output_path.exists());
    }
}
