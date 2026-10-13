# #406 — layered/localized composition checkpoint

Base: merged #448 at `5afae5cc6023dac8bd64881bc9ed29a01be1df2e`.
Branch: `feat/renderflow-406-localized-composition`. Obtain the eventual merge
commit from the checkpoint PR. The maintainer authorized #406 under the ongoing
implementation sprint; all executable qualification remains deferred.

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
- SVG/HTML candidates embed captured art/fonts. Transcript JSON/text retain
  semantic IDs and reading order. Explicit line breaks remain unchanged.
  No automatic wrapping, shrinking, translation or art repair occurs.
- Known line-box overflow, unsafe text bounds, clipping and unsupported visual
  writing modes are actionable. Unreviewed/stale/missing content blocks the
  affected locale. Unavailable outputs remain visible.
- Receipt-last bundles retain captured inputs, frozen plans, terminal records,
  hashes, parent IDs, review/loss declarations and observed cache outcomes.
  Shared immutable source storage is distinct from derivative cache hits.
- A legally reusable two-panel fixture and source-only regressions are authored.
  Its 45-byte font is explicitly a structural stub, not a usable typeface or
  proof of glyph/visual fidelity. No successful receipts are checked in.

The direct `base64` dependency reuses the already locked 0.22.1 package. The
dependency edge and lockfile were edited without fetching or building.

## Contract and cache boundaries

Profile: `renderflow.layered-svg/v1`. Plans/runs use
`renderflow.composition-plan/v1` and `renderflow.composition-run/v1`.
This is a scoped composition interface, not a new `renderflow/v2` source,
general `ArtifactForest` extension or existing Flow adapter. The generic DAG
currently keys nodes by format; locale/output identity must be integrated
explicitly before two SVG locales can share that planner.

`composition-run.json` maps each published branch to its exact
`artifact_manifest.artifacts` index, ID and bundle locator. Inputs under
`inputs/<sha256>` map logical roles to retained bytes. This evidence layout
does not automatically reconstruct the original relative-path package; keep
the canonical package for editing and reloading.

Derivative cache keys conservatively commit the entire manifest, all captured
inputs, selected fonts, target identity, profile and engine version. One locale
edit can invalidate other derivative keys while source bytes remain shared.
A cache hit requires matching parent IDs, native metadata, terminal identity,
canonical payload path, actual digest and size. Corrupt records/payloads and
linked cache internals are rejected; no automatic repair is promised. Local
cache integrity checks are not an authenticity attestation against a party
replacing both records and payloads.

Limits: 64 MiB per output, 512 MiB aggregate derivatives and 128 MiB captured
input bytes. Output directories must be new. Partial/failed/cancelled runs
retain available evidence; early loading or publication I/O failures can occur
before a receipt exists. No generated candidate inherits publication approval.

## Qualification and deliberate limits

Source review and Git content identity checks only. Fixture authoring included
hashing original synthetic bytes. Regression cases, schemas, examples, CLI/SDK
behavior and platform handling are **unrun**. No tests, compiler/build,
formatter/linter, schema validator, native renderer/probe/conversion,
installer/download or GitHub Actions was invoked. Commit hooks remain disabled
and commit/merge messages use `[skip ci]` under the sprint policy.

Header/signature checks do not prove image/font decodability or glyph coverage;
source records carry scoped validation warnings. Serialized SVG/HTML bytes can
be deterministic while viewer appearance differs. Actual cmap coverage,
shaping, horizontal overflow, glyph ink, browser fallback, print readability,
ICC/color fidelity and cross-viewer rendering remain unverified. Nonzero gutter
intent lacks a binding side and receives an explicit unresolved warning.

The profile supports normal blend, rectangular clips, raster plates, a restricted
vector subset, rect/ellipse shapes and horizontal LTR text. Arbitrary masks,
editor effects, rich typography, PSD text and general SVG round trips are absent.
A flattened raster is not recovered into editable layers. AI reconstruction is
neither invoked nor registered. RTL/vertical visual layout is refused; reviewed
transcripts can retain those declarations. The fixture's RTL/vertical content
is additionally unreviewed and therefore blocked until explicitly reviewed.

## Continue #406 in this order

1. Adopt a pinned shaping/raster provider using the evaluation. Add actual
   glyph/fallback, line/ink overflow and locale typography diagnostics; select
   real redistributable font fixtures for the execution session.
2. Add composited PNG and qualified print-PDF routes, color/font/profile
   evidence and explicit gutter semantics. Existing image-only PDF publication
   can consume composed raster pages; it does not itself compose text.
3. Integrate locale/output identities with the general build planner, artifact
   forest and Flow's released contract. Qualify shared intermediate reuse;
   sharing source storage alone does not prove intermediate cache reuse.
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
