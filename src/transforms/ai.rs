use std::fmt;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;
use tracing::{debug, warn};

use crate::cache::{compute_ai_input_hash, current_unix_timestamp, load_ai_cache, save_ai_cache, AiCacheEntry};
use super::Transform;

/// The AI backend to use for an [`AiTransform`].
///
/// * [`AiBackend::Ollama`] – a locally-running Ollama instance accessed
///   via its REST API (`/api/generate`).
/// * [`AiBackend::OpenAi`] – any OpenAI-compatible chat-completion API
///   (`/v1/chat/completions`).  Set `api_key` on the transform to pass
///   the `Authorization: Bearer …` header.
#[derive(Debug, Clone, PartialEq)]
pub enum AiBackend {
    /// Ollama local-model backend (default endpoint: `http://localhost:11434`).
    Ollama,
    /// OpenAI-compatible API backend.
    OpenAi,
}

impl fmt::Display for AiBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiBackend::Ollama => write!(f, "ollama"),
            AiBackend::OpenAi => write!(f, "openai"),
        }
    }
}

impl std::str::FromStr for AiBackend {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ollama" => Ok(AiBackend::Ollama),
            "openai" => Ok(AiBackend::OpenAi),
            _ => anyhow::bail!(
                "'{}' is not a known AI backend. Supported backends: ollama, openai",
                s
            ),
        }
    }
}

/// An AI-powered [`Transform`] that submits document content to a large
/// language model and returns the generated response.
///
/// `AiTransform` supports two backends:
///
/// * **Ollama** – a locally-running Ollama server (e.g. `llava`, `mistral`).
///   The transform POSTs to `<endpoint>/api/generate` with `stream: false`.
/// * **OpenAI-compatible** – any service that implements the OpenAI chat
///   completions API.  The transform POSTs to `<endpoint>/v1/chat/completions`
///   with an optional `Authorization: Bearer <api_key>` header.
///
/// The `prompt_template` may contain an `{input}` placeholder which is
/// replaced by the transform input before the request is made.
///
/// When `artifact_path` is set the response is also written to that file
/// in addition to being returned from [`Transform::apply`].
///
/// # Example (Ollama)
///
/// ```rust,no_run
/// use renderflow::transforms::ai::{AiBackend, AiTransform};
/// use renderflow::transforms::Transform;
///
/// let t = AiTransform::builder()
///     .name("summarise")
///     .backend(AiBackend::Ollama)
///     .model("mistral")
///     .prompt_template("Summarise the following text: {input}")
///     .endpoint("http://localhost:11434")
///     .build();
///
/// // In production the call would reach the Ollama server; here we just
/// // show the builder API.
/// let _ = t; // avoid unused-variable warning in doc test
/// ```
pub struct AiTransform {
    name: String,
    backend: AiBackend,
    model: String,
    prompt_template: String,
    endpoint: String,
    api_key: Option<String>,
    artifact_path: Option<String>,
    /// Optional path to the AI result cache file.
    ///
    /// When set, [`AiTransform::apply`] checks the cache before calling the
    /// AI backend and stores the result with metadata after a successful call.
    cache_path: Option<String>,
}

// ── Builder ───────────────────────────────────────────────────────────────────

/// Fluent builder for [`AiTransform`].
///
/// Obtain one via [`AiTransform::builder`].
pub struct AiTransformBuilder {
    name: Option<String>,
    backend: Option<AiBackend>,
    model: Option<String>,
    prompt_template: Option<String>,
    endpoint: Option<String>,
    api_key: Option<String>,
    artifact_path: Option<String>,
    cache_path: Option<String>,
}

impl AiTransformBuilder {
    fn new() -> Self {
        Self {
            name: None,
            backend: None,
            model: None,
            prompt_template: None,
            endpoint: None,
            api_key: None,
            artifact_path: None,
            cache_path: None,
        }
    }

    /// Human-readable name shown in logs and error context.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// AI backend to use.
    pub fn backend(mut self, backend: AiBackend) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Model identifier (e.g. `"mistral"`, `"llava"`, `"gpt-4o"`).
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Prompt template.  Use `{input}` as a placeholder for the transform input.
    pub fn prompt_template(mut self, template: impl Into<String>) -> Self {
        self.prompt_template = Some(template.into());
        self
    }

    /// Base URL of the AI backend (e.g. `"http://localhost:11434"`).
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Optional API key sent as `Authorization: Bearer <key>`.
    ///
    /// Required for OpenAI-compatible backends that enforce authentication.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Optional file path where the AI response will also be written as an
    /// artifact.
    pub fn artifact_path(mut self, path: impl Into<String>) -> Self {
        self.artifact_path = Some(path.into());
        self
    }

