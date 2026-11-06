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
  cases for per-page SVG/HTML/transcript/PNG/PDF candidates, explicit RGB preparation
  and ordered native EPUB. Interior/cover-page PDF packets require an explicitly
  selected full-tier img2pdf 0.6.3 provider.
- `publication-44`: 44 numbered interior pages plus a separate two-cover group.
  Maximal tier plans by default; `RENDERFLOW_TEST_COMIC_LARGE=1` additionally
  selects all 46 page runs, RGB preparation and ordered EPUB. PDF packets need
  the separate explicit provider opt-in. Two story beats repeat as finite layout
  stress data, with no fabricated unique narrative or observed outputs.

Each page has two panels, separate caption/dialogue content, stable reading IDs,
artwork descriptions and an explicit human-review requirement. The 127 DPI
100 × 140 mm output is a small preview fixture, not a print-quality promise.

Source materialization is a function inside the existing
`golden_conformance` harness. It never calls an image/model service or downloads
fonts. Use the [pack guide](../../../../../../docs/document-fixture-packs.md)
for the deferred execution example, retention option, schema/contributor
convention, positive/negative assertions and provider limits. This sprint ran
no generator, compiler, test, renderer, validator or CI.

Ordered EPUB and opt-in PDF packet recipes reuse the existing packagers.
The two-cover packet is not a wrap/spine or printer-conformance exporter; that
mapping remains unavailable. RGB preparation explicitly drops color intent and
density metadata without converting the retained samples. A PDF checker or per-page candidate is not a provider exporter,
printer acceptance or publication approval. No existing golden baseline is
silently created or updated.
