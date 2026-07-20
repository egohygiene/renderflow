# `renderflow watch`

Watch files and rebuild automatically.

## Syntax

```bash
renderflow watch [--config FILE] [--debounce MS]
```

## Flags

| Flag | Description |
|---|---|
| `--config FILE` | config path, default `renderflow.yaml` |
| `--debounce MS` | debounce window in milliseconds, default `500` |

## Behavior

`watch`:

1. runs an initial build,
2. watches the config file,
3. watches the configured input file when it exists,
4. watches `templates/` recursively when the directory exists,
5. rebuilds using resilient transform handling.

## Error handling

Watch mode logs build failures but keeps running. This is backed by `build::run_resilient`, which sets transform failure mode to continue-on-error.

## Example

```bash
renderflow watch
renderflow watch --config docs.yaml --debounce 300
```
