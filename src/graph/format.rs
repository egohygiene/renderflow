use std::fmt;
use std::str::FromStr;

use anyhow::Result;

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
    /// Fountain screenplay plain-text format.
    Fountain,
    /// JPEG raster image format.
    Jpeg,
    /// PNG raster image format.
    Png,
    /// TIFF raster image format, commonly used in print/press workflows.
    Tiff,
    /// Comic Book ZIP archive (`.cbz`).
    Cbz,
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
            Format::Fountain => "fountain",
            Format::Jpeg => "jpeg",
            Format::Png => "png",
            Format::Tiff => "tiff",
            Format::Cbz => "cbz",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for Format {
    type Err = anyhow::Error;

    /// Parse a [`Format`] from a case-insensitive string.
    ///
    /// Accepted values: `markdown` / `md`, `html`, `pdf`, `docx`, `epub`,
    /// `rst`, `latex` / `tex`, `fountain`, `jpeg` / `jpg`, `png`, `tiff`,
    /// `cbz`.
    ///
    /// Returns an error that lists all supported formats when the string is
    /// unrecognised.
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Ok(Format::Markdown),
            "html" => Ok(Format::Html),
            "pdf" => Ok(Format::Pdf),
            "docx" => Ok(Format::Docx),
            "epub" => Ok(Format::Epub),
            "rst" => Ok(Format::Rst),
            "latex" | "tex" => Ok(Format::Latex),
            "fountain" => Ok(Format::Fountain),
            "jpeg" | "jpg" => Ok(Format::Jpeg),
            "png" => Ok(Format::Png),
            "tiff" | "tif" => Ok(Format::Tiff),
            "cbz" => Ok(Format::Cbz),
            _ => anyhow::bail!(
                "'{}' is not a known format. Supported formats are: \
                 markdown, html, pdf, docx, epub, rst, latex, fountain, \
                 jpeg, png, tiff, cbz",
                s
            ),
        }
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

    // ── FromStr tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_from_str_markdown() {
        assert_eq!("markdown".parse::<Format>().unwrap(), Format::Markdown);
        assert_eq!("md".parse::<Format>().unwrap(), Format::Markdown);
        assert_eq!("Markdown".parse::<Format>().unwrap(), Format::Markdown);
    }

    #[test]
    fn test_from_str_html() {
        assert_eq!("html".parse::<Format>().unwrap(), Format::Html);
        assert_eq!("HTML".parse::<Format>().unwrap(), Format::Html);
    }

    #[test]
    fn test_from_str_pdf() {
        assert_eq!("pdf".parse::<Format>().unwrap(), Format::Pdf);
    }

    #[test]
    fn test_from_str_docx() {
        assert_eq!("docx".parse::<Format>().unwrap(), Format::Docx);
    }

    #[test]
    fn test_from_str_epub() {
        assert_eq!("epub".parse::<Format>().unwrap(), Format::Epub);
    }

    #[test]
    fn test_from_str_rst() {
        assert_eq!("rst".parse::<Format>().unwrap(), Format::Rst);
    }

    #[test]
    fn test_from_str_latex() {
        assert_eq!("latex".parse::<Format>().unwrap(), Format::Latex);
        assert_eq!("tex".parse::<Format>().unwrap(), Format::Latex);
    }

    #[test]
    fn test_from_str_unknown_returns_error() {
        let err = "xyz-unknown".parse::<Format>().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("'xyz-unknown' is not a known format"), "unexpected: {msg}");
        assert!(msg.contains("markdown"), "expected format list: {msg}");
    }

    // ── image / CBZ format tests ──────────────────────────────────────────────

    #[test]
    fn test_from_str_jpeg() {
        assert_eq!("jpeg".parse::<Format>().unwrap(), Format::Jpeg);
        assert_eq!("jpg".parse::<Format>().unwrap(), Format::Jpeg);
        assert_eq!("JPEG".parse::<Format>().unwrap(), Format::Jpeg);
    }

    #[test]
    fn test_from_str_png() {
        assert_eq!("png".parse::<Format>().unwrap(), Format::Png);
        assert_eq!("PNG".parse::<Format>().unwrap(), Format::Png);
    }

    #[test]
    fn test_from_str_tiff() {
        assert_eq!("tiff".parse::<Format>().unwrap(), Format::Tiff);
        assert_eq!("tif".parse::<Format>().unwrap(), Format::Tiff);
        assert_eq!("TIFF".parse::<Format>().unwrap(), Format::Tiff);
    }

    #[test]
    fn test_from_str_cbz() {
        assert_eq!("cbz".parse::<Format>().unwrap(), Format::Cbz);
        assert_eq!("CBZ".parse::<Format>().unwrap(), Format::Cbz);
    }

    #[test]
    fn test_display_jpeg() {
        assert_eq!(Format::Jpeg.to_string(), "jpeg");
    }

    #[test]
    fn test_display_png() {
        assert_eq!(Format::Png.to_string(), "png");
    }

    #[test]
    fn test_display_tiff() {
        assert_eq!(Format::Tiff.to_string(), "tiff");
    }

    #[test]
    fn test_display_cbz() {
        assert_eq!(Format::Cbz.to_string(), "cbz");
    }
}
