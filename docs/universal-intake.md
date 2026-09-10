# Universal intake and extraction

Renderflow accepts arbitrary local files through one binary-safe intake path. Intake hashes the source into the normal content-addressed artifact store, combines extension, magic-byte, declared media-type, and provider structural signals, and emits `renderflow.intake/v1` JSON. Unknown input is a valid result: Renderflow reports it as `unknown` / `application/octet-stream` and does not imply convertibility.

```bash
renderflow inspect --input manuscript.pdf
renderflow inspect --input book.epub --extract --recursive
renderflow inspect --input mystery.bin --export intake.json
```

The report includes the source artifact ID and digest, resolved profile and confidence, every contributing signal, explicit conflicts, provider-observed metadata with provenance, available operations, extracted child artifacts, safety diagnostics, and budget usage. The contract is published as `schemas/renderflow-intake-v1.schema.json`.

## Provider and SDK contract

`ArtifactIntakeProvider` is the stable extension seam for deterministic structural probes. Built-in native and ZIP providers cover bounded text/JSON, PNG/GIF/PDF structure, ZIP package inspection, and ZIP-family extraction. `Engine::inspect_artifact` exposes the same result to SDK/Flow callers and progress reporters. `IntakeReport::artifact_collection()` returns source and children as ordinary `Artifact` values, so downstream planning and transforms do not need an extraction-only engine.

The canonical config planner runs non-extracting intake before selecting a path and freezes the source artifact ID, digest, size, signals, conflicts, and profile into `ExecutionPlan.source_artifact`. The execution import repeats the content-addressed hash into the durable run store and retains the profile as source artifact metadata.

## Safe recursive discovery

ZIP entries are never materialized by joining archive names to a filesystem directory. They stream directly into the artifact store after policy checks. Absolute/traversal paths and symbolic links are rejected, encrypted entries require explicit SDK policy, and malformed entries remain structured diagnostics rather than panics.

The default limits are depth 3, 1,000 total artifacts, 512 MiB extracted bytes, and a 100:1 per-entry expansion ratio. CLI flags override each limit:

```bash
renderflow inspect --input package.zip --extract --recursive \
  --max-depth 2 --max-artifacts 250 \
  --max-extracted-bytes 134217728 --max-expansion-ratio 50
```

Extraction is provider-driven. A format may be inspectable without an installed extraction provider; that state is reported as unavailable instead of being guessed.
