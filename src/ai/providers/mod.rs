//! AI provider implementations.
//!
//! Re-exports the concrete provider types so callers can import from the
//! top-level [`ai`](crate::ai) module:
//!
//! ```rust,no_run
//! use renderflow::ai::providers::{OllamaProvider, OpenAiProvider};
//! ```

pub mod ollama;
pub mod openai;

pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
