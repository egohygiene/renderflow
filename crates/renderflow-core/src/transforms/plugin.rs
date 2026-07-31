use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result};

use super::Transform;

// ── Plugin capability flags ───────────────────────────────────────────────────

/// Declares which optional features a plugin supports.
///
/// All fields default to `false`; plugins opt in by setting the relevant flag
/// to `true` in their [`PluginMetadata`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PluginCapabilities {
    /// Plugin honours the `dry_run` flag in [`PluginContext`] and avoids
    /// side-effects when it is set.
    pub dry_run: bool,
    /// Plugin participates in caching: identical inputs with the same config
    /// produce identical, cacheable outputs.
    pub caching: bool,
    /// Plugin produces rich diagnostics when an execution error occurs.
    pub diagnostics: bool,
    /// Plugin exposes cost/quality metadata that the graph planner can use
    /// during optimisation.
    pub optimization: bool,
}

// ── Plugin metadata ───────────────────────────────────────────────────────────

/// Descriptive information about a plugin.
///
/// Metadata is attached to a plugin at registration time via
/// [`PluginRegistry::register_with_metadata`] and can later be retrieved with
/// [`PluginRegistry::metadata`].  It drives the `renderflow plugin list` and
/// `renderflow plugin info` CLI commands.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Unique identifier used as the lookup key (e.g. `"pandoc-md-to-html"`).
    pub id: String,
    /// Semver-style version string (e.g. `"1.0.0"`).
    pub version: String,
    /// Plugin author or maintainer.
    pub author: String,
    /// Short human-readable description of what the plugin does.
    pub description: String,
    /// Names of the transform formats this plugin handles
    /// (e.g. `["markdown→html", "html→pdf"]`).
    pub supported_transforms: Vec<String>,
    /// Optional SPDX license identifier (e.g. `"MIT"`, `"Apache-2.0"`).
    pub license: Option<String>,
    /// Optional list of external tool names that must be present for this
    /// plugin to function (e.g. `["pandoc", "pdflatex"]`).
    pub required_tools: Vec<String>,
    /// Capability flags.
    pub capabilities: PluginCapabilities,
}

#[allow(dead_code)]
impl PluginMetadata {
    /// Create a minimal [`PluginMetadata`] with required fields.
    ///
    /// Optional fields default to empty / `None` and can be overridden with
    /// builder-style methods.
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            author: String::new(),
            description: String::new(),
            supported_transforms: Vec::new(),
            license: None,
            required_tools: Vec::new(),
            capabilities: PluginCapabilities::default(),
        }
    }

    /// Set the author field.
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    /// Set the description field.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Replace the list of supported transform names.
    pub fn with_supported_transforms(mut self, transforms: Vec<String>) -> Self {
        self.supported_transforms = transforms;
        self
    }

    /// Set the SPDX license identifier.
    pub fn with_license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }

    /// Replace the list of required external tools.
    pub fn with_required_tools(mut self, tools: Vec<String>) -> Self {
        self.required_tools = tools;
        self
    }

    /// Set the capability flags.
    pub fn with_capabilities(mut self, caps: PluginCapabilities) -> Self {
        self.capabilities = caps;
        self
    }

    /// Validate the metadata and return a descriptive error for any invalid
    /// field.
    ///
    /// * `id` must not be blank.
    /// * `version` must not be blank.
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            anyhow::bail!("plugin metadata 'id' must not be empty");
        }
        if self.version.trim().is_empty() {
            anyhow::bail!(
                "plugin '{}' metadata 'version' must not be empty",
                self.id
            );
        }
        Ok(())
    }
}

// ── Plugin configuration ──────────────────────────────────────────────────────

/// Per-plugin configuration values, namespaced to avoid key collisions.
///
/// Configuration is passed to plugins through [`PluginContext`] and should be
/// validated by the plugin before first use.  Keys and values are both plain
/// strings; plugins are responsible for parsing values into their expected
/// types.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct PluginConfig {
    values: HashMap<String, String>,
}

