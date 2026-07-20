# Developing a Plugin

## 1. Implement `PluginExecutor`

```rust
use renderflow::transforms::plugin::PluginExecutor;

struct ReversePlugin;

impl PluginExecutor for ReversePlugin {
    fn name(&self) -> &str {
        "reverse"
    }

    fn execute(&self, input: String) -> anyhow::Result<String> {
        Ok(input.chars().rev().collect())
    }
}
```

## 2. Register metadata

```rust
use std::sync::Arc;
use renderflow::transforms::plugin::{PluginMetadata, PluginRegistry};

let mut registry = PluginRegistry::new();
registry.register_with_metadata(
    Arc::new(ReversePlugin),
    PluginMetadata::new("reverse", "1.0.0")
        .with_description("Reverse text")
        .with_author("Example Author"),
)?;
```

## 3. Use it from a transform definition

The non-graph transform loader can resolve YAML entries with a `plugin:` field when you pass a populated `PluginRegistry`.

```yaml
transforms:
  - name: reverse-md
    plugin: reverse
    from: markdown
    to: markdown
    cost: 1.0
    quality: 1.0
```

## Context and capabilities

`PluginContext` can provide:

- working directory
- temp directory
- dry-run flag
- namespaced plugin config values

Capability flags help describe plugin behavior for diagnostics and future optimization-aware integration.
