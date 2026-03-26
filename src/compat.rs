use crate::config::OutputType;
use crate::input_format::InputFormat;

/// Return `true` when `input` → `output` is a supported conversion.
///
/// The matrix is intentionally conservative: only combinations that pandoc
/// is known to handle well are listed.  The list will grow as more paths are
/// tested and validated.
///
/// ```text
/// Markdown  → html, pdf, docx
/// Rst       → html, pdf, docx
/// Html      → html, pdf, docx
/// Docx      → html, pdf
/// Epub      → html, pdf          (epub → docx is not yet supported)
/// Latex     → html, pdf          (latex → docx is not yet supported)
/// ```
pub fn is_supported(input: &InputFormat, output: &OutputType) -> bool {
    match (input, output) {
        // Markdown
        (InputFormat::Markdown, OutputType::Html)
        | (InputFormat::Markdown, OutputType::Pdf)
        | (InputFormat::Markdown, OutputType::Docx) => true,

        // reStructuredText
        (InputFormat::Rst, OutputType::Html)
        | (InputFormat::Rst, OutputType::Pdf)
        | (InputFormat::Rst, OutputType::Docx) => true,

        // HTML (pass-through or convert)
        (InputFormat::Html, OutputType::Html)
        | (InputFormat::Html, OutputType::Pdf)
        | (InputFormat::Html, OutputType::Docx) => true,

        // Docx (reading a Word document)
        (InputFormat::Docx, OutputType::Html) | (InputFormat::Docx, OutputType::Pdf) => true,

        // Epub
        (InputFormat::Epub, OutputType::Html) | (InputFormat::Epub, OutputType::Pdf) => true,

        // LaTeX
        (InputFormat::Latex, OutputType::Html) | (InputFormat::Latex, OutputType::Pdf) => true,

        // Everything else — including Unsupported output types — is not supported.
        _ => false,
    }
}

/// Build a user-facing error message for an unsupported input → output pair.
pub fn unsupported_combination_message(input: &InputFormat, output: &OutputType) -> String {
    format!(
        "Input format '{}' → output '{}' is not supported yet.",
        input, output
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_supported: valid combinations ---

    #[test]
    fn test_markdown_to_html_is_supported() {
        assert!(is_supported(&InputFormat::Markdown, &OutputType::Html));
    }

    #[test]
    fn test_markdown_to_pdf_is_supported() {
        assert!(is_supported(&InputFormat::Markdown, &OutputType::Pdf));
    }

    #[test]
    fn test_markdown_to_docx_is_supported() {
        assert!(is_supported(&InputFormat::Markdown, &OutputType::Docx));
    }

    #[test]
    fn test_rst_to_html_is_supported() {
        assert!(is_supported(&InputFormat::Rst, &OutputType::Html));
    }

    #[test]
    fn test_rst_to_pdf_is_supported() {
        assert!(is_supported(&InputFormat::Rst, &OutputType::Pdf));
    }

    #[test]
    fn test_rst_to_docx_is_supported() {
        assert!(is_supported(&InputFormat::Rst, &OutputType::Docx));
    }

    #[test]
    fn test_html_to_pdf_is_supported() {
        assert!(is_supported(&InputFormat::Html, &OutputType::Pdf));
    }

    #[test]
    fn test_html_to_docx_is_supported() {
        assert!(is_supported(&InputFormat::Html, &OutputType::Docx));
    }

    #[test]
    fn test_docx_to_html_is_supported() {
        assert!(is_supported(&InputFormat::Docx, &OutputType::Html));
    }

    #[test]
    fn test_docx_to_pdf_is_supported() {
        assert!(is_supported(&InputFormat::Docx, &OutputType::Pdf));
    }

    #[test]
    fn test_epub_to_html_is_supported() {
        assert!(is_supported(&InputFormat::Epub, &OutputType::Html));
    }

    #[test]
    fn test_epub_to_pdf_is_supported() {
        assert!(is_supported(&InputFormat::Epub, &OutputType::Pdf));
    }

    #[test]
    fn test_latex_to_html_is_supported() {
        assert!(is_supported(&InputFormat::Latex, &OutputType::Html));
    }

    #[test]
    fn test_latex_to_pdf_is_supported() {
        assert!(is_supported(&InputFormat::Latex, &OutputType::Pdf));
    }

    // --- is_supported: invalid combinations ---

    #[test]
    fn test_epub_to_docx_is_not_supported() {
        assert!(!is_supported(&InputFormat::Epub, &OutputType::Docx));
    }

    #[test]
    fn test_docx_to_docx_is_not_supported() {
        assert!(!is_supported(&InputFormat::Docx, &OutputType::Docx));
    }

    #[test]
    fn test_latex_to_docx_is_not_supported() {
        assert!(!is_supported(&InputFormat::Latex, &OutputType::Docx));
    }

    #[test]
    fn test_any_input_to_unsupported_output_is_not_supported() {
        assert!(!is_supported(
            &InputFormat::Markdown,
            &OutputType::Unsupported("jpeg".to_string())
        ));
    }

    // --- unsupported_combination_message ---

    #[test]
    fn test_error_message_format() {
        let msg = unsupported_combination_message(&InputFormat::Epub, &OutputType::Docx);
        assert!(
            msg.contains("epub"),
            "message should contain input format: {msg}"
        );
        assert!(
            msg.contains("docx"),
            "message should contain output type: {msg}"
        );
        assert!(
            msg.contains("not supported yet"),
            "message should say 'not supported yet': {msg}"
        );
    }
}
