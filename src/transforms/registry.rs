use std::collections::HashMap;

use anyhow::{Context, Result};
use tracing::{debug, error, warn};

use super::{EmojiTransform, SyntaxHighlightTransform, Transform, VariableSubstitutionTransform};
use crate::config::OutputType;

/// Controls how [`TransformRegistry`] reacts to a transform failure.
#[derive(Debug, Clone, PartialEq)]
pub enum FailureMode {
    /// Abort `apply_all` on the first transform error (default).
    FailFast,
    /// Log failures and continue the pipeline with the unmodified input.
    ContinueOnError,
}

/// A central registry that holds an ordered collection of [`Transform`]
/// implementations and applies them sequentially to document content.
///
/// Transforms are applied in the order they were registered.  Use
/// [`register_transforms`] to obtain a registry pre-populated with the
/// standard pipeline in the correct order.
///
/// # Error handling
///
/// By default the registry operates in [`FailureMode::FailFast`] mode: the
/// first transform error immediately aborts `apply_all` and returns an `Err`
/// whose message identifies the offending transform (e.g.
/// `"Transform failed: VariableSubstitutionTransform"`).
///
/// Set [`FailureMode::ContinueOnError`] via
/// [`TransformRegistry::with_failure_mode`] to instead skip the failing
/// transform (logging the error at `ERROR` level), pass the unmodified input
/// through to the next transform, and continue the pipeline.
pub struct TransformRegistry {
    transforms: Vec<Box<dyn Transform>>,
    /// Controls whether a transform failure aborts the pipeline or is skipped.
    failure_mode: FailureMode,
}

