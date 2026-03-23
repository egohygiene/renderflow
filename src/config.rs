use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

const VALID_OUTPUT_TYPES: &[&str] = &["html", "pdf"];

fn default_output_dir() -> String {
    "dist".to_string()
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct OutputConfig {
    #[serde(rename = "type")]
    pub output_type: String,
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub outputs: Vec<OutputConfig>,
    pub input: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.input.trim().is_empty() {
            anyhow::bail!("Config validation failed: 'input' must not be empty");
        }
        if self.outputs.is_empty() {
            anyhow::bail!(
                "Config validation failed: 'outputs' must contain at least one entry"
            );
        }
        for output in &self.outputs {
            if !VALID_OUTPUT_TYPES.contains(&output.output_type.as_str()) {
                anyhow::bail!(
                    "Config validation failed: unsupported output type '{}'. Supported types: {}",
                    output.output_type,
                    VALID_OUTPUT_TYPES.join(", ")
                );
            }
        }
        Ok(())
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path))?;
    let config: Config = serde_yaml::from_str(&content)
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
                OutputConfig { output_type: "pdf".to_string(), template: None },
                OutputConfig { output_type: "html".to_string(), template: None },
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
                output_type: "html".to_string(),
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
    fn test_load_config_invalid_output_type() {
        let yaml = r#"
outputs:
  - type: docx
input: "input.md"
output_dir: "dist"
"#;
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("unsupported output type"), "unexpected error: {}", msg);
        assert!(msg.contains("docx"), "unexpected error: {}", msg);
    }
}
