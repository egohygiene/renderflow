use anyhow::Result;
use std::process::Command;

use crate::transforms::plugin::{PluginCapabilities, PluginMetadata, PluginRegistry};
// ── list ──────────────────────────────────────────────────────────────────────

/// Run `renderflow plugin list`.
///
/// Prints a summary table of every plugin in `registry`:
///
/// ```text
/// Registered plugins (2):
///   upper       1.0.0   A test plugin
///   lower       0.5.0   Converts text to lowercase
/// ```
pub fn run_list(registry: &PluginRegistry) -> Result<()> {
    let mut names: Vec<&str> = registry.plugin_names();
    names.sort();

    if names.is_empty() {
        println!("No plugins registered.");
        return Ok(());
    }

    println!("Registered plugins ({}):", names.len());
    for name in &names {
        if let Some(meta) = registry.metadata(name) {
            println!(
                "  {:<20} {:<10}  {}",
                meta.id, meta.version, meta.description
            );
        } else {
            println!("  {:<20} {:<10}  (no metadata)", name, "-");
        }
    }

    Ok(())
}

// ── info ──────────────────────────────────────────────────────────────────────

/// Run `renderflow plugin info <name>`.
///
/// Prints the full [`PluginMetadata`] for the named plugin, or returns an
/// error when the plugin is not registered.
pub fn run_info(registry: &PluginRegistry, name: &str) -> Result<()> {
    let info = registry
        .plugin_info(name)
        .ok_or_else(|| anyhow::anyhow!("plugin '{}' is not registered", name))?;

    println!("Plugin: {}", info.name);

    if let Some(meta) = info.metadata {
        print_metadata(meta);
    } else {
        println!("  (no metadata available)");
    }

    Ok(())
}

fn print_metadata(meta: &PluginMetadata) {
    println!("  Version:     {}", meta.version);

    if !meta.author.is_empty() {
        println!("  Author:      {}", meta.author);
    }

    if !meta.description.is_empty() {
        println!("  Description: {}", meta.description);
    }

    if let Some(license) = &meta.license {
        println!("  License:     {}", license);
    }

    if !meta.supported_transforms.is_empty() {
        println!(
            "  Transforms:  {}",
            meta.supported_transforms.join(", ")
        );
    }

    if !meta.required_tools.is_empty() {
        println!("  Requires:    {}", meta.required_tools.join(", "));
    }

    print_capabilities(&meta.capabilities);
}

fn print_capabilities(caps: &PluginCapabilities) {
    let mut enabled: Vec<&str> = Vec::new();
    if caps.dry_run {
        enabled.push("dry-run");
    }
    if caps.caching {
        enabled.push("caching");
    }
    if caps.diagnostics {
        enabled.push("diagnostics");
    }
    if caps.optimization {
        enabled.push("optimization");
    }
    if enabled.is_empty() {
        println!("  Capabilities: (none)");
    } else {
        println!("  Capabilities: {}", enabled.join(", "));
    }
}

// ── validate ─────────────────────────────────────────────────────────────────

/// Run `renderflow plugin validate`.
///
/// Validates all plugin metadata in `registry` and prints a report.
/// Returns an error when any validation issue is found.
pub fn run_validate(registry: &PluginRegistry) -> Result<()> {
    let issues = registry.validate_all();

    if issues.is_empty() {
        println!(
            "All {} plugin(s) validated successfully.",
            registry.len()
        );
        return Ok(());
    }

    eprintln!("Plugin validation issues ({}):", issues.len());
    for (name, msg) in &issues {
        eprintln!("  [{}] {}", name, msg);
    }

    anyhow::bail!(
        "{} plugin validation issue(s) found",
        issues.len()
    )
}

// ── doctor ────────────────────────────────────────────────────────────────────

/// Run `renderflow plugin doctor`.
///
/// Runs diagnostics on every registered plugin:
/// * Validates metadata.
/// * Checks that required external tools are present on `PATH`.
///
/// Prints a report and returns `Ok(())` even when issues are found (so the
/// caller can decide whether to treat the output as advisory or fatal).
pub fn run_doctor(registry: &PluginRegistry) -> Result<()> {
    let mut names: Vec<&str> = registry.plugin_names();
    names.sort();

    if names.is_empty() {
        println!("No plugins registered. Nothing to diagnose.");
        return Ok(());
    }

    let mut all_ok = true;

    println!("Plugin diagnostics ({} plugin(s)):", names.len());

    for name in &names {
        let mut issues: Vec<String> = Vec::new();

        // 1. Metadata validation.
        if let Some(meta) = registry.metadata(name) {
            if let Err(e) = meta.validate() {
                issues.push(format!("invalid metadata: {}", e));
            }

            // 2. Required tools check.
            for tool in &meta.required_tools {
                if !tool_is_available(tool) {
                    issues.push(format!(
                        "required tool '{}' was not found on PATH",
                        tool
                    ));
                }
            }
        }

        if issues.is_empty() {
            println!("  [{}] ✓ OK", name);
        } else {
            all_ok = false;
            println!("  [{}] ✗ {} issue(s):", name, issues.len());
            for issue in &issues {
                println!("      - {}", issue);
            }
        }
    }

    if all_ok {
        println!("\nAll plugins passed diagnostics.");
    } else {
        println!("\nSome plugins have issues. Review the output above.");
    }

    Ok(())
}

