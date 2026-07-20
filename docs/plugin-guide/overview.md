# Plugin Guide Overview

Renderflow plugins are Rust-side runtime extensions, not shell plugins.

## Architecture summary

- implement `PluginExecutor`
- register it in `PluginRegistry`
- optionally attach `PluginMetadata`
- optionally wrap it in a `PluginTransform`

## When to use a plugin

Use a plugin when:

- you need direct Rust logic instead of an external command,
- you want tighter integration with dry-run or diagnostics,
- you are embedding Renderflow in another application.

Use a YAML command transform when an external binary is enough.
