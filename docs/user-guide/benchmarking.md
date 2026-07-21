# Benchmarking

Renderflow ships a Criterion-based benchmarking suite that continuously
measures, validates, and protects the performance characteristics of every
significant subsystem.

Performance is a first-class quality metric alongside correctness, testing,
linting, and documentation.

---

## Philosophy

Performance should be observable, reproducible, and measurable.

Every significant subsystem has benchmarks that answer:

- Is it faster or slower than before?
- Why did the measurement change?
- By how much?
- Is the regression acceptable?

Performance work is driven by data rather than assumptions.

---

## Benchmark suites

| Suite | File | What it measures |
|---|---|---|
| `graph_planner` | `benches/graph_planner.rs` | Pathfinding, DAG merge, multi-target planning |
| `dag_execution` | `benches/dag_execution.rs` | Execution wave generation, transform dispatch |
| `cache` | `benches/cache.rs` | Hash computation, cold/warm lookups, serialization |
| `large_graph` | `benches/large_graph.rs` | Planner and executor scalability under stress |

---

## Running benchmarks

### Prerequisites

No extra tooling is required.  Criterion is already a dev-dependency.

### Run all benchmark suites

```bash
cargo bench
```

This runs every suite and writes HTML reports to `target/criterion/`.

Using the Taskfile shortcut:

```bash
task bench
```

### Run a single suite

```bash
cargo bench --bench graph_planner
cargo bench --bench dag_execution
cargo bench --bench cache
cargo bench --bench large_graph
```

Using the Taskfile shortcut:

```bash
task bench-suite SUITE=graph_planner
```

### Compile without running

To verify the benchmarks build without actually collecting samples:

```bash
cargo bench --no-run
```

Using the Taskfile shortcut:

```bash
task bench-check
```

### Filter by benchmark name

Criterion accepts a regex pattern after `--` to run only matching benchmarks:

```bash
cargo bench --bench graph_planner -- find_path
```

---

## Interpreting results

Criterion prints a summary to the terminal after each benchmark run:

```
find_path/small/markdown_to_pdf
                        time:   [1.3452 µs 1.3521 µs 1.3595 µs]
                 change: [-1.23% -0.45% +0.34%] (p = 0.27 > 0.05)
                        No change in performance detected.
```

- **time** – 95 % confidence interval (lower bound, estimate, upper bound).
- **change** – percentage change from the previous run stored in `target/criterion/`.  Criterion uses a two-sample Student's t-test and reports the p-value.
- **No change / Performance has improved / Performance has regressed** – Criterion's verdict based on the configured significance threshold.

HTML reports are generated at:

```
target/criterion/<bench_name>/<function_name>/report/index.html
```

Open any `index.html` in a browser to see violin plots, iteration time
distributions, and a regression history chart.

---

## Regression detection

Criterion compares each new run against the baseline stored in
`target/criterion/`.  A regression is flagged when the change is
statistically significant at the 5 % level (p < 0.05).

To save a named baseline for later comparison:

```bash
cargo bench --bench graph_planner -- --save-baseline before_my_change
```

To compare against that baseline:

```bash
cargo bench --bench graph_planner -- --baseline before_my_change
```

---

## CI integration

The `.github/workflows/benchmarks.yml` workflow runs all benchmark suites:

- **Scheduled** – every Monday at 03:00 UTC.
- **Manual** – triggered via the GitHub Actions UI using `workflow_dispatch`.
  An optional `bench_filter` input allows running a single suite.

Published artifacts per run (retained for 90 days):

| Artifact | Contents |
|---|---|
| `criterion-reports-<run_id>` | Full Criterion HTML reports |
| `bench-output-<run_id>` | Raw bencher-format output |
| `bench-history-<run_id>` | Per-commit baseline snapshot (retained 365 days) |

---

## Adding a new benchmark

1. Create `benches/<subsystem>.rs` following the existing files as a template.
2. Add a `[[bench]]` entry in `Cargo.toml`:

   ```toml
   [[bench]]
   name = "<subsystem>"
   harness = false
   ```

3. Ensure the types you benchmark are reachable from the library crate
   (`src/lib.rs`).  Add `pub mod <module>` to `src/lib.rs` if needed.

4. Run `cargo bench --bench <subsystem> --no-run` to verify compilation.
5. Run `cargo bench --bench <subsystem>` to collect an initial baseline.

---

## Historical tracking

Each CI run uploads a per-commit baseline snapshot under
`bench-history-<run_id>`.  These artifacts can be downloaded and compared
across releases to build a performance history.

Future enhancements may include:

- JSON result files consumable by dashboard tooling.
- GitHub Pages performance graphs.
- Automated comment on pull requests when a regression is detected.
