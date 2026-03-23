use anyhow::{anyhow, Result};

use crate::strategies::{HtmlStrategy, OutputStrategy, PdfStrategy};

/// Select an output strategy based on the given format name.
///
/// The optional `template` name is forwarded to the chosen strategy so that it
/// can reference the correct Tera template when rendering.
///
/// # Errors
///
/// Returns an error if the format is not supported.
pub fn select_strategy(format: &str, template: Option<String>) -> Result<Box<dyn OutputStrategy>> {
    match format {
        "html" => Ok(Box::new(HtmlStrategy::new(template))),
        "pdf" => Ok(Box::new(PdfStrategy::new(template))),
        other => Err(anyhow!("Unsupported output format: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_strategy_html() {
        let result = select_strategy("html", None);
        assert!(result.is_ok(), "expected html strategy to be selected");
    }

    #[test]
    fn test_select_strategy_pdf() {
        let result = select_strategy("pdf", None);
        assert!(result.is_ok(), "expected pdf strategy to be selected");
    }

    #[test]
    fn test_select_strategy_unsupported() {
        let result = select_strategy("docx", None);
        assert!(result.is_err(), "expected error for unsupported format");
        let msg = format!("{}", result.err().unwrap());
        assert!(
            msg.contains("Unsupported output format"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    fn test_select_strategy_html_renders_error_on_missing_input() {
        let strategy = select_strategy("html", None).unwrap();
        let result = strategy.render("/nonexistent/input.md", "/tmp/output.html");
        assert!(result.is_err());
    }

    #[test]
    fn test_select_strategy_passes_template_to_strategy() {
        let strategy = select_strategy("html", Some("default".to_string()));
        assert!(strategy.is_ok());
    }
}