#[allow(dead_code)]
impl PluginConfig {
    /// Create an empty [`PluginConfig`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a configuration value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.values.insert(key.into(), value.into());
        self
    }

    /// Retrieve a configuration value by key.
    ///
    /// Returns `None` when the key is not present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Retrieve a required configuration value, returning an error when the
    /// key is absent.
    pub fn require(&self, key: &str) -> Result<&str> {
        self.get(key).ok_or_else(|| {
            anyhow::anyhow!(
                "required plugin configuration key '{}' is not set",
                key
            )
        })
    }
}

// ── Plugin execution context ──────────────────────────────────────────────────

/// Runtime context provided to a plugin during execution.
///
/// `PluginContext` gives plugins access to the environment without exposing
/// Renderflow internals.  Pass it to [`PluginExecutor::execute_with_context`]
/// when your executor needs access to these values.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PluginContext {
    /// Current working directory at the time of plugin invocation.
    pub working_dir: PathBuf,
    /// Temporary directory that the plugin may use for scratch files.
    /// The directory and its contents will be cleaned up after execution.
    pub temp_dir: PathBuf,
    /// When `true` the plugin must not produce side-effects (no files
    /// written, no external commands executed).
    pub dry_run: bool,
    /// Plugin-specific configuration (namespaced values from the main config).
    pub config: PluginConfig,
}

#[allow(dead_code)]
impl PluginContext {
    /// Create a [`PluginContext`] for normal (non-dry-run) execution.
    pub fn new(working_dir: PathBuf, temp_dir: PathBuf) -> Self {
        Self {
            working_dir,
            temp_dir,
            dry_run: false,
            config: PluginConfig::new(),
        }
    }

    /// Return a copy of this context with `dry_run` set to `true`.
    pub fn with_dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }

    /// Return a copy of this context with the given plugin configuration.
    pub fn with_config(mut self, config: PluginConfig) -> Self {
        self.config = config;
        self
    }
}

// ── PluginExecutor trait ──────────────────────────────────────────────────────

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

    /// Optional: return rich diagnostics when execution fails.
    ///
    /// The default implementation returns `None`.  Override this in plugins
    /// that have [`PluginCapabilities::diagnostics`] set to `true` in their
    /// metadata to surface actionable error messages (missing tool, wrong
    /// version, etc.).
    #[allow(dead_code)]
    fn diagnose(&self, _error: &anyhow::Error) -> Option<String> {
        None
    }
}

// ── PluginTransform ───────────────────────────────────────────────────────────

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
        self.executor
            .execute(input)
            .with_context(|| format!("plugin '{}' execution failed", self.executor.name()))
    }
}

// ── PluginRegistry ────────────────────────────────────────────────────────────

/// Registration entry stored inside [`PluginRegistry`].
struct PluginEntry {
    executor: Arc<dyn PluginExecutor>,
    metadata: Option<PluginMetadata>,
}

/// A registry of named [`PluginExecutor`] implementations.
///
/// Plugins are stored by name and can be looked up when constructing transform
/// pipelines from external configuration (e.g. YAML files that reference a
/// `plugin` field).
///
/// Use [`PluginRegistry::register`] for simple registration without metadata,
/// or [`PluginRegistry::register_with_metadata`] to attach [`PluginMetadata`]
/// that drives CLI commands such as `renderflow plugin list`.
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
    plugins: HashMap<String, PluginEntry>,
}

#[allow(dead_code)]
impl PluginRegistry {
    /// Create an empty `PluginRegistry`.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin executor without metadata.
    ///
    /// The executor's [`name`](PluginExecutor::name) is used as the lookup key.
    /// If a plugin with the same name is already registered it is silently
    /// replaced.
    ///
    /// Returns `&mut self` to support method chaining.
    pub fn register(&mut self, executor: Arc<dyn PluginExecutor>) -> &mut Self {
        let key = executor.name().to_string();
        self.plugins.insert(
            key,
            PluginEntry {
                executor,
                metadata: None,
            },
        );
        self
    }

