use anyhow::{Context, Result};
use std::io::ErrorKind;
use std::path::Path;
use tracing::info;

use crate::adapters::command::run_command;
use crate::strategies::{OutputStrategy, PandocArgs, RenderContext};

/// Renders a document to PDF format using pandoc with the tectonic PDF engine.
pub struct PdfStrategy {
    pub template: Option<String>,
    pub template_dir: String,
}

impl PdfStrategy {
    pub fn new(template: Option<String>, template_dir: String) -> Self {
        Self { template, template_dir }
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
    fn render(&self, ctx: &RenderContext) -> Result<()> {
        info!(input = %ctx.input_path, output = %ctx.output_path, template = ?self.template, "Rendering PDF via pandoc");

        Self::check_tectonic()?;

        // Resolve the optional template to a file path within the template directory.
        let template_path = if let Some(ref name) = self.template {
            let path = Path::new(&self.template_dir).join(name);
            if !path.exists() {
                anyhow::bail!(
                    "Template file not found: '{}'. \
                     Ensure the template exists in the configured template directory.",
                    path.display()
                );
            }
            info!("Using template: {}", name);
            Some(
                path.to_str()
                    .ok_or_else(|| anyhow::anyhow!(
                        "Template path '{}' contains invalid UTF-8",
                        path.display()
                    ))?
                    .to_owned(),
            )
        } else {
            None
        };

        let builder = PandocArgs::new(ctx.input_format.as_pandoc_format(), ctx.input_path, ctx.output_path)
            .with_pdf_engine("tectonic");
        let args = match template_path {
            Some(ref path) => builder.with_template(path.as_str()),
            None => builder,
        }
        .build();
        let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        run_command("pandoc", &args_refs)
        .with_context(|| format!(
            "Failed to render PDF output '{}'. \
             Check that pandoc and tectonic are installed (`pandoc --version`, `tectonic --version`) \
             and that the input file '{}' is valid Markdown.",
            ctx.output_path, ctx.input_path
        ))?;
        info!(output = %ctx.output_path, "PDF rendering completed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::input_format::InputFormat;

    fn default_ctx<'a>(input: &'a str, output: &'a str, vars: &'a HashMap<String, String>) -> RenderContext<'a> {
        RenderContext {
            input_path: input,
            input_format: InputFormat::Markdown,
            output_path: output,
            variables: vars,
            dry_run: false,
        }
    }

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
        let vars = HashMap::new();
        let strategy = PdfStrategy::new(None, "templates".to_string());
        let ctx = default_ctx("/nonexistent/input.md", "/tmp/output.pdf", &vars);
        let result = strategy.render(&ctx);
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
        let strategy = PdfStrategy::new(Some("default.html".to_string()), "templates".to_string());
        assert_eq!(strategy.template, Some("default.html".to_string()));
    }

    #[test]
    fn test_pdf_strategy_no_template_does_not_check_template_dir() {
        // When no template is configured the template_dir is never accessed,
        // so a non-existent directory must not cause an error at construction time.
        let strategy = PdfStrategy::new(None, "/nonexistent/dir".to_string());
        assert!(strategy.template.is_none());
    }

    #[test]
    fn test_pdf_strategy_context_carries_input_format() {
        let vars = HashMap::new();
        let ctx = RenderContext {
            input_path: "input.rst",
            input_format: InputFormat::Rst,
            output_path: "/tmp/output.pdf",
            variables: &vars,
            dry_run: false,
        };
        assert_eq!(ctx.input_format, InputFormat::Rst);
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

        let vars = HashMap::new();
        let strategy = PdfStrategy::new(None, "templates".to_string());
        let ctx = RenderContext {
            input_path: input.path().to_str().unwrap(),
            input_format: InputFormat::Markdown,
            output_path: output_path.to_str().unwrap(),
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_ok());
        assert!(output_path.exists());
    }
}
