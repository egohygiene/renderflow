# `renderflow inspect`

Inspect either an arbitrary input artifact or the transformation DAG for a config.

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
| `--all` | show all reachable targets |
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

- `inspect` requires `transforms:` in the config.
- when no `--target` is supplied, reachable formats are discovered automatically.
- `--all` is accepted for consistency but the implementation already shows all reachable formats when no explicit target is given.

## Examples

```bash
renderflow inspect
renderflow inspect --output-format dot
renderflow inspect --target pdf --export dag.dot
renderflow inspect --input unknown.bin
renderflow inspect --input comic.cbz --extract --recursive --export intake.json
```
