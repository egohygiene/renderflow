# Plugins

Renderflow exposes the artifact-native `PluginTransformV2` API. The older
`PluginExecutor`, `PluginRegistry` and `PluginMetadata` API remains for compatibility.

## What plugins are for

Plugins provide custom transforms without modifying Renderflow core. An embedding
application can register Rust implementations, while an explicit local command
declaration can connect a program to the CLI's artifact forest.

## CLI behavior

For configured command plugins, pass the transform registry:

```bash
renderflow plugin list --transforms "examples/plugins/names/transforms.yaml"
renderflow plugin validate --transforms "examples/plugins/names/transforms.yaml"
```

The ordinary build configuration references that same registry with `transforms`.
See the [command plugin guide](../plugin-guide/command-plugins.md) for the complete
contract, example and qualification limits. This source checkpoint remains unrun.
Commands without `--transforms` retain the empty standalone legacy registry.

## Legacy metadata fields

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

Configured-plugin commands validate declarations without executing plugin code.
`doctor --transforms FILE` reports this metadata-only boundary; use the explicit
tools commands for provider probes. The legacy host-registry `doctor` checks tools
on PATH and remains advisory.

## Legacy validation rules

Current validation is intentionally small:

- plugin `id` must not be blank
- plugin `version` must not be blank

See the [plugin development guide](../plugin-guide/developing.md) for implementation details.
