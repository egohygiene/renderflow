//! Provider abstraction for AI backends.
//!
//! An [`AiProvider`] is the core trait that every AI backend must implement.
//! Providers advertise their [`AiCapabilities`] and available [`AiModel`]s,
//! and execute [`AiRequest`]s returning [`AiResponse`]s.
//!
//! This file also contains [`AiExecutionPreference`], which controls how the
//! planner selects between local and remote providers.

use std::collections::HashSet;
use std::fmt;

use anyhow::Result;

use super::request::{AiRequest, AiResponse};

// ── AiCapability ──────────────────────────────────────────────────────────────

/// A single capability that an AI provider or model may support.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AiCapability {
    /// Generate free-form text from a prompt.
    TextGeneration,
    /// Condense long documents into shorter summaries.
    Summarization,
    /// Translate content between human languages.
    Translation,
    /// Generate or complete source code.
    CodeGeneration,
    /// Generate images from text prompts (requires multimodal model).
    ImageGeneration,
    /// Edit or modify existing images.
    ImageEditing,
    /// Extract text from images via optical character recognition.
    Ocr,
    /// Produce fixed-dimension vector embeddings for semantic search.
    Embeddings,
    /// Reason jointly over text and image inputs.
    MultimodalReasoning,
    /// Produce structured JSON that conforms to a given schema.
    StructuredJsonOutput,
}

impl fmt::Display for AiCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::TextGeneration => "text-generation",
            Self::Summarization => "summarization",
            Self::Translation => "translation",
            Self::CodeGeneration => "code-generation",
            Self::ImageGeneration => "image-generation",
            Self::ImageEditing => "image-editing",
            Self::Ocr => "ocr",
            Self::Embeddings => "embeddings",
            Self::MultimodalReasoning => "multimodal-reasoning",
            Self::StructuredJsonOutput => "structured-json-output",
        };
        write!(f, "{}", s)
    }
}

// ── AiCapabilities ────────────────────────────────────────────────────────────

/// The set of capabilities advertised by a provider or model.
///
/// Construct one using [`AiCapabilities::new`] and add entries with
/// [`AiCapabilities::with`].
#[derive(Debug, Clone, Default)]
pub struct AiCapabilities {
    inner: HashSet<AiCapability>,
}

impl AiCapabilities {
    /// Create an empty capability set.
    pub fn new() -> Self {
        Self {
            inner: HashSet::new(),
        }
    }

    /// Return a new [`AiCapabilities`] with `capability` added.
    #[must_use]
    pub fn with(mut self, capability: AiCapability) -> Self {
        self.inner.insert(capability);
        self
    }

    /// Return `true` when `capability` is present.
    pub fn supports(&self, capability: &AiCapability) -> bool {
        self.inner.contains(capability)
    }

    /// Iterate over all declared capabilities.
    pub fn iter(&self) -> impl Iterator<Item = &AiCapability> {
        self.inner.iter()
    }

    /// Return the number of declared capabilities.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Return `true` when no capabilities are declared.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// ── AiModel ───────────────────────────────────────────────────────────────────

/// Metadata describing a single model offered by a provider.
#[derive(Debug, Clone)]
pub struct AiModel {
    /// The model identifier used in API requests (e.g. `"mistral"`, `"gpt-4o"`).
    pub id: String,
    /// Optional human-readable description of the model.
    pub description: Option<String>,
    /// `true` when the model runs locally (e.g. via Ollama); `false` for remote
    /// cloud models.
    pub is_local: bool,
}

impl AiModel {
    /// Construct a new [`AiModel`] with the given identifier.
    pub fn new(id: impl Into<String>, is_local: bool) -> Self {
        Self {
            id: id.into(),
            description: None,
            is_local,
        }
    }

