use std::fmt::Write;

use super::PlanRenderer;
use crate::graph::execution_plan::{ExecutionPlan, NodeType};

/// Renders an [`ExecutionPlan`] as a
/// [Mermaid](https://mermaid.js.org/) flowchart diagram.
///
/// The output can be embedded directly in GitHub Markdown:
///
/// ````markdown
/// ```mermaid
/// flowchart LR
///   ...
/// ```
/// ````
///
/// Node shapes:
/// * Source – stadium shape `([label])`
/// * Intermediate – rounded rectangle `(label)`
/// * Output – rectangle `[label]`
pub struct MermaidRenderer;

impl PlanRenderer for MermaidRenderer {
    fn render(&self, plan: &ExecutionPlan) -> String {
        let mut out = String::new();

        let _ = writeln!(out, "flowchart LR");

        // ── node declarations ─────────────────────────────────────────────
        for node in &plan.nodes {
            let id = sanitise_id(&node.format);
            let label = &node.format;
            let decl = match node.node_type {
                NodeType::Source => format!("    {}([{}])", id, label),
                NodeType::Intermediate => format!("    {}({})", id, label),
                NodeType::Output => format!("    {}[{}]", id, label),
            };
            let _ = writeln!(out, "{}", decl);
        }

        let _ = writeln!(out);

        // ── edges ─────────────────────────────────────────────────────────
        let mut edge_lines: Vec<String> = plan
            .edges
            .iter()
            .map(|e| {
                let from_id = sanitise_id(&e.from);
                let to_id = sanitise_id(&e.to);
                format!(
                    "    {} -->|\"cost={:.2} q={:.2}\"| {}",
                    from_id, e.cost, e.quality, to_id
                )
            })
            .collect();
        edge_lines.sort();
        for line in &edge_lines {
            let _ = writeln!(out, "{}", line);
        }

        // ── style classes ─────────────────────────────────────────────────
        let _ = writeln!(out);
        let _ = writeln!(out, "    classDef source  fill:#add8e6,stroke:#333;");
        let _ = writeln!(out, "    classDef output  fill:#90ee90,stroke:#333;");
        let _ = writeln!(out, "    classDef mid     fill:#fff,stroke:#999;");

        // Apply classes
        for node in &plan.nodes {
            let id = sanitise_id(&node.format);
            let class = match node.node_type {
                NodeType::Source => "source",
                NodeType::Intermediate => "mid",
                NodeType::Output => "output",
            };
            let _ = writeln!(out, "    class {} {};", id, class);
        }

        out
    }
}

/// Convert a format name to a valid Mermaid node identifier.
///
/// Mermaid identifiers must not contain hyphens or spaces; replace them with
/// underscores and prefix with an underscore if the name starts with a digit.
fn sanitise_id(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{}", s)
    } else {
        s
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
    fn test_mermaid_renderer_starts_with_flowchart() {
        let output = MermaidRenderer.render(&make_plan());
        assert!(output.starts_with("flowchart LR"));
    }

    #[test]
    fn test_mermaid_renderer_contains_arrow() {
        let output = MermaidRenderer.render(&make_plan());
        assert!(output.contains("-->"));
    }

    #[test]
    fn test_mermaid_renderer_contains_classdefs() {
        let output = MermaidRenderer.render(&make_plan());
        assert!(output.contains("classDef source"));
        assert!(output.contains("classDef output"));
    }

    #[test]
    fn test_sanitise_id_replaces_hyphens() {
        assert_eq!(sanitise_id("mp3-audio"), "mp3_audio");
    }

    #[test]
    fn test_sanitise_id_prefixes_leading_digit() {
        assert_eq!(sanitise_id("3gp"), "_3gp");
    }
}
