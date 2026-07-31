//! Benchmarks for the DAG execution subsystem.
//!
//! Measures execution wave generation, dependency resolution, scheduling
//! overhead, and transform dispatch for multi-target DAGs.

use std::sync::Arc;

use anyhow::Result;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use renderflow::graph::{DagExecutor, Format, TransformEdge, TransformGraph};
use renderflow::transforms::Transform;

// ── Identity transform ────────────────────────────────────────────────────────

/// A no-op transform that echoes its input unchanged.
///
/// Used to isolate planner/scheduling overhead from actual I/O.
struct IdentityTransform;

impl Transform for IdentityTransform {
    fn apply(&self, input: String) -> Result<String> {
        Ok(input)
    }
}

// ── Graph + executor builders ─────────────────────────────────────────────────

/// Build a simple linear chain: Markdown → Html → Pdf
fn linear_executor() -> (DagExecutor, renderflow::graph::MultiTargetDag) {
    let mut g = TransformGraph::new();
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));

    let dag = g
        .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
        .expect("dag must build");

    let mut exec = DagExecutor::new();
    exec.register_single(Format::Markdown, Format::Html, Arc::new(IdentityTransform));
    exec.register_single(Format::Html, Format::Pdf, Arc::new(IdentityTransform));

    (exec, dag)
}

/// Build a fan-out executor: Markdown → Html → {Pdf, Docx, Epub}
fn fanout_executor() -> (DagExecutor, renderflow::graph::MultiTargetDag) {
    let mut g = TransformGraph::new();
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
    g.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));
    g.add_transform(TransformEdge::new(Format::Html, Format::Epub, 0.7, 0.88));

    let dag = g
        .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx, Format::Epub])
        .expect("dag must build");

    let mut exec = DagExecutor::new();
    exec.register_single(Format::Markdown, Format::Html, Arc::new(IdentityTransform));
    exec.register_single(Format::Html, Format::Pdf, Arc::new(IdentityTransform));
    exec.register_single(Format::Html, Format::Docx, Arc::new(IdentityTransform));
    exec.register_single(Format::Html, Format::Epub, Arc::new(IdentityTransform));

    (exec, dag)
}

/// Build a deep chain with multiple sequential dependencies.
fn deep_chain_executor() -> (DagExecutor, renderflow::graph::MultiTargetDag) {
    let mut g = TransformGraph::new();
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    g.add_transform(TransformEdge::new(Format::Html, Format::Latex, 0.6, 0.95));
    g.add_transform(TransformEdge::new(Format::Latex, Format::Pdf, 0.7, 0.98));
    g.add_transform(TransformEdge::new(Format::Pdf, Format::Epub, 1.0, 0.75));

    let dag = g
        .build_multi_target_dag(Format::Markdown, &[Format::Epub])
        .expect("dag must build");

    let mut exec = DagExecutor::new();
    exec.register_single(Format::Markdown, Format::Html, Arc::new(IdentityTransform));
    exec.register_single(Format::Html, Format::Latex, Arc::new(IdentityTransform));
    exec.register_single(Format::Latex, Format::Pdf, Arc::new(IdentityTransform));
    exec.register_single(Format::Pdf, Format::Epub, Arc::new(IdentityTransform));

    (exec, dag)
}

// ── Execution-order (wave generation) benchmarks ─────────────────────────────

fn bench_execution_order_linear(c: &mut Criterion) {
    let mut g = TransformGraph::new();
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
    let dag = g
        .build_multi_target_dag(Format::Markdown, &[Format::Pdf])
        .expect("dag must build");

    c.bench_function("execution_order/linear/2_edges", |b| {
        b.iter(|| dag.execution_order());
    });
}

fn bench_execution_order_fanout(c: &mut Criterion) {
    let mut g = TransformGraph::new();
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
    g.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));
    g.add_transform(TransformEdge::new(Format::Html, Format::Epub, 0.7, 0.88));
    let dag = g
        .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Docx, Format::Epub])
        .expect("dag must build");

    c.bench_function("execution_order/fanout/4_edges", |b| {
        b.iter(|| dag.execution_order());
    });
}

// ── Execute benchmarks ────────────────────────────────────────────────────────

fn bench_execute_linear(c: &mut Criterion) {
    let (exec, dag) = linear_executor();
    c.bench_function("execute/linear/markdown_to_pdf", |b| {
        b.iter(|| {
            exec.execute(&dag, Format::Markdown, "# Hello World".to_string())
                .expect("execution must succeed")
        });
    });
}

fn bench_execute_fanout(c: &mut Criterion) {
    let (exec, dag) = fanout_executor();
    c.bench_function("execute/fanout/markdown_to_3_targets", |b| {
        b.iter(|| {
            exec.execute(&dag, Format::Markdown, "# Hello World".to_string())
                .expect("execution must succeed")
        });
    });
}

fn bench_execute_deep_chain(c: &mut Criterion) {
    let (exec, dag) = deep_chain_executor();
    c.bench_function("execute/deep_chain/4_hops", |b| {
        b.iter(|| {
            exec.execute(&dag, Format::Markdown, "# Hello World".to_string())
                .expect("execution must succeed")
        });
    });
}

// ── Input-size scaling benchmarks ─────────────────────────────────────────────

fn bench_execute_input_scaling(c: &mut Criterion) {
    let (exec, dag) = linear_executor();

    let sizes = [64usize, 1_024, 16_384, 65_536];
    let mut group = c.benchmark_group("execute/input_scaling/markdown_to_pdf");
    for size in sizes {
        let input = "a".repeat(size);
        group.bench_with_input(
            BenchmarkId::new("bytes", size),
            &input,
            |b, content| {
                b.iter(|| {
                    exec.execute(&dag, Format::Markdown, content.clone())
                        .expect("execution must succeed")
                });
            },
        );
    }
    group.finish();
}

// ── Criterion wiring ──────────────────────────────────────────────────────────

criterion_group!(
    dag_execution_benches,
    bench_execution_order_linear,
    bench_execution_order_fanout,
    bench_execute_linear,
    bench_execute_fanout,
    bench_execute_deep_chain,
    bench_execute_input_scaling,
);
criterion_main!(dag_execution_benches);
