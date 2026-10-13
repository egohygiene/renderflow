# Layered page-composition fixtures

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
so explicitly. The registry allows exercising pinned bytes, role resolution,
fallback evidence and stylesheet generation, but cannot establish glyph
coverage, text shaping or rendering. A viewer may substitute another font or
reject the generated SVG; neither result qualifies typography.

Before using this composition for an actual visual review:

1. Copy the package to a working directory and replace the stub with a real,
   reviewed local font whose license permits web embedding and redistribution.
2. Retain its license alongside the font. Update the registry's path, format,
   family, provenance, license metadata, Unicode coverage claims and SHA-256.
3. Update the manifest's `font_registry.sha256` to match the revised registry
   bytes. Keep all referenced files inside the copied package.
4. Render only the supported locales and inspect every output with the intended
   viewer. Actual glyph coverage, line breaking and final typography still need
   validation; registry coverage declarations alone are not proof.

No native renderer, fixture command, schema validator or test was run while
authoring this checkpoint. JSON digests were computed directly from authored
fixture bytes. They are input pins, not execution receipts.

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
therefore includes the intentional unsupported branches; use the exact English
or English/German subset for a supported-geometry example. An unsupported branch
must remain observable and must not receive another locale's output.

See the [page-composition user guide](../../docs/user-guide/page-composition.md)
for the current CLI and supported output contract. Raster, print and layered
interchange availability must be read from the plan; this fixture supplies no
external renderer installation or fabricated outputs.

## Negative cases

The mutation descriptions are future test inputs, not observed results. For a
semantic-content mutation, update the outer content pin as well; otherwise the
correct first result is an integrity mismatch before semantic validation. Keep
the pin unchanged specifically when testing stale or tampered bytes. An
`all_declared` selection can produce several diagnostics on the RTL/vertical
cases; support refusal must never be suppressed by typography warnings.

The fixture does not promise arbitrary SVG, PSD, OpenRaster or native-publishing
round trips. SVG retains only the explicitly supported composition surface.
