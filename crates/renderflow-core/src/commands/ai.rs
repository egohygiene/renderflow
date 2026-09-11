//! Handler for `renderflow ai` subcommands.
//!
//! Implements:
//! * `renderflow ai matrix`    – inspect model-granular compatibility
//! * `renderflow ai resolve`   – resolve a versioned skill against policy
//! * `renderflow ai skills`    – inspect and validate reviewed skills
//! * `renderflow ai providers` – list available providers
//! * `renderflow ai models`   – list available models per provider
//! * `renderflow ai doctor`   – connectivity diagnostics
//! * `renderflow ai cache`    – cache statistics

use std::str::FromStr;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::ai::{
    provider::AiProvider,
    providers::{OllamaProvider, OpenAiProvider},
    AiCandidateStatus, AiExecutionPreferenceV1, AiModelCatalog, AiSkillRegistry, AiSkillSpec,
    AiSkillValidationResult,
};
use crate::cache::{load_ai_cache, AiCache};

fn load_catalog(path: Option<&str>) -> Result<AiModelCatalog> {
    match path {
        Some(path) => AiModelCatalog::load(path),
        None => AiModelCatalog::bundled(),
    }
}

fn serialize_output(value: &impl Serialize, format: &str) -> Result<String> {
    match format {
        "json" => Ok(format!("{}\n", serde_json::to_string_pretty(value)?)),
        "yaml" => Ok(serde_yaml_ng::to_string(value)?),
        _ => anyhow::bail!("unknown output format '{format}'; expected text, json, or yaml"),
    }
}

// ── matrix and resolution ────────────────────────────────────────────────────

/// Run `renderflow ai matrix`.
pub fn run_matrix(format: &str, path: Option<&str>) -> Result<()> {
    let catalog = load_catalog(path)?;
    if format != "text" {
        print!("{}", serialize_output(&catalog, format)?);
        return Ok(());
    }
    println!(
        "AI model compatibility matrix {} ({})",
        catalog.schema_version, catalog.revision
    );
    println!();
    println!(
        "  {:<30} {:<20} {:<9} {:<12} Operations",
        "Provider", "Model", "Locality", "Availability"
    );
    println!(
        "  {:-<30} {:-<20} {:-<9} {:-<12} {:-<36}",
        "", "", "", "", ""
    );
    for provider in &catalog.providers {
        for model in &provider.models {
            let locality = provider.locality.to_string();
            let availability = format!("{:?}", model.availability).to_lowercase();
            let operations = model
                .operations
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            println!(
                "  {:<30} {:<20} {:<9} {:<12} {}",
                provider.id, model.id, locality, availability, operations
            );
        }
    }
    println!();
    println!("Availability is evidence, not an installation claim; use --format json for limits, licenses, and model-specific modalities.");
    Ok(())
}

/// Run `renderflow ai resolve`.
#[allow(clippy::too_many_arguments)]
pub fn run_resolve(
    skill_id: &str,
    skill_version: Option<&str>,
    preference: &str,
    allow_remote: bool,
    allow_unverified: bool,
    format: &str,
    catalog_path: Option<&str>,
) -> Result<()> {
    let catalog = load_catalog(catalog_path)?;
    let registry = AiSkillRegistry::bundled()?;
    let skill = registry
        .get(skill_id, skill_version)
        .with_context(|| format!("AI skill '{skill_id}' was not found"))?;
    let preference = AiExecutionPreferenceV1::from_str(preference)?;
    let report =
        catalog.resolve(&skill.resolution_request(preference, allow_remote, allow_unverified));
    if format != "text" {
        print!("{}", serialize_output(&report, format)?);
        return Ok(());
    }
    println!("AI resolution for {}@{}", skill.id, skill.version);
    println!("  preference: {}", preference);
    match &report.selected {
        Some(selected) => {
            println!(
                "  selected: {}:{} ({}, {:?})",
                selected.provider_id, selected.model_id, selected.locality, selected.availability
            );
            println!("  execution ready: {}", selected.execution_ready);
        }
        None => println!("  selected: none"),
    }
    println!();
    for candidate in &report.candidates {
        let marker = match candidate.status {
            AiCandidateStatus::Selected => "selected",
            AiCandidateStatus::Compatible => "compatible",
            AiCandidateStatus::Rejected => "rejected",
        };
        println!(
            "  [{}] {}:{}",
            marker, candidate.provider_id, candidate.model_id
        );
        for reason in &candidate.reasons {
            println!("    - {}: {}", reason.code, reason.message);
        }
    }
    Ok(())
}

