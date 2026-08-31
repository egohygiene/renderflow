# tools and capabilities

Renderflow exposes the runtime provider registry through the same structured model used by planning and diagnostics.

## List providers

```bash
renderflow tools list
renderflow tools list --format json
renderflow tools list --format yaml
```

Use `--transforms <FILE>` to include providers inferred from a transform YAML file, including arbitrary command providers that are not part of the built-in catalog.

## Inspect one provider

```bash
renderflow tools inspect tool.ffmpeg
renderflow tools inspect tool.pandoc --format json
```

Inspection includes discovery strategy, live availability state, selected executable, installed version evidence, determinism/locality/fidelity metadata, capability IDs, fallbacks, and diagnostics.

## List capabilities

```bash
renderflow capabilities
renderflow capabilities --format json
```

Capability IDs and provider IDs are stable machine-readable identifiers. Human-readable CLI output is rendered from the same data returned by JSON/YAML modes.

## Toolchain fingerprints

Graph planning fingerprints only providers selected by the final DAG. The fingerprint includes the selected provider IDs, compatible installed versions, relevant executable identity, provider capability metadata, and the target OS/architecture. It does **not** hash the entire host environment.

Graph execution uses that fingerprint in artifact-cache compatibility and writes the selected toolchain evidence into Renderflow state for reproducibility/provenance consumers.
