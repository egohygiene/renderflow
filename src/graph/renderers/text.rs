use std::fmt::Write;

use super::PlanRenderer;
use crate::graph::execution_plan::{ExecutionPlan, NodeType};

/// Renders an [`ExecutionPlan`] as a human-readable text tree.
///
/// This is the default CLI output format — compact and easy to read in a
/// terminal without additional tooling.
pub struct TextRenderer;

impl PlanRenderer for TextRenderer {
    fn render(&self, plan: &ExecutionPlan) -> String {
        let mut out = String::new();

        let _ = writeln!(out, "Execution Plan");
        let _ = writeln!(out, "==============");
        let _ = writeln!(out, "Source:       {}", plan.source);
        let _ = writeln!(out, "Targets:      {}", plan.targets.join(", "));
        let _ = writeln!(out, "Optimization: {}", plan.optimization);

        // ── nodes ──────────────────────────────────────────────────────────
        let _ = writeln!(out);
        let _ = writeln!(out, "Nodes ({}):", plan.metadata.total_nodes);
        for node in &plan.nodes {
            let tag = match node.node_type {
                NodeType::Source => "[source]",
                NodeType::Intermediate => "[intermediate]",
                NodeType::Output => "[output]",
            };
            let _ = writeln!(out, "  • {}  {}", node.format, tag);
        }

        // ── edges ──────────────────────────────────────────────────────────
        let _ = writeln!(out);
        let _ = writeln!(out, "Edges ({}):", plan.metadata.total_edges);
        let max_from = plan
            .edges
            .iter()
            .map(|e| e.from.len())
            .max()
            .unwrap_or(0);
        for e in &plan.edges {
            let _ = writeln!(
                out,
                "  {:<width$} ──► {}  [cost: {:.2}, quality: {:.2}]",
                e.from,
                e.to,
                e.cost,
                e.quality,
                width = max_from
            );
        }

        // ── execution waves ────────────────────────────────────────────────
        let _ = writeln!(out);
        let _ = writeln!(out, "Execution Waves ({}):", plan.waves.len());
        for wave in &plan.waves {
            let _ = writeln!(out, "  Wave {}:", wave.index);
            for e in &wave.edges {
                let _ = writeln!(out, "    {}  →  {}", e.from, e.to);
            }
        }

        // ── metadata ───────────────────────────────────────────────────────
        let _ = writeln!(out);
        let _ = writeln!(out, "Statistics:");
        let _ = writeln!(out, "  nodes:               {}", plan.metadata.total_nodes);
        let _ = writeln!(out, "  edges:               {}", plan.metadata.total_edges);
        let _ = writeln!(out, "  depth:               {}", plan.metadata.execution_depth);
        let _ = writeln!(out, "  waves:               {}", plan.metadata.execution_waves);
        let _ = writeln!(
            out,
            "  estimated cost:      {:.2}",
            plan.metadata.estimated_cost
        );
        let _ = writeln!(
            out,
            "  estimated quality:   {:.2}",
            plan.metadata.estimated_quality
        );
        let _ = writeln!(
            out,
            "  reused intermediates: {}",
            plan.metadata.reused_intermediates
        );

        out
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
    fn test_text_renderer_contains_header() {
        let plan = make_plan();
        let output = TextRenderer.render(&plan);
        assert!(output.contains("Execution Plan"));
    }

    #[test]
    fn test_text_renderer_contains_source() {
        let plan = make_plan();
        let output = TextRenderer.render(&plan);
        assert!(output.contains("markdown"));
    }

    #[test]
    fn test_text_renderer_contains_waves() {
        let plan = make_plan();
        let output = TextRenderer.render(&plan);
        assert!(output.contains("Wave"));
    }
}
