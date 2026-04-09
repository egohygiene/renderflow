use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;
use std::fs;

use crate::compat::{is_supported, unsupported_combination_message};
use crate::input_format::InputFormat;
use crate::optimization::OptimizationMode;

fn default_output_dir() -> String {
    "dist".to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputType {
    Html,
    Pdf,
    Docx,
    /// An output type that was recognised in the YAML but is not yet implemented
    /// or is entirely unknown.  Storing the raw string allows us to produce a
    /// targeted, user-friendly error message later (in validation / strategy
    /// selection) instead of a cryptic serde parse failure.
    Unsupported(String),
}

impl<'de> Deserialize<'de> for OutputType {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "html" => Ok(OutputType::Html),
            "pdf" => Ok(OutputType::Pdf),
            "docx" => Ok(OutputType::Docx),
            other => Ok(OutputType::Unsupported(other.to_string())),
        }
    }
}

impl fmt::Display for OutputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputType::Html => write!(f, "html"),
            OutputType::Pdf => write!(f, "pdf"),
            OutputType::Docx => write!(f, "docx"),
            OutputType::Unsupported(s) => write!(f, "{}", s),
        }
    }
}

/// Return a clear, user-facing message for an unsupported output type.
///
/// Known planned types (e.g. `docx`) receive a specific "not yet supported"
/// message; truly unknown strings get a generic "invalid type" message.
pub fn unsupported_type_message(type_str: &str) -> String {
    format!(
        "'{}' is not a valid output type. Supported types are: html, pdf, docx",
        type_str
    )
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct OutputConfig {
    #[serde(rename = "type")]
    pub output_type: OutputType,
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub outputs: Vec<OutputConfig>,
    pub input: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(default)]
    pub input_format: Option<InputFormat>,
    /// Optimization strategy used when selecting transformation paths.
    /// Defaults to [`OptimizationMode::Balanced`] when omitted.
    #[serde(default)]
    pub optimization: OptimizationMode,
    /// Optional path to a YAML file defining additional command-based transforms.
    ///
    /// When set, the named transforms are loaded and applied after the standard
    /// built-in pipeline (emoji replacement, variable substitution, syntax
    /// highlighting).  The file must conform to the schema expected by
    /// [`crate::transforms::load_transforms_from_yaml`].
    #[serde(default)]
    pub transforms: Option<String>,
}

impl Config {
    /// Return the resolved input format for this config.
    ///
    /// Uses the explicitly configured `input_format` when set; otherwise
    /// auto-detects from the `input` file extension.  Falls back to
    /// [`InputFormat::Markdown`] when neither source provides a match.
    pub fn input_format(&self) -> InputFormat {
        if let Some(ref fmt) = self.input_format {
            return fmt.clone();
        }
        InputFormat::from_extension(&self.input).unwrap_or_default()
    }
    pub fn validate(&self) -> Result<()> {
        if self.input.trim().is_empty() {
            anyhow::bail!("Config validation failed: 'input' must not be empty");
        }
        if self.outputs.is_empty() {
            anyhow::bail!(
                "Config validation failed: 'outputs' must contain at least one entry"
            );
        }
        // Collect all unsupported types so the user sees every problem at once.
        let bad: Vec<String> = self
            .outputs
            .iter()
            .filter_map(|o| {
                if let OutputType::Unsupported(ref t) = o.output_type {
                    Some(unsupported_type_message(t))
                } else {
                    None
                }
            })
            .collect();
        if !bad.is_empty() {
            anyhow::bail!("{}", bad.join("\n"));
        }

        // Check that each output type is compatible with the resolved input format.
        let input_fmt = self.input_format();
        let incompatible: Vec<String> = self
            .outputs
            .iter()
            .filter(|o| !is_supported(&input_fmt, &o.output_type))
            .map(|o| unsupported_combination_message(&input_fmt, &o.output_type))
            .collect();
        if !incompatible.is_empty() {
            anyhow::bail!("{}", incompatible.join("\n"));
        }

        Ok(())
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path))?;
    let config: Config = serde_yaml_ng::from_str(&content)
        .with_context(|| format!("Failed to parse YAML config: {}", path))?;
    config.validate()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_yaml(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(content.as_bytes())
            .expect("failed to write temp file");
        f
    }

    #[test]
    fn test_load_config_success() {
        let yaml = r#"
outputs:
  - type: pdf
  - type: html
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(
            config.outputs,
            vec![
                OutputConfig { output_type: OutputType::Pdf, template: None },
                OutputConfig { output_type: OutputType::Html, template: None },
            ]
        );
        assert_eq!(config.input, "input.md");
        assert_eq!(config.output_dir, "dist");
    }

    #[test]
    fn test_load_config_with_template() {
        let yaml = r#"
outputs:
  - type: html
    template: "default"
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(
            config.outputs,
            vec![OutputConfig {
                output_type: OutputType::Html,
                template: Some("default".to_string()),
            }]
        );
    }

    #[test]
    fn test_load_config_missing_file() {
        let result = load_config("/nonexistent/path/renderflow.yaml");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Failed to read config file"));
    }

    #[test]
    fn test_load_config_invalid_yaml() {
        let yaml = "not: valid: yaml: [";
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Failed to parse YAML config"));
    }

    #[test]
    fn test_load_config_missing_required_fields() {
        // 'input' is still required (no default); missing it must fail
        let yaml = "outputs:\n  - type: pdf\n";
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_default_output_dir() {
        // When output_dir is omitted it should default to "dist"
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse with default output_dir");
        assert_eq!(config.output_dir, "dist");
    }

    #[test]
    fn test_load_config_empty_outputs_is_invalid() {
        let yaml = r#"
outputs: []
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("at least one entry"), "unexpected error: {}", msg);
    }

    #[test]
    fn test_load_config_empty_input_is_invalid() {
        let yaml = r#"
outputs:
  - type: html
input: ""
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("'input' must not be empty"), "unexpected error: {}", msg);
    }

    #[test]
    fn test_load_config_with_docx_output() {
        let yaml = r#"
outputs:
  - type: docx
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse docx output type");
        assert_eq!(
            config.outputs,
            vec![OutputConfig { output_type: OutputType::Docx, template: None }]
        );
    }

    #[test]
    fn test_load_config_truly_invalid_type() {
        // A completely unknown type must produce a clear "not a valid output type"
        // message without crashing the YAML parser.
        let yaml = r#"
outputs:
  - type: jpeg
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("not a valid output type"),
            "unexpected error: {}",
            msg
        );
        assert!(
            msg.contains("docx"),
            "supported types list should include docx: {}",
            msg
        );
    }

    #[test]
    fn test_load_config_multiple_unsupported_types_reports_all() {
        // When more than one unsupported type is present, all of them must be
        // reported in a single error rather than stopping after the first one.
        let yaml = r#"
outputs:
  - type: jpeg
  - type: epub
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("'jpeg' is not a valid output type"),
            "expected jpeg error in: {}",
            msg
        );
        assert!(
            msg.contains("'epub' is not a valid output type"),
            "expected epub error in: {}",
            msg
        );
    }

    #[test]
    fn test_load_config_with_variables() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
