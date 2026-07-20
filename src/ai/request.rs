//! [`AiRequest`] and [`AiResponse`] – the data types exchanged between the
//! graph executor and an [`AiProvider`](super::provider::AiProvider).

use std::fmt;

// ── OutputFormat ──────────────────────────────────────────────────────────────

/// The structured output format a transform expects from the AI backend.
///
/// When an [`AiRequest`] specifies an `output_format`, the executor validates
/// the response with [`super::output::validate_output`] before returning it to
/// the caller.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    /// Plain unstructured text (no validation).
    Text,
    /// Require valid JSON output.
    Json,
    /// Require valid YAML output.
    Yaml,
    /// Accept Markdown (light structural validation: non-empty).
    Markdown,
    /// Require well-formed XML output.
    Xml,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Text => "text",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Markdown => "markdown",
            Self::Xml => "xml",
        };
        write!(f, "{}", s)
    }
}

// ── GenerationParameters ──────────────────────────────────────────────────────

/// Optional sampling parameters forwarded to the AI backend.
///
/// All fields default to `None`, in which case the backend uses its own
/// defaults.  Including these in the cache key ensures that changing
/// temperature or `max_tokens` causes a cache miss.
#[derive(Debug, Clone, Default)]
pub struct GenerationParameters {
    /// Sampling temperature (typically `0.0`–`2.0`; lower ⇒ more deterministic).
    pub temperature: Option<f32>,
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// Optional stop sequences that terminate generation early.
    pub stop: Vec<String>,
}

impl GenerationParameters {
    /// Return a compact deterministic string representation for use in cache
    /// keys.  Fields that are `None` are omitted.
    pub fn cache_key_fragment(&self) -> String {
        let mut parts = Vec::new();
        if let Some(t) = self.temperature {
            // Format with fixed precision so floating-point representation is
            // stable across platforms.
            parts.push(format!("temp:{:.6}", t));
        }
        if let Some(m) = self.max_tokens {
            parts.push(format!("max_tokens:{}", m));
        }
        if !self.stop.is_empty() {
            let mut sorted = self.stop.clone();
            sorted.sort();
            parts.push(format!("stop:{}", sorted.join("|")));
        }
        parts.join(",")
    }
}

// ── AiRequest ─────────────────────────────────────────────────────────────────

/// A single request sent to an [`AiProvider`](super::provider::AiProvider).
#[derive(Debug, Clone)]
pub struct AiRequest {
    /// The model identifier to use (e.g. `"mistral"`, `"gpt-4o"`).
    pub model: String,
    /// The fully-rendered prompt text (all `{input}` placeholders already
    /// substituted).
    pub prompt: String,
    /// Optional expected output format used for post-response validation.
    pub output_format: Option<OutputFormat>,
    /// Optional generation parameters forwarded to the backend.
    pub params: GenerationParameters,
    /// Prompt template version string, included in cache keys so that changing
    /// the template invalidates existing cached responses.
    pub prompt_version: Option<String>,
}

impl AiRequest {
    /// Construct a minimal request with only `model` and `prompt`.
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            prompt: prompt.into(),
            output_format: None,
            params: GenerationParameters::default(),
            prompt_version: None,
        }
    }

    /// Attach an expected [`OutputFormat`].
    #[must_use]
    pub fn with_output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = Some(format);
        self
    }

    /// Attach [`GenerationParameters`].
    #[must_use]
    pub fn with_params(mut self, params: GenerationParameters) -> Self {
        self.params = params;
        self
    }

    /// Attach a prompt template version string.
    #[must_use]
    pub fn with_prompt_version(mut self, version: impl Into<String>) -> Self {
        self.prompt_version = Some(version.into());
        self
    }
}

// ── AiResponse ────────────────────────────────────────────────────────────────

