# `renderflow build`

Build documents from a Renderflow config.

## Syntax

```bash
renderflow build [--config FILE] [--dry-run] [--resume] [--optimization MODE] [--target FORMAT | --profile PROFILE | --all]
```

## Flags

| Flag | Description |
|---|---|
| `--config FILE` | config path, default `renderflow.yaml` |
| `--dry-run` | log intended actions without writing files or running commands |
| `--resume` | reuse only compatible, validated node checkpoints |
| `--optimization MODE` | override config optimization mode |
| `--target FORMAT` | graph-build one reachable target through registered capabilities |
| `--profile PROFILE` | build a named versioned profile; `everything` and `magazine` are bundled |
| `--all` | graph-build all policy-allowed reachable targets |

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

- registers built-in capabilities and merges optional `transforms:` from config,
- constructs a `TransformGraph`,
- resolves targets by optimization mode,
- executes the merged DAG,
- writes every produced non-source format to `output_dir`.

!!! note
    Graph build can work when `outputs:` and `transforms:` are omitted because
    it uses `load_config_for_graph` and the built-in capability registry. A
    clean-host dry run preserves unavailable branches for inspection without
    claiming their providers are installed; real execution remains blocked by
    provider preflight.

## Examples

```bash
renderflow build
renderflow build --config report.yaml
renderflow build --dry-run
renderflow build --resume
renderflow build --optimization quality
renderflow build --target pdf
renderflow build --config examples/magazine/renderflow.yaml --profile magazine --dry-run
renderflow build --all --optimization speed
```
