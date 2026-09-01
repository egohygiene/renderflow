use std::fs;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::spec::{
    json_schema_pretty, migrate_v1_file, validate_spec_file, SourceSpecVersion, SPEC_V2_ID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
    Yaml,
}

impl OutputFormat {
    fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            other => {
                anyhow::bail!("unknown spec output format '{other}'; supported: text, json, yaml")
            }
        }
    }
}

fn write_output(content: &str, output: Option<&str>) -> Result<()> {
    if let Some(path) = output {
        fs::write(path, content).with_context(|| format!("failed to write '{path}'"))?;
    } else {
        print!("{content}");
    }
    Ok(())
}

fn serialize<T: Serialize>(value: &T, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Text => anyhow::bail!("text output requires a dedicated renderer"),
        OutputFormat::Json => Ok(format!("{}\n", serde_json::to_string_pretty(value)?)),
        OutputFormat::Yaml => Ok(serde_yaml_ng::to_string(value)?),
    }
}

pub fn run_validate(config: &str, format: &str) -> Result<()> {
    let report = validate_spec_file(config);
    let format = OutputFormat::parse(format)?;

    match format {
        OutputFormat::Text => {
            if report.valid {
                let version = match report.source_version {
                    Some(SourceSpecVersion::V1) => "v1 compatibility",
                    Some(SourceSpecVersion::V2) => SPEC_V2_ID,
                    None => "unknown",
                };
                println!("Renderflow spec: valid ({version})");
            } else {
                eprintln!("Renderflow spec: invalid");
                for diagnostic in &report.diagnostics {
                    eprintln!(
                        "  {} [{}] {}",
                        diagnostic.path, diagnostic.code, diagnostic.message
                    );
                }
            }
        }
        OutputFormat::Json | OutputFormat::Yaml => {
            print!("{}", serialize(&report, format)?);
        }
    }

    if !report.valid {
        anyhow::bail!(
            "spec validation failed with {} diagnostic(s)",
            report.diagnostics.len()
        );
    }
    Ok(())
}

pub fn run_migrate(config: &str, output: Option<&str>) -> Result<()> {
    let migrated = migrate_v1_file(config)?;
    let yaml =
        serde_yaml_ng::to_string(&migrated).context("failed to serialize migrated v2 spec")?;
    write_output(&yaml, output)
}

pub fn run_schema(format: &str, output: Option<&str>) -> Result<()> {
    let format = OutputFormat::parse(format)?;
    let schema = crate::spec::json_schema();
    let content = match format {
        OutputFormat::Text | OutputFormat::Json => json_schema_pretty()?,
        OutputFormat::Yaml => serde_yaml_ng::to_string(&schema)?,
    };
    write_output(&content, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_rejects_unknown_value() {
        assert!(OutputFormat::parse("toml").is_err());
    }

    #[test]
    fn schema_command_serialization_is_machine_readable() {
        let json = json_schema_pretty().expect("schema serializes");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("schema is JSON");
        assert_eq!(parsed["properties"]["schema"]["const"], SPEC_V2_ID);
    }
}
