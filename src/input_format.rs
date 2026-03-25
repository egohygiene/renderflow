use serde::{Deserialize, Deserializer};
use std::fmt;
use std::path::Path;

/// Represents a supported document input format.
///
/// Used to determine the `--from` flag passed to pandoc when rendering output.
/// The format can be specified explicitly in the config via `input_format`, or
/// auto-detected from the input file's extension.
#[derive(Debug, Clone, PartialEq)]
pub enum InputFormat {
    Markdown,
    Docx,
    Html,
    Epub,
    Rst,
    Latex,
}

impl InputFormat {
    /// Detect the input format from a file path's extension.
    ///
    /// Returns `None` when the extension is missing or unrecognised.
    ///
    /// ```text
    /// .md / .markdown → Markdown
    /// .docx           → Docx
    /// .html / .htm    → Html
    /// .epub           → Epub
    /// .rst            → Rst
    /// .tex            → Latex
    /// ```
    pub fn from_extension(path: &str) -> Option<Self> {
        let ext = Path::new(path).extension().and_then(|e| e.to_str())?;
        match ext.to_lowercase().as_str() {
            "md" | "markdown" => Some(InputFormat::Markdown),
            "docx" => Some(InputFormat::Docx),
            "html" | "htm" => Some(InputFormat::Html),
            "epub" => Some(InputFormat::Epub),
            "rst" => Some(InputFormat::Rst),
            "tex" => Some(InputFormat::Latex),
            _ => None,
        }
    }

    /// Return the pandoc `--from` format identifier for this input format.
    pub fn as_pandoc_format(&self) -> &str {
        match self {
            InputFormat::Markdown => "markdown",
            InputFormat::Docx => "docx",
            InputFormat::Html => "html",
            InputFormat::Epub => "epub",
            InputFormat::Rst => "rst",
            InputFormat::Latex => "latex",
        }
    }
}

impl Default for InputFormat {
    fn default() -> Self {
        InputFormat::Markdown
    }
}

impl fmt::Display for InputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_pandoc_format())
    }
}

impl<'de> Deserialize<'de> for InputFormat {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Ok(InputFormat::Markdown),
            "docx" => Ok(InputFormat::Docx),
            "html" | "htm" => Ok(InputFormat::Html),
            "epub" => Ok(InputFormat::Epub),
            "rst" => Ok(InputFormat::Rst),
            "latex" | "tex" => Ok(InputFormat::Latex),
            other => Err(serde::de::Error::custom(format!(
                "'{}' is not a supported input format. \
                 Supported formats are: markdown, docx, html, epub, rst, latex",
                other
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- from_extension ---

    #[test]
    fn test_from_extension_md() {
        assert_eq!(InputFormat::from_extension("doc.md"), Some(InputFormat::Markdown));
    }

    #[test]
    fn test_from_extension_markdown() {
        assert_eq!(InputFormat::from_extension("doc.markdown"), Some(InputFormat::Markdown));
    }

    #[test]
    fn test_from_extension_docx() {
        assert_eq!(InputFormat::from_extension("doc.docx"), Some(InputFormat::Docx));
    }

    #[test]
    fn test_from_extension_html() {
        assert_eq!(InputFormat::from_extension("doc.html"), Some(InputFormat::Html));
    }

    #[test]
    fn test_from_extension_htm() {
        assert_eq!(InputFormat::from_extension("doc.htm"), Some(InputFormat::Html));
    }

    #[test]
    fn test_from_extension_epub() {
        assert_eq!(InputFormat::from_extension("doc.epub"), Some(InputFormat::Epub));
    }

    #[test]
    fn test_from_extension_rst() {
        assert_eq!(InputFormat::from_extension("doc.rst"), Some(InputFormat::Rst));
    }

    #[test]
    fn test_from_extension_tex() {
        assert_eq!(InputFormat::from_extension("doc.tex"), Some(InputFormat::Latex));
    }

    #[test]
    fn test_from_extension_unknown_returns_none() {
        assert_eq!(InputFormat::from_extension("doc.xyz"), None);
    }

    #[test]
    fn test_from_extension_no_extension_returns_none() {
        assert_eq!(InputFormat::from_extension("README"), None);
    }

    #[test]
    fn test_from_extension_case_insensitive() {
        assert_eq!(InputFormat::from_extension("doc.MD"), Some(InputFormat::Markdown));
        assert_eq!(InputFormat::from_extension("doc.HTML"), Some(InputFormat::Html));
    }

    // --- as_pandoc_format ---

    #[test]
    fn test_as_pandoc_format_markdown() {
        assert_eq!(InputFormat::Markdown.as_pandoc_format(), "markdown");
    }

    #[test]
    fn test_as_pandoc_format_html() {
        assert_eq!(InputFormat::Html.as_pandoc_format(), "html");
    }

    #[test]
    fn test_as_pandoc_format_latex() {
        assert_eq!(InputFormat::Latex.as_pandoc_format(), "latex");
    }

    // --- Display ---

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", InputFormat::Markdown), "markdown");
        assert_eq!(format!("{}", InputFormat::Docx), "docx");
        assert_eq!(format!("{}", InputFormat::Rst), "rst");
    }

    // --- Default ---

    #[test]
    fn test_default_is_markdown() {
        assert_eq!(InputFormat::default(), InputFormat::Markdown);
    }

    // --- Deserialize ---

    #[test]
    fn test_deserialize_markdown() {
        let fmt: InputFormat = serde_yaml::from_str("markdown").unwrap();
        assert_eq!(fmt, InputFormat::Markdown);
    }

    #[test]
    fn test_deserialize_md_alias() {
        let fmt: InputFormat = serde_yaml::from_str("md").unwrap();
        assert_eq!(fmt, InputFormat::Markdown);
    }

    #[test]
    fn test_deserialize_html() {
        let fmt: InputFormat = serde_yaml::from_str("html").unwrap();
        assert_eq!(fmt, InputFormat::Html);
    }

    #[test]
    fn test_deserialize_epub() {
        let fmt: InputFormat = serde_yaml::from_str("epub").unwrap();
        assert_eq!(fmt, InputFormat::Epub);
    }

    #[test]
    fn test_deserialize_rst() {
        let fmt: InputFormat = serde_yaml::from_str("rst").unwrap();
        assert_eq!(fmt, InputFormat::Rst);
    }

    #[test]
    fn test_deserialize_latex() {
        let fmt: InputFormat = serde_yaml::from_str("latex").unwrap();
        assert_eq!(fmt, InputFormat::Latex);
    }

    #[test]
    fn test_deserialize_tex_alias() {
        let fmt: InputFormat = serde_yaml::from_str("tex").unwrap();
        assert_eq!(fmt, InputFormat::Latex);
    }

    #[test]
    fn test_deserialize_unknown_returns_error() {
        let result: serde_yaml::Result<InputFormat> = serde_yaml::from_str("xml");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not a supported input format"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    fn test_deserialize_case_insensitive() {
        let fmt: InputFormat = serde_yaml::from_str("Markdown").unwrap();
        assert_eq!(fmt, InputFormat::Markdown);
    }
}
