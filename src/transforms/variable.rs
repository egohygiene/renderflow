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
pub struct VariableSubstitutionTransform {
    variables: HashMap<String, String>,
}

impl VariableSubstitutionTransform {
    pub fn new(variables: HashMap<String, String>) -> Self {
        Self { variables }
    }
}

impl Transform for VariableSubstitutionTransform {
    fn apply(&self, input: String) -> Result<String> {
        // Iterate over every `{{...}}` pattern in the input, rebuilding the
        // string in a single pass to avoid repeated allocations.
        let mut output = String::with_capacity(input.len());
        let mut remaining = input.as_str();

        while let Some(start) = remaining.find("{{") {
            // Emit everything before the opening `{{`.
            output.push_str(&remaining[..start]);
            let after_open = &remaining[start + 2..];

            if let Some(end) = after_open.find("}}") {
                let key = after_open[..end].trim();
                if let Some(value) = self.variables.get(key) {
                    output.push_str(value);
                } else {
                    warn!(key = %key, "Variable not found in config; leaving placeholder unchanged");
                    // Preserve the original placeholder text.
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

        // Append whatever is left after the last placeholder.
        output.push_str(remaining);
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
}