    /// Attach a description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl fmt::Display for AiModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

// ── AiExecutionPreference ─────────────────────────────────────────────────────

/// Controls how the planner selects between available providers.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AiExecutionPreference {
    /// Only consider providers that run models locally (e.g. Ollama).
    LocalOnly,
    /// Only consider remote providers (e.g. OpenAI).
    RemoteOnly,
    /// Prefer local providers; fall back to remote when no local provider can
    /// satisfy the request.
    #[default]
    LocalPreferred,
    /// Prefer the provider with the lowest estimated per-request cost.
    LowestCost,
    /// Prefer the provider that is expected to produce the highest-quality output.
    HighestQuality,
}

impl fmt::Display for AiExecutionPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::LocalOnly => "local-only",
            Self::RemoteOnly => "remote-only",
            Self::LocalPreferred => "local-preferred",
            Self::LowestCost => "lowest-cost",
            Self::HighestQuality => "highest-quality",
        };
        write!(f, "{}", s)
    }
}

// ── AiProvider ────────────────────────────────────────────────────────────────

/// Core trait implemented by every AI backend.
///
/// A provider is responsible for:
/// * advertising its name and whether it is local,
/// * declaring its [`AiCapabilities`],
/// * enumerating its available [`AiModel`]s,
/// * executing an [`AiRequest`] and returning an [`AiResponse`].
///
/// Multiple providers may be active simultaneously; the graph planner filters
/// candidates using capability requirements and the active
/// [`AiExecutionPreference`].
pub trait AiProvider: fmt::Debug + Send + Sync {
    /// Human-readable provider name (e.g. `"ollama"`, `"openai"`).
    fn name(&self) -> &str;

    /// Return `true` when this provider runs models locally.
    fn is_local(&self) -> bool;

    /// The set of capabilities this provider supports.
    fn capabilities(&self) -> AiCapabilities;

    /// The list of models this provider makes available.
    ///
    /// Providers with dynamic model lists (e.g. Ollama) should return a
    /// best-effort static list; the list is used for CLI discovery only and
    /// does not gate execution.
    fn models(&self) -> Vec<AiModel>;

    /// Execute `request` and return the generated response.
    fn execute(&self, request: &AiRequest) -> Result<AiResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AiCapability ──────────────────────────────────────────────────────────

    #[test]
    fn test_capability_display() {
        assert_eq!(AiCapability::TextGeneration.to_string(), "text-generation");
        assert_eq!(AiCapability::Summarization.to_string(), "summarization");
        assert_eq!(AiCapability::StructuredJsonOutput.to_string(), "structured-json-output");
    }

    // ── AiCapabilities ────────────────────────────────────────────────────────

    #[test]
    fn test_capabilities_empty_by_default() {
        let caps = AiCapabilities::new();
        assert!(caps.is_empty());
        assert_eq!(caps.len(), 0);
    }

    #[test]
    fn test_capabilities_with_adds_entries() {
        let caps = AiCapabilities::new()
            .with(AiCapability::TextGeneration)
            .with(AiCapability::Summarization);
        assert!(caps.supports(&AiCapability::TextGeneration));
        assert!(caps.supports(&AiCapability::Summarization));
        assert!(!caps.supports(&AiCapability::Embeddings));
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn test_capabilities_duplicate_not_added_twice() {
        let caps = AiCapabilities::new()
            .with(AiCapability::TextGeneration)
            .with(AiCapability::TextGeneration);
        assert_eq!(caps.len(), 1);
    }

    // ── AiModel ───────────────────────────────────────────────────────────────

    #[test]
    fn test_model_display() {
        let m = AiModel::new("mistral", true);
        assert_eq!(m.to_string(), "mistral");
    }

    #[test]
    fn test_model_with_description() {
        let m = AiModel::new("gpt-4o", false).with_description("OpenAI GPT-4o");
        assert_eq!(m.description.as_deref(), Some("OpenAI GPT-4o"));
        assert!(!m.is_local);
    }

    // ── AiExecutionPreference ─────────────────────────────────────────────────

    #[test]
    fn test_execution_preference_display() {
        assert_eq!(AiExecutionPreference::LocalOnly.to_string(), "local-only");
        assert_eq!(AiExecutionPreference::LocalPreferred.to_string(), "local-preferred");
        assert_eq!(AiExecutionPreference::HighestQuality.to_string(), "highest-quality");
    }

    #[test]
    fn test_execution_preference_default_is_local_preferred() {
        assert_eq!(AiExecutionPreference::default(), AiExecutionPreference::LocalPreferred);
    }
}
