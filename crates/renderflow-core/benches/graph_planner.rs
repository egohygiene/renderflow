//! Benchmarks for the graph planner subsystem.
//!
//! Measures pathfinding, DAG merge, and multi-target planning performance
//! across graphs of increasing complexity.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use renderflow::graph::{Format, TransformEdge, TransformGraph};
use renderflow::optimization::OptimizationMode;

// ── Graph builders ────────────────────────────────────────────────────────────

/// Build the minimal document-pipeline graph used in most single-path tests.
///
/// Topology: Markdown → Html → Pdf
///           Markdown → Html → Docx
fn small_graph() -> TransformGraph {
    let mut g = TransformGraph::new();
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
    g.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));
    g
}

/// Build a medium-complexity graph with several competing paths.
///
/// Markdown can reach Pdf via Html or directly via LaTeX.
/// Several intermediate formats widen the search space.
fn medium_graph() -> TransformGraph {
    let mut g = TransformGraph::new();
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Latex, 0.7, 0.95));
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Rst, 0.4, 0.90));
    g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
    g.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));
    g.add_transform(TransformEdge::new(Format::Html, Format::Epub, 0.7, 0.88));
    g.add_transform(TransformEdge::new(Format::Latex, Format::Pdf, 0.6, 0.98));
    g.add_transform(TransformEdge::new(Format::Rst, Format::Html, 0.5, 0.92));
    g.add_transform(TransformEdge::new(Format::Rst, Format::Latex, 0.6, 0.88));
    g
}

/// Build a graph that exercises all common document formats as nodes.
fn full_format_graph() -> TransformGraph {
    let edges = [
        (Format::Markdown, Format::Html, 0.5, 1.0),
        (Format::Markdown, Format::Latex, 0.7, 0.95),
        (Format::Markdown, Format::Rst, 0.4, 0.90),
        (Format::Html, Format::Pdf, 0.8, 0.85),
        (Format::Html, Format::Docx, 0.6, 0.90),
        (Format::Html, Format::Epub, 0.7, 0.88),
        (Format::Latex, Format::Pdf, 0.6, 0.98),
        (Format::Latex, Format::Epub, 0.9, 0.91),
        (Format::Rst, Format::Html, 0.5, 0.92),
        (Format::Rst, Format::Latex, 0.6, 0.88),
        (Format::Pdf, Format::Epub, 1.2, 0.75),
        (Format::Docx, Format::Pdf, 0.9, 0.80),
        (Format::Epub, Format::Html, 1.0, 0.82),
    ];
    let mut g = TransformGraph::new();
    for (from, to, cost, quality) in edges {
        g.add_transform(TransformEdge::new(from, to, cost, quality));
    }
    g
}

/// Build a wide, shallow fan-out graph with `width` parallel paths from source to sink.
fn fanout_graph(width: usize) -> (TransformGraph, Format, Format) {
    // All the intermediates we can squeeze out of the Format enum.
    let intermediates = [
        Format::Html,
        Format::Latex,
        Format::Rst,
        Format::Epub,
        Format::Docx,
        Format::Pdf,
        Format::Fountain,
    ];
    let width = width.min(intermediates.len());
    let mut g = TransformGraph::new();
    for &mid in &intermediates[..width] {
        g.add_transform(TransformEdge::new(Format::Markdown, mid, 0.5, 1.0));
        g.add_transform(TransformEdge::new(mid, Format::Pdf, 0.8, 0.85));
    }
    (g, Format::Markdown, Format::Pdf)
}

// ── Single-path benchmarks ────────────────────────────────────────────────────

fn bench_find_path_small(c: &mut Criterion) {
    let g = small_graph();
    c.bench_function("find_path/small/markdown_to_pdf", |b| {
        b.iter(|| {
            g.find_path(Format::Markdown, Format::Pdf)
                .expect("path must exist")
        });
    });
}

fn bench_find_path_medium(c: &mut Criterion) {
    let g = medium_graph();
    c.bench_function("find_path/medium/markdown_to_pdf", |b| {
        b.iter(|| {
            g.find_path(Format::Markdown, Format::Pdf)
                .expect("path must exist")
        });
    });
}

fn bench_find_path_full(c: &mut Criterion) {
    let g = full_format_graph();
    c.bench_function("find_path/full/markdown_to_epub", |b| {
        b.iter(|| {
            g.find_path(Format::Markdown, Format::Epub)
                .expect("path must exist")
        });
    });
}

// ── Optimization-mode benchmarks ─────────────────────────────────────────────

fn bench_find_path_modes(c: &mut Criterion) {
    let g = full_format_graph();
    let modes = [
        OptimizationMode::Speed,
        OptimizationMode::Quality,
        OptimizationMode::Balanced,
    ];
    let mut group = c.benchmark_group("find_path_with_mode/markdown_to_epub");
    for mode in modes {
        group.bench_with_input(
            BenchmarkId::new("mode", format!("{mode:?}")),
            &mode,
            |b, &m| {
                b.iter(|| {
                    g.find_path_with_mode(Format::Markdown, Format::Epub, m)
                        .expect("path must exist")
                });
            },
        );
    }
    group.finish();
}

