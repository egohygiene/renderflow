//! Intelligent transformation framework for AI-powered document processing.
//!
//! This module provides a provider-agnostic architecture for integrating AI
//! backends into Renderflow's transformation graph.  AI transforms are
//! first-class graph edges that participate in planning, optimization, caching,
//! and execution just like deterministic transforms.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  AiTransform (Transform impl)                               │
//! │  ┌──────────────────────────────────────────────────────┐   │
//! │  │  AiProvider (trait)                                  │   │
//! │  │  ├── OllamaProvider  (local)                         │   │
//! │  │  └── OpenAiProvider  (remote)                        │   │
//! │  └──────────────────────────────────────────────────────┘   │
//! │  Cache (provider + model + prompt_version + params)         │
//! │  Metrics (request_count, tokens, duration, cache hits)      │
//! │  Retry  (configurable back-off for transient failures)      │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Quick start
//!
//! ```rust,no_run
//! use renderflow::ai::providers::OllamaProvider;
//! use renderflow::ai::request::AiRequest;
//! use renderflow::ai::provider::AiProvider;
//!
//! let provider = OllamaProvider::default_local();
//! let request = AiRequest::new("mistral", "Summarise this document: …");
//! let response = provider.execute(&request).expect("AI call succeeded");
//! println!("{}", response.content);
//! ```

pub mod metrics;
pub mod output;
pub mod provider;
pub mod providers;
pub mod request;
pub mod retry;

pub use metrics::{AiExecutionMetrics, SharedMetrics};
pub use output::validate_output;
pub use provider::{AiCapabilities, AiCapability, AiExecutionPreference, AiModel, AiProvider};
pub use providers::{OllamaProvider, OpenAiProvider};
pub use request::{AiRequest, AiResponse, GenerationParameters, OutputFormat};
pub use retry::RetryConfig;

// ── compute_ai_cache_key ──────────────────────────────────────────────────────

use sha2::{Digest, Sha256};

/// Compute a stable SHA-256 cache key for an AI request.
///
/// The key incorporates all fields that affect the generated output:
///
/// | Field            | Why it matters                                          |
/// |------------------|---------------------------------------------------------|
/// | `provider`       | Different backends produce different outputs.           |
/// | `model`          | Different models produce different outputs.             |
/// | `prompt`         | Prompt text directly determines the output.             |
/// | `prompt_version` | Changing the template should invalidate cached entries. |
/// | `params`         | Temperature, max_tokens etc. affect generation.         |
///
/// This replaces the narrower hash in [`crate::cache::compute_ai_input_hash`]
/// (which only covers `prompt + model`) when full provider-aware caching is
/// needed.
pub fn compute_ai_cache_key(
    provider: &str,
    model: &str,
    prompt: &str,
    prompt_version: Option<&str>,
    params_fragment: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"provider\x00");
    hasher.update(provider.as_bytes());
    hasher.update(b"\x00model\x00");
    hasher.update(model.as_bytes());
    hasher.update(b"\x00prompt\x00");
    hasher.update(prompt.as_bytes());
    hasher.update(b"\x00prompt_version\x00");
    hasher.update(prompt_version.unwrap_or("").as_bytes());
    hasher.update(b"\x00params\x00");
    hasher.update(params_fragment.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_changes_with_provider() {
        let k1 = compute_ai_cache_key("ollama", "mistral", "hello", None, "");
        let k2 = compute_ai_cache_key("openai", "mistral", "hello", None, "");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_changes_with_model() {
        let k1 = compute_ai_cache_key("ollama", "mistral", "hello", None, "");
        let k2 = compute_ai_cache_key("ollama", "llama3", "hello", None, "");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_changes_with_prompt() {
        let k1 = compute_ai_cache_key("ollama", "mistral", "foo", None, "");
        let k2 = compute_ai_cache_key("ollama", "mistral", "bar", None, "");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_changes_with_prompt_version() {
        let k1 = compute_ai_cache_key("ollama", "mistral", "foo", Some("v1"), "");
        let k2 = compute_ai_cache_key("ollama", "mistral", "foo", Some("v2"), "");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_changes_with_params() {
        let k1 = compute_ai_cache_key("ollama", "mistral", "foo", None, "temp:0.5");
        let k2 = compute_ai_cache_key("ollama", "mistral", "foo", None, "temp:0.9");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_is_stable() {
        let k1 = compute_ai_cache_key("ollama", "mistral", "foo", Some("v1"), "temp:0.7");
        let k2 = compute_ai_cache_key("ollama", "mistral", "foo", Some("v1"), "temp:0.7");
        assert_eq!(k1, k2);
    }
}
