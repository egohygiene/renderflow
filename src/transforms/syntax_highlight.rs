use anyhow::Result;
use tracing::debug;

use super::Transform;

/// Prepares fenced code blocks in Markdown content for syntax highlighting.
///
/// V1 behaviour: every fenced code block opening fence (` ``` `) is detected
/// and its language tag is normalised to lowercase with surrounding whitespace
/// stripped.  All other content — including the code body itself — is passed
/// through unchanged, so the output is always valid Markdown.
pub struct SyntaxHighlightTransform;

impl SyntaxHighlightTransform {
    pub fn new() -> Self {
        SyntaxHighlightTransform
    }
}

impl Default for SyntaxHighlightTransform {
    fn default() -> Self {
        Self::new()
    }
}

impl Transform for SyntaxHighlightTransform {
    fn name(&self) -> &str {
        "SyntaxHighlightTransform"
    }

    fn apply(&self, input: String) -> Result<String> {
        let ends_with_newline = input.ends_with('\n');
        let mut result_lines: Vec<String> = Vec::new();
        let mut in_code_block = false;

        for line in input.lines() {
            if let Some(rest) = line.strip_prefix("```") {
                if !in_code_block {
                    // Opening fence: normalise the language tag.
                    in_code_block = true;
                    let lang = rest.trim().to_lowercase();
                    if lang.is_empty() {
                        result_lines.push("```".to_string());
                    } else {
                        debug!(language = %lang, "Detected fenced code block with language tag");
                        result_lines.push(format!("```{lang}"));
                    }
                } else if rest.trim().is_empty() {
                    // Closing fence: ``` with only optional trailing whitespace.
                    // A line that starts with ``` but has non-whitespace content
                    // after the backticks is treated as regular code content.
                    in_code_block = false;
                    result_lines.push("```".to_string());
                } else {
                    result_lines.push(line.to_string());
                }
            } else {
                result_lines.push(line.to_string());
            }
        }

        let mut result = result_lines.join("\n");
        if ends_with_newline {
            result.push('\n');
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_block_with_language_tag_preserved() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```rust\nfn main() {}\n```\n".to_string();
        let result = transform.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_language_tag_normalised_to_lowercase() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```Rust\nfn main() {}\n```\n".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(result, "```rust\nfn main() {}\n```\n");
    }

    #[test]
    fn test_language_tag_whitespace_trimmed() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```  Python  \nprint('hi')\n```\n".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(result, "```python\nprint('hi')\n```\n");
    }

    #[test]
    fn test_code_block_without_language_tag_preserved() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```\nsome code\n```\n".to_string();
        let result = transform.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_plain_text_passthrough() {
        let transform = SyntaxHighlightTransform::new();
        let input = "# Heading\n\nSome paragraph text.\n".to_string();
        let result = transform.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_empty_string() {
        let transform = SyntaxHighlightTransform::new();
        let result = transform.apply(String::new()).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_multiple_code_blocks_normalised() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```JavaScript\nconsole.log('hi');\n```\n\n```TOML\nkey = \"value\"\n```\n"
            .to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(
            result,
            "```javascript\nconsole.log('hi');\n```\n\n```toml\nkey = \"value\"\n```\n"
        );
    }

    #[test]
    fn test_code_block_body_is_unchanged() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```rust\nlet x = 42;\nprintln!(\"{}\", x);\n```\n".to_string();
        let result = transform.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_trailing_newline_preserved() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```rust\nfn main() {}\n```\n".to_string();
        let result = transform.apply(input).unwrap();
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn test_no_trailing_newline_preserved() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```rust\nfn main() {}\n```".to_string();
        let result = transform.apply(input).unwrap();
        assert!(!result.ends_with('\n'));
    }

    #[test]
    fn test_unclosed_code_block_content_preserved() {
        let transform = SyntaxHighlightTransform::new();
        let input = "```Python\ndef foo():\n    pass\n".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(result, "```python\ndef foo():\n    pass\n");
    }

    #[test]
    fn test_surrounding_prose_unchanged() {
        let transform = SyntaxHighlightTransform::new();
        let input =
            "Before the block.\n\n```Bash\necho hello\n```\n\nAfter the block.\n".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(
            result,
            "Before the block.\n\n```bash\necho hello\n```\n\nAfter the block.\n"
        );
    }

    #[test]
    fn test_code_line_starting_with_backticks_not_treated_as_closing_fence() {
        let transform = SyntaxHighlightTransform::new();
        // A line inside a code block that starts with ``` but has non-whitespace
        // content after the backticks should be treated as code, not a closing fence.
        let input = "```markdown\n```rust\nfn main() {}\n```\n```\n".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(
            result,
            "```markdown\n```rust\nfn main() {}\n```\n```\n"
        );
    }

    #[test]
    fn test_default_impl_matches_new() {
        let t1 = SyntaxHighlightTransform::new();
        let t2 = SyntaxHighlightTransform::default();
        let input = "```Rust\nfn foo() {}\n```\n".to_string();
        assert_eq!(
            t1.apply(input.clone()).unwrap(),
            t2.apply(input).unwrap()
        );
    }
}
