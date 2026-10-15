# `renderflow inspect`

Inspect either an arbitrary input artifact or the resolved build plan for a config.

## Syntax

```bash
renderflow inspect [--config FILE] [--output-format tree|dot] [--target FORMAT | --all] [--export FILE]
renderflow inspect --input FILE [--media-type TYPE] [--extract [--recursive]] [--export FILE]
```

## Flags

| Flag | Description |
|---|---|
| `--config FILE` | config path, default `renderflow.yaml` |
| `--output-format FORMAT` | `tree` (default) or `dot` |
| `--target FORMAT` | restrict output to one target |
| `--all` | graph builds: show all policy-allowed reachable targets |
| `--export FILE` | write output to a file |
| `--input FILE` | emit a machine-readable `renderflow.intake/v1` artifact report instead of a DAG |
| `--media-type TYPE` | add a source-reported media-type detection signal |
| `--extract` | extract safe provider-supported child artifacts |
| `--recursive` | recursively extract supported nested containers |
| `--store DIR` | content-addressed intake store, default `.renderflow/intake-artifacts` |
| `--max-depth N` | recursive extraction depth budget |
| `--max-artifacts N` | source-plus-child artifact budget |
| `--max-extracted-bytes N` | total extracted-byte budget |
| `--max-expansion-ratio N` | maximum per-entry archive expansion ratio |

## Notes

- Configuration inspection uses the same build facade and configured target
  intent as `build`. `transforms:` is optional when built-in capabilities cover
  an ordinary artifact build.
- `--target` replaces the configured target selection; `--all` explicitly
  selects all reachable outputs in graph mode.
- A `kind: composition` source uses the native locale-aware composition plan.
  It accepts `--target` and rejects `--all` and optimization overrides. Tree
  and DOT views summarize the common plan; use `graph plan --format "json"`
  for the full artifact-forest target inventory and diagnostics.

## Examples

```bash
renderflow inspect
renderflow inspect --output-format dot
renderflow inspect --target pdf --export dag.dot
renderflow inspect --config "examples/page-composition/raster-composition/renderflow.yaml"
renderflow inspect --input unknown.bin
renderflow inspect --input comic.cbz --extract --recursive --export intake.json
```
