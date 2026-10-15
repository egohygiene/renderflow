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
renderflow graph export --format json|yaml|mermaid|dot|markdown|text --output FILE [--config FILE] [--target FORMAT] [--optimization MODE]
```

Writes the plan to a file; `--output` is required.

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

Graph commands inspect the same resolved plan used by `build` and the SDK.
Ordinary artifact builds use registered capabilities, with optional
`transforms:` for custom definitions.

A `kind: composition` source uses the existing composition engine and distinct
locale/output forest targets. The format graph is only a projection: two
locales producing SVG still have two separate publication identities. Use
JSON or YAML to inspect the full forest inventory. Estimated costs, quality
scores and planned reuse for this bounded engine are structural planning
values, not observed performance or execution evidence.

```bash
renderflow graph plan --config "examples/page-composition/raster-composition/renderflow.yaml" --format "json"
```

Composition mode accepts `--target` but rejects profile/exclusion and
optimization overrides. See the [composition guide](../user-guide/page-composition.md)
for native settings and the ordinary build workflow.
