use anyhow::{Context, Result};
use tracing::info;

use crate::adapters::command::run_command;
use crate::strategies::OutputStrategy;

/// Renders a document to HTML format using pandoc.
pub struct HtmlStrategy {
    pub template: Option<String>,
}

impl HtmlStrategy {
    pub fn new(template: Option<String>) -> Self {
        Self { template }
    }
}

impl OutputStrategy for HtmlStrategy {
    fn render(&self, input: &str, output_path: &str) -> Result<()> {
        info!(input = %input, output = %output_path, template = ?self.template, "Rendering HTML via pandoc");
        run_command("pandoc", &["--from", "markdown", input, "-o", output_path])
            .with_context(|| format!(
                "Failed to render HTML output '{}'. \
                 Check that pandoc is installed (`pandoc --version`) and that the input file '{}' is valid Markdown.",
                output_path, input
            ))?;
        info!(output = %output_path, "HTML rendering completed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_strategy_errors_on_missing_input() {
        let strategy = HtmlStrategy::new(None);
        let result = strategy.render("/nonexistent/input.md", "/tmp/output.html");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("Failed to render HTML output"),
            "error should describe what failed: {}",
            msg
        );
    }

    #[test]
    fn test_html_strategy_stores_template() {
        let strategy = HtmlStrategy::new(Some("default".to_string()));
        assert_eq!(strategy.template, Some("default".to_string()));
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_html_strategy_produces_output() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, "# Hello\n\nThis is a test.").unwrap();

        let output = NamedTempFile::new().unwrap();
        let output_path = output.path().with_extension("html");

        let strategy = HtmlStrategy::new(None);
        let result = strategy.render(
            input.path().to_str().unwrap(),
            output_path.to_str().unwrap(),
        );
        assert!(result.is_ok());
        assert!(output_path.exists());
    }
}