impl TransformRegistry {
    /// Create an empty registry with [`FailureMode::FailFast`] enabled.
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
            failure_mode: FailureMode::FailFast,
        }
    }

    /// Configure the failure mode and return `self` for chaining.
    ///
    /// * [`FailureMode::FailFast`] (default) – abort on the first transform failure.
    /// * [`FailureMode::ContinueOnError`] – log failures and continue with the unmodified input.
    pub fn with_failure_mode(mut self, failure_mode: FailureMode) -> Self {
        self.failure_mode = failure_mode;
        self
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
    ///
    /// Each transform is logged at `DEBUG` level on start and completion.
    /// Failures are wrapped with context identifying the transform name and
    /// logged at `ERROR` level before being propagated
    /// ([`FailureMode::FailFast`]) or skipped
    /// ([`FailureMode::ContinueOnError`]).
    pub fn apply_all(&self, input: String) -> Result<String> {
        let mut current = input;
        for transform in &self.transforms {
            let name = transform.name();
            debug!(transform = %name, "Starting transform");

            if self.failure_mode == FailureMode::FailFast {
                current = transform
                    .apply(current)
                    .with_context(|| format!("Transform failed: {}", name))?;
            } else {
                // In continue mode `apply` takes ownership of `current`, so a
                // clone is required to retain the pre-transform value for the
                // error path.  This is an unavoidable cost of the `String`-by-
                // value trait signature.
                match transform.apply(current.clone()) {
                    Ok(output) => {
                        current = output;
                    }
                    Err(e) => {
                        let wrapped = e.context(format!("Transform failed: {}", name));
                        error!(
                            transform = %name,
                            error = %wrapped,
                            "Transform failed; continuing pipeline (failure_mode = ContinueOnError)"
                        );
                        warn!(
                            transform = %name,
                            "Skipping failed transform and passing input through unchanged"
                        );
                        // current is unchanged – the clone was consumed by apply
                        continue;
                    }
                }
            }

            debug!(transform = %name, "Transform completed");
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
/// 1. **Emoji** – replaces emoji characters with `[emoji]` placeholders (skipped for HTML).
/// 2. **Variables** – substitutes `{{key}}` placeholders with config values.
/// 3. **Syntax highlight** – normalises fenced code-block language tags.
///
/// The `output_type` parameter controls format-specific behaviour.  In
/// particular, [`EmojiTransform`] is skipped for `OutputType::Html` because
/// HTML renders emoji natively and replacing them would be destructive.
///
/// When `variables` is empty the variable-substitution transform is still
/// registered but becomes a no-op, ensuring consistent ordering regardless of
/// configuration.
pub fn register_transforms(variables: &HashMap<String, String>, output_type: &OutputType) -> TransformRegistry {
    let mut registry = TransformRegistry::new();
    registry
        .register(Box::new(EmojiTransform::new_for_format(output_type)))
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
            fn name(&self) -> &str {
                "BadTransform"
            }
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
        // The error message must identify the failing transform.
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Transform failed: BadTransform"),
            "expected context message, got: {msg}"
        );
    }

    // ── register_transforms integration tests ────────────────────────────────

    #[test]
    fn test_register_transforms_emoji_replaced() {
        let registry = register_transforms(&vars(&[]), &OutputType::Pdf);
        let result = registry.apply_all("Hello 😀".to_string()).unwrap();
        assert_eq!(result, "Hello [emoji]");
    }

    #[test]
    fn test_register_transforms_emoji_preserved_for_html() {
        let registry = register_transforms(&vars(&[]), &OutputType::Html);
        let result = registry.apply_all("Hello 😀".to_string()).unwrap();
        assert_eq!(result, "Hello 😀", "HTML output must preserve emoji");
    }

    #[test]
    fn test_register_transforms_variables_substituted() {
        let registry = register_transforms(&vars(&[("name", "World")]), &OutputType::Pdf);
        let result = registry.apply_all("Hello {{name}}".to_string()).unwrap();
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_register_transforms_syntax_highlight_normalised() {
        let registry = register_transforms(&vars(&[]), &OutputType::Pdf);
        let input = "```Rust\nfn main() {}\n```".to_string();
        let result = registry.apply_all(input).unwrap();
        assert!(result.starts_with("```rust\n"));
    }

    #[test]
    fn test_register_transforms_all_run_in_order() {
        // Emoji first → variable second → syntax third.
        // The code fence must start at the beginning of a line so the syntax
        // highlight transform can detect it.
        let registry = register_transforms(&vars(&[("lang", "Rust")]), &OutputType::Pdf);
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
    fn test_register_transforms_html_emoji_and_variables_and_syntax() {
        // For HTML: emoji preserved, variables substituted, syntax normalised.
        let registry = register_transforms(&vars(&[("lang", "Rust")]), &OutputType::Html);
        let input = "😀\n```{{lang}}\ncode\n```".to_string();
        let result = registry.apply_all(input).unwrap();
        assert!(
            result.starts_with("😀\n```rust\n"),
            "unexpected result: {:?}",
            result
        );
    }

    #[test]
    fn test_register_transforms_empty_variables_is_consistent() {
        let registry_empty = register_transforms(&vars(&[]), &OutputType::Pdf);
        let registry_nonempty = register_transforms(&vars(&[("x", "y")]), &OutputType::Pdf);

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

        let r1 = register_transforms(&vars_map, &OutputType::Pdf)
            .apply_all(input.clone())
            .unwrap();
        let r2 = register_transforms(&vars_map, &OutputType::Pdf).apply_all(input).unwrap();

        assert_eq!(r1, r2);
    }

    // ── failure_mode = ContinueOnError tests ─────────────────────────────────

    #[test]
    fn test_continue_on_failure_skips_bad_transform() {
        use anyhow::bail;

        struct UpperTransform;
        impl Transform for UpperTransform {
            fn apply(&self, input: String) -> Result<String> {
                Ok(input.to_uppercase())
            }
        }

        struct AlwaysFails;
        impl Transform for AlwaysFails {
            fn name(&self) -> &str {
                "AlwaysFails"
            }
            fn apply(&self, _input: String) -> Result<String> {
                bail!("intentional failure")
            }
        }

        struct AppendBang;
        impl Transform for AppendBang {
            fn apply(&self, input: String) -> Result<String> {
                Ok(format!("{}!", input))
            }
        }

        let mut registry = TransformRegistry::new().with_failure_mode(FailureMode::ContinueOnError);
        registry
            .register(Box::new(UpperTransform))
            .register(Box::new(AlwaysFails))
            .register(Box::new(AppendBang));

        // With ContinueOnError, the failing transform is skipped and the
        // pipeline continues.  The final output reflects the surrounding
        // transforms but not the failed one's effect (which was none anyway).
        let result = registry.apply_all("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO!");
    }

    #[test]
    fn test_continue_on_failure_passes_input_through_unchanged() {
        use anyhow::bail;

        struct Doubler;
        impl Transform for Doubler {
            fn apply(&self, input: String) -> Result<String> {
                Ok(format!("{}{}", input, input))
            }
        }

        struct FailMidway;
        impl Transform for FailMidway {
            fn name(&self) -> &str {
                "FailMidway"
            }
            fn apply(&self, _input: String) -> Result<String> {
                bail!("midway failure")
            }
        }

        let mut registry = TransformRegistry::new().with_failure_mode(FailureMode::ContinueOnError);
        registry
            .register(Box::new(Doubler))
            .register(Box::new(FailMidway))
            .register(Box::new(Doubler));

        // First Doubler: "AB" → "ABAB"
        // FailMidway fails: "ABAB" passes through unchanged
        // Second Doubler: "ABAB" → "ABABABAB"
        let result = registry.apply_all("AB".to_string()).unwrap();
        assert_eq!(result, "ABABABAB");
    }

    // ── transform name() tests ────────────────────────────────────────────────

    #[test]
    fn test_emoji_transform_name() {
        let t = EmojiTransform::new();
        assert_eq!(t.name(), "EmojiTransform");
    }

    #[test]
    fn test_variable_substitution_transform_name() {
        let t = VariableSubstitutionTransform::new(HashMap::new());
        assert_eq!(t.name(), "VariableSubstitutionTransform");
    }

    #[test]
    fn test_syntax_highlight_transform_name() {
        let t = SyntaxHighlightTransform::new();
        assert_eq!(t.name(), "SyntaxHighlightTransform");
    }

    #[test]
    fn test_error_message_includes_transform_name() {
        use anyhow::bail;

        struct NamedFailing;
        impl Transform for NamedFailing {
            fn name(&self) -> &str {
                "VariableSubstitutionTransform"
            }
            fn apply(&self, _input: String) -> Result<String> {
                bail!("substitution error")
            }
        }

        let mut registry = TransformRegistry::new();
        registry.register(Box::new(NamedFailing));

        let err = registry.apply_all("input".to_string()).unwrap_err();
        assert!(
            err.to_string().contains("Transform failed: VariableSubstitutionTransform"),
            "error should contain transform name, got: {}",
            err
        );
    }
}
