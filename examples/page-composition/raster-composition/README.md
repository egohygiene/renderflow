# Mixed-asset raster composition fixture

This standalone package authors a small page containing all currently admitted
composition asset kinds: PNG, JPEG and bounded SVG, plus shapes and measured
glyph paths. It is an input fixture for the v5 composition profile, including the opt-in
image-only PDF route. No decoded
asset result, generated output or visual approval is checked in.

The page is 50.8 × 50.8 mm. At 144 DPI, the declared dimensions correspond to
288 × 288 pixels by unit arithmetic; this is an expectation for a later run,
not an observed output size. Artwork stays inset, leaving canvas corners empty
so transparent versus opaque output backgrounds can be checked separately.

## Source package

| Input | Purpose |
| --- | --- |
| `page.composition.json` | Exact asset/content/font pins and ordered mixed-asset layers |
| `renderflow.yaml` | Ordinary v2 build selection: SVG, HTML and transcript JSON for all declared locales |
| `assets/alpha-tile.png` | Original 4 × 4 RGBA tile with zero, partial and full alpha |
| `assets/red-tile.jpg` | Unchanged synthetic red JPEG from the repository's print fixture |
| `assets/two-panels.svg` | Unchanged original geometric SVG from the earlier composition fixture |
| `content/en.json`, `content/de.json` | Short authored Latin LTR captions with synthetic reviewed declarations |
| `fonts/DejaVuSans.ttf` | The same unmodified captured font as the measured-typography fixture |
| `fonts/registry.json`, `fonts/DEJAVU-LICENSE.txt` | Pinned role bindings and retained complete font notice |
| `PROVENANCE.json` | Asset origins, exact authored RGBA pixels, hashes and qualification limits |
| `LICENSE` | Retained repository MIT notice for synthetic assets, copy and layout |

The alpha tile is painted with 0.8 layer opacity and an inset rectangular clip.
A separate ellipse uses 0.7 opacity. JPEG and vector plates, caption boxes and
two text layers complete the page. PNG rendering must use the same measured
glyph geometry as the SVG preview. The canonical content remains editable;
PNG pixels do not retain native text or layers. Select a transcript target when
the consumer needs readable content and reading order alongside the image.

## Start with the ordinary build command

From the repository root:

```bash
renderflow build --config "examples/page-composition/raster-composition/renderflow.yaml" --dry-run
renderflow graph plan --config "examples/page-composition/raster-composition/renderflow.yaml" --format "json"
renderflow build --config "examples/page-composition/raster-composition/renderflow.yaml"
```

The configuration selects English and German through the default
`all_declared` locale mode. Its new bundle appears at `build-output` beside
the config, with files such as `outputs/en/page.svg` and
`outputs/de/page.transcript.json`. Keep the package unchanged; select a new
`output.bundle_root` before another build. Add `png` or `pdf` to
`targets.exact`, or pass `--target "pdf"` to request only PDF for one run.