/// Return `true` when `tool` can be found on the system `PATH`.
fn tool_is_available(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::transforms::plugin::{PluginExecutor, PluginRegistry};

    struct DummyPlugin(&'static str);
    impl PluginExecutor for DummyPlugin {
        fn name(&self) -> &str {
            self.0
        }
        fn execute(&self, input: String) -> anyhow::Result<String> {
            Ok(input)
        }
    }

    fn make_meta(id: &str) -> PluginMetadata {
        PluginMetadata::new(id, "1.0.0")
            .with_description("A dummy plugin")
            .with_author("Tester")
    }

    // ── run_list ──────────────────────────────────────────────────────────────

    #[test]
    fn test_list_empty_registry_succeeds() {
        let registry = PluginRegistry::new();
        assert!(run_list(&registry).is_ok());
    }

    #[test]
    fn test_list_with_plugins_succeeds() {
        let mut registry = PluginRegistry::new();
        registry
            .register_with_metadata(Arc::new(DummyPlugin("alpha")), make_meta("alpha"))
            .unwrap();
        assert!(run_list(&registry).is_ok());
    }

    #[test]
    fn test_list_without_metadata_succeeds() {
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(DummyPlugin("bare")));
        assert!(run_list(&registry).is_ok());
    }

    // ── run_info ──────────────────────────────────────────────────────────────

    #[test]
    fn test_info_known_plugin_succeeds() {
        let mut registry = PluginRegistry::new();
        registry
            .register_with_metadata(Arc::new(DummyPlugin("alpha")), make_meta("alpha"))
            .unwrap();
        assert!(run_info(&registry, "alpha").is_ok());
    }

    #[test]
    fn test_info_unknown_plugin_returns_error() {
        let registry = PluginRegistry::new();
        let result = run_info(&registry, "ghost");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ghost"));
    }

    #[test]
    fn test_info_plugin_without_metadata_succeeds() {
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(DummyPlugin("bare")));
        assert!(run_info(&registry, "bare").is_ok());
    }

    // ── run_validate ──────────────────────────────────────────────────────────

    #[test]
    fn test_validate_empty_registry_succeeds() {
        let registry = PluginRegistry::new();
        assert!(run_validate(&registry).is_ok());
    }

    #[test]
    fn test_validate_all_valid_succeeds() {
        let mut registry = PluginRegistry::new();
        registry
            .register_with_metadata(Arc::new(DummyPlugin("alpha")), make_meta("alpha"))
            .unwrap();
        assert!(run_validate(&registry).is_ok());
    }

    // ── run_doctor ────────────────────────────────────────────────────────────

    #[test]
    fn test_doctor_empty_registry_succeeds() {
        let registry = PluginRegistry::new();
        assert!(run_doctor(&registry).is_ok());
    }

    #[test]
    fn test_doctor_all_ok_succeeds() {
        let mut registry = PluginRegistry::new();
        registry
            .register_with_metadata(Arc::new(DummyPlugin("alpha")), make_meta("alpha"))
            .unwrap();
        assert!(run_doctor(&registry).is_ok());
    }

    #[test]
    fn test_doctor_missing_tool_still_returns_ok() {
        let mut registry = PluginRegistry::new();
        let meta = PluginMetadata::new("needs-tool", "1.0.0")
            .with_required_tools(vec!["__renderflow_nonexistent_tool_xyz__".to_string()]);
        registry
            .register_with_metadata(Arc::new(DummyPlugin("needs-tool")), meta)
            .unwrap();
        // doctor returns Ok even when tools are missing (advisory output only)
        assert!(run_doctor(&registry).is_ok());
    }

    // ── tool_is_available ─────────────────────────────────────────────────────

    #[test]
    fn test_tool_is_available_returns_false_for_nonexistent() {
        assert!(!tool_is_available("__renderflow_nonexistent_xyz__"));
    }
}