/// The response returned by an [`AiProvider`](super::provider::AiProvider)
/// after executing an [`AiRequest`].
#[derive(Debug, Clone)]
pub struct AiResponse {
    /// The generated text content.
    pub content: String,
    /// The model identifier that produced this response.
    pub model: String,
    /// The provider name that executed the request.
    pub provider: String,
    /// Number of input tokens consumed, if reported by the backend.
    pub input_tokens: Option<u32>,
    /// Number of output tokens generated, if reported by the backend.
    pub output_tokens: Option<u32>,
    /// Wall-clock execution time in milliseconds.
    pub duration_ms: Option<u64>,
}

impl AiResponse {
    /// Construct a minimal response with only `content`, `model`, and `provider`.
    pub fn new(
        content: impl Into<String>,
        model: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            provider: provider.into(),
            input_tokens: None,
            output_tokens: None,
            duration_ms: None,
        }
    }

    /// Total tokens (input + output) if both are available.
    pub fn total_tokens(&self) -> Option<u32> {
        match (self.input_tokens, self.output_tokens) {
            (Some(i), Some(o)) => Some(i + o),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── OutputFormat ──────────────────────────────────────────────────────────

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Text.to_string(), "text");
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Yaml.to_string(), "yaml");
        assert_eq!(OutputFormat::Markdown.to_string(), "markdown");
        assert_eq!(OutputFormat::Xml.to_string(), "xml");
    }

    // ── GenerationParameters ──────────────────────────────────────────────────

    #[test]
    fn test_generation_parameters_cache_key_empty() {
        let p = GenerationParameters::default();
        assert!(p.cache_key_fragment().is_empty());
    }

    #[test]
    fn test_generation_parameters_cache_key_with_values() {
        let p = GenerationParameters {
            temperature: Some(0.7),
            max_tokens: Some(256),
            stop: vec!["END".to_string()],
        };
        let key = p.cache_key_fragment();
        assert!(key.contains("temp:"));
        assert!(key.contains("max_tokens:256"));
        assert!(key.contains("stop:END"));
    }

    #[test]
    fn test_generation_parameters_stop_sequences_sorted_for_stability() {
        let p1 = GenerationParameters {
            stop: vec!["Z".to_string(), "A".to_string()],
            ..Default::default()
        };
        let p2 = GenerationParameters {
            stop: vec!["A".to_string(), "Z".to_string()],
            ..Default::default()
        };
        assert_eq!(p1.cache_key_fragment(), p2.cache_key_fragment());
    }

    // ── AiRequest ─────────────────────────────────────────────────────────────

    #[test]
    fn test_request_minimal() {
        let r = AiRequest::new("mistral", "hello");
        assert_eq!(r.model, "mistral");
        assert_eq!(r.prompt, "hello");
        assert!(r.output_format.is_none());
        assert!(r.prompt_version.is_none());
    }

    #[test]
    fn test_request_builder_chain() {
        let r = AiRequest::new("gpt-4o", "prompt")
            .with_output_format(OutputFormat::Json)
            .with_prompt_version("v2")
            .with_params(GenerationParameters {
                temperature: Some(0.5),
                ..Default::default()
            });
        assert_eq!(r.output_format, Some(OutputFormat::Json));
        assert_eq!(r.prompt_version.as_deref(), Some("v2"));
        assert_eq!(r.params.temperature, Some(0.5));
    }

    // ── AiResponse ────────────────────────────────────────────────────────────

    #[test]
    fn test_response_total_tokens_both_present() {
        let r = AiResponse {
            content: "ok".into(),
            model: "m".into(),
            provider: "p".into(),
            input_tokens: Some(10),
            output_tokens: Some(20),
            duration_ms: None,
        };
        assert_eq!(r.total_tokens(), Some(30));
    }

    #[test]
    fn test_response_total_tokens_partial() {
        let r = AiResponse {
            input_tokens: Some(10),
            output_tokens: None,
            ..AiResponse::new("c", "m", "p")
        };
        assert!(r.total_tokens().is_none());
    }
}
