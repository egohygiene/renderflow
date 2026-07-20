# `renderflow graph`

Inspect and export canonical execution plans.

## Subcommands

### `graph plan`

```bash
renderflow graph plan [--config FILE] [--format text|dot|mermaid|json|yaml|markdown] [--target FORMAT] [--export FILE] [--optimization MODE]
```

Renders the canonical `ExecutionPlan`.

### `graph render`

```bash
renderflow graph render [--config FILE] [--format mermaid|dot|text|json|yaml|markdown] [--target FORMAT] [--export FILE] [--optimization MODE]
```

Alias of plan with `mermaid` as the ergonomic default.

### `graph explain`

```bash
renderflow graph explain [--config FILE] [--target FORMAT] [--optimization MODE]
```

Prints planning diagnostics.

### `graph export`

```bash
renderflow graph export --format json|yaml|mermaid|dot|markdown|text -o FILE [--config FILE] [--target FORMAT] [--optimization MODE]
```

Writes the plan to a file; `-o/--output` is required.

### `graph doctor`

```bash
renderflow graph doctor [--config FILE] [--target FORMAT] [--optimization MODE]
```

Prints diagnostics and exits non-zero when an error-level diagnostic exists.

### `graph stats`

```bash
renderflow graph stats [--config FILE] [--target FORMAT] [--optimization MODE]
```

Prints total nodes, edges, depth, waves, estimated cost/quality, reused intermediates, and output count.

## Shared requirements

All graph subcommands require `transforms:` in the config because they build the graph from YAML definitions.
