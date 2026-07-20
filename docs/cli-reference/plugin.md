# `renderflow plugin`

Inspect plugin registry state.

## Subcommands

### `plugin list`

Lists every registered plugin and prints metadata when available.

### `plugin info <name>`

Prints detailed metadata for one plugin.

### `plugin validate`

Validates all plugin metadata and returns an error if issues are found.

### `plugin doctor`

Checks plugin metadata and required tools on `PATH`. It prints issues but returns success so the output can be used as advisory diagnostics.

## Important limitation

The top-level Renderflow CLI initializes an empty registry. Unless an embedding application registers plugins before dispatching CLI logic, the plugin commands will have nothing to report.

## Examples

```bash
renderflow plugin list
renderflow plugin info my-plugin
renderflow plugin validate
renderflow plugin doctor
```
