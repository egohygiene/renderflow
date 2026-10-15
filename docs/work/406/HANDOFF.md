# #406 — composition planner and publication checkpoint

Base: composed PDF merged in
[PR #452](https://github.com/egohygiene/renderflow/pull/452) at
`024b06f83eebeeb2fbe2a6f9106936e77736a3f9`.
This is the **fifth authored, unqualified implementation checkpoint**.
The first checkpoint in #449, measured typography in #450, PNG in #451 and PDF
in #452 are historical. V5 adds an explicit shared-planner composition entry
point, locale-aware forest identities, common publication evidence and lazy
intermediate reuse within each locale. Canonical inputs remain editable and
unchanged at v1.

The maintainer authorized continuing the sprint and merging its implementation.
All executable qualification remains deferred. The coding workspace is
available again for source authoring and review. Local or remote Git content
identity is not compiler, runtime or visual qualification. No dependency changes
are introduced in this checkpoint.

## Authored scope

- `renderflow.page-composition/v1` and `renderflow.page-content/v1` remain the
  canonical contracts: physical dimensions, normalized layout, pinned artwork,
  stable layer/content IDs, reading order, font roles, locale revisions,
  explicit overrides and reconstruction provenance.
- Bounded immutable snapshots retain confined regular files, exact hashes,
  duplicate-JSON refusal, the inert SVG subset and bounded image headers.
  License files receive observed hashes because font registry v1 has no
  declared license-file digest.
- `composition inspect`, `plan` and `render` share the scoped SDK planner.
  Exact, subset and all-declared locale selectors fan out by locale/output;
  source/default languages use that same path. No implicit locale fallback.
- `planning::resolve_composition` accepts a dedicated
  `CompositionPlanningRequest { input, request }`. Its
  `ResolvedCompositionExecution` freezes a private snapshot and native plan,
  plus common execution and stage evidence. `planning::execute_composition`
  returns a native `CompositionRun` from that same resolved request; its
  `run_manifest()` method derives the common receipt.
- Locale/output targets join the common artifact forest using distinct IDs
  and relevant settings. The format graph is a projection; it cannot represent
  repeated-format locale targets by itself. Actual publications join the
  existing forest execution contract rather than being inferred from plans.
- Transcript and measured-SVG intermediates are created lazily on cache misses
  and reused within one locale. Only one locale's intermediate payloads are
  retained at a time. Actual stage observations, planned materialization counts,
  derivative cache hits and immutable input sharing remain separate evidence.
- Default targets remain SVG, HTML and transcript JSON. PNG and PDF are opt-in.
  Unsupported interchange targets remain explicit unavailable branches.
- HarfRust 0.14.0 and Skrifa 0.48.0 author measured glyph paths from captured
  static TTF/OTF fonts. The current visual scope is bounded Latin LTR horizontal
  text with explicit authored lines and whole-layer fallback. There is no
  installed-font discovery or browser-shaped page lettering.
- SVG/HTML serialize those measured paths and captured artwork. PNG uses
  resvg 0.48.1 with a closed resource resolver and no font feature. PDF reuses
  that same geometry and a separately planned opaque rasterization, then the
  existing lopdf 0.45.0 writer for one raw 8-bit DeviceRGB image per page.
  This checkpoint adds no dependency or external/native provider.
- Known shaped-advance/conservative-outline and line-box overflow, unsafe text
  bounds, clipping and unsupported font/script/writing modes remain explicit.
  No automatic wrapping, shrinking, translation or artwork repair occurs.
  Missing, unreviewed or stale content blocks that locale's branches.
- PDF settings independently declare DPI, opaque background, binding side,
  color policy and conformance. MediaBox/CropBox cover the full canvas;
  TrimBox/BleedBox preserve independent asymmetric geometry. Safe-area and
  gutter evidence is retained without drawing guides or shifting the page.
- Fresh and cached PDF candidates receive the same bounded structural
  admission against the generated envelope, expected page boxes, resource
  shape, draw matrix and exact raw RGB image length. The cache path does not
  feed arbitrary PDFs to a general parser or decompressor. These authored
  checks do not prove pixel appearance, source equivalence or print acceptance.
- Receipt-last bundles retain captured inputs, frozen plans, terminal records,
  hashes, parent IDs, review/loss declarations and observed cache outcomes.
  Shared immutable source storage is distinct from a derivative cache hit.
- The structural, measured-typography and mixed-asset fixture packages remain
  available. The structural WOFF2 stub is not a usable visual font. The latter
  packages retain the captured, unmodified DejaVu Sans font and full license.
  The mixed-asset package adds tiny authored RGBA PNG, unchanged synthetic
  JPEG/SVG, shapes and English/German text. The same package now has PDF command
  examples and authored regressions; no new rendered fixture or receipt is
  presented as evidence.

See [FORMAT_EVALUATION.md](FORMAT_EVALUATION.md) for the canonical decision and
historical checkpoint boundaries, [TYPOGRAPHY_PROVIDER.md](TYPOGRAPHY_PROVIDER.md)
for font policy, [RASTER_PROVIDER.md](RASTER_PROVIDER.md) for source image and
raster limits, [PDF_PROVIDER.md](PDF_PROVIDER.md) for the PDF envelope and
[PLANNER_INTEGRATION.md](PLANNER_INTEGRATION.md) for the shared-planner boundary.
Earlier font/raster lock additions were authored from official metadata without
Cargo resolution. Existing pins, source review and dependency reuse do not
establish a successful build.

## Contract and cache boundaries

Canonical inputs remain v1. Profile:
`renderflow.layered-composition/v5`. Plans/runs use
`renderflow.composition-plan/v5` and `renderflow.composition-run/v5`.
Cache/artifact identity is also versioned for this checkpoint; earlier
derivatives do not establish a v5 hit. Historical v1/v2/v3/v4 evidence schemas
remain available. No schema validator was run.

The CLI `composition plan` command now emits
`renderflow.composition-execution-plan/v1`, containing `schema`, `composition`
(native plan), `execution` (common plan) and `stages` (planned stage inventory).
Consumers of its earlier bare native output must read native fields under
`composition`. `inspect` still emits the native plan directly; `render` still
prints the native run receipt. Bundles retain both `composition-plan.json` and
the full wrapper at `execution-plan.json`, then native `composition-run.json`
and common `run-manifest.json` evidence. Stage inventory entries include `stage_id`,
`locale`, `kind`, `consumers`, `dependencies` and `expected_materializations`.
Native `shared_intermediates` entries retain `locale`, `kind`,
`materializations`, `reuse_count`, `sha256`, `size_bytes` and
`consumer_target_ids`; correlate them to the planned locale/kind. Access to
an intermediate does not establish a consumer's eventual publication. Native
run and branch timestamps record observed start/end times.

This is a dedicated composition entry point in the shared planning module,
not a `renderflow/v2` build-source addition or generic format-DAG route.
The ordinary v2 resolver and format graph remain unchanged. Flow has no
Renderflow adapter yet; the earlier reference to an existing released adapter
was incorrect. A future Flow-owned adapter must consume a qualified, immutable
Renderflow release and retain actual publication joins. No Flow repository
changes or downstream execution proof are included here.

`composition-run.json` maps every published branch to its exact
`artifact_manifest.artifacts` index, ID and bundle locator.
`inputs/<sha256>` maps logical roles to retained bytes. Keep the original
canonical package for editing and reloading its relative references; the hashed
input collection is evidence, not an automatic reconstruction of that layout.

All visual branches, including PDF, retain the same locale typography report.
The terminal record links its digest and provider. Private outline paths remain
in memory during execution; serialized reports are not executable replacement
geometry. The actual font is
`typography.layers[].selected_font`, with per-layer rejected candidates and
fallback index. Top-level `font_resolution` is generic registry context and can
differ from the candidate selected after whole-layer checks. Transcript branches
have no visual report and retain text/reading order independently.

`CompositionRequest.raster` defaults to 144 DPI and `transparent`.
An explicit object requires `dpi` and `background`; corresponding CLI flags
are `--raster-dpi` and `--raster-background`. Its effective
`renderflow.composition-raster/v1` plan is retained at `branch.raster` only
for PNG. Invalid settings block PNG only.

`CompositionRequest.pdf` defaults to these five fields:

| Setting | Default | Admitted policy |
| --- | --- | --- |
| `dpi` | `300` | Integer 36–600, subject to raster dimension/pixel caps |
| `background` | `#ffffff` | Opaque six-digit RGB hex, normalized lowercase |
| `binding_side` | `none` | `none`, `left`, `right`; nonzero gutter requires a side |
| `color_policy` | `assumed_srgb_no_profile_conversion` | Only this policy; no conversion or output intent |
| `conformance` | `none` | Only this value; no PDF/X claim |

Omitting the PDF object chooses defaults; an explicit object requires all five
fields. CLI flags use `--pdf-` followed by the corresponding hyphenated name.
Unsupported color/conformance strings and invalid PDF settings remain
PDF-only blocked-branch diagnostics. Invalid enum syntax is a CLI/input parse
error. `branch.pdf` retains the small
`renderflow.composition-pdf/v1` plan, nested raster plan, page-box arrays in
points, gutter and image-draw matrix. Terminal metadata retains the same plan.
A non-PDF branch carries no PDF plan. PNG settings do not affect PDF, and
PDF settings do not affect PNG/SVG/HTML/transcript branch identity.
V5 common target IDs explicitly include the locale and only relevant output
settings, independent of the full-package cache key described below.

PDF boxes convert normalized top-left coordinates to bottom-left PDF points
at the provider's f32 round-trip precision. The safe rectangle is evidence,
not an ArtBox or printed guide. Gutter means minimum total clearance from the
selected trim edge: safe must already satisfy it, and locale-effective text
bounds must remain safe. Binding choice does not add another inset, move art,
resize/reflow text, alternate page sides or infer a duplex book. The pipeline
renders a single page per locale; book assembly is separate.

Cache keys conservatively commit the entire manifest, all captured inputs,
selected fonts, typography evidence, output identity, effective target settings,
profile and engine version. One locale edit can invalidate other output keys
while immutable source bytes remain shared. A cache hit requires matching
parents, metadata, terminal identity, canonical payload path, digest, size and
the relevant PNG/PDF output contract. Linked internals and corrupt payloads are
refused. These checks are local integrity evidence, not authenticity against
a party replacing both records and payloads, and not proof of pixel fidelity.

## Bounds and output loss

The existing package caps remain: 128 MiB captured inputs, 64 MiB per output
and 512 MiB aggregate derivative outputs. Typography bounds remain:

- Per locale: 4,096 referenced text code points, 4,096 glyphs, 256 authored
  lines, 4 MiB private path bytes and 2 MiB compact report JSON.
- Per locale: 128 whole-layer font-candidate attempts and 128 MiB cumulative
  font-byte work. Repeated candidates/work count per use.
- Per request selecting visual output: 16,384 referenced code points across
  selected known locales, 16 MiB total distinct-locale compact reports and
  16 MiB total private paths. PDF/PNG/SVG/HTML share measurement per locale;
  serialized branches may repeat its report. Private paths share memory.
- Transcript-only requests bypass visual typography budgets. Oversized visual
  requests produce `composition.typography.plan_budget` before output writes.

PNG and PDF raster dimensions round upward from `millimeters × DPI / 25.4`;
each axis is at most 8,192 pixels and the canvas at most 16,777,216 pixels.
Rendering preserves uniform DPI; the extra fractional right/bottom pixel is
clipped to the physical page in PDF instead of anisotropically fitting pixels.
Opaque PDF rendering verifies all alpha bytes are fully opaque before stripping
them. The raw RGB payload is at most 48 MiB, with the full PDF under 64 MiB.
There is no image compression in this PDF profile; large files are expected.

Source raster images share those axis/pixel caps. Every repeated instance
counts toward 33,554,432 aggregate source pixels. Encoded assets are at most
16 MiB and decode buffers 64 MiB. A conservative 384 MiB estimate bounds active
pixel surfaces during resvg rendering, including output, nested isolated
groups/SVG, image decode and clip masks. It is not a total-process RSS,
allocator or wall-clock guarantee; PDF buffers and parser/encoder overhead are
not covered by that raster-surface estimate.

The PNG/JPEG admitted subsets and refusals from #451 remain in effect: bounded
8-bit PNG or grayscale/RGB JPEG; no ICC conversion, orientation repair, HDR,
animation or CMYK/YCCK. Detailed metadata rules remain in RASTER_PROVIDER.md.
The internal PNG has straight RGBA8, sRGB/perceptual and rounded density
metadata and passes the same authored decode checks as standalone PNG.

The delivered PDF is a flattened DeviceRGB image. No font is embedded; no
selectable/searchable lettering, tags, reading order, editable layers or vector
fidelity is claimed. Keep canonical text, transcript and applicable font notices.
Assumed sRGB is not an embedded ICC profile or output intent. There is no
CMYK/spot-color route, PDF/X, printer approval, grayscale PDF mode or inferred
paper color.

## Qualification and deliberate limits

Only source/documentation review and Git content identity were used.
All authored regressions, schemas, examples, CLI/SDK behavior and platform
handling remain **unrun**. No test, compiler/build, formatter/linter, schema
validator, fixture/CLI runner, renderer/native probe/conversion, asset decoder,
dependency download/installer, Cargo resolution or GitHub Actions was invoked.
Commit/merge messages retain `[skip ci]` under the sprint policy.

No pixel values, image appearance, PDF viewer result, print fit, byte
repeatability or platform consistency is established by source or fixture pins.
Bounded envelope admission is separate from general PDF conformance and visual
qualification. Conservative glyph bounds include Bézier control-point hulls,
which can overestimate curves and reject near-edge text.

The profile remains normal blend, rectangular clips, raster plates, bounded
vector art, rectangles/ellipses and static-font Latin horizontal LTR text.
WOFF/WOFF2, variable/color fonts, arbitrary masks/effects, broader scripts and
RTL/vertical visuals remain unsupported. Reviewed transcripts can retain
unsupported writing declarations; fixture RTL/vertical content is additionally
unreviewed. No OCR, inpainting, translation, art extraction or AI reconstruction
is introduced. Flattened content does not become an editable original.

## Continue #406 in this order

1. Admit canonical composition into the general `renderflow/v2` build contract,
   then implement the Flow-owned adapter once its immutable release
   prerequisites can be satisfied. The explicit
   shared-planner entry point, common forest identities and publication joins
   are now authored; they do not make either consumer path complete. Qualify
   actual intermediate reuse, all-hit behavior, target isolation and partial
   publication joins during the later execution session.
2. Extend typography through deliberate supported surfaces: broader scripts,
   bidi/vertical layout, variable/color/webfont handling and line breaking.
   Preserve canonical text and author intent; refuse unsupported requests.
3. Add justified interchange adapters with scoped round-trip evidence.
   OpenRaster is a raster-stack candidate; PSD/PSB/IDML need explicit
   text/mask/effect loss boundaries. Version native model extensions.
4. Expand print features only against concrete consumer requirements, including
   qualified color management/output intents, PDF conformance, book assembly
   and any vector/text-preserving route. The current single-page raster PDF
   does not satisfy those wider claims.
5. When authorized, execute source/CLI/cache/schema cases, followed by
   visual/typography, PDF geometry/content, adversarial and platform
   qualification, then #419's immutable release and consumer proof. Record
   findings on existing issues; no new audit issues were created.

#406 stays open and its acceptance checklist remains unchanged. Consumers
retain responsibility for artwork, translation, rights, accessibility and
publication approval. No consumer publication or Flow repository is changed
by this checkpoint.
