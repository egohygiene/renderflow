//! Ollama local AI provider.
//!
//! This provider communicates with a locally-running [Ollama](https://ollama.ai)
//! server via its REST API (`POST /api/generate`).  It is the default provider
//! for local-first execution.

use std::{fmt, time::Duration};

use anyhow::{Context, Result};
use serde_json::json;
use tracing::debug;

use crate::ai::{
    provider::{AiCapabilities, AiCapability, AiModel, AiProvider},
    request::{AiRequest, AiResponse},
};

// ── OllamaProvider ────────────────────────────────────────────────────────────

/// An [`AiProvider`] that delegates to a locally-running Ollama server.
///
/// # Configuration
///
/// * **`endpoint`** – Base URL of the Ollama API server.  Defaults to
///   `http://localhost:11434`.
/// * **`default_models`** – Static list of model IDs shown by `renderflow ai
///   models`.  Ollama does not expose a stable models endpoint so this list is
///   configured at construction time; it does not gate execution.
#[derive(Debug)]
pub struct OllamaProvider {
    endpoint: String,
    default_models: Vec<String>,
}

impl OllamaProvider {
    /// Construct an [`OllamaProvider`] pointing at the given endpoint.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            default_models: vec![
                "mistral".to_string(),
                "llava".to_string(),
                "llama3".to_string(),
                "gemma".to_string(),
                "phi".to_string(),
            ],
        }
    }

    /// Construct an [`OllamaProvider`] with the default Ollama endpoint
    /// (`http://localhost:11434`).
    pub fn default_local() -> Self {
        Self::new("http://localhost:11434")
    }

    /// Override the list of model IDs reported by [`AiProvider::models`].
    #[must_use]
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.default_models = models;
        self
    }

    fn call_api(&self, request: &AiRequest) -> Result<(String, Option<String>)> {
        anyhow::ensure!(
            !request
                .cancellation
                .as_ref()
                .is_some_and(|token| token.is_cancelled()),
            "ai.transport.cancelled"
        );
        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let mut body = json!({
            "model": request.model,
            "prompt": request.prompt,
            "stream": false,
        });
        let body_object = body.as_object_mut().expect("Ollama request is an object");
        if let Some(schema) = &request.output_schema {
            body_object.insert("format".to_string(), schema.clone());
        } else if request.output_format == Some(crate::ai::OutputFormat::Json) {
            body_object.insert("format".to_string(), json!("json"));
        }
        let mut options = serde_json::Map::new();
        if let Some(temperature) = request.params.temperature {
            options.insert("temperature".to_string(), json!(temperature));
        }
        if let Some(max_tokens) = request.params.max_tokens {
            options.insert("num_predict".to_string(), json!(max_tokens));
        }
        if let Some(seed) = request.params.seed {
            options.insert("seed".to_string(), json!(seed));
        }
        if let Some(top_p) = request.params.top_p {
            options.insert("top_p".to_string(), json!(top_p));
        }
        if !request.params.stop.is_empty() {
            options.insert("stop".to_string(), json!(request.params.stop));
        }
        if !options.is_empty() {
            body_object.insert("options".to_string(), serde_json::Value::Object(options));
        }

        debug!(
            provider = "ollama",
            endpoint = %url,
            model = %request.model,
            "Sending request to Ollama"
        );

        let agent = super::response::agent(request, &self.endpoint);
        let response = agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_json(body)
            .map_err(|_| anyhow::anyhow!("ai.transport.ollama_request_failed"))?;
        let body_str = super::response::read(response, request)?;
        let json: serde_json::Value = serde_json::from_str(&body_str)
            .map_err(|_| anyhow::anyhow!("ai.transport.invalid_json"))?;
        let content = json["response"]
            .as_str()
            .context("ai.transport.missing_content")?
            .to_string();
        let reported = json
            .get("model")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        Ok((content, reported))
    }
}

impl fmt::Display for OllamaProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ollama({})", self.endpoint)
    }
}

impl AiProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn is_local(&self) -> bool {
        true
    }

    fn capabilities(&self) -> AiCapabilities {
        AiCapabilities::new()
            .with(AiCapability::TextGeneration)
            .with(AiCapability::Summarization)
            .with(AiCapability::Translation)
            .with(AiCapability::CodeGeneration)
    }

    fn models(&self) -> Vec<AiModel> {
        self.default_models
            .iter()
            .map(|id| AiModel::new(id.clone(), true))
            .collect()
    }

    fn execute(&self, request: &AiRequest) -> Result<AiResponse> {
        let start = std::time::Instant::now();
        let (content, reported_model) = self
            .call_api(request)
            .with_context(|| format!("Ollama provider failed for model '{}'", request.model))?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(AiResponse {
            content,
            model: request.model.clone(),
            provider: self.name().to_string(),
            input_tokens: None, // Ollama does not report token counts here
            output_tokens: None,
            duration_ms: Some(duration_ms),
            reported_model,
        })
    }
}

// ── Diagnostic helper ─────────────────────────────────────────────────────────

/// Check whether the Ollama server is reachable.
///
/// Returns `Ok(())` on a successful `GET /` response, or an error describing
/// the connectivity problem.
pub fn check_ollama_connectivity(endpoint: &str) -> Result<()> {
    let url = format!("{}/api/tags", endpoint.trim_end_matches('/'));
    ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(1_500))
        .build()
        .get(&url)
        .call()
        .with_context(|| {
            format!(
                "Ollama server is not reachable at '{}'. \
                 Ensure Ollama is running: `ollama serve`",
                endpoint
            )
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_provider_name() {
        let p = OllamaProvider::default_local();
        assert_eq!(p.name(), "ollama");
    }

    #[test]
    fn test_ollama_provider_is_local() {
        let p = OllamaProvider::default_local();
        assert!(p.is_local());
    }

    #[test]
    fn test_ollama_capabilities() {
        let p = OllamaProvider::default_local();
        let caps = p.capabilities();
        assert!(caps.supports(&AiCapability::TextGeneration));
        assert!(caps.supports(&AiCapability::Summarization));
        assert!(!caps.supports(&AiCapability::Embeddings));
    }

    #[test]
    fn test_ollama_default_models() {
        let p = OllamaProvider::default_local();
        let models = p.models();
        assert!(!models.is_empty());
        assert!(models.iter().all(|m| m.is_local));
    }

    #[test]
    fn test_ollama_with_custom_models() {
        let p = OllamaProvider::default_local().with_models(vec!["custom-model".to_string()]);
        let models = p.models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "custom-model");
    }

    #[test]
    fn test_ollama_display() {
        let p = OllamaProvider::default_local();
        assert!(p.to_string().contains("ollama"));
        assert!(p.to_string().contains("11434"));
    }
}
