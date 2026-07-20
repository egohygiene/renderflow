//! Graph renderers that consume an [`ExecutionPlan`] to produce multiple
//! output representations of the same underlying transformation graph.
//!
//! # Design
//!
//! Every renderer implements [`PlanRenderer`].  The planner produces a single
//! [`ExecutionPlan`] and renderers are responsible for translating that plan
//! into a target format.  No renderer re-implements graph traversal logic.
//!
//! # Supported Formats
//!
//! | Format   | Renderer        | Description                          |
//! |----------|-----------------|--------------------------------------|
//! | `text`   | [`TextRenderer`]     | Human-readable tree (default)   |
//! | `dot`    | [`DotRenderer`]      | Graphviz DOT language            |
//! | `mermaid`| [`MermaidRenderer`]  | Mermaid flowchart                |
//! | `json`   | [`JsonRenderer`]     | Serialized JSON                  |
//! | `yaml`   | [`YamlRenderer`]     | Serialized YAML                  |
//! | `markdown`| [`MarkdownRenderer`]| GitHub-flavoured Markdown report |

mod dot;
mod json_renderer;
mod markdown;
mod mermaid;
mod text;
mod yaml_renderer;

pub use dot::DotRenderer;
pub use json_renderer::JsonRenderer;
pub use markdown::MarkdownRenderer;
pub use mermaid::MermaidRenderer;
pub use text::TextRenderer;
pub use yaml_renderer::YamlRenderer;

use super::execution_plan::ExecutionPlan;

/// A renderer that converts an [`ExecutionPlan`] into a string representation.
pub trait PlanRenderer {
    /// Render the execution plan as a string.
    fn render(&self, plan: &ExecutionPlan) -> String;
}

/// Look up a renderer by name.
///
/// Supported names (case-insensitive):
/// `text`, `tree`, `dot`, `graphviz`, `mermaid`, `json`, `yaml`, `markdown`, `md`.
///
/// Returns `None` when the name is not recognised.
pub fn renderer_for(name: &str) -> Option<Box<dyn PlanRenderer>> {
    match name.to_lowercase().as_str() {
        "text" | "tree" => Some(Box::new(TextRenderer)),
        "dot" | "graphviz" => Some(Box::new(DotRenderer)),
        "mermaid" => Some(Box::new(MermaidRenderer)),
        "json" => Some(Box::new(JsonRenderer)),
        "yaml" | "yml" => Some(Box::new(YamlRenderer)),
        "markdown" | "md" => Some(Box::new(MarkdownRenderer)),
        _ => None,
    }
}