    /// Register a plugin executor together with its [`PluginMetadata`].
    ///
    /// The metadata `id` must match the executor's `name()`.  When the
    /// metadata is invalid (blank `id` or `version`) this method returns an
    /// error and the plugin is **not** registered.
    ///
    /// If a plugin with the same name is already registered it is replaced.
    pub fn register_with_metadata(
        &mut self,
        executor: Arc<dyn PluginExecutor>,
        metadata: PluginMetadata,
    ) -> Result<&mut Self> {
        metadata
            .validate()
            .with_context(|| format!("invalid metadata for plugin '{}'", executor.name()))?;
        let key = executor.name().to_string();
        self.plugins.insert(
            key,
            PluginEntry {
                executor,
                metadata: Some(metadata),
            },
        );
        Ok(self)
    }

    /// Look up a plugin executor by name.
    ///
    /// Returns `Some(Arc<dyn PluginExecutor>)` when a plugin with the given
    /// name exists, or `None` otherwise.
    pub fn get(&self, name: &str) -> Option<Arc<dyn PluginExecutor>> {
        self.plugins.get(name).map(|e| Arc::clone(&e.executor))
    }

    /// Look up the metadata for a registered plugin.
    ///
    /// Returns `None` when the plugin is not registered or was registered
    /// without metadata (via [`register`](Self::register)).
    pub fn metadata(&self, name: &str) -> Option<&PluginMetadata> {
        self.plugins.get(name)?.metadata.as_ref()
    }

    /// Return the names of all registered plugins in unspecified order.
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.keys().map(String::as_str).collect()
    }

    /// Return an iterator over all registered plugins as `(name, metadata)`.
    ///
    /// The metadata value is `None` for plugins registered without metadata.
    pub fn entries(&self) -> impl Iterator<Item = (&str, Option<&PluginMetadata>)> {
        self.plugins
            .iter()
            .map(|(k, v)| (k.as_str(), v.metadata.as_ref()))
    }

    /// Return `true` when a plugin with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Return the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Return `true` when no plugins are registered.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Validate all registered plugins that carry metadata.
    ///
    /// Returns a list of `(plugin_name, error_message)` pairs for every
    /// plugin whose metadata fails validation.  An empty list means all
    /// plugins are valid.
    pub fn validate_all(&self) -> Vec<(String, String)> {
        let mut issues = Vec::new();
        for (name, entry) in &self.plugins {
            if let Some(meta) = &entry.metadata {
                if let Err(e) = meta.validate() {
                    issues.push((name.clone(), e.to_string()));
                }
            }
        }
        issues
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── PluginInfo (diagnostic / CLI helper) ─────────────────────────────────────

/// A snapshot of a plugin's registration state, returned by
/// [`PluginRegistry::plugin_info`] for display in CLI commands such as
/// `renderflow plugin info <name>`.
#[derive(Debug)]
pub struct PluginInfo<'a> {
    /// Plugin name / lookup key.
    pub name: &'a str,
    /// Attached metadata, if the plugin was registered with metadata.
    pub metadata: Option<&'a PluginMetadata>,
}