For explicit locales, raster/PDF settings or cache reuse, see
[`execution.composition` in the guide](../../../docs/user-guide/page-composition.md#choose-locales-and-renderer-settings).
These examples remain unrun.

## Dedicated PNG commands for a later authorized run

From the repository root, explicitly opt in to PNG:

```bash
renderflow composition plan \
  --input "examples/page-composition/raster-composition/page.composition.json" \
  --locales "en,de" \
  --targets "png,svg,transcript_json" \
  --raster-dpi 144 \
  --raster-background "transparent"

renderflow composition render \
  --input "examples/page-composition/raster-composition/page.composition.json" \
  --locales "en,de" \
  --targets "png,svg,transcript_json" \
  --raster-dpi 144 \
  --raster-background "#FFFFFF" \
  --output-dir "composition-raster-output"
```

Use a new output directory for each run. The plan command emits
`renderflow.composition-execution-plan/v1`: inspect PNG evidence under
`composition.branches`, locale/output targets under `execution`, and planned
shared intermediates under `stages`. Inspect the PNG branch's planned
dimensions, provider/settings evidence and diagnostics before execution, then
its actual output hash and run state afterward. PNG remains opt-in; default
targets are SVG, HTML and transcript JSON.

Raster settings apply to PNG branches. The integer DPI range is 36 through
600, with 144 as the default. Background is `transparent` by default or an
explicit opaque `#RRGGBB` color. The profile bounds canvas and asset decoding
work; see the [composition guide](../../../docs/user-guide/page-composition.md)
for current limits and refusals. DPI is a declared sampling choice, not a
printer acceptance or color-management result. PDF has its own independent
settings and opaque background requirement.

## PDF commands for a later authorized run

The same canonical package supplies a one-page PDF per selected locale.
At 300 DPI its 50.8 mm square canvas implies 600 × 600 pixels by arithmetic,
not by an observed render. The zero-gutter fixture uses the default binding
side `none`.

```bash
renderflow composition plan \
  --input "examples/page-composition/raster-composition/page.composition.json" \
  --locales "en,de" \
  --targets "pdf,png,transcript_json" \
  --pdf-dpi 300 \
  --pdf-background "#FFFFFF" \
  --pdf-binding-side "none" \
  --pdf-color-policy "assumed_srgb_no_profile_conversion" \
  --pdf-conformance "none"

renderflow composition render \
  --input "examples/page-composition/raster-composition/page.composition.json" \
  --locales "en,de" \
  --targets "pdf,transcript_json" \
  --pdf-dpi 300 \
  --pdf-background "#FFFFFF" \
  --output-dir "composition-pdf-output"
```

PDF defaults are 300 DPI, opaque white, no binding side, assumed sRGB without
profile conversion and no conformance claim. The `--raster-*` options affect
only the PNG branch. For later review, inspect PDF plan boxes, effective
settings and image-draw matrix, then the receipt's actual PDF publication.
A PDF flattens the measured page into one RGB image; it does not retain text,
fonts, editable layers or accessibility tags. Preserve the canonical package,
font notice and transcript. Raw RGB embedding can yield large PDFs; canvas,
surface and output-size limits still apply. No PDF was generated, parsed,
viewed or printed while authoring this checkpoint.

## Planned reuse and actual publication

Each locale has distinct common artifact-forest target identities even when
both produce PNG or PDF. Relevant target settings are explicit: PDF settings
do not change PNG target identity and PNG settings do not change PDF target
identity. The full captured package still enters conservative cache identity.

Transcript and measured-SVG intermediates are materialized lazily on cache
misses and reused within one locale. The plan's expected materializations are
not observed work. Keep derivative cache outcomes, actual stage observations
and immutable input sharing separate when reviewing a later run.

A rendered bundle retains `composition-plan.json`, `execution-plan.json`,
`composition-run.json` and `run-manifest.json` alongside retained inputs and
published outputs. The command prints the native v5 composition receipt.
Use common forest execution evidence to join locale/output target IDs to
actual published artifacts; an expected target alone is not publication proof.
The ordinary build also retains `build-plan.json` and `renderflow-run.json`
for the v2 configuration, while preserving those native sidecars. Both CLI
paths use the same composition engine. These examples remain unrun; no
Flow-owned Renderflow adapter is included.

## Authorship and retained notices

The tiny PNG was authored directly from the fixed RGBA pixel list recorded in
`PROVENANCE.json`, using Python standard-library PNG chunk construction and
zlib compression. No image was decoded or converted. The JPEG was copied
byte-for-byte from
`crates/renderflow-core/tests/fixtures/print-pdf/page-001.jpg`. Its source README
describes original 100 × 100 RGB baseline JFIF test data without EXIF; that
description was read, not revalidated by a decoder here. The SVG was copied
unchanged from the measured-typography package. Art, copy and layout are
synthetic MIT-licensed repository material, not consumer publication assets.

The font and complete packaged notice were copied unchanged from the
[measured-typography fixture](../measured-typography/README.md). Retain its
license notice when redistributing the font or font-derived outputs as its
terms require. The fixture's `reviewed: true` values do not approve consumer
translations, rights or publication.

Fixture authoring only copied bytes, serialized authored inputs and computed
their hashes. No font/image inspection tool, asset decoder, composition
renderer, native conversion, test, compiler/build, linter, schema validator,
dependency download/installer or CI was run. Final image appearance, alpha
handling, dimensions, repeatability and platform behavior require the later
authorized qualification session.
