# `renderflow inspect`

Inspect the transformation DAG for a config that points at a transform YAML file.

## Syntax

```bash
renderflow inspect [--config FILE] [--output-format tree|dot] [--target FORMAT | --all] [--export FILE]
```

## Flags

| Flag | Description |
|---|---|
| `--config FILE` | config path, default `renderflow.yaml` |
| `--output-format FORMAT` | `tree` (default) or `dot` |
| `--target FORMAT` | restrict output to one target |
| `--all` | show all reachable targets |
| `--export FILE` | write output to a file |

## Notes

- `inspect` requires `transforms:` in the config.
- when no `--target` is supplied, reachable formats are discovered automatically.
- `--all` is accepted for consistency but the implementation already shows all reachable formats when no explicit target is given.

## Examples

```bash
renderflow inspect
renderflow inspect --output-format dot
renderflow inspect --target pdf --export dag.dot
```
