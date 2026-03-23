use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

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
    pub output_dir: String,
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path))?;
    let config: Config = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML config: {}", path))?;
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
        let yaml = "outputs:\n  - type: pdf\n";
        let f = write_temp_yaml(yaml);
        let result = load_config(f.path().to_str().unwrap());
        assert!(result.is_err());
    }
}
