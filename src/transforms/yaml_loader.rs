use std::fs;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{
    aggregation::{AggregationRegistry, CommandAggregationTransform},
    ai::{AiBackend, AiTransform},
    command::CommandTransform,
    plugin::{PluginRegistry, PluginTransform},
    Transform, TransformRegistry,
};

/// Top-level structure of a YAML transform configuration file.
///
/// The file must contain a `transforms` key whose value is a list of
/// [`YamlTransformDef`] entries.
///
/// # Example
///
/// ```yaml
/// transforms:
///   - name: pandoc-md-to-html
///     program: pandoc
///     args:
///       - "{input}"
///       - -o
///       - "{output}"
///     from: markdown
///     to: html
///     cost: 0.5
///     quality: 0.9
/// ```
#[derive(Debug, Deserialize, PartialEq)]
pub struct YamlTransformConfig {
    /// The list of transform definitions.
    pub transforms: Vec<YamlTransformDef>,
}

/// A single YAML-defined transform entry.
///
/// Each entry describes either an external command, a named plugin, or an
/// AI-powered transform that converts one document format to another, along
/// with metadata used for graph-based path-finding.
///
/// Exactly one of `program`, `plugin`, or `ai` must be provided:
///
/// * Set `program` to invoke an external binary (see [`CommandTransform`]).
/// * Set `plugin` to reference a [`PluginExecutor`](super::plugin::PluginExecutor)
///   that has been registered in a [`PluginRegistry`].
/// * Set `ai` to the backend name (`"ollama"` or `"openai"`) to use an
///   AI-powered transform (see [`AiTransform`]).
#[derive(Debug, Deserialize, PartialEq)]
pub struct YamlTransformDef {
    /// Unique human-readable name; used in log messages and error context.
    pub name: String,
    /// External program to invoke (looked up on `PATH`).
    ///
    /// Required when `plugin` and `ai` are not set.  Ignored when either of
    /// those fields is set.
    #[serde(default)]
    pub program: Option<String>,
    /// Arguments passed to the program.
    ///
    /// Use `{input}` as a placeholder for a temporary file that contains the
    /// input string, and `{output}` as a placeholder for the temporary file
    /// path that the program should write its output to.  When neither
    /// placeholder is present, the input is written to the command's `stdin`
    /// and the output is read from `stdout`.
    #[serde(default)]
    pub args: Vec<String>,
    /// Name of a [`PluginExecutor`](super::plugin::PluginExecutor) registered
    /// in the [`PluginRegistry`] that should be used to execute this transform.
    ///
    /// Required when `program` and `ai` are not set.  When both `plugin` and
    /// `program` are provided, `plugin` takes precedence.
    #[serde(default)]
    pub plugin: Option<String>,
    /// AI backend name (`"ollama"` or `"openai"`).
    ///
    /// When set, an [`AiTransform`] is built using the associated `model`,
    /// `prompt`, `endpoint`, `api_key`, and `artifact_path` fields.
    /// Takes precedence over `plugin` and `program` when all three are
    /// present.
    #[serde(default)]
    pub ai: Option<String>,
    /// Model identifier for AI transforms (e.g. `"mistral"`, `"llava"`).
    ///
    /// Only used when `ai` is set.
    #[serde(default)]
    pub model: Option<String>,
    /// Prompt template for AI transforms.  Use `{input}` as a placeholder
    /// for the document content passed to the transform.
    ///
    /// Only used when `ai` is set.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Base URL of the AI backend (e.g. `"http://localhost:11434"`).
    ///
    /// Only used when `ai` is set.  Defaults to `"http://localhost:11434"`
    /// for Ollama.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Optional API key sent as `Authorization: Bearer <key>`.
    ///
    /// Only used when `ai` is set.  Required for OpenAI-compatible backends
    /// that enforce authentication.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Optional file path where the AI response will also be written as an
    /// artifact after a successful call.
    ///
    /// Only used when `ai` is set.
    #[serde(default)]
    pub artifact_path: Option<String>,
    /// Optional path to the AI result cache file.
    ///
    /// When set, the AI transform will check this file for a cached result
    /// before calling the backend, and will store the result with metadata
    /// (model, timestamp, input hash) on a cache miss.  Ignored when `ai` is
    /// not set.
    #[serde(default)]
    pub cache_path: Option<String>,
    /// Whether this transform consumes a single input or a collection.
    ///
    /// Accepted values: `"single"` (default) and `"collection"`.
    ///
    /// Set to `"collection"` for aggregation-style transforms that combine
    /// multiple source documents into a single output artifact (e.g. a set
    /// of page images into a CBZ archive or PDF document).  Collection
    /// transforms are loaded into an [`AggregationRegistry`] rather than the
    /// standard [`TransformRegistry`].
    ///
    /// [`AggregationRegistry`]: super::aggregation::AggregationRegistry
    #[serde(default)]
    pub input_kind: Option<String>,
    /// Source document format (e.g. `"markdown"`, `"html"`).
    pub from: String,
    /// Target document format produced by this transform (e.g. `"html"`, `"pdf"`).
    pub to: String,
    /// Relative cost of applying this transformation (lower is cheaper).
    pub cost: f32,
    /// Expected output quality on a `0.0`–`1.0` scale (higher is better).
    pub quality: f32,
}

