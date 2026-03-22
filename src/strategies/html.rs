use anyhow::Result;
use tracing::info;

use crate::adapters::command::run_command;
use crate::strategies::OutputStrategy;

/// Renders a document to HTML format using pandoc.
pub struct HtmlStrategy;

impl OutputStrategy for HtmlStrategy {
    fn render(&self, input: &str, output_path: &str) -> Result<()> {
        info!(input = %input, output = %output_path, "Rendering HTML via pandoc");
        run_command("pandoc", &[input, "-o", output_path])?;
        info!(output = %output_path, "HTML rendering completed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_strategy_errors_on_missing_input() {
        let strategy = HtmlStrategy;
        let result = strategy.render("/nonexistent/input.md", "/tmp/output.html");
        assert!(result.is_err());
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

        let strategy = HtmlStrategy;
        let result = strategy.render(
            input.path().to_str().unwrap(),
            output_path.to_str().unwrap(),
        );
        assert!(result.is_ok());
        assert!(output_path.exists());
    }
}
