# CLI Overview

Renderflow exposes a small top-level CLI with focused subcommands.

## Top-level commands

| Command | Purpose |
|---|---|
| `renderflow build` | Standard build pipeline; can also switch into graph mode with `--target` or `--all` |
| `renderflow watch` | Watch config/input/templates and rebuild on change |
| `renderflow audit` | Write an optimization audit report to `audits/` |
| `renderflow inspect` | Visualize a DAG as a tree or Graphviz DOT |
| `renderflow plugin ...` | Inspect plugin registry state |
| `renderflow ai ...` | Inspect AI providers, models, diagnostics, and cache |
| `renderflow graph ...` | Inspect, explain, export, and analyze execution plans |

## Global flags

- `--verbose` enables DEBUG logging
- `--debug` enables TRACE logging

## Shorthand mode

Passing a config path without a subcommand is equivalent to `renderflow build --config ...`.

```bash
renderflow my-project.yaml
```

## Build modes

### Standard build

Uses `outputs:` from `renderflow.yaml` and runs the built-in transform + render pipeline.

### Graph build

`renderflow build --target pdf` or `renderflow build --all` requires a `transforms` YAML file and resolves reachable formats through the transform graph.

## Inspection commands

- `inspect` is a lightweight DAG visualizer
- `graph plan` renders the canonical execution plan
- `graph explain` prints planner diagnostics
- `graph stats` prints metadata such as nodes, edges, depth, waves, and estimated cost/quality

Continue with the [CLI reference](../cli-reference/index.md) for flags and examples.
