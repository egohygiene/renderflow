use anyhow::Result;

use crate::config::OutputType;
use crate::strategies::{HtmlStrategy, OutputStrategy, PdfStrategy};

/// Select an output strategy based on the given output type.
///
/// The optional `template` name and `template_dir` are forwarded to the chosen
/// strategy so that it can locate the correct template file when rendering.
/// When `template` is `None` the strategy falls back to default pandoc behaviour.
pub fn select_strategy(
    output_type: OutputType,
    template: Option<String>,
    template_dir: String,
) -> Result<Box<dyn OutputStrategy>> {
    match output_type {
        OutputType::Html => Ok(Box::new(HtmlStrategy::new(template, template_dir))),
        OutputType::Pdf => Ok(Box::new(PdfStrategy::new(template, template_dir))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let strategy = select_strategy(OutputType::Html, None, "templates".to_string()).unwrap();
        let result = strategy.render("/nonexistent/input.md", "/tmp/output.html");
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
}
