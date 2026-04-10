use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use super::Transform;

/// The interface that every plugin executor must implement.
///
/// A `PluginExecutor` processes document content in the same way as a built-in
/// [`Transform`], but it is registered at runtime rather than compiled into the
/// core engine.  This lets external crates extend the transform pipeline without
/// modifying renderflow itself.
///
/// # Example
///
/// ```rust
/// use renderflow::transforms::plugin::PluginExecutor;
///
/// struct ReversePlugin;
///
/// impl PluginExecutor for ReversePlugin {
///     fn name(&self) -> &str {
///         "reverse"
///     }
///
///     fn execute(&self, input: String) -> anyhow::Result<String> {
///         Ok(input.chars().rev().collect())
///     }
/// }
/// ```
pub trait PluginExecutor: Send + Sync {
    /// Human-readable name identifying this plugin.
    ///
    /// The name is used as the lookup key when plugins are referenced from
    /// transform definitions (e.g. the `plugin` field of a YAML transform
    /// entry).  It also appears in log messages and error context so it should
    /// be descriptive and unique within a [`PluginRegistry`].
    fn name(&self) -> &str;

    /// Execute the plugin on `input` and return the transformed content.
    fn execute(&self, input: String) -> Result<String>;
}

/// A [`Transform`] that delegates to a [`PluginExecutor`].
///
/// `PluginTransform` bridges the plugin system with the standard transform
/// pipeline: it holds a shared reference to a [`PluginExecutor`] and
/// implements [`Transform`] by forwarding calls to
/// [`PluginExecutor::execute`].
///
/// Create instances via [`PluginTransform::new`].
pub struct PluginTransform {
    executor: Arc<dyn PluginExecutor>,
}

impl PluginTransform {
    /// Wrap `executor` in a `PluginTransform`.
    pub fn new(executor: Arc<dyn PluginExecutor>) -> Self {
        Self { executor }
    }
}

impl Transform for PluginTransform {
    fn name(&self) -> &str {
        self.executor.name()
    }

    fn apply(&self, input: String) -> Result<String> {
        self.executor.execute(input)
    }
}

/// A registry of named [`PluginExecutor`] implementations.
///
/// Plugins are stored by name and can be looked up when constructing transform
/// pipelines from external configuration (e.g. YAML files that reference a
/// `plugin` field).
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use renderflow::transforms::plugin::{PluginExecutor, PluginRegistry};
///
/// struct UpperPlugin;
/// impl PluginExecutor for UpperPlugin {
///     fn name(&self) -> &str { "upper" }
///     fn execute(&self, input: String) -> anyhow::Result<String> {
///         Ok(input.to_uppercase())
///     }
/// }
///
/// let mut registry = PluginRegistry::new();
/// registry.register(Arc::new(UpperPlugin));
/// assert!(registry.get("upper").is_some());
/// assert!(registry.get("missing").is_none());
/// ```
pub struct PluginRegistry {
    plugins: HashMap<String, Arc<dyn PluginExecutor>>,
}

impl PluginRegistry {
    /// Create an empty `PluginRegistry`.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin executor.
    ///
    /// The executor's [`name`](PluginExecutor::name) is used as the lookup key.
    /// If a plugin with the same name is already registered it is silently
    /// replaced.
    ///
    /// Returns `&mut self` to support method chaining.
    #[allow(dead_code)]
    pub fn register(&mut self, executor: Arc<dyn PluginExecutor>) -> &mut Self {
        self.plugins.insert(executor.name().to_string(), executor);
        self
    }

    /// Look up a plugin by name.
    ///
    /// Returns `Some(Arc<dyn PluginExecutor>)` when a plugin with the given
    /// name exists, or `None` otherwise.
    pub fn get(&self, name: &str) -> Option<Arc<dyn PluginExecutor>> {
        self.plugins.get(name).cloned()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PluginExecutor / PluginTransform ──────────────────────────────────────

    struct UpperPlugin;
    impl PluginExecutor for UpperPlugin {
        fn name(&self) -> &str {
            "upper"
        }
        fn execute(&self, input: String) -> Result<String> {
            Ok(input.to_uppercase())
        }
    }

    struct AppendPlugin(&'static str);
    impl PluginExecutor for AppendPlugin {
        fn name(&self) -> &str {
            "append"
        }
        fn execute(&self, input: String) -> Result<String> {
            Ok(format!("{}{}", input, self.0))
        }
    }

    #[test]
    fn test_plugin_transform_name_delegates_to_executor() {
        let t = PluginTransform::new(Arc::new(UpperPlugin));
        assert_eq!(t.name(), "upper");
    }

    #[test]
    fn test_plugin_transform_apply_delegates_to_executor() {
        let t = PluginTransform::new(Arc::new(UpperPlugin));
        let result = t.apply("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_plugin_transform_error_propagated() {
        use anyhow::bail;

        struct FailPlugin;
        impl PluginExecutor for FailPlugin {
            fn name(&self) -> &str {
                "fail"
            }
            fn execute(&self, _input: String) -> Result<String> {
                bail!("plugin error")
            }
        }

        let t = PluginTransform::new(Arc::new(FailPlugin));
        assert!(t.apply("input".to_string()).is_err());
    }

    // ── PluginRegistry ────────────────────────────────────────────────────────

    #[test]
    fn test_registry_get_returns_none_for_missing_plugin() {
        let registry = PluginRegistry::new();
        assert!(registry.get("missing").is_none());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(UpperPlugin));
        let executor = registry.get("upper").expect("plugin must be present");
        let result = executor.execute("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_registry_register_multiple_plugins() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Arc::new(UpperPlugin))
            .register(Arc::new(AppendPlugin("!")));
        assert!(registry.get("upper").is_some());
        assert!(registry.get("append").is_some());
    }

    #[test]
    fn test_registry_register_replaces_existing_plugin() {
        struct PrefixPlugin(&'static str);
        impl PluginExecutor for PrefixPlugin {
            fn name(&self) -> &str {
                "upper"
            }
            fn execute(&self, input: String) -> Result<String> {
                Ok(format!("{}{}", self.0, input))
            }
        }

        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(UpperPlugin));
        registry.register(Arc::new(PrefixPlugin(">>"))); // same name, different impl

        let executor = registry.get("upper").unwrap();
        let result = executor.execute("x".to_string()).unwrap();
        // The second registration must win.
        assert_eq!(result, ">>x");
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = PluginRegistry::default();
        assert!(registry.get("anything").is_none());
    }

    // ── integration with TransformRegistry ───────────────────────────────────

    #[test]
    fn test_plugin_transform_integrates_with_transform_registry() {
        use crate::transforms::TransformRegistry;

        let mut registry = TransformRegistry::new();
        registry.register(Box::new(PluginTransform::new(Arc::new(UpperPlugin))));
        registry.register(Box::new(PluginTransform::new(Arc::new(AppendPlugin("!")))));

        let result = registry.apply_all("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO!");
    }
}
