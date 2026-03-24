use std::collections::HashMap;

use anyhow::Result;

use super::{EmojiTransform, SyntaxHighlightTransform, Transform, VariableSubstitutionTransform};

/// A central registry that holds an ordered collection of [`Transform`]
/// implementations and applies them sequentially to document content.
///
/// Transforms are applied in the order they were registered.  Use
/// [`register_transforms`] to obtain a registry pre-populated with the
/// standard pipeline in the correct order.
pub struct TransformRegistry {
    transforms: Vec<Box<dyn Transform>>,
}

impl TransformRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
        }
    }

    /// Append a transform to the end of the registry.
    ///
    /// Transforms are applied in registration order, so the first registered
    /// transform runs first.
    pub fn register(&mut self, transform: Box<dyn Transform>) -> &mut Self {
        self.transforms.push(transform);
        self
    }

    /// Apply every registered transform in order, feeding the output of each
    /// into the next.
    pub fn apply_all(&self, input: String) -> Result<String> {
        let mut current = input;
        for transform in &self.transforms {
            current = transform.apply(current)?;
        }
        Ok(current)
    }
}

impl Default for TransformRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the standard transform registry used during document rendering.
///
/// Transforms are registered in the following order:
/// 1. **Emoji** – replaces emoji characters with `[emoji]` placeholders.
/// 2. **Variables** – substitutes `{{key}}` placeholders with config values.
/// 3. **Syntax highlight** – normalises fenced code-block language tags.
///
/// When `variables` is empty the variable-substitution transform is still
/// registered but becomes a no-op, ensuring consistent ordering regardless of
/// configuration.
pub fn register_transforms(variables: &HashMap<String, String>) -> TransformRegistry {
    let mut registry = TransformRegistry::new();
    registry
        .register(Box::new(EmojiTransform::new()))
        .register(Box::new(VariableSubstitutionTransform::new(
            variables.clone(),
        )))
        .register(Box::new(SyntaxHighlightTransform::new()));
    registry
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

    // ── TransformRegistry unit tests ─────────────────────────────────────────

    #[test]
    fn test_empty_registry_is_passthrough() {
        let registry = TransformRegistry::new();
        let result = registry.apply_all("hello world".to_string()).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_single_transform_applied() {
        struct UpperTransform;
        impl Transform for UpperTransform {
            fn apply(&self, input: String) -> Result<String> {
                Ok(input.to_uppercase())
            }
        }

        let mut registry = TransformRegistry::new();
        registry.register(Box::new(UpperTransform));
        let result = registry.apply_all("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_multiple_transforms_run_in_order() {
        struct AppendTransform(&'static str);
        impl Transform for AppendTransform {
            fn apply(&self, input: String) -> Result<String> {
                Ok(format!("{}{}", input, self.0))
            }
        }

        let mut registry = TransformRegistry::new();
        registry
            .register(Box::new(AppendTransform(" first")))
            .register(Box::new(AppendTransform(" second")))
            .register(Box::new(AppendTransform(" third")));

        let result = registry.apply_all("input".to_string()).unwrap();
        assert_eq!(result, "input first second third");
    }

    #[test]
    fn test_output_of_one_transform_feeds_next() {
        struct UpperTransform;
        impl Transform for UpperTransform {
            fn apply(&self, input: String) -> Result<String> {
                Ok(input.to_uppercase())
            }
        }

        struct AppendBang;
        impl Transform for AppendBang {
            fn apply(&self, input: String) -> Result<String> {
                Ok(format!("{}!", input))
            }
        }

        let mut registry = TransformRegistry::new();
        registry
            .register(Box::new(UpperTransform))
            .register(Box::new(AppendBang));

        let result = registry.apply_all("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO!");
    }

    #[test]
    fn test_transform_error_halts_pipeline() {
        use anyhow::bail;

        struct GoodTransform;
        impl Transform for GoodTransform {
            fn apply(&self, input: String) -> Result<String> {
                Ok(format!("{} ok", input))
            }
        }

        struct BadTransform;
        impl Transform for BadTransform {
            fn apply(&self, _input: String) -> Result<String> {
                bail!("transform failed")
            }
        }

        struct NeverReached;
        impl Transform for NeverReached {
            fn apply(&self, input: String) -> Result<String> {
                Ok(format!("{} unreachable", input))
            }
        }

        let mut registry = TransformRegistry::new();
        registry
            .register(Box::new(GoodTransform))
            .register(Box::new(BadTransform))
            .register(Box::new(NeverReached));

        let result = registry.apply_all("input".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("transform failed"));
    }

    // ── register_transforms integration tests ────────────────────────────────

    #[test]
    fn test_register_transforms_emoji_replaced() {
        let registry = register_transforms(&vars(&[]));
        let result = registry.apply_all("Hello 😀".to_string()).unwrap();
        assert_eq!(result, "Hello [emoji]");
    }

    #[test]
    fn test_register_transforms_variables_substituted() {
        let registry = register_transforms(&vars(&[("name", "World")]));
        let result = registry.apply_all("Hello {{name}}".to_string()).unwrap();
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_register_transforms_syntax_highlight_normalised() {
        let registry = register_transforms(&vars(&[]));
        let input = "```Rust\nfn main() {}\n```".to_string();
        let result = registry.apply_all(input).unwrap();
        assert!(result.starts_with("```rust\n"));
    }

    #[test]
    fn test_register_transforms_all_run_in_order() {
        // Emoji first → variable second → syntax third.
        // The code fence must start at the beginning of a line so the syntax
        // highlight transform can detect it.
        let registry = register_transforms(&vars(&[("lang", "Rust")]));
        let input = "😀\n```{{lang}}\ncode\n```".to_string();
        let result = registry.apply_all(input).unwrap();
        // Emoji replaced, variable substituted, language tag lowercased.
        assert!(
            result.starts_with("[emoji]\n```rust\n"),
            "unexpected result: {:?}",
            result
        );
    }

    #[test]
    fn test_register_transforms_empty_variables_is_consistent() {
        let registry_empty = register_transforms(&vars(&[]));
        let registry_nonempty = register_transforms(&vars(&[("x", "y")]));

        // Plain text without placeholders should be identical regardless of
        // whether variables are configured.
        let input = "plain text".to_string();
        let r1 = registry_empty.apply_all(input.clone()).unwrap();
        let r2 = registry_nonempty.apply_all(input).unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_register_transforms_output_is_consistent() {
        // Running the same registry twice on the same input should give the
        // same result (idempotent for stable inputs).
        let input = "Hello 😀 {{title}} ```Rust\ncode\n```".to_string();
        let vars_map = vars(&[("title", "Doc")]);

        let r1 = register_transforms(&vars_map)
            .apply_all(input.clone())
            .unwrap();
        let r2 = register_transforms(&vars_map).apply_all(input).unwrap();

        assert_eq!(r1, r2);
    }
}
