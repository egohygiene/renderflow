# Adapter Ecosystem

Renderflow adapter packs bind provider-neutral capabilities to replaceable tools. The versioned
catalog is stored in `crates/renderflow-core/data/adapter-packs.yaml`; the tool registry remains
the authority for executable discovery, version compatibility, platform support, and live
availability.

An adapter contract records its capability families, accepted and produced media types,
determinism, locality, fidelity, bounded execution policy, configuration shape, validation,
provenance contribution, maturity, selection priority, and upstream rationale. Provider names do
not become formats or domain concepts.

## Inspecting the Catalog

```bash
renderflow tools ecosystem
renderflow tools ecosystem --format json
renderflow tools ecosystem --capability document.convert
renderflow tools ecosystem --capability image.convert --preferred adapter.images.imagemagick
renderflow tools ecosystem --available-only
```

Structured output uses `renderflow.adapter-catalog/v1` and is validated by
`schemas/renderflow-adapter-catalog-v1.schema.json`. It includes live availability/version data,
the capability-to-provider projection, and the complete adopt/adapt/reject/defer evaluation
matrix.

## Provider Selection

The public catalog API ranks candidates deterministically:

1. explicitly preferred adapter IDs in caller order;
2. integrated before experimental adapters;
3. lower declared `selection_priority`;
4. stable adapter ID as the final tie-breaker.

Unavailable candidates remain in `AdapterSelection.decisions`; provider fallback is never hidden
after planning. Experimental catalog entries are discoverable but do not add execution-graph
edges by themselves. A capability becomes executable only when an artifact-native adapter
registers an edge and executor.

## Current Packs

| Family | Current provider path | Status | Direction |
| --- | --- | --- | --- |
| Documents/office | Pandoc | Integrated | Keep behind the artifact-native strategy adapter |
| PDF typesetting | Tectonic | Integrated | Enforce explicit offline/network policy |
| Images/audio | FFmpeg; ImageMagick fallback | Integrated/experimental | Keep choices typed and delegates provenance-visible |
| Video/subtitles | FFmpeg | Partial | Register only concrete implemented graph edges |
| Archives | ZIP | Experimental | Normalize ordering and timestamps before promotion |
| Image-to-PDF | img2pdf | Experimental | Promote with collection/aggregation fixtures |
| PDF processing | Ghostscript | Experimental | Require explicit licensing and fidelity policy |
| Local image AI | Upscayl NCNN | Experimental | Require model identity, license evidence, and AI opt-in |
| E-books | Calibre evaluation | Deferred to #344 | Integrate as ordinary providers |
| Video transcode | HandBrakeCLI evaluation | Deferred to #345 | Preserve the Aniflow ownership boundary |
| Searchable PDF OCR | OCRmyPDF evaluation | Adapt | Add searchable-PDF validation after the Tesseract base pack |
| Structured data | jq | Experimental | Constrain filters and validate declared output schemas |
| OCR | Tesseract | Experimental | Discover language packs and preserve confidence evidence |

The catalog also documents rejected candidates and why. Rejection prevents accidental dependency
growth while leaving the decision inspectable and revisable.

## Adding an Adapter

1. Add or reuse a stable `tool.*` entry in `tool-registry.yaml`.
2. Add an `adapter.*` contract with complete execution, validation, and provenance declarations.
3. Execute commands only through `renderflow.process/v1` using direct argv.
4. Register capability graph edges only for implemented artifact-native transforms.
5. Add a redistribution-safe fixture and validator before promoting to `integrated`.
6. Confirm `renderflow tools ecosystem --format json` and the conformance matrix describe the
   capability honestly.

The base engine does not require the maximal tool suite. Missing optional tools are represented as
availability decisions and pruned from maximal artifact forests without pretending outputs were
produced.
