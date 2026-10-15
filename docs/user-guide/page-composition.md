# Layered and localized page composition

The `composition` commands turn an explicitly authored page composition into
per-locale PDF, PNG, SVG, HTML, and accessible transcript files. Art, shapes, text content,
and typography remain separate inputs. The source language, default language,
and every translation use the same planner and renderer.

This is the **fourth authored, unqualified #406 implementation checkpoint**.
Its measured-typography path uses pinned HarfRust 0.14.0 and Skrifa 0.48.0 to
shape supported text and derive glyph outlines from captured font bytes. The
PNG branch adds pinned resvg 0.48.1 to rasterize that same composed geometry
with explicitly captured image resources. The PDF branch reuses that opaque
rasterization and the existing lopdf 0.45.0 dependency for one image-only page
per locale, with explicit geometry and color-policy evidence.
Tests, builds, lint, schema validation, CLI/fixture/render/native execution,
Cargo and CI were deferred during the sprint. Source implementation and serialized evidence do not establish print
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

The canonical composition and content schemas remain v1. Composition output
uses `renderflow.layered-composition/v4`; plans and runs use
`renderflow.composition-plan/v4` and `renderflow.composition-run/v4`. The new
profile adds independent PDF settings and page evidence. Historical v3 PNG,
v2 outlined-preview and v1 browser-text evidence schemas remain available.

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
| `outputs/<locale>/page.png` | Published composed PNG for a successful explicitly selected PNG branch |
| `outputs/<locale>/page.pdf` | Published single-page flattened PDF for a successful explicitly selected PDF branch |
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
| `png` | Opt-in rasterization of the measured composition with explicit DPI/background and captured PNG/JPEG/SVG plates |
| `pdf` | Opt-in opaque, image-only composed page with trim/bleed boxes and explicit binding-side evidence; no printer or PDF/X acceptance claim |
| `ora`, `psd`, `idml` | Explicit unsupported interchange branches; no claim of native editability |

Selecting `svg,idml`, for example, can publish the supported SVG branch while
retaining the unavailable IDML branch and reporting an incomplete run. Do not
interpret an output filename alone as proof that every requested branch ran.

## Compose a PNG

PNG is opt-in; it is not added to the default target list. The standalone
[mixed-asset example](../../examples/page-composition/raster-composition/README.md)
contains authored alpha PNG, copied synthetic JPEG, bounded vector artwork,
shapes and measured text. Its 50.8 mm square canvas corresponds to 288 × 288
pixels at 144 DPI by arithmetic; no checked-in output proves that expectation.

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
  --output-dir "dist/page-raster-first-run"
```

`--raster-dpi` accepts integers from 36 through 600 and defaults to 144.
`--raster-background` defaults to `transparent` or accepts an opaque `#RRGGBB`
color. The equivalent SDK settings live under `CompositionRequest.raster`;
omitting that object uses defaults, while an explicit object requires both
`dpi` and `background`. Invalid raster settings block PNG branches; they do not
change the geometry, output or cache identity of PDF/SVG/HTML/transcript branches.
Keep explicit quotes around hex colors in shell commands.

PNG plans carry `branches[].raster` with the provider identity, effective
settings, pixel dimensions, `rgba8`/straight-alpha encoding and
`assumed_srgb_no_profile_conversion` color policy. Dimensions round upward from
`millimeters × DPI / 25.4`. Each output axis is capped at 8,192 pixels and the
whole canvas at 16,777,216 pixels. Terminal artifact metadata retains the same
raster plan. The top-level branch raster plan is PNG-only; PDF retains its own
raster plan inside `branches[].pdf.raster`. Invalid PNG settings leave
its top-level raster plan absent and retain actionable branch diagnostics.

The PNG provider has no font discovery or browser text shaping. It receives
the measured glyph paths and a closed set of resources from the captured
composition; paths, URLs and implicit system resources cannot supply missing
artwork. Raster plates must pass the provider's decode limits before use.
Retain the plan, receipt, canonical inputs and transcript when handing PNG
output to another project. Pixels flatten layers and visual text; they do not
replace the editable source or accessibility content.

Each source raster image has the same 8,192-axis/16,777,216-pixel caps as the
output canvas. Repeated raster-image instances count toward a 33,554,432-pixel
aggregate. Encoded assets are capped at 16 MiB; decode buffers and the output
PNG are each capped at 64 MiB. A separate 384 MiB estimate bounds simultaneous
pixel surfaces during tree rendering, including nested groups, clip alpha masks and
image decode surfaces. It is not a process-memory or wall-clock guarantee.

The source-image policy accepts bounded 8-bit PNG and baseline/progressive
8-bit grayscale or RGB JPEG. PNG ICC/HDR/cICP/EXIF/animation metadata and
nonstandard gamma/chromaticity declarations are refused. JPEG EXIF/XMP, ICC,
Adobe color transforms and CMYK/YCCK are refused. Unprofiled RGB is assumed sRGB;
the provider does not repair orientation or convert color profiles. The output
is straight-alpha RGBA8 PNG with sRGB and rounded pixels-per-meter metadata.
Fresh output and cached output pass the same authored decode/dimension/metadata
checks before publication. See the [raster provider decision](../work/406/RASTER_PROVIDER.md)
for the exact admitted container subset and remaining qualification limits.

This is a bounded screen-raster composition path. It does not establish ICC
conversion, CMYK, spot colors, PDF/X or printer acceptance. DPI records the
chosen sampling density; it is not proof of readable print typography. The
PNG route, decoder handling, image appearance and platform repeatability remain
unrun and unqualified during this sprint.

