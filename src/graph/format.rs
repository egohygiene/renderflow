use std::fmt;

/// Represents all document formats that can participate in transformation edges.
///
/// Formats are modelled as graph nodes: each variant is a unique node in the
/// [`TransformGraph`](super::TransformGraph).  An edge between two nodes
/// indicates that a [`TransformEdge`](super::TransformEdge) exists that can
/// convert from the source format to the target format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Markdown,
    Html,
    Pdf,
    Docx,
    Epub,
    Rst,
    Latex,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Format::Markdown => "markdown",
            Format::Html => "html",
            Format::Pdf => "pdf",
            Format::Docx => "docx",
            Format::Epub => "epub",
            Format::Rst => "rst",
            Format::Latex => "latex",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_markdown() {
        assert_eq!(Format::Markdown.to_string(), "markdown");
    }

    #[test]
    fn test_display_all_variants() {
        assert_eq!(Format::Html.to_string(), "html");
        assert_eq!(Format::Pdf.to_string(), "pdf");
        assert_eq!(Format::Docx.to_string(), "docx");
        assert_eq!(Format::Epub.to_string(), "epub");
        assert_eq!(Format::Rst.to_string(), "rst");
        assert_eq!(Format::Latex.to_string(), "latex");
    }

    #[test]
    fn test_format_equality() {
        assert_eq!(Format::Markdown, Format::Markdown);
        assert_ne!(Format::Markdown, Format::Html);
    }

    #[test]
    fn test_format_clone_copy() {
        let f = Format::Pdf;
        let g = f;
        assert_eq!(f, g);
    }

    #[test]
    fn test_format_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Format::Markdown);
        set.insert(Format::Markdown);
        set.insert(Format::Html);
        assert_eq!(set.len(), 2);
    }
}
