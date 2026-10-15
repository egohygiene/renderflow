# `renderflow build`

Build documents from a Renderflow config.

## Syntax

```bash
renderflow build [--config FILE] [--dry-run] [--resume] [--optimization MODE] [--target FORMAT | --profile PROFILE | --all] [--exclude SELECTOR]
```

## Flags

| Flag | Description |
|---|---|
| `--config FILE` | config path, default `renderflow.yaml` |
| `--dry-run` | resolve and print the plan without publishing outputs; graph planning may perform bounded provider probes |
| `--resume` | graph mode: reuse only compatible, validated node checkpoints |
| `--optimization MODE` | graph mode: override config optimization mode |
| `--target FORMAT` | replace configured targets with one output format |
| `--profile PROFILE` | graph mode: build a named versioned profile; `everything` and `magazine` are bundled |
| `--all` | graph mode: build all policy-allowed reachable targets |
| `--exclude SELECTOR` | graph mode: exclude a target selector; repeat for multiple exclusions |

## Shared planning and execution

`build`, configuration `inspect`, `graph` and the SDK use the same build
facade. The source kind selects the engine; adding `--target` does not select
a different execution lifecycle. Target overrides replace configuration
selection for that invocation.

Ordinary artifact and collection builds use the existing capability registry,
graph planner and executor. They retain configured policies, provider preflight,
common run evidence and compatible checkpoint resume. A clean-host dry run
keeps unavailable branches visible without claiming their providers are
installed; actual execution still requires provider admission.

## Composition builds

For an authored localized page, declare one local composition package and
the required output formats:

```yaml
schema: renderflow/v2
sources:
  - id: page
    kind: composition
    path: page.composition.json
targets:
  exact:
    - format: svg
    - format: html
    - format: transcript_json
output:
  bundle_root: build-output
```

The planner captures the package once. Build execution uses that frozen native
plan and the existing composition renderer, including locale review gates,
shared preparation and optional verified cache reuse. All declared locales
are selected unless `execution.composition.locales` narrows the selection.

Paths resolve relative to the config. The output directory must be new, with
an existing parent. Published files use `outputs/<locale>/page.<extension>`;
transcripts use `page.transcript.json` or `page.transcript.txt`. The bundle
retains native receipts plus `build-plan.json` and `renderflow-run.json` for
configuration-based consumers. Partial or failed execution returns nonzero
and retains available evidence.

Composition mode accepts `--target` but rejects profiles, `--all`, exclusions,
optimization overrides and `--resume`. It also rejects unsupported generic
policies and custom output naming. Use `execution.composition.cache_directory`
outside the output bundle for native cache reuse and a new `output.bundle_root`
for another run. Omit the cache setting to use the bundle's own artifact store.
See the [composition guide](../user-guide/page-composition.md) for locale,
raster/PDF settings, exact filenames and current provider limits.

## Examples

```bash
renderflow build
renderflow build --config report.yaml
renderflow build --dry-run
renderflow build --resume
renderflow build --optimization quality
renderflow build --target pdf
renderflow build --config examples/magazine/renderflow.yaml --profile magazine --dry-run
renderflow build --all --optimization speed
renderflow build --config "examples/page-composition/raster-composition/renderflow.yaml" --dry-run
renderflow build --config "examples/page-composition/raster-composition/renderflow.yaml"
```

Composition examples and execution behavior remain authored but unqualified;
tests, builds, rendering and CI are deferred during the current sprint.