impl YamlTransformDef {
    /// Validate the definition and return a descriptive error for any invalid field.
    ///
    /// Checks:
    /// * `name` must not be blank.
    /// * Exactly one of `program`, `plugin`, or `ai` must be provided.
    /// * When `ai` is set it must be a known backend name (see [`AiBackend`]).
    /// * `from` and `to` must both be non-blank and parseable as a known [`Format`].
    ///
    /// [`Format`]: crate::graph::Format
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            anyhow::bail!("transform 'name' must not be empty");
        }
        match (&self.ai, &self.plugin, &self.program) {
            // ai takes precedence – validate the backend name.
            (Some(backend), _, _) if backend.trim().is_empty() => {
                anyhow::bail!("transform '{}': 'ai' must not be empty when provided", self.name);
            }
            (Some(backend), _, _) => {
                backend.parse::<AiBackend>().with_context(|| {
                    format!("transform '{}': invalid 'ai' backend", self.name)
                })?;
            }
            // plugin next.
            (None, Some(p), _) if p.trim().is_empty() => {
                anyhow::bail!("transform '{}': 'plugin' must not be empty when provided", self.name);
            }
            (None, None, None) => {
                anyhow::bail!("transform '{}': one of 'program', 'plugin', or 'ai' must be provided", self.name);
            }
            (None, None, Some(prog)) if prog.trim().is_empty() => {
                anyhow::bail!("transform '{}': 'program' must not be empty", self.name);
            }
            _ => {}
        }
        if self.from.trim().is_empty() {
            anyhow::bail!("transform '{}': 'from' must not be empty", self.name);
        }
        if self.to.trim().is_empty() {
            anyhow::bail!("transform '{}': 'to' must not be empty", self.name);
        }
        self.from
            .parse::<crate::graph::Format>()
            .with_context(|| format!("transform '{}': invalid 'from' format", self.name))?;
        self.to
            .parse::<crate::graph::Format>()
            .with_context(|| format!("transform '{}': invalid 'to' format", self.name))?;
        Ok(())
    }

    /// Build a [`CommandTransform`] from this definition.
    ///
    /// Returns an error when `program` is `None`.  Call [`validate`](Self::validate)
    /// first to ensure the definition is well-formed before calling this method.
    pub fn to_command_transform(&self) -> Result<CommandTransform> {
        let program = self
            .program
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("transform '{}': 'program' is required for a command transform", self.name))?;
        Ok(CommandTransform::new(self.name.clone(), program, self.args.clone()))
    }

    /// Build a [`PluginTransform`] from this definition by looking up the
    /// named plugin in `registry`.
    ///
    /// Returns an error when the plugin field is not set or the plugin name is
    /// not registered in `registry`.
    pub fn to_plugin_transform(&self, registry: &PluginRegistry) -> Result<PluginTransform> {
        let plugin_name = self
            .plugin
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("transform '{}': 'plugin' field is required for a plugin transform", self.name))?;
        let executor = registry
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("transform '{}': plugin '{}' not found in registry", self.name, plugin_name))?;
        Ok(PluginTransform::new(executor))
    }

    /// Build an [`AiTransform`] from this definition.
    ///
    /// Returns an error when the `ai` field is not set or contains an unknown
    /// backend name.  Call [`validate`](Self::validate) first to ensure the
    /// definition is well-formed before calling this method.
    pub fn to_ai_transform(&self) -> Result<AiTransform> {
        let backend_str = self
            .ai
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("transform '{}': 'ai' field is required for an AI transform", self.name))?;
        let backend: AiBackend = backend_str.parse().with_context(|| {
            format!("transform '{}': invalid 'ai' backend '{}'", self.name, backend_str)
        })?;

        let mut builder = AiTransform::builder()
            .name(self.name.clone())
            .backend(backend);

        if let Some(model) = &self.model {
            builder = builder.model(model.clone());
        }
        if let Some(prompt) = &self.prompt {
            builder = builder.prompt_template(prompt.clone());
        }
        if let Some(endpoint) = &self.endpoint {
            builder = builder.endpoint(endpoint.clone());
        }
        if let Some(api_key) = &self.api_key {
            builder = builder.api_key(api_key.clone());
        }
        if let Some(artifact_path) = &self.artifact_path {
            builder = builder.artifact_path(artifact_path.clone());
        }
        if let Some(cache_path) = &self.cache_path {
            builder = builder.cache_path(cache_path.clone());
        }

        Ok(builder.build())
    }

    /// Return `true` when `input_kind` is set to `"collection"`.
    ///
    /// Collection transforms aggregate multiple inputs into a single output
    /// and are registered in an [`AggregationRegistry`] rather than the
    /// standard [`TransformRegistry`].
    ///
    /// [`AggregationRegistry`]: super::aggregation::AggregationRegistry
    pub fn is_collection(&self) -> bool {
        self.input_kind
            .as_deref()
            .map(|s| s.trim().eq_ignore_ascii_case("collection"))
            .unwrap_or(false)
    }

    /// Build a [`CommandAggregationTransform`] from this definition.
    ///
    /// Only valid when `program` is set; call [`validate`](Self::validate)
    /// first.  `plugin` and `ai` fields are not supported for aggregation
    /// transforms.
    #[allow(dead_code)]
    pub fn to_aggregation_transform(&self) -> Result<CommandAggregationTransform> {
        let program = self.program.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "transform '{}': 'program' is required for a collection transform",
                self.name
            )
        })?;
        Ok(CommandAggregationTransform::new(
            self.name.clone(),
            program,
            self.args.clone(),
        ))
    }
}

