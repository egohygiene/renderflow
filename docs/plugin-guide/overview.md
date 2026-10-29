# Plugin Guide Overview

Renderflow has an artifact-native [Transform v2 SDK](v2-sdk.md). A host can register
Rust implementations directly, and the CLI can load explicitly declared
[local command plugins](command-plugins.md) through that same SDK.

The command adapter is source-authored and unqualified under #466. It uses direct
argv and scoped artifact files; it does not load dynamic libraries or discover
packages automatically. Start with the [names extractor example](../../examples/plugins/names/README.md).

## Legacy text compatibility

- implement `PluginExecutor`
- register it in `PluginRegistry`
- optionally attach `PluginMetadata`
- optionally wrap it in a `PluginTransform`

## When to use a plugin

Use a plugin when:

- you need direct Rust logic instead of an external command,
- you want tighter integration with dry-run or diagnostics,
- you are embedding Renderflow in another application.

Legacy YAML `program:` transforms retain their UTF-8 interface. Use a configured
command plugin for artifact-native file I/O and explicit version/capability/loss
metadata. The [legacy author guide](developing.md) documents the older API.
