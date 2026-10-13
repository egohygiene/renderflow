# Layered and localized page composition

The `composition` commands turn an explicitly authored page composition into
per-locale SVG, HTML, and accessible transcript files. Art, shapes, text content,
and typography remain separate inputs. The source language, default language,
and every translation use the same planner and renderer.

This is the **second authored, unqualified #406 implementation checkpoint**.
Its measured-typography path uses pinned HarfRust 0.14.0 and Skrifa 0.48.0 to
shape supported text and derive glyph outlines from captured font bytes.
Tests, builds, visual review, native execution, and CI were deferred during the
sprint. Source implementation and serialized evidence do not establish print
readability, font rights, or acceptance by a design application.

## Start with an authored composition

The canonical input declares `renderflow.page-composition/v1`. It references
SHA-256-pinned local files instead of embedding translations into artwork:

| Input | Purpose |
| --- | --- |
| Composition manifest | Page geometry, paint order, stable layer IDs, reading order, locale declarations, and provenance |
| Content document for each locale | Text keyed by content ID, source revision, review state, direction, and writing mode |
| Font registry | Pinned local fonts, typography roles, fallback policy, and licensing metadata |
| Shared art assets | Raster or bounded vector artwork referenced by stable asset IDs |
| Original source references | Retained source identity for original, flattened, or reconstructed material |

Composition references resolve from the manifest's directory; font and license
paths resolve from the font registry's directory. References must stay within
their directory and cannot traverse through symlinks. Keep these files together.
The loader reads and verifies the local pins. It does not fetch missing files,
install fonts, translate text, or separate flattened artwork.

Font registries must be JSON in this profile. Font bytes and the registry are
pinned; license files receive an observed hash because the current font
registry contract has no license-file digest field.

Declare each locale explicitly, including the source and default locale. There
is no implicit substitution of English or another language when a translation
is missing. A translation records the source revision it was reviewed against,
so editing source text can expose a stale translation instead of silently
publishing it as current. Unreviewed content, stale source revisions, missing
text, and unknown text IDs block that locale's branches. Unreadable or mismatched
file pins prevent loading the composition.

The model supports raster and vector asset layers, rectangles and ellipses,
and editable text layers. Layers have normalized bounds, optional clipping,
opacity, and normal blending. Text layers select semantic font roles and
explicit font-size, minimum-size, line-height, padding, alignment, and color
settings. An explicit reading order is independent of visual paint order.

The canonical composition and content schemas remain v1. Measured preview
output uses `renderflow.layered-svg/v2`; plans and runs use
`renderflow.composition-plan/v2` and `renderflow.composition-run/v2`. The new
profile separates its geometry and cache evidence from older v1 browser-text
previews.

## Inspect and plan without output writes

```bash
renderflow composition inspect \
  --input "examples/page-composition/measured-typography/page.composition.json"

renderflow composition plan \
  --input "examples/page-composition/measured-typography/page.composition.json" \
  --locales "en,de" \
  --targets "svg,html,transcript_json"
```

The [measured-typography example](../../examples/page-composition/measured-typography/README.md)
includes a real, unmodified DejaVu Sans TTF, its retained license and exact
capture provenance. English and German are the authored horizontal LTR cases;
Arabic RTL and Japanese vertical content are deliberate unsupported visual
cases and remain unreviewed. Selecting `all_declared` retains those blocked
branches. No fixture run or visual result is claimed.

The original [structural fixture](../../examples/page-composition/README.md)
is retained unchanged. Its 45-byte WOFF2 stub is not a usable font and cannot
satisfy the measured visual provider. Its reviewed English/German content can
still produce transcripts. Use the real-font fixture for later visual work.

Both commands print JSON to standard output and leave the source, output, and
cache directories unchanged. They read the manifest and referenced local
assets and fonts. `inspect` emits the default plan: all declared locales and
the default `svg,html,transcript_json` targets.

`--locales` accepts exactly one locale, a comma-separated subset, or
`all_declared` (the default). Unknown locales do not fall back. Empty and
duplicate selector values are rejected. `--targets` accepts the same
comma-separated syntax; every selected target is required for a complete run.

Planning retains unavailable branches and their reasons. A plan containing an
unsupported target is still a successfully emitted plan; inspect its branch
diagnostics before executing. Invalid input or unreadable/mismatched pinned
files cause an error instead of a fabricated plan.

