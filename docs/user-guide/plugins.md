# Plugins

Renderflow exposes a runtime plugin API through `PluginExecutor`, `PluginRegistry`, and `PluginMetadata`.

## What plugins are for

Plugins let an embedding application register custom transforms without modifying Renderflow core.

## CLI behavior

The standalone CLI creates an empty `PluginRegistry` in `main.rs`. That means:

- `renderflow plugin list` may show no plugins,
- `renderflow plugin info ...` only works if a host application has already registered plugins,
- the CLI commands are most useful when Renderflow is embedded as a library.

## Metadata fields

Plugins can declare:

- `id`
- `version`
- `author`
- `description`
- `supported_transforms`
- `license`
- `required_tools`
- capability flags: `dry_run`, `caching`, `diagnostics`, `optimization`

## Diagnostics

`renderflow plugin doctor` validates metadata and checks required tools on `PATH`. It prints issues but still returns success so the output can be used as advisory diagnostics.

## Validation rules

Current validation is intentionally small:

- plugin `id` must not be blank
- plugin `version` must not be blank

See the [plugin development guide](../plugin-guide/developing.md) for implementation details.
