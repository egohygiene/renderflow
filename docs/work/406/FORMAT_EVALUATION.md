# Layered composition: format and tool evaluation

Issue: [#406](https://github.com/egohygiene/renderflow/issues/406). Source review
date: **2026-10-13**. This is a design evaluation, not native-format qualification.
No candidate tool was installed, probed, executed, benchmarked, or used to convert
a fixture. Tests, builds, lint, schema validation, and CI remain deferred under
the implementation sprint authorization.

## Decision

Use a **versioned Renderflow manifest with referenced immutable assets** as the
canonical page composition. Keep reviewed locale text outside the artwork,
addressed by stable semantic content IDs. Treat a deliberately limited SVG
document as a useful editable derivative, and raster/print output as separately
qualified derivatives. Preserve every imported original.

This is an engineering recommendation inferred from the evidence below. None of
the evaluated interchange formats supplies Renderflow's complete content
revision, locale selection, asset identity, review, provenance, and output
evidence contract. A manifest can retain those relationships without pretending
to implement an editor's entire document model. It also permits future adapters
without making a proprietary editor a prerequisite.

SVG is the preferred first interchange direction because it represents vector
geometry, raster references, and actual text in an inspectable form. Its text
model still depends on fonts and shaping. Supporting SVG as a format does not
establish equal typography in every viewer. OpenRaster is useful for raster
layer exchange. PSD/PSB and IDML are optional compatibility directions that need
feature-specific evidence before advertisement. [S1–S8]

## Operation matrix

The matrix distinguishes **upstream feasibility** from **Renderflow support**.
“Candidate” means there is relevant source documentation, not an implemented or
qualified Renderflow adapter. “Subset” is an explicit proposed boundary, not a
claim that a general file of that format can be accepted.

| Candidate | Detect | Inspect | Extract | Generate | Validate | Round-trip |
| --- | --- | --- | --- | --- | --- | --- |
| Native manifest + assets | Proposed version discriminator | Proposed bounded typed model, references, digests | Referenced original assets/content; no recovered layers | Canonical serialization and explicit locale/output plans | Structural/reference/geometry checks; visual checks separate | Preserve only versioned model values and immutable bytes; not arbitrary editor state |
| SVG | XML root/namespace is a candidate signal; graph already names SVG | Candidate XML + static renderer tree; feature inventory required | Candidate IDs, shapes, text, image references; no inferred consumer semantics | Preferred narrow export subset | XML/subset checks plus renderer/font evidence; well-formed XML is insufficient | No general promise; paths, CSS, transforms, editor metadata, and text can normalize or disappear |
| OpenRaster | Candidate ZIP + exact first `mimetype` entry | Candidate `stack.xml`, PNG assets, supported stack features | Candidate separate stored layers | Candidate bounded raster stack plus required preview and thumbnail | Archive/layout/stack schema plus decoded images and composite comparison | Raster pixels/order/offset/opacity only after explicit adapter proof; no editable text promise |
| PSD/PSB | Image API recognizes names; future parser must confirm header/version | Candidate `psd-tools` low-level records/layer API | Candidate raw layers and stored text metadata | Limited pixel-layer construction upstream; editable typography not supported by that tool | Parser acceptance + declared feature inventory; no full Photoshop equivalence validator selected | No general promise; unknown tagged data, effects, type layers, smart objects need separate treatment |
| Layered TIFF | TIFF container detection does not establish Photoshop layers | Candidate baseline TIFF metadata; private layer data needs its own parser | Raster pages/subimages are candidates, not automatically creative layers | Raster TIFF feasible upstream; layered Photoshop TIFF deferred | Pixel/IFD checks do not validate private editable layer payloads | No layered-document promise; page/IFD preservation alone is insufficient |
| Layered PDF | PDF detection does not establish optional content or editor payloads | Candidate PDF objects/OCGs; distinguish Photoshop editing payload | Candidate content/resources; not original text containers/layout | Candidate OCG output in future; current print lane is image-only | Syntax, font, print, and accessibility checks are separate | No original-editability promise; OCG visibility is not an editor scene graph |
| IDML | Candidate ZIP/package/XML markers | Candidate XML parts and references | Candidate story/resource extraction | Candidate template/document manipulation | Schema/reference checks plus a qualified layout engine | No general cross-engine text/layout fidelity promise |

Format evidence: SVG [S1, S2]; OpenRaster [S3, S4, S5]; PSD/PSB [S6, S7];
TIFF [S9, S10]; PDF [S11, S12, S13]; IDML [S8, S14]. The native row is a proposed
Renderflow contract, not an external format claim.

### Current repository boundary

Source inspection found SVG/TIFF/PDF graph identifiers in
`crates/renderflow-core/src/graph/format.rs`, and design format names including
PSD/PSB in `crates/renderflow-core/src/image/format.rs`. The latter explicitly
labels vector/design formats as lacking FFmpeg support. Recognition is not a
layer extraction or composition capability.

The existing `print_pdf_inspect.rs` contract checks a narrowly constructed
single-image-per-page PDF; it is not a general layered PDF, font, or print-color
validator. `font.rs` already owns pinned local font assets, semantic roles,
declared coverage, and licensing metadata. Reuse that ownership, but do not
equate a signature or declared Unicode range with actual glyph coverage,
shaping, readable line breaking, or measured ink bounds. No general layered
interchange round-trip was established by this review.

### Scope selected for the first #406 checkpoint

The implementation lane selected from this evaluation is a separate composition
SDK/CLI surface: a native manifest, bounded immutable asset/content/font
snapshots, locale planning, and SVG/HTML/JSON/text derivatives. This checkpoint
does not modify the existing format-keyed artifact DAG. PNG, PDF, OpenRaster,
PSD/PSB and IDML composition outputs remain unavailable. Typography evidence
explicitly leaves actual glyph, shaping and ink-bound checks unverified.
Authored capabilities still require the deferred runtime and interoperability
qualification before they become proven capabilities.

## Features and known loss

These are format/tool affordances and implementation decisions. They are not
checkboxes asserting current renderer support.

| Candidate | Text, typography, semantics | Layers, masks, clipping, blending | Color, print, and fonts | Consequence for canonical choice |
| --- | --- | --- | --- | --- |
| Native manifest | Stable content IDs, reading order, accessibility relationships, locale revisions and overrides can be explicit | Ordered original raster/vector references; unsupported masks/blends must remain observable | Physical size/trim/bleed/gutter/safe intent and font/profile references can be recorded independently of renderer support | Selected; define only bounded executable subsets and preserve unsupported intent explicitly |
| SVG | Editable text, IDs and semantic markup are possible; glyph selection and bidi/vertical layout depend on renderer/font support [S1] | Vector/raster elements, groups, clipping and masking exist; subset support needs qualification [S2] | Pin fonts and dimensions; screen RGB output is not proof of CMYK, ICC, spot color or PDF/X behavior | Good derivative; exporting outlines loses editable/searchable text and needs a companion transcript |
| OpenRaster | No portable consumer text-ID, locale, or typography contract selected | Stack specification describes order, opacity, visibility, offset, compositing and isolation; Krita's documented interchange stores PNG layers [S3, S5] | Image resolution metadata exists; no qualified print-color or font-embedding route selected | Raster interchange only until vector/text extensions are independently proven |
| PSD/PSB | Format contains rich application data; `psd-tools` cannot edit type text or render fonts [S6, S7] | Layers, masks and additional information are extensive; upstream composition support is partial | Preserve profiles/original bytes; native rendering must establish print appearance | Compatibility source/output only; a flattened preview cannot substitute for layer fidelity |
| TIFF | Baseline raster data carries no selected editable type or reading-order contract | Multiple pages/pyramids are not equivalent to editor layers; Photoshop's layer option is a distinct behavior [S9, S10] | Useful raster delivery candidate; CMYK decoding alone proves neither profile correctness nor print approval | Do not promote ordinary TIFF conversion into a layered composition claim |
| PDF | Final text/content may remain selectable, but that is not source text-container semantics | OCGs control visibility; Photoshop PDF can additionally preserve Photoshop editing data [S11, S12] | Requires separate embedded-font, page-box, profile and accessibility evidence | Delivery/inspection direction; do not reverse it into an invented editable original |
| IDML | XML interchange supports document/content manipulation; a typesetter still controls layout [S8] | Publishing interchange candidate; unsupported objects/import normalization need a loss report | Fonts, links and a chosen engine remain execution dependencies | Justified when a consumer needs InDesign/Scribus exchange; defer canonical dependence |

### Typography and rights boundaries

Font identity must include actual font bytes, face/style/variation selection,
resolution policy, and renderer/shaper version. The same family name on two
machines is not an immutable font identity. Consumer text remains editable in
the canonical content catalog even when a particular output deliberately uses
outlines or pixels. Record that output loss and retain stable transcript IDs.

OpenType `OS/2.fsType` distinguishes embedding permissions and includes
restrictions on subsetting and outline embedding. Store applicable license
records with the font asset and honor the output's embedding mode; a format
supporting embedded fonts does not itself authorize embedding. [S15] This
evaluation does not acquire or redistribute any fonts.

Missing glyphs require actual font/engine inspection. Overflow requires shaped
advances and ink bounds, not character counts or average width. RTL, mixed
direction, vertical writing, language-specific line breaks, fallback, and
minimum readable size need distinct diagnostics. An initial renderer may refuse
unsupported writing modes. It must not silently rasterize, shrink, drop text,
or claim a typography pass from metadata alone. [S1, S15]

## Tool candidates

Licenses below are upstream declarations, not an integration approval or a
transitive dependency audit. Platform availability is upstream scope, not
Renderflow platform qualification. Maintenance observations are limited to
the linked documentation/release records available on the review date.

| Tool | Role worth evaluating | License/platform evidence | Maintenance observation and limits | Decision |
| --- | --- | --- | --- | --- |
| `resvg` / `usvg` | Parse/render a bounded static SVG derivative; Rust library or CLI | Apache-2.0 OR MIT; portable Rust/C/CLI interfaces [S16] | Official changelog lists 0.48.1 (2026-08-02), MSRV 1.85. The 0.48 font-stack change can alter rendering [S17] | Preferred rasterization candidate for a later pinned provider; no installation or dependency change from this evaluation |
| Inkscape | Human-authored SVG and optional CLI export compatibility | GPL; upstream documents Windows/macOS/Linux [S18, S19] | Public CLI documents actions/export behavior, including imports that convert text to paths. Exact package/headless behavior unqualified [S20] | Optional editor/provider; do not use implicit system fonts or treat export as round-trip proof |
| Krita | Human-authored raster layers and ORA interoperability check | GPLv3; upstream lists Windows/macOS/Linux [S21] | Maintained download/manual surfaces; reviewed manual is 5.3.0. No exact executable qualified [S5] | Optional interchange reference application, not canonical or mandatory CI dependency |
| `psd-tools` | Inspect/extract bounded PSD/PSB features and preserve original | MIT, Python package [S7, S22] | Reviewed stable documentation labels itself 1.24.0 and lists material editing/rendering limits; Python dependencies/native color support require platform packaging checks | Candidate read/extract adapter first; do not claim editable PSD text export |
| libvips | Raster delivery, TIFF pages and color-processing experiments | LGPL-2.1-or-later; project documents image-processing/library interfaces [S23] | Reviewed 8.17 TIFF API describes baseline TIFF and named extensions [S10]; no Photoshop layer payload support established | Useful raster provider candidate, not a creative-layer parser |
| qpdf | PDF structure inspection/transformation | Apache-2.0 [S24] | Current documentation identifies 12.4.2. Explicitly not a content renderer and does not interpret content-stream semantics [S13] | Structural evidence helper only; cannot establish typography, color or compositing correctness |
| Scribus | Publishing/IDML import qualification and possible print provider | Official project distribution lists GPLv2/GPLv3 and desktop platform support [S25] | Upstream 1.6.0 release notes identify IDML import [S14]. Direct site requests were bot-blocked; indexed official pages/distribution were available. Exact current import subset unqualified | Optional future publishing adapter; imported IDML is not evidence of export or round-trip symmetry |

The existing Rust JSON/XML/ZIP/hash infrastructure can express and inspect the
native manifest without a new composition engine. That reuse is an implementation
cost advantage, not proof that Renderflow already supports every candidate
operation. Prefer a pinned library/provider over new format-specific logic only
where its documented semantics match the chosen subset. Do not invent a PSD,
IDML, PDF, or general font engine within the first checkpoint.

## Bounded canonical design

The following is the recommended contract scope; the implementation handoff
must distinguish the fields it accepts from the features it executes.

1. **Page and geometry:** physical dimensions with explicit units, a normalized
   coordinate system, ordered layers, and trim/bleed/gutter/safe regions. Define
   transform order and range/precision rules. Avoid locale-dependent numeric
   serialization.
2. **Assets:** immutable original artwork, textless plates, fonts, masks and
   referenced vector data with media type, byte length, digest, role and
   provenance. Restrict resolved inputs to declared package assets; never let
   external SVG/XML/ZIP references trigger untracked reads or downloads.
3. **Content and semantics:** content IDs independent of layer names and paint
   order; a separate per-locale content catalog, source revision linkage,
   reading order and accessibility references. Source/default locale uses the
   same branch mechanism as every other selected locale.
4. **Typography:** semantic font roles, alignment, line-height, padding,
   minimum size, writing direction/mode and explicit fallback policy. Preserve
   original text; treat any locale-specific layout override as a separate,
   attributable change.
5. **Evidence:** source/manifest/content/font digests, effective configuration,
   provider and tool identity, declared loss, validation, review requirements,
   and actual published output references. Consumer approval is an input record
   with bounded applicability, never something Renderflow awards.
6. **Flattened legacy sources:** mark editable layers unavailable. A human may
   author a composition around an original plate. OCR, segmentation, inpainting,
   or AI reconstruction creates a new, attributable review candidate and cannot
   inherit source approval or claim lossless recovery.

For the first SVG subset, prioritize ordered groups, simple shapes, explicit
image placement, stable element IDs and explicit text containers. Reject or
report filters, arbitrary scripting/CSS, network resources, unknown blend modes,
and unsupported masks/writing modes. Do not advertise a capability merely
because the manifest can retain its intent. Generated SVG should remain a
derivative: editing it does not silently rewrite the canonical manifest or
translation review state.

## Determinism, validation, and promotion gates

These are future qualification requirements, not checks executed during this
evaluation.

- **Separate determinism levels.** Stable request/plan/SVG serialization is
  different from repeated raster byte identity, cross-platform pixel identity,
  semantic text preservation, and print equivalence. Record the level actually
  demonstrated; pin renderer, fonts, assets, locale data and output parameters.
- **Separate cache identity from a cache hit.** A shared art plate digest allows
  locale-neutral work to be identified. Reuse still needs retained outputs and
  verification; adding locale branches must not invent shared execution.
- **Make output validation specific.** A JSON schema, XML parser or qpdf syntax
  pass is not a glyph/overflow/print check. Plans report unsupported outputs
  before execution. Failed or unattempted outputs never become publications.
- **Qualify narrowly.** Use original synthetic fixtures for expanded text,
  stale source revisions, missing fonts/glyphs, exact safe/bleed bounds,
  unsupported RTL/vertical modes, unavailable providers and flattened-source
  claims. Native application interoperability is a separate optional suite.
- **Define round-trip invariants first.** For a future ORA route, enumerate
  preserved pixel bytes or decoded pixels, layer order/names/offsets/opacity and
  exclusions. For SVG, enumerate retained IDs/text/geometry. No broad
  “lossless” claim without both property and rendering evidence.
- **Keep publication authority with the consumer.** A successful composition
  and a structurally valid print derivative cannot approve translations, art,
  rights, accessibility, or printer acceptance. Flow consumes machine-readable
  outcomes and retained evidence rather than guessing from extension names.

## Source register

All sources were consulted on **2026-10-13**. Links are primary specifications,
maintainer documentation, or the project's official distribution. Mutable
documentation versions are observations, not dependency pins. No linked
executables or assets were downloaded.

- **S1:** W3C, [SVG 2 text](https://www.w3.org/TR/SVG2/text.html).
- **S2:** W3C, [SVG 2 rendering model](https://www.w3.org/TR/SVG2/render.html).
- **S3:** OpenRaster, [layer stack specification](https://www.openraster.org/baseline/layer-stack-spec.html).
- **S4:** OpenRaster, [file layout specification](https://www.openraster.org/baseline/file-layout-spec.html).
- **S5:** Krita, [OpenRaster interchange manual](https://docs.krita.org/en/general_concepts/file_formats/file_ora.html).
- **S6:** Adobe, [Photoshop file formats specification](https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/).
- **S7:** psd-tools, [supported and unsupported features](https://psd-tools.readthedocs.io/en/stable/).
- **S8:** Adobe, [InDesign developer overview and IDML](https://developer.adobe.com/indesign/).
- **S9:** Adobe, [save TIFF and layer options](https://helpx.adobe.com/photoshop/desktop/save-and-export/save-files/save-in-tiff-format.html) (official indexed page available; direct retrieval timed out).
- **S10:** libvips, [TIFF reader API 8.17](https://www.libvips.org/API/8.17/ctor.Image.tiffload.html).
- **S11:** Adobe, [PDF optional content layers](https://helpx.adobe.com/in/acrobat/desktop/edit-documents/pdf-layers/pdf-layers.html).
- **S12:** Adobe, [Photoshop PDF editing and export options](https://helpx.adobe.com/photoshop/desktop/save-and-export/save-files/save-as-photoshop-pdf.html).
- **S13:** qpdf, [scope and limitations](https://qpdf.readthedocs.io/en/stable/overview.html).
- **S14:** Scribus, [1.6.0 release and IDML import](https://wiki.scribus.net/canvas/1.6.0_Release) (official indexed text; direct retrieval bot-blocked).
- **S15:** Microsoft, [OpenType OS/2 metrics and embedding permissions](https://learn.microsoft.com/en-us/typography/opentype/spec/os2#fstype).
- **S16:** Linebender, [resvg scope, interfaces and license](https://github.com/linebender/resvg/blob/main/README.md).
- **S17:** Linebender, [resvg changelog](https://github.com/linebender/resvg/blob/main/CHANGELOG.md).
- **S18:** Inkscape, [license](https://inkscape.org/en/about/license/).
- **S19:** Inkscape, [platform overview](https://inkscape.org/cs/about/overview/).
- **S20:** Inkscape, [command-line interface](https://wiki.inkscape.org/wiki/Using_the_Command_Line).
- **S21:** Krita, [platforms and license declaration](https://krita.org/en/download/) and [license](https://krita.org/en/about/license/).
- **S22:** psd-tools, [MIT license](https://github.com/psd-tools/psd-tools/blob/main/LICENSE).
- **S23:** libvips, [project and license](https://www.libvips.org/).
- **S24:** qpdf, [license](https://qpdf.readthedocs.io/en/12.0/license.html).
- **S25:** Scribus, [official project distribution](https://sourceforge.net/projects/scribus/) and [distribution ownership](https://wiki.scribus.net/canvas/Download).