## Render into a new bundle

```bash
mkdir -p "dist"

renderflow composition render \
  --input "examples/page-composition/measured-typography/page.composition.json" \
  --locales "en,de" \
  --targets "svg,html,transcript_json,transcript_text" \
  --output-dir "dist/page-first-run" \
  --cache-dir "dist/page-composition-cache"
```

The output directory must be new and its parent must already exist. Renderflow
does not replace the original artwork, content documents, fonts, or an existing
bundle. The optional cache is a separate local directory; reuse verifies the
cached payload before publication. Without `--cache-dir`, the bundle uses its
own `.artifact-store`.

| Bundle member | Purpose |
| --- | --- |
| `composition-plan.json` | The exact plan used by this run |
| `composition-run.json` | Run receipt written after publication, including branch outcomes and artifact evidence |
| `inputs/<sha256>` | Captured input bytes, shared by content identity within the bundle |
| `outputs/<locale>/page.svg` | Published SVG for a successful selected SVG branch |
| `outputs/<locale>/page.html` | Published HTML for a successful selected HTML branch |
| `outputs/<locale>/page.transcript.json` | Published structured transcript |
| `outputs/<locale>/page.transcript.txt` | Published plain transcript |

Using an external cache does not remove the retained inputs and published
outputs from the bundle. Preserve the receipt and plan with them.
Visual branches carry their measured report in `branches[].typography`; the
run retains that same plan. Terminal artifact metadata links the report's
digest and provider identity, so the published candidate can be traced to its
planned font and geometry decisions. Transcript branches do not require a
visual typography report.
For the actual rendered font, read
`branches[].typography.layers[].selected_font`, together with that layer's
rejected candidates and selected fallback index. Top-level `font_resolution`
is the generic registry report retained for context; its candidate can differ
from the font selected after checking the whole layer's actual text.
The hashed input copies retain evidence; keep the original package for editing
and reloading its relative paths. Cache keys conservatively include the entire
captured package, so changing one locale can invalidate other output keys while
immutable artwork bytes remain shared. Cache reuse does not approve a candidate.

The command prints its run receipt to standard output. A partial, failed, or
cancelled run returns nonzero after emitting its available receipt. Successful
branches remain distinguishable from unavailable or failed branches. A command
error before a run receipt can be created is not evidence of publication.

| Target | Checkpoint behavior |
| --- | --- |
| `svg` | Measured glyph paths and embedded artwork; editable text remains in the canonical package |
| `html` | The same path-based page preview plus a separate readable transcript |
| `transcript_json` | Structured localized text in declared reading order |
| `transcript_text` | Plain localized text in declared reading order |
| `png`, `pdf` | Explicit unsupported branches; no implicit browser or image renderer |
| `ora`, `psd`, `idml` | Explicit unsupported interchange branches; no claim of native editability |

Selecting `svg,pdf`, for example, can publish the supported SVG branch while
retaining the unavailable PDF branch and reporting an incomplete run. Do not
interpret an output filename alone as proof that every requested branch ran.

## Layout and review boundaries

The visual profile is bounded to supported Latin-script, left-to-right,
horizontal text in static TTF/OTF fonts. Webfont containers (WOFF/WOFF2),
variable fonts, collections, color/bitmap fonts, right-to-left layout, vertical
writing and other unsupported font/script surfaces do not become supported
through a changed filename or registry declaration. The character allowlist
covers selected Latin letters, combining marks and common punctuation; it does
not include emoji, joining controls, bidi controls or variation selectors. A
line cannot start with a combining mark. Use a supported captured font and
inspect diagnostics. There is no installed-font or browser-font fallback.

Each text layer tries the role's declared primary font and fallbacks in order.
A usable candidate must serve the entire layer; the provider does not quietly
mix fonts within a line. The typography evidence records the selected font,
fallback decisions, shaped glyph geometry and layout diagnostics. Its stable
digest is part of branch/cache identity. Registry Unicode declarations are
metadata, not proof of coverage for the actual text.

