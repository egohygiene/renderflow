# Presentation build and forest checkpoint

November 7, 2026. Implementation base: Renderflow main
`4d887e95fbb43dc86167b7029e49595bb6704295`. #350 stays open.

`planning::build::resolve` now admits one `kind: presentation` source in v2.
The ordinary CLI build/inspect and public SDK use the same boundary. It captures
the model and bounded digest-pinned images once, including note exclusion, then
executes those bytes through the existing source/Slidev publisher. It never
reloads the source/config after resolution and introduces no second job engine.

Exact required targets are `presentation_source` (source ZIP),
`presentation_web` (web ZIP), and `pdf`. Canonical branch IDs bind the native
plan and effective policy; the forest identity additionally binds the effective
specification. Missing provider/browser or unqualified opt-in produces an
unavailable native branch and refuses execution before publication. Source-only
builds need no provider. One selected output failing fails this bounded batch;
there is no implicit partial branch-success or automatic fallback.

The existing fixed rendering dependencies are a common plan projection, not
general conversion routes. Quality zero and unit edge costs are unqualified
placeholders. Presets/options/selectors/custom transforms, conflicting policies,
resume and watch are refused. Future execution fields are rejected unless they
retain defaults. Local/offline flags remain intent; the native adapter does not
provide an OS/network/RSS/scratch sandbox and PDF uses loopback.

The native `bundle.json` remains intact. After native completion the adapter
reads and hashes the actual bounded published files, correlates each target
with its record index/ID/locator and writes `build-plan.json` then
`renderflow-run.json`. The common plan digest identifies compact plan bytes;
the source-spec digest identifies the effective normalized v2 spec. Native
receipt pins and note/rights/review declarations retain their existing meaning.
Auxiliary transcript/CSS/media files remain in the native inventory/output list;
the common forest names only the three requested primary publications. Original
model bytes are represented as a hash/size source record, not copied to a public
bundle containing private notes. Captured asset pins remain in the native plan.

Dry-run and pre-execution cancellation have no observed artifacts/publication.
Native failure or cancellation preserves partial files and returns an error;
it does not manufacture a common completion receipt. A filesystem error during
final common receipt writes can leave a completed native bundle without a
completed common build. No directory is overwritten and no publishing occurs.

Four focused regression cases are authored, uncompiled/unrun: closed admission,
exact target vocabulary/duplicates, frozen-source execution/publication joins,
and unavailable native target dry-run/refusal. No tests/compiler/build, Cargo
resolution/install, lint/format, schema/docs validators, product/native/provider/
browser execution or CI ran. No dependencies, lockfiles or corpus goldens changed.
Schema additions are authored directly, not generated or validated. Native,
visual/accessibility/reproducibility/platform and human product-content review
remain unobserved under #350/#458/#419. Source inclusion is not qualification.
