//! Structured output validation for AI responses.
//!
//! When an [`AiRequest`](super::request::AiRequest) specifies an
//! [`OutputFormat`](super::request::OutputFormat), the executor calls
//! [`validate_output`] after receiving the response to ensure the content is
//! well-formed before passing it downstream.

use anyhow::{Context, Result};

use super::request::OutputFormat;

// ── validate_output ───────────────────────────────────────────────────────────

/// Validate `content` against the expectations for `format`.
///
/// | Format       | Validation performed                                    |
/// |--------------|--------------------------------------------------------|
/// | `Text`       | Always succeeds (no structural requirements).          |
/// | `Markdown`   | Non-empty check; content must not be blank.            |
/// | `Json`       | `serde_json::from_str` must succeed.                   |
/// | `Yaml`       | `serde_yaml_ng::from_str::<serde_json::Value>` must succeed. |
/// | `Xml`        | Must contain at least one `<` character (light check). |
///
/// Returns `Ok(())` on success or an error describing the first validation
/// failure.
pub fn validate_output(content: &str, format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            // No structural validation for plain text.
            Ok(())
        }
        OutputFormat::Markdown => {
            if content.trim().is_empty() {
                anyhow::bail!("Markdown output validation failed: response is empty");
            }
            Ok(())
        }
        OutputFormat::Json => {
            serde_json::from_str::<serde_json::Value>(content)
                .context("JSON output validation failed: response is not valid JSON")?;
            Ok(())
        }
        OutputFormat::Yaml => {
            serde_yaml_ng::from_str::<serde_json::Value>(content)
                .context("YAML output validation failed: response is not valid YAML")?;
            Ok(())
        }
        OutputFormat::Xml => {
            if !content.contains('<') {
                anyhow::bail!("XML output validation failed: response contains no XML tags");
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── text ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_text_always_valid() {
        assert!(validate_output("", &OutputFormat::Text).is_ok());
        assert!(validate_output("anything", &OutputFormat::Text).is_ok());
    }

    // ── markdown ──────────────────────────────────────────────────────────────

    #[test]
    fn test_markdown_non_empty_is_valid() {
        assert!(validate_output("# Hello\n\nworld", &OutputFormat::Markdown).is_ok());
    }

    #[test]
    fn test_markdown_empty_fails() {
        assert!(validate_output("", &OutputFormat::Markdown).is_err());
        assert!(validate_output("   ", &OutputFormat::Markdown).is_err());
    }

    // ── json ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_json_valid_object() {
        assert!(validate_output(r#"{"key":"value"}"#, &OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_json_valid_array() {
        assert!(validate_output("[1, 2, 3]", &OutputFormat::Json).is_ok());
    }

    #[test]
    fn test_json_invalid_fails() {
        let err = validate_output("not json", &OutputFormat::Json).unwrap_err();
        assert!(
            err.to_string().contains("JSON output validation failed"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_json_empty_fails() {
        assert!(validate_output("", &OutputFormat::Json).is_err());
    }

    // ── yaml ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_yaml_valid() {
        assert!(validate_output("key: value\nlist:\n  - a\n  - b", &OutputFormat::Yaml).is_ok());
    }

    #[test]
    fn test_yaml_invalid_fails() {
        // Tabs at the start of a block mapping are invalid YAML.
        let err = validate_output("\t: invalid", &OutputFormat::Yaml).unwrap_err();
        assert!(
            err.to_string().contains("YAML output validation failed"),
            "unexpected error: {}",
            err
        );
    }

    // ── xml ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_xml_valid_with_tags() {
        assert!(validate_output("<root><child/></root>", &OutputFormat::Xml).is_ok());
    }

    #[test]
    fn test_xml_without_tags_fails() {
        let err = validate_output("no tags here", &OutputFormat::Xml).unwrap_err();
        assert!(
            err.to_string().contains("XML output validation failed"),
            "unexpected error: {}",
            err
        );
    }
}
