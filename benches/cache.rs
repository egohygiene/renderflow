//! Benchmarks for the cache subsystem.
//!
//! Measures hash computation, cache lookup, insertion, and serialization
//! performance across cold and warm cache scenarios.

use std::collections::HashMap;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use renderflow::cache::{
    AiCache, AiCacheEntry, OutputCache, TransformCache, compute_ai_input_hash,
    compute_dag_node_hash, compute_input_hash, compute_output_hash, load_cache, save_cache,
};

// ── Hash computation benchmarks ───────────────────────────────────────────────

fn bench_compute_output_hash(c: &mut Criterion) {
    let content = "# Title\n\nSome document content with enough text to be representative.";
    let mut group = c.benchmark_group("compute_output_hash");
    group.bench_function("no_template", |b| {
        b.iter(|| compute_output_hash(content, "pdf", None, None));
    });
    group.bench_function("with_template_name", |b| {
        b.iter(|| compute_output_hash(content, "pdf", Some("report.html"), None));
    });
    group.bench_function("with_template_content", |b| {
        b.iter(|| {
            compute_output_hash(
                content,
                "pdf",
                Some("report.html"),
                Some("<html>{{body}}</html>"),
            )
        });
    });
    group.finish();
}

fn bench_compute_input_hash(c: &mut Criterion) {
    let content = "# Title\n\nSome document content.";
    let config = "output: [pdf, html]\nvariables:\n  author: Alice\n";

    let mut vars_small: HashMap<String, String> = HashMap::new();
    vars_small.insert("author".to_string(), "Alice".to_string());

    let mut vars_large: HashMap<String, String> = HashMap::new();
    for i in 0..20 {
        vars_large.insert(format!("var_{i}"), format!("value_{i}"));
    }

    let mut group = c.benchmark_group("compute_input_hash");
    group.bench_function("no_vars", |b| {
        b.iter(|| compute_input_hash(content, config, &HashMap::new()));
    });
    group.bench_function("5_vars", |b| {
        b.iter(|| compute_input_hash(content, config, &vars_small));
    });
    group.bench_function("20_vars", |b| {
        b.iter(|| compute_input_hash(content, config, &vars_large));
    });
    group.finish();
}

fn bench_compute_dag_node_hash(c: &mut Criterion) {
    c.bench_function("compute_dag_node_hash", |b| {
        b.iter(|| compute_dag_node_hash("# Title\n\nContent", "markdown", "html"));
    });
}

fn bench_compute_ai_input_hash(c: &mut Criterion) {
    let prompt = "Summarize the following document:\n\n# Title\n\nContent goes here.";
    c.bench_function("compute_ai_input_hash", |b| {
        b.iter(|| compute_ai_input_hash(prompt, "mistral"));
    });
}

// ── Hash scaling benchmarks ───────────────────────────────────────────────────

fn bench_hash_input_size_scaling(c: &mut Criterion) {
    let sizes = [64usize, 1_024, 16_384, 65_536];
    let vars: HashMap<String, String> = HashMap::new();
    let config = "";

    let mut group = c.benchmark_group("compute_input_hash/content_size_scaling");
    for size in sizes {
        let content = "a".repeat(size);
        group.bench_with_input(BenchmarkId::new("bytes", size), &content, |b, c| {
            b.iter(|| compute_input_hash(c, config, &vars));
        });
    }
    group.finish();
}

// ── TransformCache lookup / insert benchmarks ─────────────────────────────────

fn bench_transform_cache_lookup(c: &mut Criterion) {
    let mut cache = TransformCache::default();
    // Pre-populate with entries.
    for i in 0..100u32 {
        cache.insert(format!("{i:064x}"), format!("output_{i}"));
    }
    let hit_key = "99".to_string() + &"0".repeat(62);
    let miss_key = "ff".to_string() + &"0".repeat(62);

    let mut group = c.benchmark_group("transform_cache/lookup");
    group.bench_function("hit", |b| {
        b.iter(|| cache.get(&hit_key));
    });
    group.bench_function("miss", |b| {
        b.iter(|| cache.get(&miss_key));
    });
    group.finish();
}

