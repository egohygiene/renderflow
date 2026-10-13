# Layered page-composition fixtures

There are three independent packages:

| Package | Purpose |
| --- | --- |
| `page.composition.json` with `structural-only/` | Original contract fixture with an intentionally unusable font stub; visual branches are blocked by the measured provider |
| [`measured-typography/page.composition.json`](measured-typography/README.md) | Real, retained DejaVu Sans TTF and license for the bounded static-font Latin LTR path; no executed or approved visual result |
| [`raster-composition/page.composition.json`](raster-composition/README.md) | Small mixed-asset page with alpha PNG, synthetic JPEG, bounded SVG, shapes and text for opt-in composed PNG; no decoded/rendered qualification |

All retain editable canonical text and authored reading order. The v3 visual
profile uses measured glyph paths for SVG/HTML and opt-in PNG rasterization,
while transcript targets preserve localized strings. The real-font packages
are starting points for later typography and visual qualification; none includes successful
execution receipts.

`page.composition.json` is an original two-panel contract fixture. Its
textless SVG plate is shared by separate English (`en`) and expanded German
(`de`) copy documents. Arabic (`ar`) and Japanese (`ja`) documents exercise
explicit unsupported right-to-left and vertical-writing branches. Each text
container has a stable content ID and reading-order relationship. The German
override records its author and reason without copying the plate.

The plate, copy and layout are synthetic examples authored for this repository
under its MIT license. They are not material from a consumer publication. The
Arabic and Japanese strings are capability examples, not reviewed publication
translations. `reviewed: true` is a synthetic test declaration for the English
and German examples; it does not grant consumer approval.

## Font limitation

**This package is structural-only, not a visually qualified page.** Its retained
`fonts/Fixture-Regular.woff2` is the repository's existing 45-byte CC0 synthetic
font-container stub. It is not a usable font. The accompanying CC0 notice says
so explicitly. The registry exercises pinned bytes and role declarations, but
cannot establish glyph coverage, text shaping or rendering. The measured
provider refuses this unsupported WOFF2 stub for visual output instead of
delegating lettering to browser fallback. English/German transcripts remain
available when their semantic content gates are satisfied.

Before using this composition for an actual visual review:

1. Start from the standalone [real-font package](measured-typography/README.md),
   or copy this package and replace the stub with a real static TTF/OTF whose
   license permits the intended redistribution and use.
2. Retain its license alongside the font. Update the registry's path, format,
   family, provenance, license metadata, Unicode coverage claims and SHA-256.
3. Update the manifest's `font_registry.sha256` to match the revised registry
   bytes. Keep all referenced files inside the copied package.
4. Request only supported locales and inspect typography diagnostics before
   rendering. Inspect every resulting output with the intended viewer. The
   provider's glyph/geometry results still need qualification; registry
   coverage declarations alone are not proof.

No font/image inspection tool, asset decoder, native renderer, fixture command,
schema validator or test was run while authoring these checkpoints. Digests were
computed directly from authored or copied bytes. They are input pins, not
execution receipts. The tiny alpha PNG in the raster package was authored
directly from recorded fixed pixels, without decoding or converting an image.

## Source package

| File | Role |
| --- | --- |
| `page.composition.json` | Locale-neutral canvas, paint order, text containers, provenance and exact input pins |
| `structural-only/assets/two-panels.svg` | Shared immutable textless vector plate |
| `structural-only/content/en.json` | Source/default English content; explicit line breaks |
| `structural-only/content/de.json` | Expanded German content plus a bounded layout override in the manifest |
| `structural-only/content/ar.json` | Right-to-left capability case; expected unsupported |
| `structural-only/content/ja.json` | Vertical-writing capability case; expected unsupported |
| `structural-only/fonts/registry.json` | Explicit structural-only typography registry |
| `negative-cases.json` | Authored mutations and expected diagnostic categories for later testing |

The source/default locale is an ordinary branch. Selecting all declared locales
therefore includes the intentional unsupported branches. With this structural
package, use English/German transcript targets for content-only work; its stub
blocks visual output. With the real-font package, the English/German subset is
the authored measured-typography case, subject to diagnostics. An unsupported
branch must remain observable and must not receive another locale's output.

See the [page-composition user guide](../../docs/user-guide/page-composition.md)
for the current CLI and supported output contract. PNG is an explicit target
with bounded DPI/background settings; composed print PDF and layered
interchange remain unavailable. This fixture supplies no external renderer
installation or fabricated outputs.

## Negative cases

The mutation descriptions are future test inputs, not observed results. For a
semantic-content mutation, update the outer content pin as well; otherwise the
correct first result is an integrity mismatch before semantic validation. Keep
the pin unchanged specifically when testing stale or tampered bytes. An
`all_declared` selection can produce several diagnostics on the RTL/vertical
cases; support refusal must never be suppressed by typography warnings.

The fixtures do not promise arbitrary SVG, PSD, OpenRaster or native-publishing
round trips. SVG previews carry outlined lettering, not native editable text
objects; keep the canonical package for editing.
