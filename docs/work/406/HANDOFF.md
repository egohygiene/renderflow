# #406 — measured typography checkpoint

Base: first composition checkpoint merged in
[PR #449](https://github.com/egohygiene/renderflow/pull/449) at
`8f60d6f4398232f6956091012f533eb2908dfd35`.
Branch: `feat/renderflow-406-measured-typography`. Obtain the eventual PR and
merge commit from this checkpoint's publication. The maintainer authorized
continuing #406 under the ongoing implementation sprint; all executable
qualification remains deferred. The first checkpoint's browser-text previews
are historical; v2 authors measured glyph paths.

## Authored scope

- [Format/tool evaluation](FORMAT_EVALUATION.md): SVG, OpenRaster, PSD/PSB,
  TIFF, PDF, IDML and a native manifest with immutable assets.
- `renderflow.page-composition/v1` and separate `renderflow.page-content/v1`:
  physical dimensions, normalized layout, asset pins, layer/content IDs, reading
  order, typography, locale revisions/overrides and reconstruction provenance.
- Bounded immutable SDK snapshots: confined regular files, exact hashes,
  duplicate JSON refusal, inert SVG subset and bounded image headers. The
  existing font registry now resolves captured bytes without changing ordinary
  filesystem-loaded behavior. License files receive observed hashes because
  font registry v1 has no declared license-file hash.
- `composition inspect`, `plan`, and `render` share the scoped SDK planner.
  Exact, subset and all-declared selectors fan out by `(locale, output)`;
  source/default languages use the same path. No implicit locale fallback.
- Pinned HarfRust 0.14.0 and Skrifa 0.48.0 author shaping, glyph lookup and
  outline extraction from captured static TTF/OTF fonts. The current visual
  scope is bounded Latin LTR horizontal text with explicit authored lines.
  Whole-layer fallback follows the declared role order and records decisions;
  there is no implicit installed-font or browser-font selection.
- SVG/HTML candidates serialize the measured glyph paths and captured artwork.
  They do not use browser-shaped text or font-face CSS for page lettering.
  The canonical composition retains editable text; transcripts retain semantic
  IDs and reading order. Outlined preview lettering loses native SVG text-object
  editability/searchability. HTML includes a separate readable transcript.
- Known shaped-advance/conservative-outline and line-box overflow, unsafe text bounds, clipping
  and unsupported font/script/writing surfaces are actionable. No automatic
  wrapping, shrinking, translation or art repair occurs. Unreviewed/stale/missing
  content blocks the affected locale; unavailable outputs remain visible.
- `renderflow.composition-typography/v1` evidence and its stable digest connect font selection and
  measured geometry with branch/cache identity. It records provider results
  without turning this unrun checkpoint into passing qualification evidence.
- Receipt-last bundles retain captured inputs, frozen plans, terminal records,
  hashes, parent IDs, review/loss declarations and observed cache outcomes.
  Shared immutable source storage is distinct from derivative cache hits.
- Two separate two-panel packages and source-only regressions are authored.
  The original 45-byte WOFF2 stub remains explicitly structural and fails the
  measured visual provider. The new `measured-typography` package retains
  unmodified DejaVu Sans TTF bytes from local `fonts-dejavu-core` 2.37-8, the
  full packaged license, hashes and capture provenance. English/German are
  synthetic reviewed declarations; Arabic/Japanese remain unreviewed blocked
  cases. No font tool was run, and no successful receipts are checked in.

The [provider decision](TYPOGRAPHY_PROVIDER.md) records dependency identities,
the bounded API surface and qualification gaps. Its six new dependency nodes
and lockfile entries were authored from official registry/source metadata;
the lockfile was not resolved or validated by Cargo. The earlier direct
`base64` dependency remains the already locked 0.22.1 package. No dependency
build or runtime qualification is implied by pinning versions.

## Contract and cache boundaries

Canonical inputs remain `renderflow.page-composition/v1` and
`renderflow.page-content/v1`. Profile: `renderflow.layered-svg/v2`. Plans/runs
use `renderflow.composition-plan/v2` and `renderflow.composition-run/v2`.
The version change separates outlined measured previews and their evidence
from v1 browser-text outputs; old derivative cache records do not establish a
v2 hit.
This is a scoped composition interface, not a new `renderflow/v2` source,
general `ArtifactForest` extension or existing Flow adapter. The generic DAG
currently keys nodes by format; locale/output identity must be integrated
explicitly before two SVG locales can share that planner.

`composition-run.json` maps each published branch to its exact
`artifact_manifest.artifacts` index, ID and bundle locator. Inputs under
`inputs/<sha256>` map logical roles to retained bytes. This evidence layout
does not automatically reconstruct the original relative-path package; keep
the canonical package for editing and reloading.

Visual plan branches retain `typography`; the run includes that same plan.
Terminal metadata links the report's digest and provider identity. Private SVG
path geometry remains in memory during execution while its identity is carried
by the report; serialized evidence is not a replayable replacement for captured
inputs and the supported provider. Transcript branches carry no visual report.
`typography.layers[].selected_font` is authoritative for the actual rendered
font, with rejected candidates and the selected fallback index on each layer.
Top-level `font_resolution` is the generic registry report for context only;
it can select a different candidate before whole-layer coverage/shaping checks.

Derivative cache keys conservatively commit the entire manifest, all captured
inputs, selected fonts, typography evidence, target identity, profile and engine
version. One locale edit can invalidate other derivative keys while source bytes
remain shared.
A cache hit requires matching parent IDs, native metadata, terminal identity,
canonical payload path, actual digest and size. Corrupt records/payloads and
linked cache internals are rejected; no automatic repair is promised. Local
cache integrity checks are not an authenticity attestation against a party
replacing both records and payloads.

Limits: 64 MiB per output, 512 MiB aggregate derivatives and 128 MiB captured
input bytes. Typography adds these bounds:

- Per locale across referenced text layers: 4,096 text code points, 4,096 shaped
  glyphs, 256 authored lines, 4 MiB private path bytes and 2 MiB compact report
  JSON. Repeated glyph outlines count per occurrence.
- Per locale: 128 whole-layer font-candidate attempts and 128 MiB cumulative
  font-byte work. Attempts include the same candidate used on different layers;
  work charges policy/shaping use repeatedly, not only unique file sizes.
- Any request selecting a visual target: 16,384 referenced text code points
  across distinct selected known locales, including spaces and LF delimiters.
  The request also caps their distinct-locale compact reports at 16 MiB and
  private outline paths at 16 MiB. SVG/HTML branches may each serialize the same
  report, but it is charged once per locale; private paths share an `Arc`.
- Transcript-only requests bypass visual typography budgets. An oversized
  visual request returns `composition.typography.plan_budget` before output
  writes and recommends selecting fewer locales.

Output directories must be new. Partial/failed/cancelled runs
retain available evidence; early loading or publication I/O failures can occur
before a receipt exists. No generated candidate inherits publication approval.

## Qualification and deliberate limits

Source review and Git content identity checks only. Fixture authoring included
hashing synthetic bytes and the copied font/license. Regression cases, schemas,
examples, CLI/SDK behavior and platform handling are **unrun**. No tests, compiler/build,
formatter/linter, schema validator, native renderer/probe/conversion,
dependency download/installer or GitHub Actions was invoked. Official source,
index metadata and documentation were read. Commit hooks remain disabled
and commit/merge messages use `[skip ci]` under the sprint policy.

Loader header/signature checks do not prove full image/font decodability;
source records carry scoped validation warnings. The separate measured
typography provider authors actual selected-font coverage and shaped geometry
checks, but those code paths and their serialized reports have not been run or
qualified. No all-script coverage, complex-script correctness, pixel identity,
print readability, ICC/color fidelity or cross-viewer result is established.
Conservative outline bounds include the Bézier control-point hull, which can
overestimate the curve and reject near-edge text; they are not exact curve
bounds or observed raster ink.
Nonzero gutter intent lacks a binding side and receives an unresolved warning.

The profile supports normal blend, rectangular clips, raster plates, a restricted
vector subset, rect/ellipse shapes and bounded static-font Latin horizontal LTR
text. WOFF/WOFF2, variable fonts, unsupported scripts and writing modes are not
enabled by changing a filename or registry declaration. Arbitrary masks,
editor effects, rich typography, PSD text and general SVG round trips are absent.
A flattened raster is not recovered into editable layers. AI reconstruction is
neither invoked nor registered. RTL/vertical visual layout is refused; reviewed
transcripts can retain those declarations. The fixture's RTL/vertical content
is additionally unreviewed and therefore blocked until explicitly reviewed.

## Continue #406 in this order

1. Add composited PNG and qualified print-PDF routes, color/font/profile
   evidence and explicit gutter semantics. Existing image-only PDF publication
   can consume composed raster pages; it does not itself compose text. A pinned
   raster provider and actual output qualification are still required.
2. Integrate locale/output identities with the general build planner, artifact
   forest and Flow's released contract. Qualify shared intermediate reuse;
   sharing source storage alone does not prove intermediate cache reuse.
3. Extend typography only through explicit supported surfaces and evidence:
   broader scripts, bidi/vertical layout, variable/color/webfont handling and
   automatic line breaking remain separate work. Keep the canonical text and
   author intent; do not silently fit or substitute unsupported content.
4. Add justified layered interchange adapters and scoped round-trip evidence.
   OpenRaster can serve raster stacks; PSD/PSB/IDML need explicit text/mask/effect
   loss boundaries. Extend the native model through versioned contracts.
5. When authorized, execute the authored source/CLI/cache/schema cases, then
   visual/typography, adversarial and platform qualification, followed by #419's
   immutable release and consumer proof. Record findings on existing issues;
   no new audit issues were created during this checkpoint.

#406 stays open. Consumers retain their art, translations, rights, accessibility
decisions and publication approval. No consumer page, translation, external
publication or Flow repository was changed.