fn bench_transform_cache_insert(c: &mut Criterion) {
    c.bench_function("transform_cache/insert", |b| {
        b.iter_batched(
            || TransformCache::default(),
            |mut cache| {
                cache.insert(
                    "abc123".to_string(),
                    "transformed output".to_string(),
                );
                cache
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

// ── OutputCache lookup / insert benchmarks ────────────────────────────────────

fn bench_output_cache_lookup(c: &mut Criterion) {
    let mut cache = OutputCache::default();
    for i in 0..100u32 {
        cache.insert(format!("/dist/output_{i}.pdf"), format!("{i:064x}"));
    }
    let hit_path = "/dist/output_99.pdf";
    let miss_path = "/dist/output_999.pdf";

    let mut group = c.benchmark_group("output_cache/lookup");
    group.bench_function("hit", |b| {
        b.iter(|| cache.get(hit_path));
    });
    group.bench_function("miss", |b| {
        b.iter(|| cache.get(miss_path));
    });
    group.finish();
}

// ── Cold / warm cache round-trip benchmarks ───────────────────────────────────

fn bench_cache_cold_warm(c: &mut Criterion) {
    let tmp = tempfile::tempdir().expect("failed to create tmp dir");
    let cache_path = tmp.path().join("transform_cache.json");

    // Pre-populate and save a warm cache.
    let mut warm_cache = TransformCache::default();
    for i in 0..50u32 {
        warm_cache.insert(format!("{i:064x}"), format!("output_{i}"));
    }
    save_cache(&warm_cache, &cache_path).expect("failed to save warm cache");

    let mut group = c.benchmark_group("transform_cache/load");
    group.bench_function("cold_cache/empty_file", |b| {
        let cold_path = tmp.path().join("cold.json");
        std::fs::write(&cold_path, "{}").expect("failed to write empty cache");
        b.iter(|| load_cache(&cold_path));
    });
    group.bench_function("warm_cache/50_entries", |b| {
        b.iter(|| load_cache(&cache_path));
    });
    group.finish();
}

fn bench_cache_serialization(c: &mut Criterion) {
    let sizes = [10usize, 50, 200];
    let tmp = tempfile::tempdir().expect("failed to create tmp dir");

    let mut group = c.benchmark_group("transform_cache/save");
    for size in sizes {
        let mut cache = TransformCache::default();
        for i in 0..size {
            cache.insert(format!("{i:064x}"), "x".repeat(512));
        }
        let path = tmp.path().join(format!("cache_{size}.json"));
        group.bench_with_input(BenchmarkId::new("entries", size), &(cache, path), |b, (c, p)| {
            b.iter(|| save_cache(c, p).expect("save must succeed"));
        });
    }
    group.finish();
}

// ── AiCache benchmarks ────────────────────────────────────────────────────────

fn bench_ai_cache_lookup(c: &mut Criterion) {
    let mut cache = AiCache::default();
    for i in 0..50u32 {
        let hash = format!("{i:064x}");
        cache.insert(
            hash.clone(),
            AiCacheEntry {
                input_hash: hash,
                model: "mistral".to_string(),
                timestamp: 1_700_000_000,
                output: format!("AI output {i}"),
            },
        );
    }
    let hit_key = format!("{:064x}", 49u32);
    let miss_key = format!("{:064x}", 999u32);

    let mut group = c.benchmark_group("ai_cache/lookup");
    group.bench_function("hit", |b| {
        b.iter(|| cache.get(&hit_key));
    });
    group.bench_function("miss", |b| {
        b.iter(|| cache.get(&miss_key));
    });
    group.finish();
}

// ── Criterion wiring ──────────────────────────────────────────────────────────

criterion_group!(
    cache_benches,
    bench_compute_output_hash,
    bench_compute_input_hash,
    bench_compute_dag_node_hash,
    bench_compute_ai_input_hash,
    bench_hash_input_size_scaling,
    bench_transform_cache_lookup,
    bench_transform_cache_insert,
    bench_output_cache_lookup,
    bench_cache_cold_warm,
    bench_cache_serialization,
    bench_ai_cache_lookup,
);
criterion_main!(cache_benches);
