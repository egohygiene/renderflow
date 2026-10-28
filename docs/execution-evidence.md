# Execution evidence

Canonical executions that reach run finalization write a versioned `renderflow-run.json` manifest in the configured output directory. The manifest is authoritative for what the executor actually produced, reused, skipped, validated, or failed to produce; it is not reconstructed from requested targets. An early planning/configuration or persistence failure can prevent a manifest from being written and must not be interpreted as a successful empty run.

Dry runs return the same evidence type with `state: planned`, but remain side-effect free and do not persist a manifest.

Validated node completion is also written immediately to the corruption-detectable checkpoint store. See [Resumable provider contract](provider-contract.md) for compatibility assessment, resume, invalidation, cancellation, and orchestration-facing APIs.

## Outcome states

The top-level `state` is one of:

- `complete`: every selected target was produced and materialized;
- `partial`: at least one selected output was materialized and another step or output failed;
- `failed`: no selected output was materialized successfully;
- `cancelled`: execution was cancelled after planning and before transforms started;
- `planned`: dry-run evidence only.

The CLI exits unsuccessfully for `partial`, `failed`, and `cancelled` outcomes after reporting the manifest path. SDK callers receive the structured `ExecutionResult` and should inspect `run_manifest.state`.

## Artifact and step evidence

The artifact manifest contains source, retained intermediate, and terminal artifact records. Each record includes a stable artifact ID, logical role, lifecycle, safe store or bundle locator, canonical format and media type, SHA-256 digest, size, producer identity, source lineage, cache status, validation status, fidelity declaration, and per-validator evidence. Validator evidence identifies the implementation version and provider and carries structured diagnostics.

Terminal artifacts are validated before materialization. A required invalid artifact is never published; unavailable validation also blocks publication unless the spec explicitly allows it. When validation is disabled, the terminal state is recorded as `skipped` rather than inferred as valid.

When a publication-hygiene policy is selected, the generated candidate passes through a `publication.hygiene` step before terminal validation and materialization. Artifact evidence embeds the policy/provider identity, source and sanitized artifact IDs, changed metadata field classes, safe findings, and a `passed`, `review_required`, or `blocked` decision. Blocked candidates retain an `artifact-store:` locator but never receive a `bundle:` locator. See [Publication hygiene](publication-hygiene.md).

Each executed DAG edge produces step evidence with transform/capability/provider identity, input and output artifact IDs, a configuration digest, timestamps, duration, cache disposition, validation and fidelity states, and structured diagnostics. Cache hits use `state: reused`; transforms blocked by a failed dependency use `state: skipped` with a reason.

The machine-readable contract is [`schemas/renderflow-run-v1.schema.json`](https://github.com/egohygiene/renderflow/blob/main/schemas/renderflow-run-v1.schema.json).

## Flow compatibility

`RunManifest::flow_artifacts_v1()` explicitly projects native artifact evidence into Flow's provisional `flow.artifact/v1` interchange shape. Renderflow keeps its richer native evidence independent from Flow and pins a compatibility fixture to the contract present in `egohygiene/flow` commit `a7d28ee812f9d6ccd93b6786be924e24c455accd`.

Native IDs such as `artifact:sha256:<digest>` are mapped deterministically to Flow-compatible IDs such as `artifact:sha256-<digest>`. Producer fields are projected as `owner`, `capability_id`, and `provider_version`.

The projection includes every native inventory record, including sources and
intermediates. It omits locators, full provider identity, validation, loss/review
metadata and toolchain evidence. Use the native
`artifact_forest.execution.targets` publication records to select actual
deliverables and retain the native manifest alongside projected records. See
the [artifact forest handoff](architecture/flow-artifact-forest.md).

## Forest publication outcomes

The planned `renderflow/artifact-forest-v1` branches remain planning evidence.
Canonical run manifests add `artifact_forest.execution` using
`renderflow/forest-execution-v1`, with one target outcome: `not_selected`,
`planned`, `published`, or `not_published`. Published rows reference the exact
artifact record by index, ID and locator; this preserves distinct destinations
even if their bytes and artifact IDs agree. Run state and diagnostics remain
authoritative for why a selected target was not published.

`produced_artifacts` contains only unique IDs of published terminal derivatives.
It excludes source records, retained intermediates and rejected/unpublished
outputs. An explicit unchanged source export is separately marked
`source_identity: true`. A published derivative can still require human review;
publication is not creative approval or runtime qualification.

### Select publications through the Rust SDK

`RunManifest::published_targets()` joins the existing forest execution rows to
their exact native artifact records. Each borrowed `PublishedTarget` exposes
`branch`, `execution`, `artifact`, and the optional `producing_step`. The helper
keeps planning order and separate destinations even when content IDs are equal.
It preserves all native metadata and leaves the inventory-wide
`flow_artifacts_v1()` projection unchanged.

```rust
use renderflow::{PublishedTargetsError, RunManifest};

fn list_publications(run: &RunManifest) -> Result<(), PublishedTargetsError> {
    println!("Run state: {:?}", run.state);
    for item in run.published_targets()? {
        println!("{} -> {} ({}, source identity: {})",
            item.branch.target_id, item.artifact.locator, item.artifact.format,
            item.execution.source_identity);
        // Retain item.artifact.metadata, fidelity, validation and hygiene.
        // Resolve and verify the payload separately before consuming it.
    }
    Ok(())
}
```

Legacy records without forest/execution evidence return a typed error rather
than an empty success. A dry run with explicit planned outcomes returns an
empty list. Unsuccessful runs can retain publications; the helper never turns
`partial`, `failed` or `cancelled` into `complete`. An unpublished candidate's
inventory index is not a deliverable.

The join checks contract versions, run/inventory identity, exact target
membership and order, unique record/destination references, terminal format
and locator agreement, contradictory publication markers, source identity,
producing-step references and the derivative-ID summary. Source identity uses
observed source IDs and digests: a hygiene-derived record remains a derivative
even after a zero-edge source plan and even when its bytes are unchanged.

This helper performs no I/O and introduces no serialized contract. It does not
validate the complete manifest, recompute plan/artifact identities, check output
path membership or confinement, hash actual payloads, or interpret family-specific
review/qualification evidence. For copied or untrusted receipts, retain the
native evidence and perform those checks at the consuming boundary described
in the [Flow handoff](architecture/flow-artifact-forest.md). A returned locator
is a declaration, not a safe path to open automatically.

## Sensitive data boundary

Run manifests contain digests of the resolved plan and source spec, not serialized configuration or environment variables. Step configuration is represented only by a SHA-256 digest. Artifact locators are relative `artifact-store:` or `bundle:` locators. Provider diagnostics are retained for operability, so provider implementations must not place credentials or secret values in error messages. Hygiene secret findings identify only a safe credential class; the matched value is never written to evidence.
