use anyhow::{Context, Result};
use std::path::Path;
use tracing::info;

use crate::adapters::command::run_command;
use crate::input_format::InputFormat;
use crate::strategies::OutputStrategy;

/// Renders a document to DOCX (Microsoft Word) format using pandoc.
pub struct DocxStrategy {
    pub template: Option<String>,
    pub template_dir: String,
    pub input_format: InputFormat,
}

impl DocxStrategy {
    pub fn new(template: Option<String>, template_dir: String, input_format: InputFormat) -> Self {
        Self { template, template_dir, input_format }
    }
}

impl OutputStrategy for DocxStrategy {
    fn render(&self, input: &str, output_path: &str) -> Result<()> {
        info!(input = %input, output = %output_path, template = ?self.template, "Rendering DOCX via pandoc");

        // Resolve the optional template to a reference document path within the
        // template directory.  DOCX customisation is applied via pandoc's
        // `--reference-doc` flag rather than `--template`.
        let reference_doc = if let Some(ref name) = self.template {
            let path = Path::new(&self.template_dir).join(name);
            if !path.exists() {
                anyhow::bail!(
                    "Template file not found: '{}'. \
                     Ensure the template exists in the configured template directory.",
                    path.display()
                );
            }
            info!("Using reference doc: {}", name);
            Some(path.to_string_lossy().into_owned())
        } else {
            None
        };

        let mut args = vec!["--from", self.input_format.as_pandoc_format(), input, "-o", output_path];
        if let Some(ref path) = reference_doc {
            args.extend_from_slice(&["--reference-doc", path.as_str()]);
        }

        run_command("pandoc", &args)
            .with_context(|| format!(
                "Failed to render DOCX output '{}'. \
                 Check that pandoc is installed (`pandoc --version`) and that the input file '{}' is valid Markdown.",
                output_path, input
            ))?;
        info!(output = %output_path, "DOCX rendering completed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docx_strategy_errors_on_missing_input() {
        let strategy = DocxStrategy::new(None, "templates".to_string(), InputFormat::Markdown);
        let result = strategy.render("/nonexistent/input.md", "/tmp/output.docx");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("Failed to render DOCX output"),
            "error should describe what failed: {}",
            msg
        );
    }

    #[test]
    fn test_docx_strategy_stores_template() {
        let strategy = DocxStrategy::new(Some("reference.docx".to_string()), "templates".to_string(), InputFormat::Markdown);
        assert_eq!(strategy.template, Some("reference.docx".to_string()));
    }

    #[test]
    fn test_docx_strategy_no_template_does_not_check_template_dir() {
        // When no template is configured the template_dir is never accessed,
        // so a non-existent directory must not cause an error at construction time.
        let strategy = DocxStrategy::new(None, "/nonexistent/dir".to_string(), InputFormat::Markdown);
        assert!(strategy.template.is_none());
    }

    #[test]
    fn test_docx_strategy_missing_template_file_returns_error() {
        let strategy = DocxStrategy::new(
            Some("nonexistent.docx".to_string()),
            "/nonexistent/dir".to_string(),
            InputFormat::Markdown,
        );
        let result = strategy.render("/any/input.md", "/tmp/output.docx");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("Template file not found"),
            "error should describe missing template: {}",
            msg
        );
    }

    #[test]
    fn test_docx_strategy_stores_input_format() {
        let strategy = DocxStrategy::new(None, "templates".to_string(), InputFormat::Html);
        assert_eq!(strategy.input_format, InputFormat::Html);
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_docx_strategy_produces_output() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, "# Hello\n\nThis is a test.").unwrap();

        let output = NamedTempFile::new().unwrap();
        let output_path = output.path().with_extension("docx");

        let strategy = DocxStrategy::new(None, "templates".to_string(), InputFormat::Markdown);
        let result = strategy.render(
            input.path().to_str().unwrap(),
            output_path.to_str().unwrap(),
        );
        assert!(result.is_ok());
        assert!(output_path.exists());
    }
}
