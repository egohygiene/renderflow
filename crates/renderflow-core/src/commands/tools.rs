use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::toolchain::{ToolAvailability, ToolDescriptor, ToolRegistry};
use crate::transforms::yaml_loader::load_tool_registry_from_yaml;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructuredFormat {
    Text,
    Json,
    Yaml,
}

impl StructuredFormat {
    fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            other => {
                anyhow::bail!("unknown tools output format '{other}'; supported: text, json, yaml")
            }
        }
    }
}

fn load_registry(transforms: Option<&str>) -> Result<ToolRegistry> {
    match transforms {
        Some(path) => load_tool_registry_from_yaml(path)
            .with_context(|| format!("failed to load tool providers from '{path}'")),
        None => Ok(ToolRegistry::builtins()),
    }
}

fn emit_serialized<T: Serialize>(value: &T, format: StructuredFormat) -> Result<()> {
    match format {
        StructuredFormat::Text => anyhow::bail!("text output requires a dedicated renderer"),
        StructuredFormat::Json => {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
        StructuredFormat::Yaml => {
            print!("{}", serde_yaml_ng::to_string(value)?);
        }
    }
    Ok(())
}

pub fn run_list(transforms: Option<&str>, format: &str) -> Result<()> {
    let registry = load_registry(transforms)?;
    let inventory = registry.assess_all_current();
    let format = StructuredFormat::parse(format)?;

    if format != StructuredFormat::Text {
        return emit_serialized(&inventory, format);
    }

    println!("Renderflow Tools");
    println!("================");
    println!("registry: {}", registry.schema());
    println!();
    for tool in &inventory.tools {
        let executable = tool.selected_executable.as_deref().unwrap_or("-");
        let version = tool
            .normalized_version
            .as_deref()
            .or(tool.version_line.as_deref())
            .unwrap_or("-");
        println!(
            "{:<24} {:<24} {:<12} {:<16} {}",
            tool.id,
            tool.status.as_str(),
            tool.support_tier.as_str(),
            executable,
            version
        );
    }
    Ok(())
}

#[derive(Serialize)]
struct ToolInspection<'a> {
    descriptor: &'a ToolDescriptor,
    availability: &'a ToolAvailability,
}

pub fn run_inspect(id: &str, transforms: Option<&str>, format: &str) -> Result<()> {
    let registry = load_registry(transforms)?;
    let descriptor = registry
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("tool provider '{id}' is not registered"))?;
    let inventory = registry.assess_ids_current([id]);
    let availability = inventory
        .get(id)
        .expect("requested registered tool is present in its inventory");
    let format = StructuredFormat::parse(format)?;

    if format != StructuredFormat::Text {
        return emit_serialized(
            &ToolInspection {
                descriptor,
                availability,
            },
            format,
        );
    }

    println!("Tool: {}", descriptor.id);
    println!("Name: {}", descriptor.name);
    println!("Status: {}", availability.status.as_str());
    println!("Support tier: {}", descriptor.support_tier.as_str());
    println!(
        "Executable: {}",
        availability.selected_executable.as_deref().unwrap_or("-")
    );
    println!(
        "Version: {}",
        availability
            .version_line
            .as_deref()
            .unwrap_or("not captured")
    );
    println!("Discovery: {:?}", descriptor.discovery);
    println!("Determinism: {:?}", descriptor.determinism);
    println!("Locality: {:?}", descriptor.locality);
    println!("Fidelity: {:?}", descriptor.fidelity);
    println!("Capabilities:");
    for capability in &descriptor.capabilities {
        println!("  - {capability}");
    }
    if !descriptor.fallbacks.is_empty() {
        println!("Fallbacks:");
        for fallback in &descriptor.fallbacks {
            println!("  - {fallback}");
        }
    }
    if let Some(diagnostic) = &availability.diagnostic {
        println!("Diagnostic: {diagnostic}");
    }
    Ok(())
}

pub fn run_capabilities(transforms: Option<&str>, format: &str) -> Result<()> {
    let registry = load_registry(transforms)?;
    let capabilities: BTreeMap<String, Vec<String>> = registry.capabilities();
    let format = StructuredFormat::parse(format)?;

    if format != StructuredFormat::Text {
        return emit_serialized(&capabilities, format);
    }

    println!("Renderflow Capabilities");
    println!("=======================");
    for (capability, providers) in capabilities {
        println!("{capability}");
        for provider in providers {
            println!("  - {provider}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_format_rejects_unknown_value() {
        assert!(StructuredFormat::parse("toml").is_err());
    }

    #[test]
    fn built_in_capability_projection_is_not_empty() {
        let registry = ToolRegistry::builtins();
        assert!(!registry.capabilities().is_empty());
    }

    #[test]
    fn inventory_type_remains_structured_for_cli_serialization() {
        let inventory = crate::toolchain::ToolInventory { tools: Vec::new() };
        assert!(serde_json::to_string(&inventory).is_ok());
    }
}
