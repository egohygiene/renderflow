//! OpenAI-compatible AI provider.
//!
//! This provider communicates with any API that implements the OpenAI chat
//! completions format (`POST /v1/chat/completions`).  It supports OpenAI's
//! public API as well as compatible alternatives (Azure OpenAI, OpenRouter,
//! LM Studio, etc.).
//!
//! # Security
//!
//! API keys are resolved from an environment variable (`api_key_env`) at
//! execution time.  Plaintext keys (`api_key`) are accepted for backward
//! compatibility but emit a `WARN` log to encourage migration to environment
//! variables.  Keys are **never logged** at any log level.

use std::fmt;

use anyhow::{Context, Result};
use serde_json::json;
use tracing::warn;

use crate::ai::{
    provider::{AiCapabilities, AiCapability, AiModel, AiProvider},
    request::{AiRequest, AiResponse},
};

// ── OpenAiProvider ────────────────────────────────────────────────────────────

/// An [`AiProvider`] that uses the OpenAI chat completions API.
///
/// # Configuration
///
/// * **`endpoint`** – Base URL for the API.  Defaults to
///   `https://api.openai.com`.
/// * **`api_key_env`** – Name of the environment variable containing the API
///   key.  Resolved at execution time.
/// * **`api_key`** – Plaintext API key (fallback when `api_key_env` is not set
///   or the variable is absent).  Discouraged for production use.
/// * **`default_models`** – Static list of model IDs shown by `renderflow ai
///   models`.
#[derive(Debug)]
pub struct OpenAiProvider {
    endpoint: String,
    api_key_env: Option<String>,
    api_key: Option<String>,
    default_models: Vec<String>,
}

impl OpenAiProvider {
    /// Construct an [`OpenAiProvider`] pointing at the OpenAI production API.
    pub fn new() -> Self {
        Self {
            endpoint: "https://api.openai.com".to_string(),
            api_key_env: None,
            api_key: None,
            default_models: vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ],
        }
    }

    /// Override the API base URL (useful for compatible APIs or proxies).
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set the environment variable name used to resolve the API key.
    ///
    /// The variable is read at execution time, not at construction time.
    #[must_use]
    pub fn with_api_key_env(mut self, var_name: impl Into<String>) -> Self {
        self.api_key_env = Some(var_name.into());
        self
    }

    /// Set a plaintext API key.
    ///
    /// Prefer [`with_api_key_env`](Self::with_api_key_env) in production.
    #[must_use]
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Override the list of model IDs reported by [`AiProvider::models`].
    #[must_use]
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.default_models = models;
        self
    }

    /// Resolve the API key from the environment variable or the plaintext
    /// fallback, without logging the key value.
    fn resolve_api_key(&self) -> Result<Option<String>> {
        let Some(var_name) = self.api_key_env.as_deref() else {
            // No env var configured; fall back to plaintext (with a warning).
            if self.api_key.is_some() {
                warn!(
                    provider = "openai",
                    "Using plaintext api_key; prefer api_key_env to keep secrets out of configs"
                );
            }
            return Ok(self.api_key.clone());
        };

        match std::env::var(var_name) {
            Ok(value) if !value.is_empty() => Ok(Some(value)),
            Ok(_) => {
                if let Some(key) = &self.api_key {
                    warn!(
                        provider = "openai",
                        env_var = %var_name,
                        "API key env var is empty; falling back to plaintext api_key"
                    );
                    Ok(Some(key.clone()))
                } else {
                    anyhow::bail!(
                        "OpenAI API key env var '{}' is set but empty and no fallback api_key is configured",
                        var_name
                    )
                }
            }
            Err(std::env::VarError::NotPresent) => {
                if let Some(key) = &self.api_key {
                    warn!(
                        provider = "openai",
                        env_var = %var_name,
                        "API key env var not set; falling back to plaintext api_key"
                    );
                    Ok(Some(key.clone()))
                } else {
                    anyhow::bail!(
                        "OpenAI API key env var '{}' is not set and no fallback api_key is configured",
                        var_name
                    )
                }
            }
            Err(std::env::VarError::NotUnicode(_)) => anyhow::bail!(
                "OpenAI API key env var '{}' contains non-UTF-8 data",
                var_name
            ),
        }
    }


    /// Extract token usage from an OpenAI response JSON object.
    fn extract_tokens(json: &serde_json::Value) -> (Option<u32>, Option<u32>) {
        let input = json["usage"]["prompt_tokens"]
            .as_u64()
            .map(|v| v as u32);
        let output = json["usage"]["completion_tokens"]
            .as_u64()
            .map(|v| v as u32);
        (input, output)
    }

    fn call_api_with_tokens(&self, request: &AiRequest) -> Result<(String, Option<u32>, Option<u32>)> {
        let url = format!(
            "{}/v1/chat/completions",
            self.endpoint.trim_end_matches('/')
        );
        let body = json!({
            "model": request.model,
            "messages": [{"role": "user", "content": request.prompt}],
        });

        let mut req = ureq::post(&url).set("Content-Type", "application/json");
        if let Some(key) = self.resolve_api_key()? {
            req = req.set("Authorization", &format!("Bearer {}", key));
        }

        let body_str = req
            .send_json(body)
            .with_context(|| format!("Failed to POST to OpenAI endpoint '{}'", url))?
            .into_string()
            .context("Failed to read OpenAI HTTP response body")?;

        let json: serde_json::Value = serde_json::from_str(&body_str).with_context(|| {
            format!(
                "Failed to parse OpenAI JSON response; body was: {}",
                body_str
            )
        })?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "OpenAI response missing 'choices[0].message.content'; received: {}",
                    body_str
                )
            })?;

        let (input_tokens, output_tokens) = Self::extract_tokens(&json);
        Ok((content, input_tokens, output_tokens))
    }
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OpenAiProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "openai({})", self.endpoint)
    }
}

impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn is_local(&self) -> bool {
        false
    }

    fn capabilities(&self) -> AiCapabilities {
        AiCapabilities::new()
            .with(AiCapability::TextGeneration)
            .with(AiCapability::Summarization)
            .with(AiCapability::Translation)
            .with(AiCapability::CodeGeneration)
            .with(AiCapability::StructuredJsonOutput)
    }

    fn models(&self) -> Vec<AiModel> {
        self.default_models
            .iter()
            .map(|id| AiModel::new(id.clone(), false))
            .collect()
    }

    fn execute(&self, request: &AiRequest) -> Result<AiResponse> {
        let start = std::time::Instant::now();
        let (content, input_tokens, output_tokens) = self
            .call_api_with_tokens(request)
            .with_context(|| {
                format!(
                    "OpenAI provider failed for model '{}'",
                    request.model
                )
            })?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(AiResponse {
            content,
            model: request.model.clone(),
            provider: self.name().to_string(),
            input_tokens,
            output_tokens,
            duration_ms: Some(duration_ms),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_name() {
        let p = OpenAiProvider::new();
        assert_eq!(p.name(), "openai");
    }

    #[test]
    fn test_openai_is_not_local() {
        let p = OpenAiProvider::new();
        assert!(!p.is_local());
    }

    #[test]
    fn test_openai_capabilities() {
        let p = OpenAiProvider::new();
        let caps = p.capabilities();
        assert!(caps.supports(&AiCapability::TextGeneration));
        assert!(caps.supports(&AiCapability::StructuredJsonOutput));
    }

    #[test]
    fn test_openai_default_models() {
        let p = OpenAiProvider::new();
        let models = p.models();
        assert!(!models.is_empty());
        assert!(models.iter().all(|m| !m.is_local));
    }

    #[test]
    fn test_openai_resolve_key_from_env() {
        let p = OpenAiProvider::new().with_api_key_env("PATH");
        let expected = std::env::var("PATH").unwrap();
        let key = p.resolve_api_key().unwrap();
        assert_eq!(key.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn test_openai_resolve_key_missing_env_falls_back_to_plaintext() {
        let p = OpenAiProvider::new()
            .with_api_key_env("RENDERFLOW_TEST_MISSING_KEY")
            .with_api_key("plaintext-key");
        let key = p.resolve_api_key().unwrap();
        assert_eq!(key.as_deref(), Some("plaintext-key"));
    }

    #[test]
    fn test_openai_resolve_key_missing_env_no_fallback_errors() {
        let p = OpenAiProvider::new().with_api_key_env("RENDERFLOW_TEST_MISSING_KEY");
        assert!(p.resolve_api_key().is_err());
    }

    #[test]
    fn test_openai_extract_tokens_from_json() {
        let json = serde_json::json!({
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5
            }
        });
        let (input, output) = OpenAiProvider::extract_tokens(&json);
        assert_eq!(input, Some(10));
        assert_eq!(output, Some(5));
    }

    #[test]
    fn test_openai_extract_tokens_missing_returns_none() {
        let json = serde_json::json!({});
        let (input, output) = OpenAiProvider::extract_tokens(&json);
        assert!(input.is_none());
        assert!(output.is_none());
    }
}
