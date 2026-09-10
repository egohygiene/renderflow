# Resumable provider contract

Renderflow exposes a versioned Rust provider seam for orchestration systems such as Flow. Renderflow owns transform execution and artifact validation; the caller remains responsible for cross-provider orchestration and recovery policy.

`RenderflowProvider` exposes structured operations for capability inspection, planning, execution, checkpoint assessment, and resume. The contract uses `renderflow.provider/v1`; progress callbacks use `renderflow.progress/v1`. Consumers must use these serialized models rather than human stdout or stderr.

```rust
use renderflow::{EngineBuilder, ExecutionRequest, RenderflowProvider};

let engine = EngineBuilder::new().build()?;
let capabilities = engine.inspect_capabilities();
let assessment = engine.assess_saved_run(
    ExecutionRequest::from_path("renderflow.yaml"),
)?;
let result = engine.resume_saved_run(
    ExecutionRequest::from_path("renderflow.yaml"),
)?;
# Ok::<(), renderflow::RenderflowError>(())
```

## Checkpoint lifecycle

Every checkpoint-eligible deterministic DAG node is recorded immediately in `.renderflow/checkpoints.json` after its content-addressed output has been atomically written and validated. Non-deterministic and explicitly non-cacheable plugin work is never silently reused. The checkpoint includes exact input artifact identities and digests, transform and implementation version, provider identity, configuration digest, selected toolchain fingerprint, outputs, fidelity, and validator evidence.

The checkpoint file is atomically replaced and carries a SHA-256 checksum over its payload. Invalid JSON, unsupported schema versions, and checksum mismatches are terminal incompatibilities during assessment; they are never interpreted as reusable work.

Resume evaluates each node independently. A checkpoint is reused only when its inputs, transform configuration, implementation/tool version, output records, payload sizes and payload digests remain compatible. Missing or corrupt payloads force recomputation. Content-addressed source lineage ensures a source change invalidates downstream nodes while independent branches keep compatible keys.

Recovery decisions distinguish `retry`, `reuse`, `recompute`, `skip`, and `terminal_incompatibility`. `Engine::invalidate_checkpoints` explicitly invalidates one step or the complete checkpoint set.

## Cancellation and events

The SDK carries cancellation into the DAG executor. Generic synchronous transforms stop at bounded node-wave boundaries; process-backed plugin operations retain their own process cancellation boundary. Remaining nodes are recorded as `cancelled`, and the run manifest intentionally returns `cancelled` rather than ambiguous partial success.

Progress events include their schema version and, when available, run ID, step ID, output artifact IDs, state, and structured diagnostics. The completed run manifest remains the authoritative event/evidence record.

## Schemas and Flow artifacts

- `schemas/renderflow-provider-v1.schema.json` describes provider results and events.
- `schemas/renderflow-checkpoints-v1.schema.json` describes durable checkpoint state.
- `schemas/renderflow-run-v1.schema.json` describes authoritative run evidence.
- `RunManifest::flow_artifacts_v1()` projects outputs into `flow.artifact/v1` without importing Flow source.