/// Load YAML transform definitions from a file and return a populated [`TransformRegistry`].
///
/// The file must conform to the [`YamlTransformConfig`] schema.  Every entry in
/// the `transforms` list is validated before any transform is registered; the
/// function returns an error as soon as it encounters the first invalid entry.
///
/// # Errors
///
/// Returns an error when:
/// * the file cannot be read,
/// * the YAML is malformed,
/// * any transform definition fails validation (see [`YamlTransformDef::validate`]).
pub fn load_transforms_from_yaml(path: &str) -> Result<TransformRegistry> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read transform config: {}", path))?;
    parse_transforms_from_str(&content)
        .with_context(|| format!("Failed to load transforms from: {}", path))
}

/// Load YAML transform definitions from a file and return a populated [`TransformRegistry`].
///
/// Equivalent to [`load_transforms_from_yaml`] but also accepts a
/// [`PluginRegistry`] so that transforms with a `plugin` field can be
/// resolved.  Use [`load_transforms_from_yaml`] when no plugins are needed.
///
/// # Errors
///
/// Returns an error when:
/// * the file cannot be read,
/// * the YAML is malformed,
/// * any transform definition fails validation (see [`YamlTransformDef::validate`]),
/// * a referenced plugin is not registered in `plugins`.
#[allow(dead_code)]
pub fn load_transforms_from_yaml_with_plugins(path: &str, plugins: &PluginRegistry) -> Result<TransformRegistry> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read transform config: {}", path))?;
    parse_transforms_from_str_with_plugins(&content, plugins)
        .with_context(|| format!("Failed to load transforms from: {}", path))
}

/// Parse YAML transform definitions from a string and return a populated [`TransformRegistry`].
///
/// See [`load_transforms_from_yaml`] for the expected schema and error behaviour.
pub fn parse_transforms_from_str(yaml: &str) -> Result<TransformRegistry> {
    parse_transforms_from_str_with_plugins(yaml, &PluginRegistry::new())
}

