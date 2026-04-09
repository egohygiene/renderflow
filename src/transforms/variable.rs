use std::collections::HashMap;

use anyhow::Result;
use tracing::warn;

use super::Transform;

/// Replaces `{{key}}` placeholders in text with values from a user-defined
/// variables map.
///
/// If a placeholder references a key that is not present in the map the
/// placeholder is left unchanged and a warning is emitted so the user knows
/// a substitution was skipped.
///
/// Substitution is intentionally skipped inside code regions: fenced code
/// blocks (` ``` ... ``` `) and inline code spans (`` `...` ``) are passed
/// through unchanged so that example code is not corrupted.
pub struct VariableSubstitutionTransform {
    variables: HashMap<String, String>,
}

impl VariableSubstitutionTransform {
    pub fn new(variables: HashMap<String, String>) -> Self {
        Self { variables }
    }

    /// Replaces `{{key}}` placeholders in `text`, appending the result to
    /// `output`.  No code-block awareness — callers must ensure `text` does
    /// not contain code segments that should be preserved verbatim.
    fn substitute_text_segment(&self, text: &str, output: &mut String) {
        let mut remaining = text;

        while let Some(start) = remaining.find("{{") {
            output.push_str(&remaining[..start]);
            let after_open = &remaining[start + 2..];

            if let Some(end) = after_open.find("}}") {
                let key = after_open[..end].trim();
                if let Some(value) = self.variables.get(key) {
                    output.push_str(value);
                } else {
                    warn!(key = %key, "Variable not found in config; leaving placeholder unchanged");
                    output.push_str("{{");
                    output.push_str(&after_open[..end]);
                    output.push_str("}}");
                }
                remaining = &after_open[end + 2..];
            } else {
                // No closing `}}` found — treat the rest as literal text.
                output.push_str("{{");
                remaining = after_open;
            }
        }

        output.push_str(remaining);
    }

    /// Finds the first run of exactly `bt_count` consecutive backtick characters
    /// in `text`, returning its byte offset.  Runs of a different length are
    /// skipped, so a single-backtick span is never accidentally closed by a
    /// double-backtick sequence inside the content, and vice-versa.
    fn find_closing_delimiter(text: &str, bt_count: usize) -> Option<usize> {
        let text_bytes = text.as_bytes();
        let len = text.len();
        let mut pos = 0;

        while pos < len {
            if text_bytes[pos] == b'`' {
                let run_start = pos;
                while pos < len && text_bytes[pos] == b'`' {
                    pos += 1;
                }
                if pos - run_start == bt_count {
                    return Some(run_start);
                }
            } else {
                pos += 1;
            }
        }

        None
    }

    /// Applies variable substitution to `line`, skipping any inline code
    /// spans (`` `...` ``, ` `` ... `` `, etc.).
    fn substitute_skipping_inline_code(&self, line: &str, output: &mut String) {
        let mut remaining = line;

        while !remaining.is_empty() {
            if let Some(bt_pos) = remaining.find('`') {
                // Apply substitution to the text before the opening backtick.
                self.substitute_text_segment(&remaining[..bt_pos], output);

                // Count consecutive opening backticks to determine the delimiter.
                let bt_count = remaining[bt_pos..].chars().take_while(|&c| c == '`').count();
                let delimiter = &remaining[bt_pos..bt_pos + bt_count];
                let after_open = &remaining[bt_pos + bt_count..];

                if let Some(close_pos) = Self::find_closing_delimiter(after_open, bt_count) {
                    // Copy the inline code span verbatim (delimiters + content).
                    output.push_str(delimiter);
                    output.push_str(&after_open[..close_pos]);
                    output.push_str(delimiter);
                    remaining = &after_open[close_pos + bt_count..];
                } else {
                    // No matching closing delimiter: emit the backticks literally
                    // and continue processing the remainder of the line normally.
                    output.push_str(delimiter);
                    remaining = after_open;
                }
            } else {
                self.substitute_text_segment(remaining, output);
                remaining = "";
            }
        }
    }
}

impl Transform for VariableSubstitutionTransform {
    fn name(&self) -> &str {
        "VariableSubstitutionTransform"
    }