// ── All-paths benchmarks ──────────────────────────────────────────────────────

fn bench_find_all_paths(c: &mut Criterion) {
    let g = medium_graph();
    c.bench_function("find_all_paths/medium/markdown_to_pdf", |b| {
        b.iter(|| g.find_all_paths(Format::Markdown, Format::Pdf));
    });
}

// ── Fan-out benchmarks ────────────────────────────────────────────────────────

fn bench_fanout_pathfinding(c: &mut Criterion) {
    let widths = [1usize, 3, 5, 7];
    let mut group = c.benchmark_group("find_path/fanout");
    for width in widths {
        let (g, from, to) = fanout_graph(width);
        group.bench_with_input(
            BenchmarkId::new("width", width),
            &width,
            |b, _| {
                b.iter(|| g.find_path(from, to).expect("path must exist"));
            },
        );
    }
    group.finish();
}

// ── Multi-target DAG benchmarks ───────────────────────────────────────────────

fn bench_multi_target_dag_small(c: &mut Criterion) {
    let g = small_graph();
    let targets = [Format::Pdf, Format::Docx];
    c.bench_function("build_multi_target_dag/small/2_targets", |b| {
        b.iter(|| {
            g.build_multi_target_dag(Format::Markdown, &targets)
                .expect("dag must build")
        });
    });
}

fn bench_multi_target_dag_medium(c: &mut Criterion) {
    let g = medium_graph();
    let targets = [Format::Pdf, Format::Docx, Format::Epub];
    c.bench_function("build_multi_target_dag/medium/3_targets", |b| {
        b.iter(|| {
            g.build_multi_target_dag(Format::Markdown, &targets)
                .expect("dag must build")
        });
    });
}

fn bench_multi_target_dag_scaling(c: &mut Criterion) {
    let g = full_format_graph();
    let target_sets: &[(&str, &[Format])] = &[
        ("1_target", &[Format::Pdf]),
        ("2_targets", &[Format::Pdf, Format::Docx]),
        ("3_targets", &[Format::Pdf, Format::Docx, Format::Epub]),
        ("4_targets", &[Format::Pdf, Format::Docx, Format::Epub, Format::Html]),
    ];
    let mut group = c.benchmark_group("build_multi_target_dag/full_graph");
    for (label, targets) in target_sets {
        group.bench_with_input(BenchmarkId::new("targets", label), targets, |b, t| {
            b.iter(|| {
                g.build_multi_target_dag(Format::Markdown, t)
                    .expect("dag must build")
            });
        });
    }
    group.finish();
}

// ── Shared-node detection ─────────────────────────────────────────────────────

fn bench_shared_node_detection(c: &mut Criterion) {
    // A graph where many targets share the Html intermediate.
    let mut g = TransformGraph::new();
    g.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.5, 1.0));
    g.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.8, 0.85));
    g.add_transform(TransformEdge::new(Format::Html, Format::Docx, 0.6, 0.90));
    g.add_transform(TransformEdge::new(Format::Html, Format::Epub, 0.7, 0.88));

    let targets = [Format::Pdf, Format::Docx, Format::Epub];
    c.bench_function("build_multi_target_dag/shared_html_node/3_targets", |b| {
        b.iter(|| {
            g.build_multi_target_dag(Format::Markdown, &targets)
                .expect("dag must build")
        });
    });
}

// ── Graph construction benchmarks ────────────────────────────────────────────

fn bench_graph_construction(c: &mut Criterion) {
    let edge_counts = [5usize, 10, 15];
    let mut group = c.benchmark_group("graph_construction/edge_count");
    for &n in &edge_counts {
        group.bench_with_input(BenchmarkId::new("edges", n), &n, |b, &count| {
            b.iter(|| {
                let mut g = TransformGraph::new();
                let formats = [
                    Format::Markdown,
                    Format::Html,
                    Format::Pdf,
                    Format::Docx,
                    Format::Epub,
                    Format::Rst,
                ];
                for i in 0..count {
                    let from = formats[i % formats.len()];
                    let to = formats[(i + 1) % formats.len()];
                    g.add_transform(TransformEdge::new(from, to, 0.5, 1.0));
                }
                g
            });
        });
    }
    group.finish();
}

// ── Execution-order benchmarks ────────────────────────────────────────────────

fn bench_execution_order(c: &mut Criterion) {
    let g = full_format_graph();
    let dag = g
        .build_multi_target_dag(Format::Markdown, &[Format::Pdf, Format::Epub, Format::Docx])
        .expect("dag must build");

    c.bench_function("execution_order/full_graph/3_targets", |b| {
        b.iter(|| dag.execution_order());
    });
}

// ── Criterion wiring ──────────────────────────────────────────────────────────

criterion_group!(
    planner_benches,
    bench_find_path_small,
    bench_find_path_medium,
    bench_find_path_full,
    bench_find_path_modes,
    bench_find_all_paths,
    bench_fanout_pathfinding,
    bench_multi_target_dag_small,
    bench_multi_target_dag_medium,
    bench_multi_target_dag_scaling,
    bench_shared_node_detection,
    bench_graph_construction,
    bench_execution_order,
);
criterion_main!(planner_benches);