## Compose a PDF

PDF is also opt-in and independent of PNG settings. It produces one page for
each selected locale from the same measured glyph geometry, art and shapes.
The default PDF sampling density is 300 DPI and its background is opaque white.
The same font, text-review, overflow, decoding and surface-budget gates apply.

```bash
renderflow composition plan \
  --input "examples/page-composition/raster-composition/page.composition.json" \
  --locales "en,de" \
  --targets "pdf,svg,transcript_json" \
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
  --output-dir "dist/page-pdf-first-run"
```

The render example assumes the `dist` parent already exists. These are commands
for a later authorized run; no PDF was generated or viewed during this sprint.

| PDF option | Default | Admitted behavior |
| --- | --- | --- |
| `--pdf-dpi` | `300` | Integer 36–600, subject to the same 8,192-axis and 16,777,216-total-pixel caps |
| `--pdf-background` | `#ffffff` | Opaque `#RRGGBB`; transparent is refused and hex colors normalize to lowercase |
| `--pdf-binding-side` | `none` | `none`, `left` or `right`; nonzero gutter requires an explicit left/right side |
| `--pdf-color-policy` | `assumed_srgb_no_profile_conversion` | The only admitted policy; no ICC conversion or output intent |
| `--pdf-conformance` | `none` | The only admitted value; PDF/X and other conformance requests are blocked |

The equivalent SDK settings live in `CompositionRequest.pdf`. Omission chooses
defaults; an explicit object requires all five fields. Unsupported color or
conformance strings and invalid PDF settings produce blocked PDF branches while
other selected outputs can proceed. Invalid CLI enum syntax is an argument
error. PDF settings affect PDF cache identity only; `--raster-*` settings remain
PNG-only and cannot silently change a PDF.

A PDF plan carries `branches[].pdf`, including the effective settings, nested
raster plan, provider identity, page boxes in points, gutter clearance and
image-draw matrix. Terminal artifact metadata retains that plan. These boxes
convert the normalized top-left composition coordinates to PDF's bottom-left
point coordinates; values use the PDF provider's f32 round-trip precision.
MediaBox and CropBox cover the full canvas. TrimBox and BleedBox retain their
independent, potentially asymmetric authored rectangles. The safe rectangle
is evidence and a layout constraint, not a separate PDF page box or printed
guide. No crop marks or registration marks are added.

`page.gutter_mm` means the minimum **total clearance from the selected trim
edge**. For a left binding, the safe area's left edge must already lie at least
that distance to the right of the trim's left edge; right binding is symmetric.
The safe area must remain within trim, and effective locale text bounds must
remain within the safe area. The setting does not add another margin to an
existing safe inset. It does not translate artwork, resize the page, reflow
text, alternate sides by page number or infer a duplex layout. Adjust the
canonical layout explicitly when its authored safe area does not satisfy the
requested gutter.

The provider renders an opaque PNG in memory, verifies it, removes the opaque
alpha channel and writes one uncompressed 8-bit DeviceRGB image into the PDF
using the existing lopdf dependency. It keeps a uniform physical sampling
scale; the final fractional pixel at the right or bottom is clipped to the
page canvas instead of stretching both axes to independently rounded pixel
dimensions. Each locale is an independent one-page PDF, not an assembled book.
Raw RGB bytes can occupy up to 48 MiB; the whole PDF remains capped at 64 MiB.
Selecting a high DPI can therefore yield a large file even for simple artwork.

Fresh and cached PDF candidates pass the same narrow authored envelope
admission before publication. The provider compares every structural byte with
the current pinned writer's expected output, skipping only the fixed-size raw
RGB payload. The surrounding output/cache SHA-256 commits those pixel bytes.
It does not parse arbitrary cached PDFs or decompress candidate streams.
Envelope identity does not prove source-to-pixel appearance, independent PDF
conformance or printer acceptance. Unknown PDFs are not repaired or certified.

The PDF contains flattened pixels: it has no editable layers, embedded fonts,
selectable/searchable lettering, tags or accessibility reading order. Keep
canonical inputs and a transcript. The unchanged font provenance and typography
evidence explain which captured font produced the pixels; they are not a claim
that the PDF embeds that font. RGB is assumed sRGB without ICC conversion,
CMYK, spot colors, output intent or PDF/X conformance. An opaque white default
does not establish paper color or printer color management.

See the [PDF provider decision](../work/406/PDF_PROVIDER.md) for the exact
bounded output contract and later qualification work. The authored route,
schemas, regressions, cache behavior, PDF viewer appearance and print fitness
remain unrun and unqualified.

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
block the visual branch. SVG/HTML/PNG retain an unresolved binding-side warning for nonzero gutter
intent. PDF instead requires an explicit side and validates the existing safe
area against that clearance, as described in “Compose a PDF” above; it never guesses or shifts art.

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
Each locale counts once toward request limits when PDF, PNG, SVG and HTML are
selected; the full serialized plan can repeat that locale's report in those
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
establish PDF/X, a printer profile, or commercial print acceptance. The bounded
composed PDF route above records these choices without qualifying print output;
native design-file interchange remains separate work.

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
for general planner/Flow integration, execution and visual qualification,
broader typography, and the remaining native interoperability requirements.
The [format evaluation](../work/406/FORMAT_EVALUATION.md) explains the canonical
manifest choice and the boundaries of each proposed interchange format.
