use std::fmt;
use std::io::Write;

use anyhow::{Context, Result};
use serde_json::json;

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
        }
    }
}

impl AiTransform {
    /// Return a new [`AiTransformBuilder`].
    pub fn builder() -> AiTransformBuilder {
        AiTransformBuilder::new()
    }

    /// Render the prompt by replacing `{input}` with `input`.
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

        let json: serde_json::Value = response
            .into_json()
            .context("Failed to parse Ollama JSON response")?;

        json["response"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Ollama response missing 'response' field"))
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

        let json: serde_json::Value = response
            .into_json()
            .context("Failed to parse OpenAI JSON response")?;

        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                anyhow::anyhow!("OpenAI response missing 'choices[0].message.content' field")
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
    fn apply(&self, input: String) -> Result<String> {
        let prompt = self.render_prompt(&input);
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
            .build();

        assert_eq!(t.name(), "test-ai");
        assert_eq!(t.backend, AiBackend::OpenAi);
        assert_eq!(t.model, "gpt-4o");
        assert_eq!(t.prompt_template, "Summarise: {input}");
        assert_eq!(t.endpoint, "https://api.openai.com");
        assert_eq!(t.api_key.as_deref(), Some("sk-test"));
        assert_eq!(t.artifact_path.as_deref(), Some("/tmp/output.txt"));
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
}
