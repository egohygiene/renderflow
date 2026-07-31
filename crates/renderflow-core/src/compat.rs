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

/// Return the output types that are supported for a given input format.
pub fn supported_outputs_for(input: &InputFormat) -> Vec<OutputType> {
    [OutputType::Html, OutputType::Pdf, OutputType::Docx]
        .into_iter()
        .filter(|o| is_supported(input, o))
        .collect()
}

/// Build a user-facing error message for an unsupported input → output pair.
///
/// The message names the invalid combination and lists the outputs that *are*
/// supported for the given input format so the user knows what to use instead.
pub fn unsupported_combination_message(input: &InputFormat, output: &OutputType) -> String {
    let supported: Vec<String> = supported_outputs_for(input)
        .iter()
        .map(|o| o.to_string())
        .collect();
    let alternatives = if supported.is_empty() {
        "none".to_string()
    } else {
        supported.join(", ")
    };
    format!(
        "Input format '{}' → output '{}' is not supported yet. \
         Supported outputs for '{}' are: {}",
        input, output, input, alternatives
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

    // --- supported_outputs_for ---

    #[test]
    fn test_supported_outputs_for_epub() {
        let supported = supported_outputs_for(&InputFormat::Epub);
        assert!(supported.contains(&OutputType::Html));
        assert!(supported.contains(&OutputType::Pdf));
        assert!(!supported.contains(&OutputType::Docx));
    }

    #[test]
    fn test_supported_outputs_for_markdown_includes_all() {
        let supported = supported_outputs_for(&InputFormat::Markdown);
        assert!(supported.contains(&OutputType::Html));
        assert!(supported.contains(&OutputType::Pdf));
        assert!(supported.contains(&OutputType::Docx));
    }

    #[test]
    fn test_supported_outputs_for_docx_excludes_docx() {
        let supported = supported_outputs_for(&InputFormat::Docx);
        assert!(supported.contains(&OutputType::Html));
        assert!(supported.contains(&OutputType::Pdf));
        assert!(!supported.contains(&OutputType::Docx));
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

    #[test]
    fn test_error_message_includes_supported_alternatives() {
        let msg = unsupported_combination_message(&InputFormat::Epub, &OutputType::Docx);
        assert!(
            msg.contains("html"),
            "message should list 'html' as a supported alternative: {msg}"
        );
        assert!(
            msg.contains("pdf"),
            "message should list 'pdf' as a supported alternative: {msg}"
        );
    }

    #[test]
    fn test_error_message_latex_to_docx_includes_alternatives() {
        let msg = unsupported_combination_message(&InputFormat::Latex, &OutputType::Docx);
        assert!(
            msg.contains("latex"),
            "message should contain input format: {msg}"
        );
        assert!(
            msg.contains("html"),
            "message should list 'html' as a supported alternative: {msg}"
        );
        assert!(
            msg.contains("pdf"),
            "message should list 'pdf' as a supported alternative: {msg}"
        );
    }
}
