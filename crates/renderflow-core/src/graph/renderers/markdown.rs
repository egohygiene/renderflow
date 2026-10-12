use std::fmt::Write;

use super::PlanRenderer;
use crate::graph::execution_plan::{DiagnosticLevel, ExecutionPlan, NodeType};

/// Renders an [`ExecutionPlan`] as a GitHub-flavoured Markdown report.
///
/// The output is human-readable and can be committed to a repository,
/// published as a CI artifact, or embedded in documentation.
pub struct MarkdownRenderer;

fn cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ").replace('`', "'")
}

impl PlanRenderer for MarkdownRenderer {
    fn render(&self, plan: &ExecutionPlan) -> String {
        let mut out = String::new();

        let _ = writeln!(out, "# Execution Plan");
        let _ = writeln!(out);
        let _ = writeln!(out, "| Property | Value |");
        let _ = writeln!(out, "|----------|-------|");
        let _ = writeln!(out, "| Source | `{}` |", plan.source);
        let _ = writeln!(
            out,
            "| Targets | {} |",
            plan.targets
                .iter()
                .map(|t| format!("`{}`", t))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(out, "| Optimization | `{}` |", plan.optimization);
        let _ = writeln!(out);

        if let Some(forest) = &plan.artifact_forest {
            let _ = writeln!(out, "## Artifact Forest");
            let _ = writeln!(out);
            let _ = writeln!(out, "Schema: `{}`. Identity: `{}`.", cell(&forest.schema), cell(&forest.identity));
            let _ = writeln!(out);
            let _ = writeln!(out, "Policy: `{}`.", cell(&forest.policy.to_string()));
            let _ = writeln!(out);
            let _ = writeln!(out, "Branch routes are authoritative. The format graph below is a projection of isolated execution groups.");
            let _ = writeln!(out);
            let _ = writeln!(out, "| Target identity | Format | State | Reason |");
            let _ = writeln!(out, "|---|---|---|---|");
            for branch in &forest.branches {
                let _ = writeln!(out, "| `{}` | `{}` | {} | {}: {} |",
                    cell(&branch.target_id), cell(&branch.format), branch.state.as_str(),
                    cell(&branch.reason_code), cell(&branch.reason));
            }
            let _ = writeln!(out);
            for branch in &forest.branches {
                let Some(route) = &branch.route else { continue };
                let _ = writeln!(out, "### `{}`", cell(&branch.target_id));
                let _ = writeln!(out);
                let _ = writeln!(out, "Format: `{}`; preset: `{}`; template: `{}`; variant: `{}`.",
                    cell(&branch.format), cell(branch.preset.as_deref().unwrap_or("default")),
                    cell(branch.template.as_deref().unwrap_or("default")),
                    cell(branch.variant.as_deref().unwrap_or("default")));
                let _ = writeln!(out);
                if !branch.options.is_empty() {
                    let _ = writeln!(out, "Options: `{}`.", cell(&serde_json::json!(branch.options).to_string()));
                    let _ = writeln!(out);
                }
                let _ = writeln!(out,
                    "Quality: {:.6}; loss: {:.6}; hops: {}; weighted cost: {:.6}; candidates: {}.",
                    route.cumulative_quality, route.cumulative_loss, route.hop_count,
                    route.weighted_cost, route.explored_candidates);
                let _ = writeln!(out);
                let _ = writeln!(out, "Ranking: {}.", cell(&route.ranking));
                let _ = writeln!(out);
                if route.source_identity {
                    let _ = writeln!(out, "Original source identity; no transformation.");
                    let _ = writeln!(out);
                }
                for step in &route.steps {
                    let _ = writeln!(out, "- `{}` → `{}` — provider `{}`, capability `{}`, variant `{}`, transform `{}`.",
                        cell(&step.from), cell(&step.to),
                        cell(step.provider_id.as_deref().unwrap_or("builtin")),
                        cell(step.capability_id.as_deref().unwrap_or("unspecified")),
                        cell(step.variant_id.as_deref().unwrap_or("default")),
                        cell(step.evidence.get("transform_id").map(String::as_str).unwrap_or("unspecified")));
                }
                let _ = writeln!(out);
            }
        }

        // ── statistics ────────────────────────────────────────────────────
        let _ = writeln!(out, "## Statistics");
        let _ = writeln!(out);
        let _ = writeln!(out, "| Metric | Value |");
        let _ = writeln!(out, "|--------|-------|");
        let _ = writeln!(out, "| Nodes | {} |", plan.metadata.total_nodes);
        let _ = writeln!(out, "| Edges | {} |", plan.metadata.total_edges);
        let _ = writeln!(out, "| Depth | {} |", plan.metadata.execution_depth);
        let _ = writeln!(out, "| Waves | {} |", plan.metadata.execution_waves);
        let _ = writeln!(
            out,
            "| Estimated Cost | {:.2} |",
            plan.metadata.estimated_cost
        );
        let _ = writeln!(
            out,
            "| Estimated Quality | {:.2} |",
            plan.metadata.estimated_quality
        );
        let _ = writeln!(
            out,
            "| Reused Intermediates | {} |",
            plan.metadata.reused_intermediates
        );
        let _ = writeln!(out);

        // ── nodes ─────────────────────────────────────────────────────────
        let _ = writeln!(out, "## Nodes");
        let _ = writeln!(out);
        let _ = writeln!(out, "| Format | Type |");
        let _ = writeln!(out, "|--------|------|");
        for node in &plan.nodes {
            let type_str = match node.node_type {
                NodeType::Source => "source",
                NodeType::Intermediate => "intermediate",
                NodeType::Output => "output",
            };
            let _ = writeln!(out, "| `{}` | {} |", node.format, type_str);
        }
        let _ = writeln!(out);

        // ── edges ─────────────────────────────────────────────────────────
        let _ = writeln!(out, "## Edges");
        let _ = writeln!(out);
        let _ = writeln!(out, "| From | To | Cost | Quality | Type |");
        let _ = writeln!(out, "|------|----|------|---------|------|");
        for e in &plan.edges {
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {:.2} | {:.2} | {:?} |",
                e.from, e.to, e.cost, e.quality, e.edge_type
            );
        }
        let _ = writeln!(out);

        // ── execution waves ───────────────────────────────────────────────
        let _ = writeln!(out, "## Execution Waves");
        let _ = writeln!(out);
        for wave in &plan.waves {
            let _ = writeln!(out, "### Wave {}", wave.index);
            let _ = writeln!(out);
            for e in &wave.edges {
                let _ = writeln!(out, "- `{}` → `{}`", e.from, e.to);
            }
            let _ = writeln!(out);
        }

        // ── mermaid diagram ───────────────────────────────────────────────
        let _ = writeln!(out, "## Graph");
        let _ = writeln!(out);
        let _ = writeln!(out, "```mermaid");
        let mermaid = super::mermaid::MermaidRenderer.render(plan);
        let _ = write!(out, "{}", mermaid);
        let _ = writeln!(out, "```");
        let _ = writeln!(out);

        // ── diagnostics ───────────────────────────────────────────────────
        if !plan.diagnostics.is_empty() {
            let _ = writeln!(out, "## Diagnostics");
            let _ = writeln!(out);
            for diag in &plan.diagnostics {
                let prefix = match diag.level {
                    DiagnosticLevel::Info => "ℹ️",
                    DiagnosticLevel::Warning => "⚠️",
                    DiagnosticLevel::Error => "❌",
                };
                let _ = writeln!(out, "{} {}", prefix, diag.message);
                let _ = writeln!(out);
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::execution_plan::ExecutionPlan;
    use crate::graph::{Format, TransformEdge, TransformGraph};
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
    fn test_markdown_renderer_contains_h1() {
        let output = MarkdownRenderer.render(&make_plan());
        assert!(output.contains("# Execution Plan"));
    }

    #[test]
    fn test_markdown_renderer_contains_statistics_section() {
        let output = MarkdownRenderer.render(&make_plan());
        assert!(output.contains("## Statistics"));
    }

    #[test]
    fn test_markdown_renderer_contains_mermaid_block() {
        let output = MarkdownRenderer.render(&make_plan());
        assert!(output.contains("```mermaid"));
    }

    #[test]
    fn test_markdown_renderer_contains_diagnostics_section() {
        let output = MarkdownRenderer.render(&make_plan());
        assert!(output.contains("## Diagnostics"));
    }
}
