# Composed PDF provider decision — #406 checkpoint 4

Status: **authored, unrun and unqualified** on 2026-10-15.
Base: [PR #451](https://github.com/egohygiene/renderflow/pull/451), merge
`c605436445aa26b9b5d496e37cd6a6805f44999f`.
The coding workspace was unavailable; source was read and authored through
GitHub. No PDF was rendered, decoded, opened, printed or independently validated.
Tests, builds, lint, schema validation, CLI/fixture/native execution, Cargo,
dependency installation/download and CI remain deferred.

## Selected route

Reuse the measured composition renderer and the bounded opaque raster path,
then the repository's existing **lopdf 0.45.0** dependency to author one PDF 1.4
page per selected locale. No new dependency or external executable is added.
The provider profile is:

```text
renderflow.composition-pdf/v1;lopdf=0.45.0;pdf=1.4;image=opaque-rgb8;compression=none;placement=uniform-dpi-top-left;boxes=explicit-f32;output-intent=none;fonts=flattened
```

The existing measured glyph paths, ordered shapes, opacity/clips and captured
artwork first become the same composed SVG used by the PNG provider. Its
closed resource resolver and bounded source-image policy still apply. The PDF
path supplies its own DPI and opaque background, obtains an in-memory PNG,
checks its exact raster contract, verifies every alpha byte is 255 and retains
the RGB bytes. It writes one uncompressed RGB image into the page. There is no
font discovery, new shaping engine, PDF import, PDF text extraction or implicit
native rendering step.

This deliberately bounded delivery route preserves canonical editing and
transcript semantics outside the PDF. It does not claim that a raster page is
a layered, vector, tagged, text-searchable or font-embedded document.

## Configuration and evidence

The canonical composition/content schemas stay v1. The output profile,
plans/runs and cache/artifact identities advance to v4; historical evidence
schemas remain intact. The typed plan is
`renderflow.composition-pdf/v1`.

`CompositionRequest.pdf` may be omitted for deterministic defaults. An
explicit object requires every field:

```json
{
  "dpi": 300,
  "background": "#ffffff",
  "binding_side": "none",
  "color_policy": "assumed_srgb_no_profile_conversion",
  "conformance": "none"
}
```

DPI accepts integers 36 through 600, subject to canvas limits. Background accepts
opaque six-digit `#RRGGBB` and normalizes case; `transparent` is refused.
Binding side is the enum `none`, `left` or `right`. The displayed color policy
and conformance are the only admitted values. Keeping these latter settings as
strings allows unsupported requests to appear as PDF-only branch diagnostics
instead of silently downgrading a PDF/X or color-managed request.

CLI flags are `--pdf-dpi`, `--pdf-background`, `--pdf-binding-side`,
`--pdf-color-policy` and `--pdf-conformance`. Unknown binding enum syntax is
a CLI/input parse error. Other invalid settings block PDF branches only.
PNG's `--raster-*` options and `CompositionRequest.raster` remain independent.
Neither PNG nor PDF is added to the default SVG/HTML/transcript-JSON selection.

The plan retains its effective settings, nested raster plan, media/trim/bleed/
safe boxes, gutter clearance and image-draw matrix, with
`runtime_qualified: false`. The corresponding terminal metadata retains the
same small plan. PDF cache identity includes this plan and the captured source
and typography identity; settings for unrelated outputs do not affect it.

## Page geometry and gutter

The physical canvas becomes both MediaBox and CropBox. TrimBox and BleedBox
retain the declared normalized rectangles independently, including asymmetric
margins. Coordinates convert from top-left normalized page space to
bottom-left PDF points. PDF numbers use finite f32 round-trip precision;
this is explicit bounded precision, not exact arbitrary real-number geometry.

The safe rectangle is retained as plan evidence and an authored layout
constraint. It is not an ArtBox, annotation, visible guide or print mark.
The provider does not invent crop/registration marks.

A nonzero `page.gutter_mm` requires left or right binding. Gutter means
minimum **total clearance from the selected trim edge**. Safe must already
clear that distance and remain within trim; locale-effective text bounds must
remain inside safe. The provider does not add another gutter to the safe inset,
shift content, resize the canvas, shrink text, alternate page sides or infer
duplex/book ordering. A nonconforming authored layout is blocked.

Raster dimensions are `ceil(mm × DPI / 25.4)`. The image-draw matrix uses
uniform physical sampling rather than separate width/height fit ratios.
Top-left placement and an explicit full-canvas clip discard the fractional
right/bottom pixel overhang without stretching the page. PDF is one page per
locale; assembling pages into a book remains separate work.

## Generated envelope and narrow admission

The writer emits a narrow PDF 1.4 envelope with five indirect objects,
a classic cross-reference table, one page, one resource image and one content
stream. The image is opaque 8-bit DeviceRGB with no filter/compression.
The content draws it through the planned matrix and canvas clip. The page
contains the explicit MediaBox, CropBox, TrimBox and BleedBox.

The profile includes no font dictionary, text operators, annotations, actions,
external references, ICC profile, output intent, optional-content layers,
accessibility tags or author-created extra PDF metadata.

Fresh output and cached output use the same admission function. It **does not
parse a candidate PDF with a generic PDF parser or decompress candidate
streams**. It derives the expected envelope from the current pinned lopdf
writer: an empty reference identifies the payload offset, and a bounded
zero-RGB serialization retains only the prefix and suffix around the known
raw-image payload. Candidate length and every envelope byte, including page
boxes, content, cross-references and trailer, must match. Only the fixed-length
raw RGB region is skipped in that comparison. The combined retained envelope
has a 16 KiB cap.

The surrounding cache logic independently checks metadata, parent identities,
paths, byte length and SHA-256. Those checks commit the actual payload bytes.
They do not prove that arbitrary replaced pixels resemble the source or make
a forged cache authentic. Narrow writer-envelope admission is not independent
PDF conformance, a general PDF structural inspection, visual equivalence,
PDF/X certification or a proof that a printer accepts the output.

The exact writer dependency and provider version are therefore part of the
contract. A future writer/version change must revisit envelope admission and
cache/profile identity rather than accepting arbitrary structurally plausible
PDFs as the same output.

## Bounds and memory scope

The existing raster policy caps each target axis at 8,192 pixels and the target
canvas at 16,777,216 pixels. Source-image decode and repeated-instance limits,
closed resource resolution and the 384 MiB active-surface estimate remain in
[RASTER_PROVIDER.md](RASTER_PROVIDER.md).

| Resource | Bound |
| --- | --- |
| Raw RGBA canvas | 64 MiB |
| Raw RGB image stream | 48 MiB |
| Final PDF | 64 MiB |
| Retained expected PDF envelope | 16 KiB |
| Aggregate derivative outputs in a composition run | 512 MiB |

Uncompressed RGB is intentional in this profile. Even a visually simple page
can create a large PDF at high DPI. Pixel limits can reject a large physical
page at 300 DPI; lowering DPI changes sampling quality and should be an
explicit choice, not an automatic fallback.

Rendering releases PNG/RGBA/writer buffers before fresh PDF admission.
Admission can retain the candidate PDF, one bounded zero-RGB payload and the
small expected envelope. These are bounded-stage buffers, not a whole-process
RSS or wall-clock isolation proof. The raster surface estimate does not include
all Rust allocations, provider internals or PDF serialization overhead.

## Color, text and publication limits

The only color policy is assumed sRGB without profile conversion. The PDF
contains DeviceRGB bytes, no ICCBased image profile and no output intent.
The PNG stage's sRGB metadata does not become a PDF ICC profile. CMYK,
grayscale PDF mode, spot colors, paper simulation and printer transforms are
not supported. Conformance is `none`; no PDF/X or archival/accessibility
standard is claimed.

The final page is flattened. It contains neither selectable/searchable text
nor embedded fonts, vector outlines, editable layers, tags or reading order.
Keep the canonical package, companion transcript, font identity and license
notices. The typography report identifies the captured font that produced
the pixels; it does not assert font embedding in the PDF. Source review and
synthetic `reviewed: true` fixture data do not grant consumer rights or
publication approval.

Unsupported scripts, writing modes, fonts and overflow remain bounded by
[TYPOGRAPHY_PROVIDER.md](TYPOGRAPHY_PROVIDER.md). Opaque composition does not
expand that typography support.

## Deferred qualification

Authored regressions and examples are preparation only. The later authorized
session must cover the actual writer API/build, full image decode/composition,
asymmetric box placement, gutter boundary/locale overrides, independent
PDF parsing/viewing, image orientation and DPI, opaque colors, candidate/cache
tampering, size/memory limits, repeatability and platform differences.
Printer or PDF-standard qualification requires separate evidence and a
concrete consumer profile. No executable result in this document closes #406.

Next implementation work is locale/output identity integration with the
general planner, artifact forest and Flow consumption. Broader print features,
typography and native interchange remain explicit follow-up work.

## Source basis

The contract is based on source inspection of this repository's composition
raster/typography modules, its existing locked lopdf dependency, and the pinned
upstream lopdf writer/content/object APIs. The generated envelope is
implemented by the composed-PDF provider; the schema records intended evidence
without executing or independently validating it.

Pinned primary source references:

- lopdf 0.45.0 [writer and stream serialization](https://github.com/J-F-Liu/lopdf/blob/v0.45.0/src/writer.rs).
- lopdf 0.45.0 [objects, stream lengths and compression controls](https://github.com/J-F-Liu/lopdf/blob/v0.45.0/src/object.rs).
- Repository [PDF settings and geometry](../../../crates/renderflow-core/src/composition/pdf_plan.rs)
  and [generated PDF envelope](../../../crates/renderflow-core/src/composition/pdf.rs).
