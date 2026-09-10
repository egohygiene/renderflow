# Execution evidence

Every non-dry canonical build writes a versioned `renderflow-run.json` manifest in the configured output directory. The manifest is authoritative for what the executor actually produced, reused, skipped, validated, or failed to produce; it is not reconstructed from requested targets.

Dry runs return the same evidence type with `state: planned`, but remain side-effect free and do not persist a manifest.

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

Each executed DAG edge produces step evidence with transform/capability/provider identity, input and output artifact IDs, a configuration digest, timestamps, duration, cache disposition, validation and fidelity states, and structured diagnostics. Cache hits use `state: reused`; transforms blocked by a failed dependency use `state: skipped` with a reason.

The machine-readable contract is [`schemas/renderflow-run-v1.schema.json`](https://github.com/egohygiene/renderflow/blob/main/schemas/renderflow-run-v1.schema.json).

## Flow compatibility

`RunManifest::flow_artifacts_v1()` explicitly projects native artifact evidence into Flow's provisional `flow.artifact/v1` interchange shape. Renderflow keeps its richer native evidence independent from Flow and pins a compatibility fixture to the contract present in `egohygiene/flow` commit `a7d28ee812f9d6ccd93b6786be924e24c455accd`.

Native IDs such as `artifact:sha256:<digest>` are mapped deterministically to Flow-compatible IDs such as `artifact:sha256-<digest>`. Producer fields are projected as `owner`, `capability_id`, and `provider_version`.

## Sensitive data boundary

Run manifests contain digests of the resolved plan and source spec, not serialized configuration or environment variables. Step configuration is represented only by a SHA-256 digest. Artifact locators are relative `artifact-store:` or `bundle:` locators. Provider diagnostics are retained for operability, so provider implementations must not place credentials or secret values in error messages.
