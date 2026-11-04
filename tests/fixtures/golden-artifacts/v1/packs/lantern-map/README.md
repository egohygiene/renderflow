# The Lantern Map — original comic fixture

Pip, an orange circle courier, finds a blue dot. Vale, a blue square mapmaker,
recognizes a route. Together they carry a lantern to the pier and share the map.
The [original story](story.json) and deterministic geometric art are independent
of private consumer publications. See [LICENSE](LICENSE) for CC0 and the separate
retained font license boundary.

This is a source recipe, **not a rendered or visually approved comic**. The
existing golden manifest pins its story, recipe, toolchain and font assets.
`pack.lantern-map` selects two variants from the same recipe:

- `minimal`: two interior pages, front cover and back cover; authored public-API
  cases for per-page SVG/HTML/transcript/PNG/PDF candidates.
- `publication-44`: 44 numbered interior pages plus a separate two-cover group;
  source/planning-only case in the maximal tier. Two story beats repeat as a
  finite layout stress recipe, with no fabricated unique narrative or outputs.

Each page has two panels, separate caption/dialogue content, stable reading IDs,
artwork descriptions and an explicit human-review requirement. The 72 DPI
100 × 140 mm output is a small preview fixture, not a print-quality promise.

Source materialization is a function inside the existing
`golden_conformance` harness. It never calls an image/model service or downloads
fonts. Use the [pack guide](../../../../../../docs/document-fixture-packs.md)
for the deferred execution example, retention option, schema/contributor
convention, positive/negative assertions and provider limits. This sprint ran
no generator, compiler, test, renderer, validator or CI.

Combined interior/cover PDF and comic EPUB mappings remain explicitly
unavailable. A PDF checker or per-page candidate is not a provider exporter,
printer acceptance or publication approval. No existing golden baseline is
silently created or updated.
