# Artifact kernel

Renderflow's graph is format-oriented, but the original execution substrate moved every edge through UTF-8 `String` values. That was sufficient for early document transforms and unsafe as a universal media abstraction. The artifact kernel makes payloads file-backed and binary-safe while preserving the existing text-transform API during migration.

## Model

An `Artifact` records:

- a stable artifact-record ID;
- canonical format and media type;
- SHA-256 payload digest and byte size;
- a store-relative payload handle;
- structured metadata;
- ordered source/parent artifact IDs;
- a lifecycle/storage class (`source`, `intermediate`, `terminal`, `cached`, or `ephemeral`).

Payload identity and artifact-record identity are intentionally distinct. Payloads live at deterministic content-addressed paths derived only from their SHA-256 bytes. Artifact-record IDs also commit to canonical format and ordered source lineage. This allows two records to reuse the same stored bytes without collapsing distinct derivations.

`ArtifactCollection` is an ordered first-class value. Aggregation transforms receive payload paths in collection order, which is required for operations such as pages → book, tracks → album, or frames → media package.

## Store lifecycle

`ArtifactStore` streams inputs into a temporary file while hashing them. A complete payload is atomically promoted to:

```text
objects/sha256/<first-two-hex>/<full-sha256>
```

Canonical source files are imported rather than modified in place. Intermediate artifacts stay in the work store and do not need to appear beside final user outputs. Final materialization copies to a temporary file in the destination directory, syncs it, and only then atomically replaces the destination.

A transform that fails after creating a temporary output cannot publish that partial file as a complete content-addressed artifact because import happens only after transform success.

## Execution APIs

`DagExecutor` now has an artifact-native substrate:

- `execute_artifact(...)` for one source artifact;
- `execute_artifacts(...)` for an ordered source collection;
- `register_artifact(...)` for binary-safe transforms.

The existing `execute(..., String)` and `register_single(..., Transform)` APIs remain compatibility surfaces. Existing transforms are wrapped by `TextTransformAdapter`; the adapter reads UTF-8 at the compatibility boundary, runs the legacy transform, then writes the result back into the artifact store with source lineage.

Binary input sent to a legacy text transform fails explicitly instead of being lossily decoded. New binary transforms should implement `ArtifactTransform` until the versioned Transform v2/plugin contract is stabilized by issue #357.

## Cache identity

The artifact DAG cache no longer needs the complete in-memory input string to compute a node key. Its key commits to:

- artifact-record ID;
- SHA-256 payload digest;
- byte size;
- source and target formats;
- transform cache identity.

`register_single_with_identity(...)` and `ArtifactTransform::cache_identity()` provide a seam for configuration-aware identity. Full provider/tool/environment fingerprinting belongs to issues #357 and #359 and is not guessed by this kernel.

Legacy string-cache files are treated as cache misses and replaced by the artifact cache schema after a successful run.

## CLI graph builds

Graph builds import the configured source into a hidden Renderflow state store before execution. The source is read as bytes, not UTF-8 text. Produced artifacts remain in the store until selected outputs are atomically materialized into the configured output directory.

This removes the source and final-write UTF-8 assumptions from graph execution. A graph still needs an artifact-native transform for any binary edge; the kernel does not claim that every currently declared format has a production transform provider.

## Flow boundary

The kernel deliberately does not depend on `egohygiene/flow`. Its types contain the information needed to project a Renderflow artifact into Flow's artifact interchange contract later: stable ID, media type, SHA-256 digest, byte size, producer/metadata extension points, and ordered sources. Provenance-complete execution results and the concrete Flow projection are tracked separately by issues #355 and #358.

## Migration sequence

1. **#352 — artifact kernel:** binary-safe payloads, content-addressed storage, collections, atomic materialization, legacy text adapter.
2. **#357 — Transform v2/plugin SDK:** stabilize typed artifact I/O and execution context.
3. **#356/#359 — process/tool contracts:** central process policy and reproducible tool capability fingerprints.
4. **#355/#358 — evidence/resume:** provenance-complete results, checkpoints, and Flow provider seam.

This sequence keeps existing document transforms working while moving the canonical execution substrate away from `String`.