variables:
  title: "My Document"
  author: "Alan"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse with variables");
        assert_eq!(config.variables.get("title").map(String::as_str), Some("My Document"));
        assert_eq!(config.variables.get("author").map(String::as_str), Some("Alan"));
    }

    #[test]
    fn test_load_config_without_variables_defaults_to_empty() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse without variables");
        assert!(config.variables.is_empty());
    }

    #[test]
    fn test_load_config_optimization_defaults_to_balanced() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(config.optimization, crate::optimization::OptimizationMode::Balanced);
    }

    #[test]
    fn test_load_config_optimization_speed() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
optimization: speed
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(config.optimization, crate::optimization::OptimizationMode::Speed);
    }

    #[test]
    fn test_load_config_optimization_quality() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
optimization: quality
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(config.optimization, crate::optimization::OptimizationMode::Quality);
    }

    #[test]
    fn test_load_config_optimization_balanced_explicit() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
optimization: balanced
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(config.optimization, crate::optimization::OptimizationMode::Balanced);
    }

    #[test]
    fn test_input_format_explicit_override() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
input_format: html
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse with explicit input_format");
        assert_eq!(config.input_format(), InputFormat::Html);
    }

    #[test]
    fn test_input_format_auto_detect_from_md_extension() {
        let yaml = r#"
outputs:
  - type: html
input: "document.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(config.input_format(), InputFormat::Markdown);
    }

    #[test]
    fn test_input_format_auto_detect_from_rst_extension() {
        let yaml = r#"
outputs:
  - type: html
input: "document.rst"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(config.input_format(), InputFormat::Rst);
    }

    #[test]
    fn test_input_format_fallback_to_markdown_when_unknown_extension() {
        let yaml = r#"
outputs:
  - type: html
input: "document.xyz"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(config.input_format(), InputFormat::Markdown);
    }

    #[test]
    fn test_input_format_override_takes_precedence_over_extension() {
        // Even though the file has a .rst extension, the explicit config wins.
        let yaml = r#"
outputs:
  - type: html
input: "document.rst"
output_dir: "dist"
input_format: markdown
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse");
        assert_eq!(config.input_format(), InputFormat::Markdown);
    }

    #[test]
    fn test_load_config_unsupported_input_format_returns_error() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
input_format: xml
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("not a supported input format"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    fn test_valid_combination_epub_to_html_passes() {
        let yaml = r#"
outputs:
  - type: html
input: "input.epub"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap())
            .expect("epub → html should be a valid combination");
        assert_eq!(config.input_format(), InputFormat::Epub);
    }

    #[test]
    fn test_invalid_combination_epub_to_docx_returns_error() {
        let yaml = r#"
outputs:
  - type: docx
input: "input.epub"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("epub"),
            "error should mention input format: {msg}"
        );
        assert!(
            msg.contains("docx"),
            "error should mention output type: {msg}"
        );
        assert!(
            msg.contains("not supported yet"),
            "error should say 'not supported yet': {msg}"
        );
    }

    #[test]
    fn test_invalid_combination_latex_to_docx_returns_error() {
        let yaml = r#"
outputs:
  - type: docx
input: "input.tex"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("latex"),
            "error should mention input format: {msg}"
        );
        assert!(
            msg.contains("docx"),
            "error should mention output type: {msg}"
        );
        assert!(
            msg.contains("not supported yet"),
            "error should say 'not supported yet': {msg}"
        );
    }

    #[test]
    fn test_multiple_invalid_combinations_reports_all() {
        // epub → docx and epub → (another invalid) should both be reported.
        // We use two outputs where at least one is unsupported.
        let yaml = r#"
outputs:
  - type: docx
  - type: html
input: "input.epub"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("epub"),
            "error should mention input format: {msg}"
        );
        assert!(
            msg.contains("docx"),
            "error should mention unsupported output: {msg}"
        );
    }

    #[test]
    fn test_load_config_with_transforms_path() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
transforms: "transforms.yaml"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse with transforms");
        assert_eq!(config.transforms.as_deref(), Some("transforms.yaml"));
    }

    #[test]
    fn test_load_config_without_transforms_defaults_to_none() {
        let yaml = r#"
outputs:
  - type: html
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let config = load_config(f.path().to_str().unwrap()).expect("should parse without transforms");
        assert!(config.transforms.is_none());
    }
}
