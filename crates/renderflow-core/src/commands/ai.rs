//! Handler for `renderflow ai` subcommands.
//!
//! Implements:
//! * `renderflow ai providers` – list available providers
//! * `renderflow ai models`   – list available models per provider
//! * `renderflow ai doctor`   – connectivity diagnostics
//! * `renderflow ai cache`    – cache statistics

use anyhow::Result;

use crate::ai::{
    provider::AiProvider,
    providers::{OllamaProvider, OpenAiProvider},
};
use crate::cache::{load_ai_cache, AiCache};

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
        let locality = if provider.is_local() { "local" } else { "remote" };
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
        let locality = if provider.is_local() { "local" } else { "remote" };
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
    let mut by_model: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
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