// ── skills ───────────────────────────────────────────────────────────────────

/// Run `renderflow ai skills list`.
pub fn run_skills_list(format: &str) -> Result<()> {
    let registry = AiSkillRegistry::bundled()?;
    if format != "text" {
        let skills = registry.iter().collect::<Vec<_>>();
        print!("{}", serialize_output(&skills, format)?);
        return Ok(());
    }
    println!("Bundled Renderflow AI skills:");
    for skill in registry.iter() {
        println!("  {}@{} — {}", skill.id, skill.version, skill.purpose);
    }
    Ok(())
}

/// Run `renderflow ai skills inspect`.
pub fn run_skills_inspect(id: &str, version: Option<&str>, format: &str) -> Result<()> {
    let registry = AiSkillRegistry::bundled()?;
    let skill = registry
        .get(id, version)
        .with_context(|| format!("AI skill '{id}' was not found"))?;
    if format != "text" {
        print!("{}", serialize_output(skill, format)?);
        return Ok(());
    }
    println!("{}@{}", skill.id, skill.version);
    println!("  purpose: {}", skill.purpose);
    println!(
        "  inputs: {}",
        skill
            .input_modalities
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "  outputs: {}",
        skill
            .output_modalities
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("  hygiene policy: {}", skill.hygiene.policy_id);
    println!("  initial state: {:?}", skill.approval.initial_state);
    println!(
        "  human review required: {}",
        skill.approval.human_review_required
    );
    Ok(())
}

/// Run `renderflow ai skills validate`.
pub fn run_skills_validate(path: Option<&str>, format: &str) -> Result<()> {
    let results = if let Some(path) = path {
        let skill = AiSkillSpec::load(path)?;
        vec![AiSkillValidationResult {
            id: skill.id,
            version: skill.version,
            valid: true,
            error: None,
        }]
    } else {
        AiSkillRegistry::bundled()?.validate_all()
    };
    if format != "text" {
        print!("{}", serialize_output(&results, format)?);
        return Ok(());
    }
    for result in &results {
        if result.valid {
            println!("✓ {}@{}", result.id, result.version);
        } else {
            println!(
                "✗ {}@{}: {}",
                result.id,
                result.version,
                result
                    .error
                    .as_deref()
                    .unwrap_or("unknown validation error")
            );
        }
    }
    if results.iter().any(|result| !result.valid) {
        anyhow::bail!("one or more AI skills are invalid");
    }
    Ok(())
}

// ── providers ─────────────────────────────────────────────────────────────────

/// Run `renderflow ai providers`.
///
/// Prints a table of all built-in providers with their locality and declared
/// capabilities.
pub fn run_providers() -> Result<()> {
    let providers: Vec<Box<dyn AiProvider>> = vec![
        Box::new(OllamaProvider::default_local()),
        Box::new(OpenAiProvider::new()),
    ];

    println!("Available AI providers ({}):", providers.len());
    println!();
    println!("  {:<12} {:<10} Capabilities", "Provider", "Type");
    println!("  {:-<12} {:-<10} {:-<40}", "", "", "");

    for provider in &providers {
        let locality = if provider.is_local() {
            "local"
        } else {
            "remote"
        };
        let caps: Vec<String> = provider
            .capabilities()
            .iter()
            .map(|c| c.to_string())
            .collect();
        let mut sorted_caps = caps;
        sorted_caps.sort();
        println!(
            "  {:<12} {:<10} {}",
            provider.name(),
            locality,
            sorted_caps.join(", ")
        );
    }

    Ok(())
}

// ── models ────────────────────────────────────────────────────────────────────

/// Run `renderflow ai models`.
///
/// Prints the default model list for each known provider.
pub fn run_models() -> Result<()> {
    let providers: Vec<Box<dyn AiProvider>> = vec![
        Box::new(OllamaProvider::default_local()),
        Box::new(OpenAiProvider::new()),
    ];

    for provider in &providers {
        let models = provider.models();
        let locality = if provider.is_local() {
            "local"
        } else {
            "remote"
        };
        println!("{} ({}):", provider.name(), locality);
        if models.is_empty() {
            println!("  (no models configured)");
        } else {
            for model in &models {
                if let Some(desc) = &model.description {
                    println!("  {}  —  {}", model.id, desc);
                } else {
                    println!("  {}", model.id);
                }
            }
        }
        println!();
    }

    Ok(())
}

// ── doctor ────────────────────────────────────────────────────────────────────

/// Run `renderflow ai doctor`.
///
/// Probes each provider's endpoint and prints a connectivity report.
/// Returns `Ok(())` even when issues are found (advisory output).
pub fn run_doctor(ollama_endpoint: &str) -> Result<()> {
    println!("AI provider diagnostics:");
    println!();

    // ── Ollama ────────────────────────────────────────────────────────────────
    print!("  [ollama] Checking connectivity to {} … ", ollama_endpoint);
    match crate::ai::providers::ollama::check_ollama_connectivity(ollama_endpoint) {
        Ok(()) => println!("✓ OK"),
        Err(e) => {
            println!("✗ FAILED");
            println!("          {}", e);
            println!("          Tip: start Ollama with `ollama serve`");
        }
    }

    // ── OpenAI ────────────────────────────────────────────────────────────────
    println!();
    print!("  [openai] Checking environment variable OPENAI_API_KEY … ");
    match std::env::var("OPENAI_API_KEY") {
        Ok(v) if !v.is_empty() => println!("✓ set"),
        Ok(_) => {
            println!("✗ empty");
            println!("          Set OPENAI_API_KEY to your OpenAI API key");
        }
        Err(_) => {
            println!("✗ not set");
            println!("          Set OPENAI_API_KEY to your OpenAI API key");
        }
    }

    Ok(())
}

// ── cache ─────────────────────────────────────────────────────────────────────

/// Run `renderflow ai cache`.
///
/// Reads the AI cache file (if it exists) and prints summary statistics.
pub fn run_cache(path: &str) -> Result<()> {
    use std::path::Path;

    let cache_path = Path::new(path);

    if !cache_path.exists() {
        println!("AI cache file not found: {}", path);
        println!("Cache is empty (no entries).");
        return Ok(());
    }

    let cache: AiCache = load_ai_cache(cache_path);

    // Gather statistics.
    let total = cache.len();

    if total == 0 {
        println!("AI cache ({}): 0 entries", path);
        return Ok(());
    }

    // Count entries per model.
    let mut by_model: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for entry in cache.entries() {
        *by_model.entry(entry.model.as_str()).or_default() += 1;
    }

    let mut model_list: Vec<(&str, usize)> = by_model.into_iter().collect();
    model_list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    println!("AI cache: {} ({} entries)", path, total);
    println!();
    println!("  {:<30} Entries", "Model");
    println!("  {:-<30} {:-<7}", "", "");
    for (model, count) in &model_list {
        println!("  {:<30} {}", model, count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_providers_succeeds() {
        assert!(run_providers().is_ok());
    }

    #[test]
    fn test_run_models_succeeds() {
        assert!(run_models().is_ok());
    }

    #[test]
    fn test_run_doctor_succeeds_even_when_ollama_unreachable() {
        // Ollama is not running in the test environment; doctor must not fail.
        assert!(run_doctor("http://localhost:19999").is_ok());
    }

    #[test]
    fn test_run_cache_missing_file_succeeds() {
        assert!(run_cache("/tmp/__renderflow_nonexistent_cache_xyz__.json").is_ok());
    }

    #[test]
    fn test_run_cache_existing_file_succeeds() {
        use crate::cache::{save_ai_cache, AiCache, AiCacheEntry};

        let dir = tempfile::tempdir().unwrap();
        let cache_file = dir.path().join("test-cache.json");

        let mut cache = AiCache::default();
        cache.insert(
            "hash1".to_string(),
            AiCacheEntry {
                input_hash: "hash1".to_string(),
                model: "mistral".to_string(),
                timestamp: 0,
                output: "hello".to_string(),
            },
        );
        save_ai_cache(&cache, &cache_file).unwrap();

        let path = cache_file.to_str().unwrap();
        assert!(run_cache(path).is_ok());
    }
}