/// Parse YAML transform definitions from a string and return a populated [`TransformRegistry`].
///
/// Equivalent to [`parse_transforms_from_str`] but also accepts a
/// [`PluginRegistry`] so that transforms with a `plugin` field can be
/// resolved.  Use [`parse_transforms_from_str`] when no plugins are needed.
///
/// Collection transforms (those with `input_kind: collection`) are silently
/// skipped; use [`parse_aggregation_transforms_from_str`] to load those.
///
/// See [`load_transforms_from_yaml_with_plugins`] for the expected schema and error behaviour.
pub fn parse_transforms_from_str_with_plugins(yaml: &str, plugins: &PluginRegistry) -> Result<TransformRegistry> {
    let config: YamlTransformConfig =
        serde_yaml_ng::from_str(yaml).context("Failed to parse YAML transform config")?;

    let mut registry = TransformRegistry::new();
    for def in &config.transforms {
        // Collection transforms belong in an AggregationRegistry; skip them here.
        if def.is_collection() {
            continue;
        }
        def.validate()?;
        let transform: Box<dyn Transform> = if def.ai.is_some() {
            Box::new(def.to_ai_transform()?)
        } else if def.plugin.is_some() {
            Box::new(def.to_plugin_transform(plugins)?)
        } else {
            Box::new(def.to_command_transform()?)
        };
        registry.register(transform);
    }
    Ok(registry)
}

/// Parse YAML transform definitions from a string and return a populated
/// [`AggregationRegistry`].
///
/// Only entries with `input_kind: collection` are loaded; all other entries
/// are silently skipped.  Every collection entry is validated before being
/// registered; the function returns an error on the first invalid entry.
///
/// # Errors
///
/// Returns an error when:
/// * the YAML is malformed,
/// * any collection transform definition fails validation,
/// * a collection entry has no `program` field.
#[allow(dead_code)]
pub fn parse_aggregation_transforms_from_str(yaml: &str) -> Result<AggregationRegistry> {
    let config: YamlTransformConfig =
        serde_yaml_ng::from_str(yaml).context("Failed to parse YAML transform config")?;

    let mut registry = AggregationRegistry::new();
    for def in &config.transforms {
        if !def.is_collection() {
            continue;
        }
        def.validate()?;
        let transform = def.to_aggregation_transform()?;
        registry.register(Box::new(transform));
    }
    Ok(registry)
}

