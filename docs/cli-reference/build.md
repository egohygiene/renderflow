# `renderflow build`

Build documents from a Renderflow config.

## Syntax

```bash
renderflow build [--config FILE] [--dry-run] [--optimization MODE] [--target FORMAT | --all]
```

## Flags

| Flag | Description |
|---|---|
| `--config FILE` | config path, default `renderflow.yaml` |
| `--dry-run` | log intended actions without writing files or running commands |
| `--optimization MODE` | override config optimization mode |
| `--target FORMAT` | graph-build one reachable target; requires `transforms` |
| `--all` | graph-build all reachable targets; requires `transforms` |

## Standard build behavior

Without `--target` or `--all`, Renderflow uses the standard build path from `src/commands/build.rs`:

- load and validate config,
- normalize asset paths,
- run built-in transforms,
- optionally run YAML-defined transforms,
- render each output concurrently.

## Graph build behavior

With `--target` or `--all`, `main.rs` dispatches to `src/commands/graph_build.rs`.

That mode:

- loads `transforms:` from config,
- constructs a `TransformGraph`,
- resolves targets by optimization mode,
- executes the merged DAG,
- writes every produced non-source format to `output_dir`.

!!! note
    Graph build can work even when `outputs:` is omitted because it uses `load_config_for_graph` instead of full standard-build validation.

## Examples

```bash
renderflow build
renderflow build --config report.yaml
renderflow build --dry-run
renderflow build --optimization quality
renderflow build --target pdf
renderflow build --all --optimization speed
```
