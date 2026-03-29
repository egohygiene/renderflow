use anyhow::Result;

use crate::config::{unsupported_type_message, OutputType};
use crate::strategies::{DocxStrategy, HtmlStrategy, OutputStrategy, PdfStrategy};

/// Select an output strategy based on the given output type.
///
/// The optional `template` name and `template_dir` are forwarded to the chosen
/// strategy so that it can locate the correct template file when rendering.
/// When `template` is `None` the strategy falls back to default pandoc behaviour.
pub fn select_strategy(
    output_type: OutputType,
    template: Option<String>,
    template_dir: String,
) -> Result<Box<dyn OutputStrategy + Send + Sync>> {
    match output_type {
        OutputType::Html => Ok(Box::new(HtmlStrategy::new(template, template_dir))),
        OutputType::Pdf => Ok(Box::new(PdfStrategy::new(template, template_dir))),
        OutputType::Docx => Ok(Box::new(DocxStrategy::new(template, template_dir))),
        OutputType::Unsupported(ref t) => {
            anyhow::bail!("{}", unsupported_type_message(t))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::input_format::InputFormat;
    use crate::strategies::RenderContext;

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
    fn test_select_strategy_html() {
        let result = select_strategy(OutputType::Html, None, "templates".to_string());
        assert!(result.is_ok(), "expected html strategy to be selected");
    }

    #[test]
    fn test_select_strategy_pdf() {
        let result = select_strategy(OutputType::Pdf, None, "templates".to_string());
        assert!(result.is_ok(), "expected pdf strategy to be selected");
    }

    #[test]
    fn test_select_strategy_html_renders_error_on_missing_input() {
        let vars = HashMap::new();
        let strategy = select_strategy(OutputType::Html, None, "templates".to_string()).unwrap();
        let ctx = default_ctx("/nonexistent/input.md", "/tmp/output.html", &vars);
        let result = strategy.render(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_strategy_passes_template_to_strategy() {
        let strategy = select_strategy(
            OutputType::Html,
            Some("default.html".to_string()),
            "templates".to_string(),
        );
        assert!(strategy.is_ok());
    }

    #[test]
    fn test_select_strategy_docx() {
        let result = select_strategy(OutputType::Docx, None, "templates".to_string());
        assert!(result.is_ok(), "expected docx strategy to be selected");
    }

    #[test]
    fn test_select_strategy_unsupported_type_returns_error() {
        // A truly unknown type must return a clear error from select_strategy.
        let result = select_strategy(
            OutputType::Unsupported("epub".to_string()),
            None,
            "templates".to_string(),
        );
        assert!(result.is_err());
        let msg = format!("{}", result.err().expect("expected an error"));
        assert!(
            msg.contains("not a valid output type"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    fn test_select_strategy_truly_invalid_type_returns_error() {
        let result = select_strategy(
            OutputType::Unsupported("jpeg".to_string()),
            None,
            "templates".to_string(),
        );
        assert!(result.is_err());
        let msg = format!("{}", result.err().expect("expected an error"));
        assert!(
            msg.contains("not a valid output type"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    fn test_select_strategy_html_with_non_markdown_input_format() {
        let vars = HashMap::new();
        let strategy = select_strategy(OutputType::Html, None, "templates".to_string()).unwrap();
        let ctx = RenderContext {
            input_path: "/nonexistent/input.html",
            input_format: InputFormat::Html,
            output_path: "/tmp/output.html",
            variables: &vars,
            dry_run: false,
        };
        // Strategy can be created and render can be attempted (will fail due to missing file)
        let result = strategy.render(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_strategy_docx_with_html_input_format() {
        let vars = HashMap::new();
        let strategy = select_strategy(OutputType::Docx, None, "templates".to_string()).unwrap();
        let ctx = RenderContext {
            input_path: "/nonexistent/input.html",
            input_format: InputFormat::Html,
            output_path: "/tmp/output.docx",
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_strategy_pdf_with_rst_input_format() {
        let vars = HashMap::new();
        let strategy = select_strategy(OutputType::Pdf, None, "templates".to_string()).unwrap();
        let ctx = RenderContext {
            input_path: "/nonexistent/input.rst",
            input_format: InputFormat::Rst,
            output_path: "/tmp/output.pdf",
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_err());
    }
}
