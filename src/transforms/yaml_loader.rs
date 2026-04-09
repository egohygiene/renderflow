use std::fs;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{command::CommandTransform, plugin::{PluginRegistry, PluginTransform}, Transform, TransformRegistry};

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
/// Each entry describes either an external command or a named plugin that
/// converts one document format to another, along with metadata used for
/// graph-based path-finding.
///
/// Exactly one of `program` or `plugin` must be provided:
///
/// * Set `program` to invoke an external binary (see [`CommandTransform`]).
/// * Set `plugin` to reference a [`PluginExecutor`](super::plugin::PluginExecutor)
///   that has been registered in a [`PluginRegistry`].
#[derive(Debug, Deserialize, PartialEq)]
pub struct YamlTransformDef {
    /// Unique human-readable name; used in log messages and error context.
    pub name: String,
    /// External program to invoke (looked up on `PATH`).
    ///
    /// Required when `plugin` is not set.  Ignored when `plugin` is set.
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
    /// Required when `program` is not set.  When both `plugin` and `program`
    /// are provided, `plugin` takes precedence.
    #[serde(default)]
    pub plugin: Option<String>,
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
    /// * Exactly one of `program` or `plugin` must be provided.
    /// * `from` and `to` must both be non-blank and parseable as a known [`Format`].
    ///
    /// [`Format`]: crate::graph::Format
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            anyhow::bail!("transform 'name' must not be empty");
        }
        match (&self.plugin, &self.program) {
            (Some(p), _) if p.trim().is_empty() => {
                anyhow::bail!("transform '{}': 'plugin' must not be empty when provided", self.name);
            }
            (None, None) => {
                anyhow::bail!("transform '{}': one of 'program' or 'plugin' must be provided", self.name);
            }
            (None, Some(prog)) if prog.trim().is_empty() => {
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
    /// # Panics
    ///
    /// Panics when `program` is `None`.  Always call [`validate`](Self::validate)
    /// first, or check that `program` is `Some` before calling this method.
    pub fn to_command_transform(&self) -> CommandTransform {
        let program = self
            .program
            .as_deref()
            .expect("to_command_transform called without a program field");
        CommandTransform::new(self.name.clone(), program, self.args.clone())
    }

    /// Build a [`PluginTransform`] from this definition by looking up the
    /// named plugin in `registry`.
    ///
    /// Returns an error when the plugin name is not registered.
    pub fn to_plugin_transform(&self, registry: &PluginRegistry) -> Result<PluginTransform> {
        let plugin_name = self
            .plugin
            .as_deref()
            .expect("to_plugin_transform called without a plugin field");
        let executor = registry
            .get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("transform '{}': plugin '{}' not found in registry", self.name, plugin_name))?;
        Ok(PluginTransform::new(executor))
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
/// See [`load_transforms_from_yaml_with_plugins`] for the expected schema and error behaviour.
pub fn parse_transforms_from_str_with_plugins(yaml: &str, plugins: &PluginRegistry) -> Result<TransformRegistry> {
    let config: YamlTransformConfig =
        serde_yaml_ng::from_str(yaml).context("Failed to parse YAML transform config")?;

    let mut registry = TransformRegistry::new();
    for def in &config.transforms {
        def.validate()?;
        let transform: Box<dyn Transform> = if def.plugin.is_some() {
            Box::new(def.to_plugin_transform(plugins)?)
        } else {
            Box::new(def.to_command_transform())
        };
        registry.register(transform);
    }
    Ok(registry)
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
    from: jpeg
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
            err.to_string().contains("one of 'program' or 'plugin' must be provided"),
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
}
