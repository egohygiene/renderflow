use std::fmt::Write;

use super::PlanRenderer;
use crate::graph::execution_plan::{ExecutionPlan, NodeType};

/// Renders an [`ExecutionPlan`] as a
/// [Graphviz DOT](https://graphviz.org/doc/info/lang.html) language string.
///
/// The output can be saved to a `.dot` file and rendered with Graphviz:
///
/// ```sh
/// dot -Tsvg pipeline.dot -o pipeline.svg
/// ```
///
/// Node colours:
/// * Source nodes – light blue
/// * Output (leaf) nodes – light green
/// * Intermediate nodes – default (white)
pub struct DotRenderer;

impl PlanRenderer for DotRenderer {
    fn render(&self, plan: &ExecutionPlan) -> String {
        let mut out = String::new();

        let _ = writeln!(out, "digraph renderflow {{");
        let _ = writeln!(out, "    rankdir=LR;");
        let _ = writeln!(out, "    node [shape=box fontname=\"monospace\"];");
        let _ = writeln!(out);

        // ── node declarations ─────────────────────────────────────────────
        for node in &plan.nodes {
            let style = match node.node_type {
                NodeType::Source => " style=filled fillcolor=lightblue".to_string(),
                NodeType::Output => " style=filled fillcolor=lightgreen".to_string(),
                NodeType::Intermediate => String::new(),
            };
            let _ = writeln!(
                out,
                "    \"{}\" [label=\"{}\"{style}];",
                node.format, node.format
            );
        }

        let _ = writeln!(out);

        // ── edges ─────────────────────────────────────────────────────────
        let mut edge_lines: Vec<String> = plan
            .edges
            .iter()
            .map(|e| {
                format!(
                    "    \"{}\" -> \"{}\" [label=\"cost={:.2}\\nquality={:.2}\"];",
                    e.from, e.to, e.cost, e.quality
                )
            })
            .collect();
        edge_lines.sort();
        for line in &edge_lines {
            let _ = writeln!(out, "{}", line);
        }

        let _ = writeln!(out, "}}");
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
    fn test_dot_renderer_starts_with_digraph() {
        let output = DotRenderer.render(&make_plan());
        assert!(output.starts_with("digraph renderflow {"));
    }

    #[test]
    fn test_dot_renderer_contains_edge_arrow() {
        let output = DotRenderer.render(&make_plan());
        assert!(output.contains("->"));
    }

    #[test]
    fn test_dot_renderer_contains_source_node_style() {
        let output = DotRenderer.render(&make_plan());
        assert!(output.contains("lightblue"));
    }

    #[test]
    fn test_dot_renderer_contains_output_node_style() {
        let output = DotRenderer.render(&make_plan());
        assert!(output.contains("lightgreen"));
    }
}
