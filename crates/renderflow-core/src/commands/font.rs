//! Handlers for local font registry validation and semantic role resolution.

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::font::{FontTarget, LoadedFontRegistry};

pub fn run_validate(registry: &str, format: &str) -> Result<()> {
    let loaded = LoadedFontRegistry::load(registry)?;
    let report = loaded.validate();
    emit(&report, format)?;
    if !report.valid {
        anyhow::bail!("font registry validation failed");
    }
    Ok(())
}

pub fn run_resolve(registry: &str, target: &str, output: Option<&str>, format: &str) -> Result<()> {
    let target: FontTarget = target.parse()?;
    let loaded = LoadedFontRegistry::load(registry)?;
    let report = loaded.resolve(target)?;
    let serialized = serialize(&report, format)?;
    if let Some(output) = output {
        std::fs::write(output, serialized)?;
        println!("Font resolution written to {}", Path::new(output).display());
    } else {
        print!("{serialized}");
    }
    Ok(())
}

pub fn run_css(registry: &str, output: Option<&str>) -> Result<()> {
    let loaded = LoadedFontRegistry::load(registry)?;
    let report = loaded.resolve(FontTarget::Html)?;
    let css = report.css();
    if let Some(output) = output {
        std::fs::write(output, css)?;
        println!("Local font CSS written to {}", Path::new(output).display());
    } else {
        print!("{css}");
    }
    Ok(())
}

fn emit(report: &crate::font::FontValidationReport, format: &str) -> Result<()> {
    if format == "text" {
        println!(
            "{} {} ({})",
            if report.valid { "valid" } else { "invalid" },
            report.registry_id,
            report.registry_digest.value
        );
        for diagnostic in &report.diagnostics {
            println!(
                "{:?} [{}]: {}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            );
        }
        return Ok(());
    }
    print!("{}", serialize(report, format)?);
    Ok(())
}

fn serialize(value: &impl Serialize, format: &str) -> Result<String> {
    match format {
        "json" => Ok(format!("{}\n", serde_json::to_string_pretty(value)?)),
        "yaml" => Ok(serde_yaml_ng::to_string(value)?),
        "text" => anyhow::bail!("text output is only available for validation"),
        _ => anyhow::bail!("unknown output format '{format}'; expected text, json, or yaml"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_rejects_unknown_format() {
        let value = serde_json::json!({"valid": true});
        assert!(serialize(&value, "toml").is_err());
    }

    #[test]
    fn severity_is_available_for_text_diagnostics() {
        assert_eq!(
            format!("{:?}", crate::font::FontDiagnosticSeverity::Warning),
            "Warning"
        );
    }
}