Text keeps its authored line breaks and size; the renderer does not wrap,
shrink, translate, or fit it automatically. It checks shaped line advances and
conservative outline bounds against authored bounds, padding and alignment,
alongside vertical line boxes. Outline bounds include the Bézier control-point
hull, which can overestimate the actual curve extent. They are not tight curve
bounds or measured raster ink, and can conservatively reject a near-edge glyph.
Known overflow blocks the visual branch rather than publishing clipped text as
if it fit. These checks describe the implemented provider path; execution and
visual qualification are still pending.
Text containers outside the declared safe area or excluded by their clip also
block the visual branch. A nonzero gutter reports an unresolved binding-side
warning; the renderer does not guess which side needs the gutter.

Transcript outputs preserve text and declared reading order without applying
those visual layout restrictions. This retains readable content for review
even when its requested visual branch is unsupported. Content review,
completeness and source-revision gates still apply to transcripts.

SVG/HTML previews use the measured glyph paths, without browser-shaped text or
font-face CSS for the page lettering. This makes the authored path geometry
the geometry that is checked. Visual text in the SVG is not selectable or
editable as a native text object; retain the canonical content and manifest
for editing. HTML provides a separate text transcript, and both transcript
targets retain reading order and localized strings.

The measured profile also bounds work before publication:

| Scope | Limit |
| --- | --- |
| One locale, across referenced text layers | 4,096 text code points, 4,096 shaped glyphs, and 256 authored lines |
| One locale, retained geometry/evidence | 4 MiB private outline-path bytes and 2 MiB compact typography-report JSON |
| One locale, font work | 128 whole-layer candidate attempts and 128 MiB cumulative font bytes processed by policy/shaping |
| One request selecting a visual target | 16,384 referenced text code points across distinct selected known locales |
| One visual request, retained geometry/evidence | 16 MiB private outline-path bytes and 16 MiB compact report JSON across distinct selected known locales |

Spaces and newline delimiters count toward text-code-point limits. Repeated
glyph outlines count per occurrence in the path budget, and retrying the same
font on a different layer counts as another candidate attempt. Font work counts
repeated policy/shaping use of the bytes, not just unique font file sizes.
Each locale counts once toward request limits when both SVG and HTML are
selected; the full serialized plan can repeat that locale's report in both
branches. The private paths are shared in memory. Request-budget failures use
`composition.typography.plan_budget` and advise selecting fewer locales before
any output writes. Transcript-only requests bypass visual typography budgets.

Layer bounds and clip rectangles use the full page canvas as their reference.
Art plates stretch to their authored bounds; preserve the original aspect
ratio when choosing those bounds if stretching is unwanted.

The planner preserves deterministic layout decisions and reports constraints
that this profile cannot satisfy. Locale-specific overrides must identify a
layer and record their author and reason. They are explicit reviewed inputs,
not silent automatic repositioning or generated text edits.

Font and layout decisions are local. A supported provider's coverage and
geometry results refer to the selected captured font and requested text;
they do not assert support for every script or glyph in that font. The
checkpoint does not qualify complex-script shaping, bidirectional behavior,
automatic line breaking or raster fidelity. SVG/HTML viewers can still differ
in antialiasing and image rendering. Review the page in the intended viewer
and at its intended size before publication, and retain the font license
notices when distributing font-derived assets as their terms require.

Trim, bleed, safe area, and gutter values describe page geometry. They do not
establish PDF/X, a printer profile, or commercial print acceptance. Native
PNG/PDF generation and native design-file interchange remain separate work.

For flattened or reconstructed sources, keep the original references and
honest provenance flags. The composition path does not perform AI inpainting,
OCR, art extraction, translation, or recovery of lost editable layers.
Human review remains explicit; generating a derivative does not approve it.

## Consumption by other projects

Use the same SDK composition request for inspection, planning, and execution.
Preserve the canonical inputs and the full native run evidence with the
published files. A consumer should use recorded branch state and actual output
hashes to decide what is available, and carry review flags forward.

This candidate composition contract is separate from `renderflow/v2` build
specs and the general artifact forest. It does not extend `flow.artifact/v1`
or imply that a released Flow adapter already consumes the new contract. See
the [Flow artifact handoff](../architecture/flow-artifact-forest.md) for the
existing distinction between planned inventory and actual publication.

Issue [#406](https://github.com/egohygiene/renderflow/issues/406) remains open
for execution and visual qualification, broader typography, and the remaining
native output and interoperability requirements.
The [format evaluation](../work/406/FORMAT_EVALUATION.md) explains the canonical
manifest choice and the boundaries of each proposed interchange format.
