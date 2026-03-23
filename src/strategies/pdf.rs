use anyhow::{Context, Result};
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
}

impl OutputStrategy for PdfStrategy {
    fn render(&self, input: &str, output_path: &str) -> Result<()> {
        info!(input = %input, output = %output_path, template = ?self.template, "Rendering PDF via pandoc");
        run_command(
            "pandoc",
            &[input, "-o", output_path, "--pdf-engine=tectonic"],
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

    #[test]
    fn test_pdf_strategy_errors_on_missing_input() {
        let strategy = PdfStrategy::new(None);
        let result = strategy.render("/nonexistent/input.md", "/tmp/output.pdf");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("Failed to render PDF output"),
            "error should describe what failed: {}",
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