    /// Optional path to the AI result cache file.
    ///
    /// When set, [`AiTransform::apply`] will:
    /// 1. Compute a hash of the rendered prompt and model.
    /// 2. Return a cached output immediately on a cache hit (skipping the AI call).
    /// 3. Store the result with metadata (model, timestamp, input hash) on a miss.
    pub fn cache_path(mut self, path: impl Into<String>) -> Self {
        self.cache_path = Some(path.into());
        self
    }

    /// Consume the builder and return an [`AiTransform`].
    ///
    /// Uses sensible defaults for any unset fields:
    /// * `name` → `"AiTransform"`
    /// * `backend` → [`AiBackend::Ollama`]
    /// * `model` → `"mistral"`
    /// * `prompt_template` → `"{input}"`
    /// * `endpoint` → `"http://localhost:11434"`
    pub fn build(self) -> AiTransform {
        AiTransform {
            name: self.name.unwrap_or_else(|| "AiTransform".to_string()),
            backend: self.backend.unwrap_or(AiBackend::Ollama),
            model: self.model.unwrap_or_else(|| "mistral".to_string()),
            prompt_template: self.prompt_template.unwrap_or_else(|| "{input}".to_string()),
            endpoint: self.endpoint.unwrap_or_else(|| "http://localhost:11434".to_string()),
            api_key: self.api_key,
            artifact_path: self.artifact_path,
            cache_path: self.cache_path,
        }
    }
}

impl AiTransform {
    /// Return a new [`AiTransformBuilder`].
    pub fn builder() -> AiTransformBuilder {
        AiTransformBuilder::new()
    }

    /// Render the prompt by replacing every `{input}` occurrence with `input`.
    ///
    /// This is a simple string replacement; `{input}` in the AI-generated
    /// output is never affected because only the _template_ is processed here,
    /// not the model's response.  If the document content itself contains the
    /// literal text `{input}` it will be substituted too, which is a known
    /// limitation of this approach.
    fn render_prompt(&self, input: &str) -> String {
        self.prompt_template.replace("{input}", input)
    }

    /// Write `content` to the artifact file when `artifact_path` is set.
    fn write_artifact(&self, content: &str) -> Result<()> {
        if let Some(path) = &self.artifact_path {
            let mut f = std::fs::File::create(path)
                .with_context(|| format!("Failed to create artifact file '{}'", path))?;
            f.write_all(content.as_bytes())
                .with_context(|| format!("Failed to write artifact file '{}'", path))?;
        }
        Ok(())
    }

    /// Call the Ollama `/api/generate` endpoint.
    fn call_ollama(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
        });

        let mut req = ureq::post(&url).set("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.set("Authorization", &format!("Bearer {}", key));
        }

        let response = req
            .send_json(body)
            .with_context(|| format!("Failed to POST to Ollama endpoint '{}'", url))?;

        let body_str = response
            .into_string()
            .context("Failed to read Ollama response body")?;

        let json: serde_json::Value =
            serde_json::from_str(&body_str).with_context(|| {
                format!("Failed to parse Ollama JSON response; body was: {}", body_str)
            })?;

        json["response"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Ollama response missing 'response' field; received: {}",
                    body_str
                )
            })
    }

    /// Call an OpenAI-compatible `/v1/chat/completions` endpoint.
    fn call_openai(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/v1/chat/completions", self.endpoint.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
        });

        let mut req = ureq::post(&url).set("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.set("Authorization", &format!("Bearer {}", key));
        }

        let response = req
            .send_json(body)
            .with_context(|| format!("Failed to POST to OpenAI-compatible endpoint '{}'", url))?;

        let body_str = response
            .into_string()
            .context("Failed to read OpenAI response body")?;

        let json: serde_json::Value =
            serde_json::from_str(&body_str).with_context(|| {
                format!(
                    "Failed to parse OpenAI JSON response; body was: {}",
                    body_str
                )
            })?;

        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "OpenAI response missing 'choices[0].message.content' field; received: {}",
                    body_str
                )
            })
    }
}

impl Transform for AiTransform {
    fn name(&self) -> &str {
        &self.name
    }

