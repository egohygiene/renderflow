# #406 — composition planning and publication integration

This is the fifth **authored, unqualified** #406 checkpoint. It follows composed
PDF in [PR #452](https://github.com/egohygiene/renderflow/pull/452), merged at
`024b06f83eebeeb2fbe2a6f9106936e77736a3f9`. It connects the bounded composition
engine to shared planning and artifact-forest evidence without adding a new
provider, dependency or execution engine.

## Why a separate composition entry point

The existing generic graph identifies nodes by format. A composition can emit
English SVG and German SVG with different content, review state and output
identity. Collapsing both to a single SVG node loses the information a consumer
needs to select or publish either page.

`planning::resolve_composition` therefore accepts
`CompositionPlanningRequest { input, request }`, where `request` is the native
`CompositionRequest`. Its `ResolvedCompositionExecution` retains the captured
composition snapshot and native plan privately. It exposes a common execution
plan and a locale-aware stage inventory alongside native evidence. Execution
uses this resolved object rather than reloading mutable source paths or
accepting a replacement plan from the caller.

`planning::execute_composition` borrows the resolved value and accepts an
output path, optional cache path and cancellation token. It returns a native
`CompositionRun`; the run's `run_manifest()` method derives the common run
manifest. The common manifest joins actual native publications
to the existing forest execution contract. This is a dedicated entry point in
the shared planning module; the generic `renderflow/v2` resolver and format DAG
remain unchanged. A canonical composition manifest is not newly admitted as a
generic v2 build source.

## Contract versions and CLI boundary

| Surface | Contract |
| --- | --- |
| Editable composition/content | Existing `renderflow.page-composition/v1` and `renderflow.page-content/v1` |
| Native provider profile | `renderflow.layered-composition/v5` |
| Native plan and run | `renderflow.composition-plan/v5` and `renderflow.composition-run/v5` |
| CLI plan wrapper | `renderflow.composition-execution-plan/v1` |
| Cache and terminal artifact identity | V5; earlier derivative records do not establish a v5 hit |
| PNG and PDF provider evidence | Existing bounded raster/PDF provider contracts |

Historical v4 PDF evidence and earlier v1/v2/v3 evidence remain available.
Canonical package files do not need a version migration for this checkpoint.

The `composition plan` wrapper contains `schema`, `composition` (the native
plan), `execution` (the common plan) and `stages`. Each planned stage records
`stage_id`, `locale`, `kind`, `consumers`, `dependencies` and
`expected_materializations`. Existing consumers of the bare native plan must
read native fields under `composition` when using this command.

`composition inspect` continues to print the native plan directly. It uses
all declared locales and default SVG/HTML/transcript-JSON targets.
`composition render` prints the native v5 run receipt. The bundle retains:

| File | Meaning |
| --- | --- |
| `composition-plan.json` | Frozen native plan, provider/layout evidence and branch diagnostics |
| `execution-plan.json` | Full execution-plan wrapper with native plan, common plan and stage inventory |
| `composition-run.json` | Native branch outcomes, artifact evidence and actual intermediate observations |
| `run-manifest.json` | Common run evidence with actual publication joins |

Plans and expected output paths are not proof of publication. Preserve the
receipts and retained inputs with successful outputs, including partial runs.

## Target identity and cache identity

The common artifact forest keeps a target for each locale/output branch.
Locale and settings relevant to that output enter its target identity. PNG
settings cannot change PDF, SVG, HTML or transcript target IDs; PDF settings
cannot change PNG, SVG, HTML or transcript target IDs. A format-only graph is
a projection for common presentation, not the authoritative target inventory.
Unavailable or blocked branches remain visible with their diagnostics instead
of disappearing from the inventory.

The projected graph's unit costs and zero quality scores are conservative
structural placeholders, not measured runtime, fidelity or route optimization.
Waves express dependencies, not observed parallelism. The common forest uses
the existing `cache_only` intermediate-policy value, with the explicit
`ephemeral_per_locale` storage declaration in its policy. No intermediate is
retained as a published artifact or inserted into a persistent intermediate
cache; planned-stage metadata does not synthesize such artifact records.

Target identity and derivative cache identity have different jobs. The cache
still conservatively commits the full captured canonical package, source
hashes, typography evidence, target identity, provider profile and effective
settings. Editing one locale can invalidate another locale's cache key.
Shared immutable artwork storage does not establish intermediate execution
reuse, and a target ID alone does not establish that a cached payload is valid.

Existing cache admission still checks terminal records, parents, settings,
payload locator, digest and size, plus the relevant bounded PNG/PDF admission.
Native diagnostics and review/loss declarations remain available to the common
publication record. This is local integrity evidence, not authenticity against
a party replacing both cache records and payloads.

## Lazy intermediate materialization

The composition executor groups selected branches by locale. It creates a
transcript or measured SVG only when an uncached branch needs that
intermediate. Other consumers within the same locale reuse that value. It
retains one locale's intermediate payloads at a time rather than retaining all
locales' serialized intermediates throughout the run.

This concerns execution intermediates. Typography planning still measures
once per selected locale and retains bounded private geometry/evidence under
the existing request limits. It does not mean all captured inputs or all
planned typography are reduced to one locale at a time. PDF and PNG retain
their own provider settings and still perform their distinct output work.

Read the evidence according to what it records:

| Evidence | Interpretation |
| --- | --- |
| Planned stage and expected materializations | Work that can be shared if an uncached consumer needs it |
| Actual stage observations | What execution reports was materialized or reused |
| Derivative cache hit | An admitted terminal payload supplied that branch without rendering it anew |
| Shared retained input hash | Identical captured bytes stored once; no intermediate-execution claim |
| Actual published artifact join | A successful publication associated with a specific planned target |

An all-hit locale need not materialize either intermediate. Blocked consumers
and failures before materialization do not establish that a stage ran. A
planned count or repeated stage ID must not be promoted to an observed reuse
claim. These execution paths have been authored and source-reviewed, not run
in this checkpoint.

Native `CompositionRun.shared_intermediates` entries contain `locale`, `kind`,
`materializations`, `reuse_count`, `sha256`, `size_bytes` and
`consumer_target_ids`. Match these to planned stages by locale and kind.
`materializations` records successful preparation; `reuse_count` records
subsequent access by another attempted target. Consumer IDs are not publication
IDs: a provider or publication can fail after accessing a stage. The transcript
digest covers pretty JSON plus the final newline; the SVG digest covers the
exact UTF-8 bytes supplied to visual adapters. Native run and branch
`started_at_unix_ms` and `completed_at_unix_ms` fields record observed timings.
The common manifest carries the same stage observations in
`composition.shared_stage.observed` diagnostics, with the correlated
`stage_id` and `in_memory_only` storage declaration. They remain observations
about run-local preparation, not independently published intermediate artifacts.

## Publication and downstream boundary

Common forest execution evidence retains the actual association between a
locale/output target and its published artifact identity, manifest index,
bundle locator, digest and branch outcome. Missing or failed branches do not
inherit publication from successful siblings. Consumers must preserve review
flags and losses instead of treating supported output syntax as publication
approval.

The common projection validates structured receipt correlation; it does not
reread filesystem payloads, prove visual quality or implement Flow's acceptance
boundary. Its execution-plan digest commits the exact wrapper bytes at
`execution-plan.json`, including captured input pins and selected branches.
Its source-spec digest identifies the canonical manifest bytes only. Consumers
must still verify actual published payloads and retain the native receipt.

Flow currently has no Renderflow adapter. The
[Flow handoff](../../architecture/flow-artifact-forest.md) describes a future
consumer boundary, not a released integration. A Flow-owned adapter must consume
a qualified immutable Renderflow release, preserve target/publication joins
and map only actual admitted artifacts into Flow's contract. This checkpoint
does not change `flow.artifact/v1`, import Renderflow source into Flow, add an
adapter or establish downstream compatibility.

## Qualification and next work

The coding workspace is available for source changes in this checkpoint.
Tests, builds, formatting/lint, schema validators, CLI/native/render execution,
asset decoders, Cargo resolution/downloads and CI remain deferred under the
sprint policy. No dependency changes are introduced. Authored regressions,
schemas and command examples are unrun; source review does not prove runtime
reuse, cache behavior, output appearance or platform compatibility.

The next bounded implementation is general v2 build admission, followed by the
Flow-owned adapter with its release prerequisites explicit. Broader typography,
native interchange and print extensions remain separate work. The later test
session must cover actual stage reuse, all-hit and mixed-hit execution,
relevant-setting identity, partial publication joins, schema compatibility,
source mutation after planning, output rendering and platform behavior before
#419's release and downstream proof.

#406 remains open with its acceptance checklist unchanged. See
[HANDOFF.md](HANDOFF.md) for retained provider limits and checkpoint history.
