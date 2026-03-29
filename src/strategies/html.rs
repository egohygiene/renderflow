use anyhow::{Context, Result};
use std::path::Path;
use tracing::info;

use crate::adapters::command::run_command;
use crate::strategies::{OutputStrategy, PandocArgs, RenderContext};

/// Renders a document to HTML format using pandoc.
pub struct HtmlStrategy {
    pub template: Option<String>,
    pub template_dir: String,
}

impl HtmlStrategy {
    pub fn new(template: Option<String>, template_dir: String) -> Self {
        Self { template, template_dir }
    }
}

impl OutputStrategy for HtmlStrategy {
    fn render(&self, ctx: &RenderContext) -> Result<()> {
        info!(input = %ctx.input_path, output = %ctx.output_path, template = ?self.template, "Rendering HTML via pandoc");

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

        let builder = PandocArgs::new(ctx.input_format.as_pandoc_format(), ctx.input_path, ctx.output_path);
        let args = match template_path {
            Some(ref path) => builder.with_template(path.as_str()),
            None => builder,
        }
        .build();
        let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        run_command("pandoc", &args_refs)
            .with_context(|| format!(
                "Failed to render HTML output '{}'. \
                 Check that pandoc is installed (`pandoc --version`) and that the input file '{}' is valid Markdown.",
                ctx.output_path, ctx.input_path
            ))?;
        info!(output = %ctx.output_path, "HTML rendering completed successfully");
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

    #[test]
    fn test_html_strategy_errors_on_missing_input() {
        let vars = HashMap::new();
        let strategy = HtmlStrategy::new(None, "templates".to_string());
        let ctx = default_ctx("/nonexistent/input.md", "/tmp/output.html", &vars);
        let result = strategy.render(&ctx);
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
        let strategy = HtmlStrategy::new(Some("default.html".to_string()), "templates".to_string());
        assert_eq!(strategy.template, Some("default.html".to_string()));
    }

    #[test]
    fn test_html_strategy_missing_template_returns_clear_error() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, "# Hello\n\nThis is a test.").unwrap();

        let vars = HashMap::new();
        let strategy = HtmlStrategy::new(
            Some("nonexistent.html".to_string()),
            "/nonexistent/template/dir".to_string(),
        );
        let ctx = RenderContext {
            input_path: input.path().to_str().unwrap(),
            input_format: InputFormat::Markdown,
            output_path: "/tmp/output.html",
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Template file not found"),
            "error should mention missing template, got: {}",
            msg
        );
    }

    #[test]
    fn test_html_strategy_no_template_does_not_check_template_dir() {
        // When no template is configured the template_dir is never accessed,
        // so a non-existent directory must not cause an error at construction time.
        let strategy = HtmlStrategy::new(None, "/nonexistent/dir".to_string());
        assert!(strategy.template.is_none());
    }

    #[test]
    fn test_html_strategy_context_carries_input_format() {
        let vars = HashMap::new();
        let ctx = RenderContext {
            input_path: "input.html",
            input_format: InputFormat::Html,
            output_path: "/tmp/output.html",
            variables: &vars,
            dry_run: false,
        };
        assert_eq!(ctx.input_format, InputFormat::Html);
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

        let vars = HashMap::new();
        let strategy = HtmlStrategy::new(None, "templates".to_string());
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

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_html_strategy_with_template_produces_output() {
        use std::fs;
        use std::io::Write;
        use tempfile::{NamedTempFile, TempDir};

        let template_dir = TempDir::new().unwrap();
        let template_path = template_dir.path().join("custom.html");
        // Use pandoc template syntax ($body$) rather than Tera syntax.
        fs::write(&template_path, "$body$").unwrap();

        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, "# Hello\n\nThis is a test.").unwrap();

        let output = NamedTempFile::new().unwrap();
        let output_path = output.path().with_extension("html");

        let vars = HashMap::new();
        let strategy = HtmlStrategy::new(
            Some("custom.html".to_string()),
            template_dir.path().to_str().unwrap().to_string(),
        );
        let ctx = RenderContext {
            input_path: input.path().to_str().unwrap(),
            input_format: InputFormat::Markdown,
            output_path: output_path.to_str().unwrap(),
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_ok(), "expected render to succeed with a valid template: {:?}", result);
        assert!(output_path.exists());
    }
}