    fn apply(&self, input: String) -> Result<String> {
        let ends_with_newline = input.ends_with('\n');
        let mut output = String::with_capacity(input.len());
        let mut in_fenced_block = false;
        let mut line_iter = input.lines().peekable();

        while let Some(line) = line_iter.next() {
            let is_last = line_iter.peek().is_none();

            if in_fenced_block {
                // Inside a fenced code block: pass through without substitution.
                output.push_str(line);
                // Detect the closing fence (``` with only optional trailing whitespace).
                if let Some(rest) = line.strip_prefix("```") {
                    if rest.trim().is_empty() {
                        in_fenced_block = false;
                    }
                }
            } else if let Some(fence_rest) = line.strip_prefix("```") {
                // Opening fence: apply substitution to the language tag portion,
                // but mark subsequent lines as inside the fenced block.
                in_fenced_block = true;
                output.push_str("```");
                self.substitute_skipping_inline_code(fence_rest, &mut output);
            } else {
                // Normal text: substitute variables but leave inline code spans intact.
                self.substitute_skipping_inline_code(line, &mut output);
            }

            if !is_last || ends_with_newline {
                output.push('\n');
            }
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_single_variable_replaced() {
        let transform = VariableSubstitutionTransform::new(vars(&[("title", "My Document")]));
        let result = transform.apply("Title: {{title}}".to_string()).unwrap();
        assert_eq!(result, "Title: My Document");
    }

    #[test]
    fn test_multiple_variables_replaced() {
        let transform = VariableSubstitutionTransform::new(vars(&[
            ("title", "My Document"),
            ("author", "Alan"),
            ("date", "2024-01-01"),
        ]));
        let result = transform
            .apply("{{title}}, {{author}}, {{date}}".to_string())
            .unwrap();
        assert_eq!(result, "My Document, Alan, 2024-01-01");
    }

    #[test]
    fn test_missing_variable_left_unchanged() {
        let transform = VariableSubstitutionTransform::new(vars(&[]));
        let result = transform.apply("Hello {{missing}}!".to_string()).unwrap();
        assert_eq!(result, "Hello {{missing}}!");
    }

    #[test]
    fn test_no_placeholders_passthrough() {
        let transform = VariableSubstitutionTransform::new(vars(&[("key", "value")]));
        let input = "No placeholders here.".to_string();
        let result = transform.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_empty_string() {
        let transform = VariableSubstitutionTransform::new(vars(&[("key", "value")]));
        let result = transform.apply(String::new()).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_empty_variables_map() {
        let transform = VariableSubstitutionTransform::new(HashMap::new());
        let result = transform.apply("{{title}}".to_string()).unwrap();
        assert_eq!(result, "{{title}}");
    }

    #[test]
    fn test_placeholder_with_whitespace_trimmed() {
        let transform =
            VariableSubstitutionTransform::new(vars(&[("title", "My Document")]));
        // Keys with surrounding whitespace inside `{{ }}` should still match.
        let result = transform.apply("{{ title }}".to_string()).unwrap();
        assert_eq!(result, "My Document");
    }

    #[test]
    fn test_unclosed_placeholder_preserved() {
        let transform = VariableSubstitutionTransform::new(vars(&[("key", "value")]));
        let result = transform.apply("{{unclosed".to_string()).unwrap();
        assert_eq!(result, "{{unclosed");
    }

    #[test]
    fn test_repeated_placeholder() {
        let transform =
            VariableSubstitutionTransform::new(vars(&[("name", "Renderflow")]));
        let result = transform
            .apply("{{name}} and {{name}}".to_string())
            .unwrap();
        assert_eq!(result, "Renderflow and Renderflow");
    }

    #[test]
    fn test_multiline_content() {
        let transform = VariableSubstitutionTransform::new(vars(&[
            ("title", "My Doc"),
            ("author", "Alan"),
        ]));
        let input = "# {{title}}\n\nWritten by {{author}}.".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(result, "# My Doc\n\nWritten by Alan.");
    }

    // --- code-block protection tests ---

    #[test]
    fn test_fenced_code_block_not_substituted() {
        let transform =
            VariableSubstitutionTransform::new(vars(&[("key", "VALUE")]));
        let input = "```rust\nlet x = \"{{key}}\";\n```\n".to_string();
        let result = transform.apply(input.clone()).unwrap();
        // The placeholder inside the fenced block must be left untouched.
        assert_eq!(result, input);
    }

    #[test]
    fn test_inline_code_not_substituted() {
        let transform =
            VariableSubstitutionTransform::new(vars(&[("key", "VALUE")]));
        let input = "Use `{{key}}` as a placeholder.".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(result, "Use `{{key}}` as a placeholder.");
    }

    #[test]
    fn test_substitution_outside_fenced_block_applied() {
        let transform = VariableSubstitutionTransform::new(vars(&[
            ("title", "My Doc"),
            ("key", "VALUE"),
        ]));
        let input = "# {{title}}\n\n```rust\nlet x = \"{{key}}\";\n```\n\nEnd.\n".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(
            result,
            "# My Doc\n\n```rust\nlet x = \"{{key}}\";\n```\n\nEnd.\n"
        );
    }

    #[test]
    fn test_substitution_skips_inline_code_applies_to_rest() {
        let transform =
            VariableSubstitutionTransform::new(vars(&[("author", "Alan")]));
        let input = "Written by `{{author}}`, not {{author}}.".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(result, "Written by `{{author}}`, not Alan.");
    }

    #[test]
    fn test_fenced_block_without_trailing_newline_not_substituted() {
        let transform =
            VariableSubstitutionTransform::new(vars(&[("key", "VALUE")]));
        let input = "```\n{{key}}\n```".to_string();
        let result = transform.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_double_backtick_inline_code_not_substituted() {
        let transform =
            VariableSubstitutionTransform::new(vars(&[("key", "VALUE")]));
        let input = "See ``{{key}}`` for details.".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(result, "See ``{{key}}`` for details.");
    }

    #[test]
    fn test_single_backtick_span_containing_double_backtick_not_substituted() {
        // A single-backtick inline span whose content includes a double-backtick
        // sequence must not be closed early at the double-backtick.
        let transform =
            VariableSubstitutionTransform::new(vars(&[("key", "VALUE")]));
        let input = "Use `code `` {{key}}` here.".to_string();
        let result = transform.apply(input).unwrap();
        assert_eq!(result, "Use `code `` {{key}}` here.");
    }
}
