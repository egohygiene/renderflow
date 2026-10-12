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

#[cfg(test)]
mod forest_tests {
    use super::*;
    use crate::graph::execution_plan::ArtifactForest;
    use crate::graph::{Format, TransformEdge, TransformGraph};
    use crate::optimization::OptimizationMode;

    fn forest_plan() -> ExecutionPlan {
        let mut graph = TransformGraph::new();
        graph.add_transform(
            TransformEdge::new(Format::Html, Format::Pdf, 0.4, 0.9)
                .with_provider("tool.print", "document.pdf")
                .with_variant("variant.print")
                .with_evidence("transform_id", "html-print-pdf"),
        );
        let dag = graph.build_multi_target_dag(Format::Html, &[Format::Pdf]).unwrap();
        let mut plan = ExecutionPlan::from_dag(&dag, Format::Html, &[Format::Pdf], OptimizationMode::Quality);
        let forest: ArtifactForest = serde_json::from_value(serde_json::json!({
            "schema": "renderflow/artifact-forest-v1", "identity": "forest:sample",
            "policy": {"exclude_source": true}, "profiles": [], "intermediates": "cache_only",
            "branches": [{
                "target_id": "target:print", "format": "pdf", "preset": "press-ready",
                "requirement": "required", "state": "selected", "reason_code": "selected",
                "reason": "Best eligible source-rooted route.",
                "route": {"steps": plan.edges, "source_identity": false,
                    "cumulative_quality": 0.9, "cumulative_loss": 0.1, "hop_count": 1,
                    "weighted_cost": 0.4, "ranking": "quality_then_hops_then_cost",
                    "explored_candidates": 2}
            }, {
                "target_id": "target:unavailable", "format": "docx", "requirement": "optional",
                "state": "unavailable", "reason_code": "provider_unavailable",
                "reason": "Converter is unavailable."
            }, {
                "target_id": "target:unsupported", "format": "epub", "requirement": "optional",
                "state": "unsupported", "reason_code": "no_route", "reason": "No eligible route."
            }]
        })).unwrap();
        plan.attach_artifact_forest(forest);
        plan
    }

    #[test]
    fn text_and_markdown_expose_exact_routes_and_unmaterialized_targets() {
        let plan = forest_plan();
        for rendered in [TextRenderer.render(&plan), MarkdownRenderer.render(&plan)] {
            for expected in ["renderflow/artifact-forest-v1", "forest:sample", "target:print",
                "press-ready", "tool.print", "document.pdf", "variant.print", "html-print-pdf",
                "0.900000", "0.100000", "quality_then_hops_then_cost", "provider_unavailable",
                "target:unsupported", "unsupported", "No eligible route.", "projection"]
            {
                assert!(rendered.contains(expected), "missing {expected}: {rendered}");
            }
        }
    }
}