    /// Send the transform input to the configured AI backend and return the
    /// generated response.
    ///
    /// The `{input}` placeholder in `prompt_template` is replaced with
    /// `input` before the request is made.  When `artifact_path` is set the
    /// response is also written to that file.
    ///
    /// When `cache_path` is set, the rendered prompt and model name are hashed
    /// and the result cache is consulted first.  A cache hit returns the stored
    /// output immediately, skipping the AI call.  On a miss the backend is
    /// called, and the result is stored in the cache together with metadata
    /// (model, Unix timestamp, input hash) before being returned.
    fn apply(&self, input: String) -> Result<String> {
        let prompt = self.render_prompt(&input);
        let input_hash = compute_ai_input_hash(&prompt, &self.model);

        // ── Cache lookup ──────────────────────────────────────────────────────
        if let Some(ref cache_path) = self.cache_path {
            let path = Path::new(cache_path);
            let cache = load_ai_cache(path);
            if let Some(entry) = cache.get(&input_hash) {
                debug!(
                    transform = %self.name,
                    model = %self.model,
                    input_hash = %input_hash,
                    "AI cache hit — returning cached output"
                );
                return Ok(entry.output.clone());
            }
            debug!(
                transform = %self.name,
                model = %self.model,
                input_hash = %input_hash,
                "AI cache miss — calling backend"
            );
        }

        // ── Backend call ──────────────────────────────────────────────────────
        let output = match self.backend {
            AiBackend::Ollama => self.call_ollama(&prompt),
            AiBackend::OpenAi => self.call_openai(&prompt),
        }
        .with_context(|| {
            format!(
                "AiTransform '{}' failed (backend={}, model='{}')",
                self.name, self.backend, self.model
            )
        })?;

        // ── Cache store ───────────────────────────────────────────────────────
        if let Some(ref cache_path) = self.cache_path {
            let path = Path::new(cache_path);
            let mut cache = load_ai_cache(path);
            cache.insert(
                input_hash.clone(),
                AiCacheEntry {
                    input_hash,
                    model: self.model.clone(),
                    timestamp: current_unix_timestamp(),
                    output: output.clone(),
                },
            );
            if let Err(e) = save_ai_cache(&cache, path) {
                warn!(
                    transform = %self.name,
                    error = %e,
                    "Failed to save AI cache; result will not be cached"
                );
            }
        }

        self.write_artifact(&output)?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── builder / field access ────────────────────────────────────────────────

    #[test]
    fn test_builder_defaults() {
        let t = AiTransform::builder().build();
        assert_eq!(t.name(), "AiTransform");
        assert_eq!(t.backend, AiBackend::Ollama);
        assert_eq!(t.model, "mistral");
        assert_eq!(t.prompt_template, "{input}");
        assert_eq!(t.endpoint, "http://localhost:11434");
        assert!(t.api_key.is_none());
        assert!(t.artifact_path.is_none());
        assert!(t.cache_path.is_none());
    }

    #[test]
    fn test_builder_all_fields() {
        let t = AiTransform::builder()
            .name("test-ai")
            .backend(AiBackend::OpenAi)
            .model("gpt-4o")
            .prompt_template("Summarise: {input}")
            .endpoint("https://api.openai.com")
            .api_key("sk-test")
            .artifact_path("/tmp/output.txt")
            .cache_path("/tmp/ai-cache.json")
            .build();

        assert_eq!(t.name(), "test-ai");
        assert_eq!(t.backend, AiBackend::OpenAi);
        assert_eq!(t.model, "gpt-4o");
        assert_eq!(t.prompt_template, "Summarise: {input}");
        assert_eq!(t.endpoint, "https://api.openai.com");
        assert_eq!(t.api_key.as_deref(), Some("sk-test"));
        assert_eq!(t.artifact_path.as_deref(), Some("/tmp/output.txt"));
        assert_eq!(t.cache_path.as_deref(), Some("/tmp/ai-cache.json"));
    }

    // ── prompt rendering ──────────────────────────────────────────────────────

    #[test]
    fn test_render_prompt_replaces_input_placeholder() {
        let t = AiTransform::builder()
            .prompt_template("Describe: {input}")
            .build();
        assert_eq!(t.render_prompt("hello"), "Describe: hello");
    }

    #[test]
    fn test_render_prompt_no_placeholder() {
        let t = AiTransform::builder()
            .prompt_template("Fixed prompt")
            .build();
        assert_eq!(t.render_prompt("anything"), "Fixed prompt");
    }

    #[test]
    fn test_render_prompt_multiple_placeholders() {
        let t = AiTransform::builder()
            .prompt_template("{input} then {input}")
            .build();
        assert_eq!(t.render_prompt("x"), "x then x");
    }

    // ── AiBackend FromStr / Display ───────────────────────────────────────────

    #[test]
    fn test_backend_from_str_ollama() {
        let b: AiBackend = "ollama".parse().unwrap();
        assert_eq!(b, AiBackend::Ollama);
    }

    #[test]
    fn test_backend_from_str_openai() {
        let b: AiBackend = "openai".parse().unwrap();
        assert_eq!(b, AiBackend::OpenAi);
    }

    #[test]
    fn test_backend_from_str_case_insensitive() {
        let b: AiBackend = "Ollama".parse().unwrap();
        assert_eq!(b, AiBackend::Ollama);
        let b2: AiBackend = "OpenAI".parse().unwrap();
        assert_eq!(b2, AiBackend::OpenAi);
    }

    #[test]
    fn test_backend_from_str_unknown_returns_error() {
        let err = "anthropic".parse::<AiBackend>().unwrap_err();
        assert!(
            err.to_string().contains("'anthropic' is not a known AI backend"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_backend_display() {
        assert_eq!(AiBackend::Ollama.to_string(), "ollama");
        assert_eq!(AiBackend::OpenAi.to_string(), "openai");
    }

    // ── artifact writing ──────────────────────────────────────────────────────

    #[test]
    fn test_write_artifact_no_path_is_noop() {
        let t = AiTransform::builder().build();
        // No artifact_path set – must not error.
        assert!(t.write_artifact("some content").is_ok());
    }

    #[test]
    fn test_write_artifact_writes_content() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        let t = AiTransform::builder().artifact_path(&path).build();
        t.write_artifact("hello artifact").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello artifact");
    }

    #[test]
    fn test_write_artifact_invalid_path_returns_error() {
        let t = AiTransform::builder()
            .artifact_path("/nonexistent/dir/artifact.txt")
            .build();
        let err = t.write_artifact("data").unwrap_err();
        assert!(
            err.to_string().contains("Failed to create artifact file"),
            "unexpected error: {}",
            err
        );
    }

    // ── caching ───────────────────────────────────────────────────────────────

    #[test]
    fn test_cache_path_builder_sets_field() {
        let t = AiTransform::builder()
            .cache_path("/tmp/test-ai-cache.json")
            .build();
        assert_eq!(t.cache_path.as_deref(), Some("/tmp/test-ai-cache.json"));
    }

    #[test]
    fn test_apply_returns_cached_output_on_cache_hit() {
        use crate::cache::{compute_ai_input_hash, save_ai_cache, AiCache, AiCacheEntry};

        let dir = tempfile::tempdir().unwrap();
        let cache_file = dir.path().join(".renderflow-ai-cache.json");

        // Build the transform (without a real endpoint — the cache will short-circuit).
        let t = AiTransform::builder()
            .name("cached-ai")
            .model("mistral")
            .prompt_template("Summarise: {input}")
            .cache_path(cache_file.to_str().unwrap())
            .build();

        // Pre-populate the cache with the expected hash.
        let rendered_prompt = "Summarise: hello world";
        let hash = compute_ai_input_hash(rendered_prompt, "mistral");
        let mut cache = AiCache::default();
        cache.insert(
            hash,
            AiCacheEntry {
                input_hash: compute_ai_input_hash(rendered_prompt, "mistral"),
                model: "mistral".to_string(),
                timestamp: 1_700_000_000,
                output: "cached AI output".to_string(),
            },
        );
        save_ai_cache(&cache, &cache_file).unwrap();

        // apply() must return the cached output without contacting any backend.
        let result = t.apply("hello world".to_string()).unwrap();
        assert_eq!(result, "cached AI output");
    }

    #[test]
    fn test_apply_without_cache_path_skips_cache() {
        // A transform with no cache_path – the cache is never consulted.
        // We cannot call apply() without a live backend, so we just verify the
        // field is absent and that the transform is constructed correctly.
        let t = AiTransform::builder()
            .model("mistral")
            .build();
        assert!(t.cache_path.is_none());
    }

    #[test]
    fn test_apply_cache_miss_attempts_backend_call() {
        use crate::cache::{load_ai_cache, AiCache};

        let dir = tempfile::tempdir().unwrap();
        let cache_file = dir.path().join(".renderflow-ai-cache.json");

        // Start with an empty cache – guaranteed miss.
        let empty_cache = AiCache::default();
        crate::cache::save_ai_cache(&empty_cache, &cache_file).unwrap();

        let t = AiTransform::builder()
            .name("miss-ai")
            .model("mistral")
            // Point at a non-listening address so the backend call fails fast.
            .endpoint("http://127.0.0.1:1")
            .cache_path(cache_file.to_str().unwrap())
            .build();

        // The backend call will fail (nothing listening on port 1).
        // That is expected; what matters is that the cache was checked first.
        let result = t.apply("test input".to_string());
        assert!(result.is_err(), "expected backend error on cache miss");
        // The cache file should still be empty (no successful result to store).
        // Use the rendered prompt (as apply() does) to compute the expected hash.
        let rendered = t.render_prompt("test input");
        let reloaded = load_ai_cache(&cache_file);
        assert!(reloaded.get(&crate::cache::compute_ai_input_hash(&rendered, "mistral")).is_none());
    }
}
