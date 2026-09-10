# Transform v2 Plugin SDK

`renderflow-plugin-sdk` exposes the artifact-native contract for new plugins.
The serialized contract identifier is `renderflow.plugin/v2alpha1`; the
`v2alpha1` label makes the current pre-1.0 stability level explicit.

## Contract shape

A v2 plugin implements `PluginTransformV2` and returns a
`PluginTransformDescriptor`. The descriptor is validated before registration
or execution and declares:

- semantic transform version and capability ID
- single or ordered-collection input
- accepted input and output formats
- deterministic, environment-dependent, or nondeterministic behavior
- content-addressed caching or disabled caching
- lossless, partial, lossy, or path-dependent fidelity
- required provider IDs

Content-addressed caching is rejected for anything except a deterministic
transform. The canonical DAG also bypasses cache reads and writes when the
plugin disables caching.

## Minimal artifact transform

```rust
use std::collections::BTreeMap;

use renderflow_plugin_sdk::{
    ArtifactCollection, PluginExecutionContext, PluginInputKind,
    PluginTransformDescriptor, PluginTransformRequest, PluginTransformResult,
    PluginTransformV2,
};

struct CopyBytes;

impl PluginTransformV2 for CopyBytes {
    fn descriptor(&self) -> PluginTransformDescriptor {
        PluginTransformDescriptor::new(
            "example.copy-bytes",
            "1.0.0",
            "transform.binary.copy",
            PluginInputKind::Single,
        )
        .with_formats(["png"], ["png"])
    }

    fn execute(
        &self,
        request: PluginTransformRequest,
        context: &PluginExecutionContext,
    ) -> anyhow::Result<PluginTransformResult> {
        let input = request.inputs.into_one()?;
        let bytes = context.artifacts.read_input_bytes(&input)?;
        let output = context
            .artifacts
            .commit_bytes(&bytes, None, BTreeMap::new())?;
        Ok(PluginTransformResult {
            outputs: ArtifactCollection::one(output),
            ..PluginTransformResult::default()
        })
    }
}
```

The same API handles text without making UTF-8 the universal substrate. A text
plugin explicitly decodes bytes after reading its typed artifact.

## Ordered collections

Declare `PluginInputKind::OrderedCollection`. `request.inputs.iter()` preserves
the planner-defined order. Commit the result through `context.artifacts`; the
host attaches all ordered source IDs automatically.

## Typed configuration

Return a versioned `PluginConfigSchema` and override `validate_config`. The host
calls validation before execution or process side effects. Runtime values are
JSON rather than an unstructured string map.

```rust
fn config_schema(&self) -> PluginConfigSchema {
    PluginConfigSchema {
        id: "example.copy/config/v1".to_string(),
        schema: serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": { "quality": { "type": "integer", "minimum": 1 } },
            "required": ["quality"],
            "additionalProperties": false
        }),
    }
}
```

## Runtime services

`PluginExecutionContext` exposes only scoped capabilities:

- immutable reads of authorized inputs and commits of new output artifacts
- a cancellation token
- progress reporting
- the bounded, policy-aware process service from Renderflow core
- execution and step identity

Use `context.process.execute_checked(ProcessRequest::direct(...))` for external
tools. The host cancellation token is attached automatically. Credential-like
environment values remain filtered by default.

After execution, Renderflow verifies every input digest, validates output format
and ordered lineage, and seals plugin diagnostics, metrics, version, capability,
and redacted provenance into namespaced artifact metadata.

## Registration and replacement

`PluginRegistryV2::register` rejects duplicate transform IDs. A host must use
`register_with_policy(..., PluginReplacementPolicy::ReplaceSameCapability)` to
replace an implementation, and replacement across capability IDs is rejected.

Likewise, `DagExecutor::register_plugin_v2` rejects an occupied format edge.
`replace_plugin_v2` is the explicit edge replacement operation.

## Observers

`PluginLifecycleObserver` receives immutable, versioned event snapshots.
Observers do not receive artifact-store, plan, validation, approval, or release
mutation handles. Any extension that changes content must be a declared
`PluginTransformV2` and produce a new immutable artifact.

## v1 migration

Existing `PluginExecutor::execute(String) -> Result<String>` implementations
remain supported. Wrap one with `LegacyTextPluginAdapter`, register that adapter
as v2, and migrate internals incrementally. Binary input fails at the explicit
UTF-8 compatibility boundary instead of being decoded lossily.

## Compatibility policy

- `renderflow.plugin/v2alpha1` source APIs and serialized metadata may receive
  breaking changes before Renderflow 1.0; changes require a contract identifier
  bump and migration notes.
- Stable descriptor fields are serialized independently of Rust feature flags.
- Optional additions must have backward-compatible defaults.
- Plugins share Renderflow's workspace MSRV, currently Rust 1.94.
- The legacy v1 adapter remains available through the v2 migration window.
- Compile/runtime fixtures belong in the conformance suite tracked by #366.

## Runnable examples

The plugin SDK crate includes focused examples for every supported authoring
pattern:

- `text_transform`
- `binary_transform`
- `collection_transform`
- `external_tool_adapter`

Compile an example with:

```bash
cargo check --package renderflow-plugin-sdk --example binary_transform
```