impl PluginRegistry {
    /// Return display information for a registered plugin, or `None` when the
    /// plugin is not found.
    pub fn plugin_info<'r>(&'r self, name: &str) -> Option<PluginInfo<'r>> {
        let (key, entry) = self.plugins.get_key_value(name)?;
        Some(PluginInfo {
            name: key.as_str(),
            metadata: entry.metadata.as_ref(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ──────────────────────────────────────────────────────────

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

    fn make_metadata(id: &str) -> PluginMetadata {
        PluginMetadata::new(id, "1.0.0")
            .with_author("Test Author")
            .with_description("A test plugin")
            .with_license("MIT")
    }

    // ── PluginMetadata ────────────────────────────────────────────────────────

    #[test]
    fn test_metadata_validate_ok() {
        let meta = make_metadata("my-plugin");
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn test_metadata_validate_empty_id_fails() {
        let meta = PluginMetadata::new("", "1.0.0");
        assert!(meta.validate().is_err());
    }

    #[test]
    fn test_metadata_validate_whitespace_id_fails() {
        let meta = PluginMetadata::new("   ", "1.0.0");
        assert!(meta.validate().is_err());
    }

    #[test]
    fn test_metadata_validate_empty_version_fails() {
        let meta = PluginMetadata::new("my-plugin", "");
        assert!(meta.validate().is_err());
    }

    #[test]
    fn test_metadata_builder_methods() {
        let meta = PluginMetadata::new("p", "2.0.0")
            .with_author("Alice")
            .with_description("Desc")
            .with_license("Apache-2.0")
            .with_required_tools(vec!["pandoc".to_string()])
            .with_supported_transforms(vec!["md→html".to_string()])
            .with_capabilities(PluginCapabilities {
                dry_run: true,
                caching: true,
                diagnostics: false,
                optimization: false,
            });

        assert_eq!(meta.author, "Alice");
        assert_eq!(meta.description, "Desc");
        assert_eq!(meta.license.as_deref(), Some("Apache-2.0"));
        assert_eq!(meta.required_tools, vec!["pandoc"]);
        assert_eq!(meta.supported_transforms, vec!["md→html"]);
        assert!(meta.capabilities.dry_run);
        assert!(meta.capabilities.caching);
        assert!(!meta.capabilities.diagnostics);
    }

    // ── PluginConfig ──────────────────────────────────────────────────────────

    #[test]
    fn test_config_get_returns_none_for_missing_key() {
        let cfg = PluginConfig::new();
        assert!(cfg.get("key").is_none());
    }

    #[test]
    fn test_config_set_and_get() {
        let mut cfg = PluginConfig::new();
        cfg.set("key", "value");
        assert_eq!(cfg.get("key"), Some("value"));
    }

    #[test]
    fn test_config_set_overwrites() {
        let mut cfg = PluginConfig::new();
        cfg.set("key", "first");
        cfg.set("key", "second");
        assert_eq!(cfg.get("key"), Some("second"));
    }

    #[test]
    fn test_config_require_missing_key_returns_error() {
        let cfg = PluginConfig::new();
        assert!(cfg.require("missing").is_err());
    }

    #[test]
    fn test_config_require_present_key_returns_value() {
        let mut cfg = PluginConfig::new();
        cfg.set("endpoint", "http://localhost");
        assert_eq!(cfg.require("endpoint").unwrap(), "http://localhost");
    }

    // ── PluginContext ─────────────────────────────────────────────────────────

    #[test]
    fn test_context_default_is_not_dry_run() {
        let ctx = PluginContext::new("/tmp".into(), "/tmp".into());
        assert!(!ctx.dry_run);
    }

    #[test]
    fn test_context_as_dry_run() {
        let ctx = PluginContext::new("/tmp".into(), "/tmp".into()).with_dry_run();
        assert!(ctx.dry_run);
    }

    #[test]
    fn test_context_with_config() {
        let mut cfg = PluginConfig::new();
        cfg.set("model", "mistral");
        let ctx = PluginContext::new("/tmp".into(), "/tmp".into()).with_config(cfg);
        assert_eq!(ctx.config.get("model"), Some("mistral"));
    }

    // ── PluginExecutor / PluginTransform ──────────────────────────────────────

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
    fn test_plugin_transform_error_wrapped_with_context() {
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
        let err = t.apply("input".to_string()).unwrap_err();
        assert!(
            err.to_string().contains("plugin 'fail' execution failed"),
            "expected context wrapper, got: {}",
            err
        );
    }

    #[test]
    fn test_plugin_executor_diagnose_default_returns_none() {
        let err = anyhow::anyhow!("some error");
        assert!(UpperPlugin.diagnose(&err).is_none());
    }

    #[test]
    fn test_plugin_executor_diagnose_override_returns_message() {
        struct DiagPlugin;
        impl PluginExecutor for DiagPlugin {
            fn name(&self) -> &str {
                "diag"
            }
            fn execute(&self, input: String) -> Result<String> {
                Ok(input)
            }
            fn diagnose(&self, _error: &anyhow::Error) -> Option<String> {
                Some("hint: check that pandoc is installed".to_string())
            }
        }

        let p = DiagPlugin;
        let err = anyhow::anyhow!("missing tool");
        assert_eq!(
            p.diagnose(&err).as_deref(),
            Some("hint: check that pandoc is installed")
        );
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
        assert_eq!(result, ">>x");
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = PluginRegistry::default();
        assert!(registry.get("anything").is_none());
    }

    #[test]
    fn test_registry_contains_returns_true_when_registered() {
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(UpperPlugin));
        assert!(registry.contains("upper"));
        assert!(!registry.contains("missing"));
    }

    #[test]
    fn test_registry_len_and_is_empty() {
        let mut registry = PluginRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(Arc::new(UpperPlugin));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        registry.register(Arc::new(AppendPlugin("!")));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_registry_plugin_names_returns_all_names() {
        let mut registry = PluginRegistry::new();
        registry
            .register(Arc::new(UpperPlugin))
            .register(Arc::new(AppendPlugin("!")));

        let mut names = registry.plugin_names();
        names.sort();
        assert_eq!(names, vec!["append", "upper"]);
    }

    #[test]
    fn test_registry_register_with_metadata_stores_metadata() {
        let mut registry = PluginRegistry::new();
        let meta = make_metadata("upper");
        registry
            .register_with_metadata(Arc::new(UpperPlugin), meta)
            .unwrap();

        let stored = registry.metadata("upper").expect("metadata must be present");
        assert_eq!(stored.id, "upper");
        assert_eq!(stored.version, "1.0.0");
    }

    #[test]
    fn test_registry_register_with_invalid_metadata_returns_error() {
        let mut registry = PluginRegistry::new();
        let bad_meta = PluginMetadata::new("", "1.0.0"); // blank id
        let result = registry.register_with_metadata(Arc::new(UpperPlugin), bad_meta);
        assert!(result.is_err());
        // Plugin must NOT have been registered.
        assert!(!registry.contains("upper"));
    }

    #[test]
    fn test_registry_metadata_returns_none_for_unregistered() {
        let registry = PluginRegistry::new();
        assert!(registry.metadata("missing").is_none());
    }

    #[test]
    fn test_registry_metadata_returns_none_for_plugin_without_metadata() {
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(UpperPlugin)); // no metadata
        assert!(registry.metadata("upper").is_none());
    }

    #[test]
    fn test_registry_validate_all_empty_returns_no_issues() {
        let registry = PluginRegistry::new();
        assert!(registry.validate_all().is_empty());
    }

    #[test]
    fn test_registry_validate_all_valid_metadata_returns_no_issues() {
        let mut registry = PluginRegistry::new();
        registry
            .register_with_metadata(Arc::new(UpperPlugin), make_metadata("upper"))
            .unwrap();
        assert!(registry.validate_all().is_empty());
    }

    #[test]
    fn test_registry_entries_iterates_all_plugins() {
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(UpperPlugin));
        registry
            .register_with_metadata(Arc::new(AppendPlugin("!")), make_metadata("append"))
            .unwrap();

        let mut names: Vec<&str> = registry.entries().map(|(name, _)| name).collect();
        names.sort();
        assert_eq!(names, vec!["append", "upper"]);
    }

    #[test]
    fn test_registry_plugin_info_returns_some_for_registered() {
        let mut registry = PluginRegistry::new();
        registry
            .register_with_metadata(Arc::new(UpperPlugin), make_metadata("upper"))
            .unwrap();

        let info = registry.plugin_info("upper").expect("info must be present");
        assert_eq!(info.name, "upper");
        assert!(info.metadata.is_some());
    }

    #[test]
    fn test_registry_plugin_info_returns_none_for_missing() {
        let registry = PluginRegistry::new();
        assert!(registry.plugin_info("ghost").is_none());
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

    // ── PluginCapabilities defaults ───────────────────────────────────────────

    #[test]
    fn test_plugin_capabilities_default_all_false() {
        let caps = PluginCapabilities::default();
        assert!(!caps.dry_run);
        assert!(!caps.caching);
        assert!(!caps.diagnostics);
        assert!(!caps.optimization);
    }
}
