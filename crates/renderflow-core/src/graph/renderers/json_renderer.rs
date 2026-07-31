use super::PlanRenderer;
use crate::graph::execution_plan::ExecutionPlan;

/// Renders an [`ExecutionPlan`] as a pretty-printed JSON string.
pub struct JsonRenderer;

impl PlanRenderer for JsonRenderer {
    fn render(&self, plan: &ExecutionPlan) -> String {
        serde_json::to_string_pretty(plan).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Format, TransformEdge, TransformGraph};
    use crate::graph::execution_plan::ExecutionPlan;
    use crate::optimization::OptimizationMode;

    fn make_plan() -> ExecutionPlan {
        let mut g = TransformGraph::new();
        g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
        g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
        let dag = g
            .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
            .unwrap();
        ExecutionPlan::from_dag(
            &dag,
            Format::Markdown,
            &[Format::Pdf],
            OptimizationMode::Balanced,
        )
    }

    #[test]
    fn test_json_renderer_produces_valid_json() {
        let output = JsonRenderer.render(&make_plan());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["source"], "markdown");
    }

    #[test]
    fn test_json_renderer_contains_metadata() {
        let output = JsonRenderer.render(&make_plan());
        assert!(output.contains("\"metadata\""));
    }

    #[test]
    fn test_json_renderer_contains_waves() {
        let output = JsonRenderer.render(&make_plan());
        assert!(output.contains("\"waves\""));
    }
}