/// Load YAML transform definitions from a file and return a populated
/// [`AggregationRegistry`].
///
/// The file must conform to the [`YamlTransformConfig`] schema.  Only entries
/// with `input_kind: collection` are registered; all others are ignored.
///
/// # Errors
///
/// Returns an error when:
/// * the file cannot be read,
/// * the YAML is malformed,
/// * any collection transform definition fails validation (see
///   [`YamlTransformDef::validate`]).
#[allow(dead_code)]
pub fn load_aggregation_transforms_from_yaml(path: &str) -> Result<AggregationRegistry> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read transform config: {}", path))?;
    parse_aggregation_transforms_from_str(&content)
        .with_context(|| format!("Failed to load aggregation transforms from: {}", path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_yaml(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("failed to create temp file");
        f.write_all(content.as_bytes())
            .expect("failed to write temp file");
        f
    }

    // ── parse_transforms_from_str ─────────────────────────────────────────────

    #[test]
    fn test_parse_minimal_valid_config() {
        let yaml = r#"
transforms:
  - name: test-transform
    program: cat
    from: markdown
    to: html
    cost: 1.0
    quality: 0.8
"#;
        let registry = parse_transforms_from_str(yaml).expect("should parse");
        // Apply the registry: cat with no args pipes stdin to stdout.
        let result = registry.apply_all("hello".to_string()).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_parse_multiple_transforms() {
        let yaml = r#"
transforms:
  - name: first
    program: cat
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
  - name: second
    program: cat
    from: html
    to: pdf
    cost: 2.0
    quality: 0.7
"#;
        let registry = parse_transforms_from_str(yaml).expect("should parse two transforms");
        let result = registry.apply_all("content".to_string()).unwrap();
        assert_eq!(result, "content");
    }

    #[test]
    fn test_parse_with_args_no_placeholders() {
        let yaml = r#"
transforms:
  - name: echo-hello
    program: echo
    args: ["-n", "hello"]
    from: markdown
    to: html
    cost: 0.5
    quality: 1.0
"#;
        let registry = parse_transforms_from_str(yaml).expect("should parse");
        let result = registry.apply_all("ignored".to_string()).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_parse_empty_args_defaults() {
        // args is optional; omitting it should work the same as an empty list.
        let yaml = r#"
transforms:
  - name: no-args
    program: cat
    from: rst
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let result = parse_transforms_from_str(yaml);
        assert!(result.is_ok(), "omitting args should not be an error");
    }

    #[test]
    fn test_parse_all_known_from_to_formats() {
        // Verify that all Format variants round-trip through the YAML parser.
        let pairs = &[
            ("markdown", "html"),
            ("md", "pdf"),
            ("html", "pdf"),
            ("html", "docx"),
            ("epub", "html"),
            ("rst", "html"),
            ("latex", "html"),
            ("tex", "html"),
        ];
        for (from, to) in pairs {
            let yaml = format!(
                "transforms:\n  - name: t\n    program: cat\n    from: {from}\n    to: {to}\n    cost: 1.0\n    quality: 1.0\n"
            );
            assert!(
                parse_transforms_from_str(&yaml).is_ok(),
                "failed for from={from} to={to}"
            );
        }
    }

    // ── validation errors ─────────────────────────────────────────────────────

    #[test]
    fn test_invalid_yaml_returns_error() {
        let yaml = "not: valid: yaml: [";
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        assert!(
            err.to_string().contains("Failed to parse YAML transform config"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_empty_name_returns_error() {
        let yaml = r#"
transforms:
  - name: ""
    program: cat
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        assert!(
            err.to_string().contains("'name' must not be empty"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_empty_program_returns_error() {
        let yaml = r#"
transforms:
  - name: my-transform
    program: ""
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        assert!(
            err.to_string().contains("'program' must not be empty"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_empty_from_returns_error() {
        let yaml = r#"
transforms:
  - name: my-transform
    program: cat
    from: ""
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        assert!(
            err.to_string().contains("'from' must not be empty"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_empty_to_returns_error() {
        let yaml = r#"
transforms:
  - name: my-transform
    program: cat
    from: markdown
    to: ""
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        assert!(
            err.to_string().contains("'to' must not be empty"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_unknown_from_format_returns_error() {
        let yaml = r#"
transforms:
  - name: my-transform
    program: cat
    from: avif
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        let msg = err.to_string();
        assert!(
            msg.contains("invalid 'from' format"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn test_unknown_to_format_returns_error() {
        let yaml = r#"
transforms:
  - name: my-transform
    program: cat
    from: markdown
    to: mp4
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        let msg = err.to_string();
        assert!(
            msg.contains("invalid 'to' format"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn test_missing_required_field_returns_error() {
        // Neither 'program' nor 'plugin' is provided; must fail.
        let yaml = r#"
transforms:
  - name: no-program
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let result = parse_transforms_from_str(yaml);
        assert!(result.is_err(), "missing 'program' or 'plugin' should be an error");
    }

    // ── load_transforms_from_yaml ─────────────────────────────────────────────

    #[test]
    fn test_load_from_file_success() {
        let yaml = r#"
transforms:
  - name: file-test
    program: cat
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let f = write_temp_yaml(yaml);
        let registry = load_transforms_from_yaml(f.path().to_str().unwrap())
            .expect("should load from file");
        let result = registry.apply_all("data".to_string()).unwrap();
        assert_eq!(result, "data");
    }

    #[test]
    fn test_load_from_missing_file_returns_error() {
        let err = load_transforms_from_yaml("/nonexistent/transforms.yaml").err().expect("expected an error");
        let msg = err.to_string();
        assert!(
            msg.contains("Failed to read transform config"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn test_load_from_file_invalid_yaml_returns_error() {
        let f = write_temp_yaml("not: valid: yaml: [");
        let err = load_transforms_from_yaml(f.path().to_str().unwrap()).err().expect("expected an error");
        let msg = err.to_string();
        assert!(
            msg.contains("Failed to load transforms from"),
            "unexpected: {msg}"
        );
    }

    // ── YamlTransformDef metadata fields ─────────────────────────────────────

    #[test]
    fn test_parsed_def_fields() {
        let yaml = r#"
transforms:
  - name: meta-test
    program: pandoc
    args: ["{input}", "-o", "{output}"]
    from: markdown
    to: pdf
    cost: 2.5
    quality: 0.95
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        let def = &config.transforms[0];
        assert_eq!(def.name, "meta-test");
        assert_eq!(def.program, Some("pandoc".to_string()));
        assert_eq!(def.args, vec!["{input}", "-o", "{output}"]);
        assert_eq!(def.from, "markdown");
        assert_eq!(def.to, "pdf");
        assert!((def.cost - 2.5).abs() < 1e-5);
        assert!((def.quality - 0.95).abs() < 1e-5);
    }

    // ── plugin field ──────────────────────────────────────────────────────────

    #[test]
    fn test_plugin_transform_executes_correctly() {
        use std::sync::Arc;
        use crate::transforms::plugin::{PluginExecutor, PluginRegistry};

        struct UpperPlugin;
        impl PluginExecutor for UpperPlugin {
            fn name(&self) -> &str { "upper" }
            fn execute(&self, input: String) -> anyhow::Result<String> {
                Ok(input.to_uppercase())
            }
        }

        let mut plugins = PluginRegistry::new();
        plugins.register(Arc::new(UpperPlugin));

        let yaml = r#"
transforms:
  - name: upper-plugin-transform
    plugin: upper
    from: markdown
    to: html
    cost: 0.5
    quality: 1.0
"#;
        let registry = parse_transforms_from_str_with_plugins(yaml, &plugins)
            .expect("should parse with plugin");
        let result = registry.apply_all("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_plugin_field_parsed_correctly() {
        let yaml = r#"
transforms:
  - name: my-plugin-transform
    plugin: my-plugin
    from: markdown
    to: html
    cost: 1.0
    quality: 0.8
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        let def = &config.transforms[0];
        assert_eq!(def.plugin, Some("my-plugin".to_string()));
        assert_eq!(def.program, None);
    }

    #[test]
    fn test_missing_plugin_in_registry_returns_error() {
        let yaml = r#"
transforms:
  - name: ghost-plugin
    plugin: nonexistent
    from: markdown
    to: html
    cost: 0.5
    quality: 1.0
"#;
        let plugins = PluginRegistry::new();
        let err = parse_transforms_from_str_with_plugins(yaml, &plugins)
            .err()
            .expect("should fail: plugin not registered");
        assert!(
            err.to_string().contains("not found in registry"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_empty_plugin_field_returns_error() {
        let yaml = r#"
transforms:
  - name: empty-plugin
    plugin: ""
    from: markdown
    to: html
    cost: 0.5
    quality: 1.0
"#;
        let plugins = PluginRegistry::new();
        let err = parse_transforms_from_str_with_plugins(yaml, &plugins)
            .err()
            .expect("should fail: empty plugin name");
        assert!(
            err.to_string().contains("'plugin' must not be empty"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_neither_program_nor_plugin_returns_error() {
        let yaml = r#"
transforms:
  - name: no-executor
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        assert!(
            err.to_string().contains("one of 'program', 'plugin', or 'ai' must be provided"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_plugin_takes_precedence_over_program_when_both_set() {
        use std::sync::Arc;
        use crate::transforms::plugin::{PluginExecutor, PluginRegistry};

        struct AppendBangPlugin;
        impl PluginExecutor for AppendBangPlugin {
            fn name(&self) -> &str { "bang" }
            fn execute(&self, input: String) -> anyhow::Result<String> {
                Ok(format!("{}!", input))
            }
        }

        let mut plugins = PluginRegistry::new();
        plugins.register(Arc::new(AppendBangPlugin));

        // Both plugin and program are set; plugin must win.
        let yaml = r#"
transforms:
  - name: plugin-wins
    plugin: bang
    program: cat
    from: markdown
    to: html
    cost: 0.5
    quality: 1.0
"#;
        let registry = parse_transforms_from_str_with_plugins(yaml, &plugins)
            .expect("should parse");
        let result = registry.apply_all("hello".to_string()).unwrap();
        assert_eq!(result, "hello!");
    }

    #[test]
    fn test_load_from_file_with_plugins() {
        use std::sync::Arc;
        use crate::transforms::plugin::{PluginExecutor, PluginRegistry};

        struct ReversePlugin;
        impl PluginExecutor for ReversePlugin {
            fn name(&self) -> &str { "reverse" }
            fn execute(&self, input: String) -> anyhow::Result<String> {
                Ok(input.chars().rev().collect())
            }
        }

        let yaml = r#"
transforms:
  - name: reverse-transform
    plugin: reverse
    from: markdown
    to: html
    cost: 0.5
    quality: 1.0
"#;
        let f = write_temp_yaml(yaml);
        let mut plugins = PluginRegistry::new();
        plugins.register(Arc::new(ReversePlugin));

        let registry = load_transforms_from_yaml_with_plugins(f.path().to_str().unwrap(), &plugins)
            .expect("should load from file with plugin");
        let result = registry.apply_all("hello".to_string()).unwrap();
        assert_eq!(result, "olleh");
    }

    // ── ai field ──────────────────────────────────────────────────────────────

    #[test]
    fn test_ai_field_parsed_correctly() {
        let yaml = r#"
transforms:
  - name: ai-summarise
    ai: ollama
    model: mistral
    prompt: "Summarise: {input}"
    endpoint: http://localhost:11434
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        let def = &config.transforms[0];
        assert_eq!(def.ai, Some("ollama".to_string()));
        assert_eq!(def.model, Some("mistral".to_string()));
        assert_eq!(def.prompt, Some("Summarise: {input}".to_string()));
        assert_eq!(def.endpoint, Some("http://localhost:11434".to_string()));
        assert_eq!(def.program, None);
        assert_eq!(def.plugin, None);
    }

    #[test]
    fn test_ai_transform_built_from_yaml_def() {
        let yaml = r#"
transforms:
  - name: ai-transform
    ai: openai
    model: gpt-4o
    prompt: "Describe: {input}"
    endpoint: https://api.openai.com
    api_key: sk-test
    artifact_path: /tmp/test_artifact.txt
    from: markdown
    to: html
    cost: 2.0
    quality: 0.95
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        let def = &config.transforms[0];
        let t = def.to_ai_transform().expect("should build AI transform");
        assert_eq!(t.name(), "ai-transform");
    }

    #[test]
    fn test_ai_transform_validates_correctly() {
        let yaml = r#"
transforms:
  - name: ai-valid
    ai: ollama
    model: llava
    prompt: "Analyse: {input}"
    endpoint: http://localhost:11434
    from: markdown
    to: fountain
    cost: 1.5
    quality: 0.85
"#;
        let result = parse_transforms_from_str(yaml);
        assert!(result.is_ok(), "valid AI transform should parse successfully");
    }

    #[test]
    fn test_unknown_ai_backend_returns_error() {
        let yaml = r#"
transforms:
  - name: bad-ai
    ai: anthropic
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        assert!(
            err.to_string().contains("invalid 'ai' backend"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_empty_ai_field_returns_error() {
        let yaml = r#"
transforms:
  - name: empty-ai
    ai: ""
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let err = parse_transforms_from_str(yaml).err().expect("expected an error");
        assert!(
            err.to_string().contains("'ai' must not be empty"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn test_ai_field_takes_precedence_over_program_and_plugin() {
        // When `ai` is set together with `program` and `plugin`, the AI
        // transform must be built (ai > plugin > program priority).
        // We can't run the AI backend in tests, but we can verify the
        // transform's name via the built registry (name() is accessible
        // through the Transform trait object before calling apply).
        // Instead, test that validation passes and the def builds correctly.
        let yaml = r#"
transforms:
  - name: ai-wins
    ai: ollama
    model: mistral
    program: cat
    plugin: some-plugin
    from: markdown
    to: html
    cost: 0.5
    quality: 1.0
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        let def = &config.transforms[0];
        // The AI transform should build without looking up the plugin.
        let t = def.to_ai_transform().expect("ai transform should build");
        assert_eq!(t.name(), "ai-wins");
    }

    #[test]
    fn test_fountain_format_valid_in_yaml() {
        let yaml = r#"
transforms:
  - name: md-to-fountain
    ai: ollama
    model: llava
    prompt: "Convert to Fountain: {input}"
    from: markdown
    to: fountain
    cost: 2.0
    quality: 0.8
"#;
        let result = parse_transforms_from_str(yaml);
        assert!(result.is_ok(), "fountain format must be accepted: {:?}", result.err());
    }

    #[test]
    fn test_ai_cache_path_parsed_from_yaml() {
        let yaml = r#"
transforms:
  - name: cached-ai
    ai: ollama
    model: mistral
    prompt: "Summarise: {input}"
    cache_path: /tmp/.renderflow-ai-cache.json
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        let def = &config.transforms[0];
        assert_eq!(def.cache_path, Some("/tmp/.renderflow-ai-cache.json".to_string()));
    }

    #[test]
    fn test_ai_cache_path_wired_through_to_ai_transform() {
        use crate::cache::{compute_ai_input_hash, save_ai_cache, AiCache, AiCacheEntry};

        let dir = tempfile::tempdir().unwrap();
        let cache_file = dir.path().join(".renderflow-ai-cache.json");

        // Pre-populate the cache for the prompt "Summarise: hello" + model "mistral".
        let rendered_prompt = "Summarise: hello";
        let hash = compute_ai_input_hash(rendered_prompt, "mistral");
        let mut cache = AiCache::default();
        cache.insert(
            hash.clone(),
            AiCacheEntry {
                input_hash: hash,
                model: "mistral".to_string(),
                timestamp: 1_700_000_000,
                output: "cached from yaml cache_path".to_string(),
            },
        );
        save_ai_cache(&cache, &cache_file).unwrap();

        // Build the AiTransform via the YAML def with the cache_path set.
        let yaml = format!(
            r#"
transforms:
  - name: cached-ai
    ai: ollama
    model: mistral
    prompt: "Summarise: {{input}}"
    cache_path: {}
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#,
            cache_file.to_str().unwrap()
        );
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(&yaml).expect("should parse");
        let def = &config.transforms[0];
        let t = def.to_ai_transform().expect("should build AI transform");

        // apply() must return the cached output without contacting any backend.
        let result = t.apply("hello".to_string()).unwrap();
        assert_eq!(result, "cached from yaml cache_path");
    }

    #[test]
    fn test_ai_cache_path_defaults_to_none() {
        let yaml = r#"
transforms:
  - name: no-cache-ai
    ai: ollama
    model: mistral
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        let def = &config.transforms[0];
        assert!(def.cache_path.is_none());
    }

    // ── input_kind / aggregation transform tests ──────────────────────────────

    #[test]
    fn test_input_kind_defaults_to_none() {
        let yaml = r#"
transforms:
  - name: no-kind
    program: cat
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        assert!(config.transforms[0].input_kind.is_none());
        assert!(!config.transforms[0].is_collection());
    }

    #[test]
    fn test_input_kind_collection_detected() {
        let yaml = r#"
transforms:
  - name: agg
    program: zip
    args: ["-j", "{output}", "{inputs}"]
    input_kind: collection
    from: jpeg
    to: cbz
    cost: 1.0
    quality: 0.9
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        assert!(config.transforms[0].is_collection());
    }

    #[test]
    fn test_input_kind_single_is_not_collection() {
        let yaml = r#"
transforms:
  - name: single
    program: cat
    input_kind: single
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse");
        assert!(!config.transforms[0].is_collection());
    }

    #[test]
    fn test_parse_aggregation_transforms_loads_collection_entries() {
        let yaml = r#"
transforms:
  - name: pages-to-cbz
    program: zip
    args: ["-j", "{output}", "{inputs}"]
    input_kind: collection
    from: jpeg
    to: cbz
    cost: 1.0
    quality: 0.9
"#;
        let registry =
            parse_aggregation_transforms_from_str(yaml).expect("should parse aggregation");
        assert!(
            registry.get("pages-to-cbz").is_some(),
            "aggregation registry must contain 'pages-to-cbz'"
        );
    }

    #[test]
    fn test_parse_aggregation_transforms_skips_non_collection_entries() {
        let yaml = r#"
transforms:
  - name: regular
    program: cat
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
  - name: aggregated
    program: zip
    args: ["-j", "{output}", "{inputs}"]
    input_kind: collection
    from: jpeg
    to: cbz
    cost: 1.0
    quality: 0.9
"#;
        let registry =
            parse_aggregation_transforms_from_str(yaml).expect("should parse");
        assert!(registry.get("aggregated").is_some(), "collection entry must be loaded");
        assert!(registry.get("regular").is_none(), "non-collection entry must be skipped");
    }

    #[test]
    fn test_parse_transforms_skips_collection_entries() {
        let yaml = r#"
transforms:
  - name: regular
    program: cat
    from: markdown
    to: html
    cost: 1.0
    quality: 0.9
  - name: aggregated
    program: zip
    args: ["-j", "{output}", "{inputs}"]
    input_kind: collection
    from: jpeg
    to: cbz
    cost: 1.0
    quality: 0.9
"#;
        // The standard registry must not contain the collection transform.
        let registry = parse_transforms_from_str(yaml).expect("should parse");
        let result = registry.apply_all("hello".to_string()).unwrap();
        // Only `cat` (regular) is registered; it echoes input unchanged.
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_to_aggregation_transform_requires_program() {
        let yaml = r#"
transforms:
  - name: no-program-agg
    input_kind: collection
    from: jpeg
    to: cbz
    cost: 1.0
    quality: 0.9
"#;
        let config: YamlTransformConfig =
            serde_yaml_ng::from_str(yaml).expect("should parse yaml");
        let def = &config.transforms[0];
        // to_aggregation_transform must fail when program is absent.
        assert!(def.to_aggregation_transform().is_err());
    }

    #[test]
    fn test_load_aggregation_transforms_from_yaml_file() {
        let yaml = r#"
transforms:
  - name: images-to-pdf
    program: img2pdf
    args: ["--output", "{output}", "{inputs}"]
    input_kind: collection
    from: jpeg
    to: pdf
    cost: 1.0
    quality: 0.95
"#;
        let f = write_temp_yaml(yaml);
        let registry = load_aggregation_transforms_from_yaml(f.path().to_str().unwrap())
            .expect("should load aggregation transforms");
        assert!(registry.get("images-to-pdf").is_some());
    }
}
