//! Large-graph stress tests.
//!
//! Simulates production-sized transformation networks with hundreds of nodes
//! and edges.  These benchmarks validate planner scalability and executor
//! throughput under representative heavy workloads.
//!
//! Because Format is an enum with a fixed set of variants, large graphs are
//! constructed by adding many parallel edges between the available variants
//! rather than by adding new node types.

use std::sync::Arc;

use anyhow::Result;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use renderflow::graph::{DagExecutor, Format, TransformEdge, TransformGraph};
use renderflow::optimization::OptimizationMode;
use renderflow::transforms::Transform;

// ── Identity transform ────────────────────────────────────────────────────────

struct IdentityTransform;

impl Transform for IdentityTransform {
    fn apply(&self, input: String) -> Result<String> {
        Ok(input)
    }
}

// ── Large graph builders ──────────────────────────────────────────────────────

/// All document-format variants available for building stress graphs.
const ALL_FORMATS: &[Format] = &[
    Format::Markdown,
    Format::Html,
    Format::Pdf,
    Format::Docx,
    Format::Epub,
    Format::Rst,
    Format::Latex,
    Format::Fountain,
    Format::Jpeg,
    Format::Png,
    Format::Tiff,
    Format::Cbz,
];

/// Build a densely-connected graph where every format can reach every other
/// format directly (complete directed graph over all document variants).
///
/// This represents a worst-case planning scenario with maximum branching.
fn dense_document_graph() -> TransformGraph {
    let mut g = TransformGraph::new();
    for (i, &from) in ALL_FORMATS.iter().enumerate() {
        for (j, &to) in ALL_FORMATS.iter().enumerate() {
            if i != j {
                let cost = 0.5 + (i as f32) * 0.1 + (j as f32) * 0.05;
                let quality = 1.0 - (i as f32) * 0.02 - (j as f32) * 0.01;
                g.add_transform(TransformEdge::new(from, to, cost, quality.max(0.5)));
            }
        }
    }
    g
}

/// Build a deep chain of sequential transformations:
/// Markdown → Html → Rst → Latex → Pdf → Epub → Docx → Fountain
fn deep_chain_graph() -> TransformGraph {
    let chain = [
        Format::Markdown,
        Format::Html,
        Format::Rst,
        Format::Latex,
        Format::Pdf,
        Format::Epub,
        Format::Docx,
        Format::Fountain,
    ];
    let mut g = TransformGraph::new();
    for pair in chain.windows(2) {
        g.add_transform(TransformEdge::new(pair[0], pair[1], 0.5, 0.95));
    }
    g
}

/// Build a graph with many parallel competing edges between each format pair.
fn parallel_edges_graph(edges_per_pair: usize) -> TransformGraph {
    let pairs = [
        (Format::Markdown, Format::Html),
        (Format::Html, Format::Pdf),
        (Format::Html, Format::Docx),
        (Format::Markdown, Format::Latex),
        (Format::Latex, Format::Pdf),
    ];
    let mut g = TransformGraph::new();
    for &(from, to) in &pairs {
        for k in 0..edges_per_pair {
            let cost = 0.5 + (k as f32) * 0.1;
            let quality = 1.0 - (k as f32) * 0.05;
            g.add_transform(TransformEdge::new(from, to, cost, quality.max(0.5)));
        }
    }
    g
}

// ── Planner stress benchmarks ─────────────────────────────────────────────────

fn bench_dense_graph_planning(c: &mut Criterion) {
    let g = dense_document_graph();
    c.bench_function("large_graph/dense/find_path/markdown_to_pdf", |b| {
        b.iter(|| g.find_path(Format::Markdown, Format::Pdf).expect("path must exist"));
    });
}

fn bench_dense_graph_all_paths(c: &mut Criterion) {
    let g = dense_document_graph();
    c.bench_function("large_graph/dense/find_all_paths/markdown_to_pdf", |b| {
        b.iter(|| g.find_all_paths(Format::Markdown, Format::Pdf));
    });
}

fn bench_dense_graph_multi_target(c: &mut Criterion) {
    let g = dense_document_graph();
    let targets = [Format::Pdf, Format::Docx, Format::Epub, Format::Html, Format::Rst];
    c.bench_function("large_graph/dense/build_multi_target_dag/5_targets", |b| {
        b.iter(|| {
            g.build_multi_target_dag(Format::Markdown, &targets)
                .expect("dag must build")
        });
    });
}

