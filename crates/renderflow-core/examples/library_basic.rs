use renderflow::graph::{ExecutionPlan, Format, TransformEdge, TransformGraph};
use renderflow::optimization::OptimizationMode;

fn main() -> anyhow::Result<()> {
    let mut graph = TransformGraph::new();
    graph.add_transform(TransformEdge::new(
        Format::Markdown,
        Format::Html,
        0.4,
        0.98,
    ));
    graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.9));

    let targets = [Format::Html, Format::Pdf];
    let dag = graph
        .build_multi_target_dag_with_mode(Format::Markdown, &targets, OptimizationMode::Balanced)
        .ok_or_else(|| anyhow::anyhow!("no route found for requested targets"))?;

    let plan =
        ExecutionPlan::from_dag(&dag, Format::Markdown, &targets, OptimizationMode::Balanced);

    println!(
        "planned {} edges across {} waves",
        plan.metadata.total_edges, plan.metadata.execution_waves
    );

    Ok(())
}