fn bench_dense_graph_all_modes(c: &mut Criterion) {
    let g = dense_document_graph();
    let modes = [
        OptimizationMode::Speed,
        OptimizationMode::Quality,
        OptimizationMode::Balanced,
    ];
    let mut group = c.benchmark_group("large_graph/dense/find_path_with_mode");
    for mode in modes {
        group.bench_with_input(
            BenchmarkId::new("mode", format!("{mode:?}")),
            &mode,
            |b, &m| {
                b.iter(|| {
                    g.find_path_with_mode(Format::Markdown, Format::Pdf, m)
                        .expect("path must exist")
                });
            },
        );
    }
    group.finish();
}

// ── Deep chain benchmarks ─────────────────────────────────────────────────────

fn bench_deep_chain_planning(c: &mut Criterion) {
    let g = deep_chain_graph();
    c.bench_function("large_graph/deep_chain/find_path/markdown_to_fountain", |b| {
        b.iter(|| {
            g.find_path(Format::Markdown, Format::Fountain)
                .expect("path must exist")
        });
    });
}

fn bench_deep_chain_execution(c: &mut Criterion) {
    let g = deep_chain_graph();
    let dag = g
        .build_multi_target_dag(Format::Markdown, &[Format::Fountain])
        .expect("dag must build");

    let mut exec = DagExecutor::new();
    exec.register_single(Format::Markdown, Format::Html, Arc::new(IdentityTransform));
    exec.register_single(Format::Html, Format::Rst, Arc::new(IdentityTransform));
    exec.register_single(Format::Rst, Format::Latex, Arc::new(IdentityTransform));
    exec.register_single(Format::Latex, Format::Pdf, Arc::new(IdentityTransform));
    exec.register_single(Format::Pdf, Format::Epub, Arc::new(IdentityTransform));
    exec.register_single(Format::Epub, Format::Docx, Arc::new(IdentityTransform));
    exec.register_single(Format::Docx, Format::Fountain, Arc::new(IdentityTransform));

    c.bench_function("large_graph/deep_chain/execute/7_hops", |b| {
        b.iter(|| {
            exec.execute(&dag, Format::Markdown, "# Document".to_string())
                .expect("execution must succeed")
        });
    });
}

// ── Parallel-edges benchmarks ─────────────────────────────────────────────────

fn bench_parallel_edges_planning(c: &mut Criterion) {
    let edge_counts = [1usize, 5, 10, 20];
    let mut group = c.benchmark_group("large_graph/parallel_edges/find_path");
    for &n in &edge_counts {
        let g = parallel_edges_graph(n);
        group.bench_with_input(
            BenchmarkId::new("edges_per_pair", n),
            &n,
            |b, _| {
                b.iter(|| {
                    g.find_path(Format::Markdown, Format::Pdf)
                        .expect("path must exist")
                });
            },
        );
    }
    group.finish();
}

// ── Graph construction scalability ───────────────────────────────────────────

fn bench_large_graph_construction(c: &mut Criterion) {
    c.bench_function("large_graph/construction/complete_12_node_graph", |b| {
        b.iter(dense_document_graph);
    });
}

// ── Multi-target execution at scale ──────────────────────────────────────────

fn bench_dense_graph_execution(c: &mut Criterion) {
    let g = dense_document_graph();
    let targets = [Format::Pdf, Format::Docx, Format::Epub];
    let dag = g
        .build_multi_target_dag(Format::Markdown, &targets)
        .expect("dag must build");

    let mut exec = DagExecutor::new();
    // Register all transitions between the formats involved in this DAG.
    let all: &[(Format, Format)] = &[
        (Format::Markdown, Format::Pdf),
        (Format::Markdown, Format::Docx),
        (Format::Markdown, Format::Epub),
        (Format::Markdown, Format::Html),
        (Format::Html, Format::Pdf),
        (Format::Html, Format::Docx),
        (Format::Html, Format::Epub),
        (Format::Pdf, Format::Epub),
        (Format::Pdf, Format::Docx),
        (Format::Epub, Format::Pdf),
        (Format::Epub, Format::Docx),
        (Format::Docx, Format::Pdf),
        (Format::Docx, Format::Epub),
    ];
    for &(from, to) in all {
        exec.register_single(from, to, Arc::new(IdentityTransform));
    }

    c.bench_function("large_graph/dense/execute/3_targets", |b| {
        b.iter(|| {
            exec.execute(&dag, Format::Markdown, "# Document".to_string())
                .expect("execution must succeed")
        });
    });
}

// ── Criterion wiring ──────────────────────────────────────────────────────────

criterion_group!(
    large_graph_benches,
    bench_dense_graph_planning,
    bench_dense_graph_all_paths,
    bench_dense_graph_multi_target,
    bench_dense_graph_all_modes,
    bench_deep_chain_planning,
    bench_deep_chain_execution,
    bench_parallel_edges_planning,
    bench_large_graph_construction,
    bench_dense_graph_execution,
);
criterion_main!(large_graph_benches);
