---
schema: aether.architecture-document/v1
id: renderflow-roadmap
title: Renderflow Roadmap
kind: architecture-document
version: 0.1.10
status: draft
owners:
  - egohygiene
created: 2026-08-19
updated: 2026-10-13
governed_by:
  - architecture-roadmap
depends_on:
  - renderflow-vision
  - renderflow-pillars
  - renderflow-architecture
  - renderflow-decisions
related:
  - renderflow-purpose
  - renderflow-principles
  - renderflow-manifesto
  - renderflow-epistemology
supersedes: []
---

# Renderflow Roadmap

## 2026-10-13 measured composition typography checkpoint

The first [#406](https://github.com/egohygiene/renderflow/issues/406) checkpoint
merged in [PR #449](https://github.com/egohygiene/renderflow/pull/449) at
`8f60d6f4398232f6956091012f533eb2908dfd35`. The next checkpoint, on
`feat/renderflow-406-measured-typography`, authors measured previews using
pinned HarfRust 0.14.0 and Skrifa 0.48.0, captured static fonts, explicit
whole-layer fallback, and shaped line/conservative-outline diagnostics.

The native composition/content contracts remain v1. The v2 visual profile
serializes measured glyph paths; editable text and reading order remain in the
canonical package and transcripts. Plan/run evidence and cache identity carry
the typography result. The bounded visual scope is Latin LTR horizontal text,
static TTF/OTF and authored line breaks; unsupported scripts, font surfaces,
writing modes and known overflow remain explicit. No automatic wrapping or
shrinking is introduced.

A [real-font fixture](examples/page-composition/measured-typography/README.md)
retains unmodified DejaVu Sans, its license and byte provenance; the original
structural stub remains a negative visual case. All code, fixtures and evidence
contracts remain source-reviewed and unqualified. Tests, builds, lint, schema
validation, font/native/visual execution and CI remain deferred. The checkpoint
does not close any #406 acceptance checkbox.

Next implementation work is composed PNG/print PDF, color and gutter intent,
locale/output integration with the general artifact planner, and justified
native interchange. Broader typography and actual visual/platform/release
qualification remain open. See the [provider decision](docs/work/406/TYPOGRAPHY_PROVIDER.md),
[composition guide](docs/user-guide/page-composition.md) and
[handoff](docs/work/406/HANDOFF.md). #412/#413, #429 and #419 retain the later
fixture/toolchain/release qualification work.

## 2026-10-13 layered and localized composition checkpoint (historical)

[#430](https://github.com/egohygiene/renderflow/issues/430) merged its authored
integration checkpoint in [PR #448](https://github.com/egohygiene/renderflow/pull/448)
at `5afae5cc6023dac8bd64881bc9ed29a01be1df2e`. The current
[#406](https://github.com/egohygiene/renderflow/issues/406) checkpoint introduces
a bounded native page-composition contract, immutable artwork/content/font
snapshots, and a shared SDK/CLI locale-and-output planner.

The [composition guide](docs/user-guide/page-composition.md) covers authored SVG,
HTML preview and transcript branches, exact publication evidence, shared source
storage and verified derivative cache reuse. Source/default languages follow the
same path as other declared locales. Content revisions, human review, layout
overrides and known geometry failures remain explicit. The
[format evaluation](docs/work/406/FORMAT_EVALUATION.md) records the canonical choice.

This first vertical slice was unqualified. Tests, builds, lint, schema validation,
native/visual execution and CI remain deferred. The included font is a clearly
marked structural stub, not a usable typeface. Actual glyph coverage, shaping,
ink bounds, measured line breaking, raster/print output, native interchange and
general artifact-DAG integration remain open within #406. No acceptance checkbox
is closed by the merge. The [handoff](docs/work/406/HANDOFF.md) orders those next
pieces; #412/#413, #429 and #419 retain the later fixture/toolchain/release work.

## 2026-10-13 multi-family forest and Flow handoff checkpoint (historical)

[#429](https://github.com/egohygiene/renderflow/issues/429) reconciliation
merged in [PR #447](https://github.com/egohygiene/renderflow/pull/447) at
`1853824691041491bc6284c9e67c59c6ecda7a48`. The current
[#430](https://github.com/egohygiene/renderflow/issues/430) checkpoint authors
per-target publication evidence, an isolated multi-family CLI fixture, a
generated creative capability matrix shared with doctor, and the
[Flow consumption contract](docs/architecture/flow-artifact-forest.md).

The forest now distinguishes selected intent from actual target publications;
sources, intermediates and rejected candidates cannot count as produced
derivatives. Native evidence retains hashes, exact producers, losses and review
requirements. The [fixture guide](docs/user-guide/creative-forest.md) groups
separate source-family jobs without pretending that incompatible policies form
one native graph.

This is authored integration, not an executed proof. Tests, builds, lint,
schema validation, native execution and CI remain deferred. General
Fountain-to-native/canonical exports and graph plain-transcript text output
remain unsupported; #430's broader scenario must retain these gaps until
implemented or explicitly narrowed. #430 and #421 remain open for that scope
reconciliation, native fixture/CLI-SDK evidence and verified release consumption.
The [handoff](docs/work/430/HANDOFF.md) records the remaining work.

After this integration checkpoint, #406 is the next planned implementation lane
for layered/localized composition. #412/#413 fixture expansion, #429 exact
toolchain qualification and #419 verified release remain aligned with the later
deep test session. No new audit issues are created before actual findings.

## 2026-10-13 creative toolchain reconciliation checkpoint (historical)

[#422](https://github.com/egohygiene/renderflow/issues/422) merged in
[PR #445](https://github.com/egohygiene/renderflow/pull/445), followed by
[#426](https://github.com/egohygiene/renderflow/issues/426) in
[PR #446](https://github.com/egohygiene/renderflow/pull/446) at
`8e0f5f4a6132364965cc31bbee9c3e72074dfd4b`. Recovery now has authored,
review-required PDF/text routes, optional pinned OCR, evidence bundles, and
bounded native body exports. It does not promise original screenplay structure
or page layout.

The current [#429](https://github.com/egohygiene/renderflow/issues/429)
checkpoint reconciles the ten-provider doctor schema and plain-doctor scope,
refreshes generated tool documentation, and adds manual platform setup guidance.
The [qualification record](docs/work/429/QUALIFICATION.md) records the exact
configuration contracts and the runtime evidence still missing. Tests, builds,
lint, native probes/conversions, schema validation, and CI remain deferred;
merged implementation is not a qualified release.

The next implementation issue is #430 for cross-family artifact forest and
Flow contract integration, followed by #406 for layered/localized composition.
#412/#413 fixture work and #419 release qualification belong to the later
testing and release pass. #429 stays open for actual toolchain qualification.

## 2026-10-12 derivative-forest implementation checkpoint (historical)

[#422](https://github.com/egohygiene/renderflow/issues/422) now has authored
cycle-safe route selection, stable target identities, constrained provider/transform
selection, distinct rendition execution groups, and versioned CLI/SDK forest evidence.
Ordinary exact requests retain optimization-first ranking; exhaustive/profile requests
prefer cumulative fidelity and fewer conversions. Unsupported and unavailable targets
remain diagnostics rather than promised output paths. Native screenplay, speech, and
collection policies retain their bounded route restrictions.

At this checkpoint the implementation was a draft, not a qualified release. Tests, compilation,
linting, native probes/conversions, schema validation, and CI were explicitly deferred.
The [forest contract](docs/architecture/derivative-forest.md) and
[checkpoint handoff](docs/work/422/HANDOFF.md) record the policy and remaining evidence.
#422 stays open for the later deep test session. The subsequent #445 and #446
merges are recorded above.

## 2026-09-28 integration-candidate release review handoff

[#418](https://github.com/egohygiene/renderflow/issues/418) merged in
[PR #436](https://github.com/egohygiene/renderflow/pull/436) at
`ff6a76f4bad17541d6554e514ccb345f7c3d3e3c`, and #414 was reconciled
and closed. [#419](https://github.com/egohygiene/renderflow/issues/419) owns
the first verified, immutable integration candidate for
[Flow #52](https://github.com/egohygiene/flow/issues/52).

The proposed version is `v0.3.0-rc.1`; it is not released by a version edit
or a review branch. A maintainer merge, exact green commit, manually created
annotated tag, staged artifact checks, independent clean-install smoke, and
verified prerelease are the gates. Initial binary scope is Ubuntu 24.04 x86_64
GNU (glibc 2.39 or newer); other distribution baselines are unverified.
Other binaries and package-manager channels are unverified or unpublished for
this candidate. Historical unsigned `v0.2.1` is preserved without retagging.
The [release guide](docs/release-candidate.md) records exact contracts,
provider/platform limits, provenance and signing status, rollback, and
compromised-release response. Flow consumes only the published immutable
version and digest; no Renderflow source import or automatic #52 start is
implied by this branch.

## 2026-09-28 fixed-layout EPUB validation review handoff

[#417](https://github.com/egohygiene/renderflow/issues/417) merged in
[PR #435](https://github.com/egohygiene/renderflow/pull/435) at
`eaf3b98d4236a74ba30b6e40456cbf9757675a76`. Its in-process packager
builds deterministic, bounded fixed-layout EPUB from an explicitly ordered
homogeneous PNG or JPEG collection. SVG and fixed-layout KEPUB remain
unsupported. The #418 validation suite uses synthetic artwork only.

[#418](https://github.com/egohygiene/renderflow/issues/418) is the current
review checkpoint: inspect the generated package independently, distinguish
native structural results from optional EPUBCheck v5 results and unavailable
providers, exercise positive and adversarial synthetic fixtures, and align the
capability surfaces with that exact route. Missing or incomplete accessibility
evidence cannot be promoted to a claim of complete accessibility. A generated
EPUB still needs its selected reading-system and distribution-channel review.

After #418 review and merge, reconcile and close parent
[#414](https://github.com/egohygiene/renderflow/issues/414) with commit and
workflow evidence. [#419](https://github.com/egohygiene/renderflow/issues/419)
then owns the immutable integration-candidate release for
[Flow #52](https://github.com/egohygiene/flow/issues/52). The release is a
separate checkpoint; this branch does not publish or approve it.

## 2026-09-28 fixed-layout EPUB implementation handoff

[#417](https://github.com/egohygiene/renderflow/issues/417) adds the exact
`ebook.generate.epub.fixed-layout` route to the canonical ordered-collection
planner. The initial native packager accepts only homogeneous local PNG or JPEG
pages with frozen IDs, digests, order, and geometry. A declared publication
contract supplies title, issue identity, date, language, contributor, rights,
cover selection, per-page descriptions, and accessibility metadata. Explicit
LTR/RTL progression never mirrors source artwork. SVG is refused until safe
parsing and fixture evidence support it. Deterministic package members, bounded
inputs/output, retained run provenance, and unchanged source files are the #417
review boundary.

After #417 review and merge,
[#418](https://github.com/egohygiene/renderflow/issues/418) independently
validates fixed-layout EPUB 3.3 output and advertises exact capability truth;
[#419](https://github.com/egohygiene/renderflow/issues/419) packages the
immutable Renderflow integration candidate for
[Flow #52](https://github.com/egohygiene/flow/issues/52). Neither this route nor
the first three-input [#433](https://github.com/egohygiene/renderflow/issues/433)
gallery claims fixed-layout KEPUB, retailer acceptance, complete accessibility,
or publication approval. Later gallery expansion remains tracked by #412.

## 2026-09-28 artifact gallery review handoff

[#433](https://github.com/egohygiene/renderflow/issues/433) is the first bounded
[#412](https://github.com/egohygiene/renderflow/issues/412) corpus checkpoint.
Its explicit local runner creates HTML from one synthetic Markdown input and a
two-page print PDF from two synthetic RGB PNG inputs through the public CLI and
real providers. It retains both builds and compares the outputs to reviewed
oracles, independent PDF structure evidence, and typed run manifests. The normal
workspace suite checks the corpus contract without treating missing external
providers as a pass. Later fixture expansion remains in #412. The independent
fixed-layout EPUB lane [#417](https://github.com/egohygiene/renderflow/issues/417)
remains the next production capability after this review; #433 does not claim
EPUB or physical print validation.

## 2026-09-28 print-interior PDF review handoff

Checkpoint #416 adds the exact `publication.generate.pdf.interior` capability
through canonical ordered-collection planning and execution. The initial route
uses homogeneous local PNG or JPEG pages, explicit uniform trim geometry and
bleed, the observed img2pdf 0.6.3 provider, bounded direct argv, and independent
PDF page/box/image-stream inspection before materialization. Synthetic tests
cover the two-page route, a generated 44-page recipe, refusal, interruption,
immutable source bytes, and reproducible clean builds. The tool and adapter
catalogs advertise only this bounded route; generic image aggregation remains
experimental. No real publication pages, printer, retailer, or upload were used.

After review and merge, #417 may proceed independently for fixed-layout EPUB.
#418 independently validates that EPUB capability; #419 releases the exact
integration candidate consumed by Flow #52. PDF physical proof and retailer
acceptance remain outside #416.

## 2026-09-27 ordered collection review handoff

Checkpoint #415 adds canonical planning and execution for one explicitly
ordered, homogeneous local artifact collection. Members require stable IDs,
root-relative locators, declared formats/media types, and SHA-256 digests;
optional geometry enters plan identity. Planning freezes member evidence,
execution rechecks paths and bytes, and run evidence records source order and
aggregate lineage. The two-page synthetic fixture and generated 44-member
recipe prove the public SDK and CLI paths without claiming PDF or EPUB output.

After review and merge, #416 (print PDF) and #417 (fixed-layout EPUB) may
proceed independently. #418 validates the EPUB capability, and #419 publishes
the immutable release consumed by Flow #52. No real publication artifacts or
personal media were processed by the #415 tests.

## 2026-09-25 live suite handoff

> [!IMPORTANT]
> This handoff supersedes the older 2026-08-24 current-gate text below where it
> conflicts. The live production path is owned by #367 and #414–#419. Re-query
> live issue, release, and CI state before starting a branch.

### Immediate production path

1. [#415](https://github.com/egohygiene/renderflow/issues/415) — make explicitly
   ordered immutable artifact collections first-class inputs through canonical
   planning, execution, validation, cache/checkpoint identity, and provenance.
2. After #415, execute these sibling consumers independently:
   - [#416](https://github.com/egohygiene/renderflow/issues/416) — deterministic
     print-interior PDF generation.
   - [#417](https://github.com/egohygiene/renderflow/issues/417) — deterministic
     EPUB 3.3 fixed-layout generation.
3. [#418](https://github.com/egohygiene/renderflow/issues/418) — independently
   validate fixed-layout EPUB output and publish exact capability truth.
4. [#419](https://github.com/egohygiene/renderflow/issues/419) — publish the
   first immutable Renderflow integration-candidate release with clean-install,
   checksum, provenance/SBOM, compatibility, and distribution evidence.
5. Hand that immutable release to
   [Flow #52](https://github.com/egohygiene/flow/issues/52) for the production
   adapter and later static-publication proof.

```text
#415
  ├──→ #416 print PDF ──┐
  └──→ #417 EPUB ──→ #418
                       └──→ #419 ──→ Flow #52
```

### Parallel and downstream lanes

- [#406](https://github.com/egohygiene/renderflow/issues/406) remains the
  parallel/later layered-composition and localization lane. It is not required
  for the default-locale ordered-page PDF/EPUB exporters, but it is required
  before claiming localization-ready publication.
- [#412](https://github.com/egohygiene/renderflow/issues/412) and
  [#413](https://github.com/egohygiene/renderflow/issues/413) follow the
  production exporters as broader adversarial and reusable synthetic-comic
  fixture coverage; they do not block #415–#419.
- [#420](https://github.com/egohygiene/renderflow/issues/420), the HTTP API/job
  service, is not on the first Flow integration critical path.
- [#421](https://github.com/egohygiene/renderflow/issues/421) owns the
  new **artifact-forest** lane: cycle-safe exhaustive derivative planning,
  screenplay interchange/recovery, timed-text extraction, local speech
  transcription, and a downstream Flow handoff. It is valuable for the
  exhaustive comic workflow but does **not** block the first #415–#419
  publication release.
- #397, #379, #378, #350, and #349 remain valid product/profile work but are
  not prerequisites for the first immutable Flow-consumable Renderflow release.
- [#409](https://github.com/egohygiene/renderflow/issues/409) is the
  post-roadmap repository/backlog/Identity audit after the active production and
  release work is reconciled.

The goal of this lane is not to finish every Renderflow idea before integration.
It is to release the smallest truthful production surface that lets Flow consume
ordered page collections and obtain validated print-PDF and fixed-layout-EPUB
artifacts without importing Renderflow source.


### Artifact-forest / creative-derivative lane

[#421](https://github.com/egohygiene/renderflow/issues/421) decomposes the
broader universal-format promise into an executable, cycle-safe creative
artifact forest:

On 2026-10-12, the maintainer approved landing the implementation sprint and
deferring the deep test pass. #442, #443, and #444 merged into `main` in that
order. #441 supplies the doctor integration, reconciled against those adapters
with one registry entry per tool and explicit configured-only Node/afterwriting
availability. Merging does not turn authored coverage into passing evidence.

| Existing issue | Implementation status | Remaining lane |
| --- | --- | --- |
| #423 | Canonical screenplay model merged in #442 | Deferred acceptance/fixture qualification |
| #424 | Three bounded native imports merged in #443 | Deferred direction/platform qualification |
| #425 | Fountain PDF, analysis, and policy fix merged in #444 | Deferred runtime, two-run, and visual qualification |
| #429 | Doctor in #441; recovery/schema/setup reconciliation merged in #447 | Exact fixture/release toolchains and platform qualification |
| #422 | Cycle-safe identities, route groups, and versioned forest evidence merged in #445 | Tests/build/native qualification deferred |
| #426 | Review-required PDF/text recovery and bounded native body exports merged in #446 | Recovery, OCR, native-application, and platform qualification |
| #430 | Per-target publication evidence, multi-family fixture and Flow handoff authored | Native integration proof, missing direction decisions, CLI/SDK and released-contract qualification |

After the #430 implementation checkpoint, #406 has an authored bounded
composition SDK/CLI slice and a measured-typography follow-up. Composed
raster/print output, general planner integration, native interoperability and
broader typography remain its implementation lane. Keep #412/#413
fixture expansion and #419 verified release work aligned with the later deep
testing session. Existing open qualification issues retain the evidence gaps;
no new audit issues are created before that session.

```text
#422 derivative-forest planner policy
  │
  ├──────────────┐
  │              │
#423 screenplay  #427 timed text
  │              │
  ├→ #424        └→ #428 local speech transcription
  ├→ #425
  └→ #426
       │
       └──────────────┐
                      ↓
                  #429 toolchain
                      ↓
                  #430 integration proof
                      ↓
               Flow exhaustive-comic lane
```

The initial screenplay family is Fountain, FDX, FadeIn and OSF through a
provider-neutral screenplay artifact model. `scripttool` and
`afterwriting` are provider candidates, not core dependencies. PDF-to-script
recovery is explicitly lossy/review-required. Timed-text work distinguishes
embedded subtitle extraction from speech recognition.

The first #423 checkpoint authors `renderflow.screenplay/v1`, ordered semantic
elements and title metadata, explicit dialogue relationships, source-specific
extensions, and directional loss evidence. Typed FDX/FadeIn/OSF/canonical JSON
formats are registered, Fountain's incorrect Pandoc conversion claim is
removed, and `screenplay inspect` validates canonical JSON without an external
provider. Native screenplay formats were recognition-only in that checkpoint.
Original synthetic fixtures and source records are authored; compilation,
tests, schema/native qualification, and CI remain deferred. This does not close
#423 or provide a released converter. Resume from
[the #423 handoff](docs/work/423/HANDOFF.md).

The #424 checkpoint authors three explicitly pinned scripttool imports:
FDX, OSF, and Fade In to Fountain. Independent readers compare a restricted
body-only profile before a standalone bundle or graph artifact is published.
Reverse native writers are rejected based on upstream source findings;
provider JSON is a separate rejected derivative candidate, never canonical
screenplay JSON. Version, binary/source/output digests, argv identity, validation,
and losses accompany accepted results. The implementation is merged in #443
and remains unqualified; no tests, builds, native conversions, or CI have run.
Keep #424 open for the deferred qualification pass. See
[the #424 handoff](docs/work/424/HANDOFF.md) and
[provider evaluation](docs/work/424/PROVIDER_EVALUATION.md).

The #425 checkpoint authors a separate Fountain → PDF capability through an
explicitly pinned Node runtime and afterwriting package installation. Versioned
A4/US Letter profiles use Courier base14 and an ASCII body-only source subset.
Independent source reports count screenplay elements and cues; independent PDF
inspection records pages, geometry, fonts, and variable metadata. No afterwriting
analytics CLI, native FDX input, visual-fidelity guarantee, or reproducible-byte
claim is invented. It merged in #444 after #424/#423 with all executable checks
deferred; #425 stays open for native and two-run qualification. See
[the #425 handoff](docs/work/425/HANDOFF.md).

The #427 candidate authors a standalone `subtitles inspect/convert/extract`
CLI, `renderflow.timed-transcript/v1`, strict plain-text SRT/WebVTT codecs,
and bounded FFprobe/FFmpeg adapters. Its second checkpoint adds opt-in
`execution.subtitles` graph/SDK routes through a canonical transcript artifact,
parser-backed validation, and native manifest provenance/loss evidence.
The third checkpoint authors an opt-in native qualification kit with synthetic
fixtures and per-case evidence for the later completion pass.
Checkpoint 4 passed an independent native-command smoke matrix, but the Rust
completion pass is blocked by an unavailable toolchain. The Rust adapter/SDK
and qualification harness remain unvalidated; released support and completion
are not claimed. Executed scope is recorded in
[the validation record](docs/work/427/VALIDATION.md). Resume from
[the #427 handoff](docs/work/427/HANDOFF.md). The user has deferred remaining
tests and authorized dependent implementation work without claiming acceptance.

The first #428 checkpoint authors a standalone local WAV speech-transcription
route with explicit executable/model hashes, segment normalization, and a
separate speech-evidence envelope around the unchanged timed-transcript model.
One recognition result produces JSON/SRT/WebVTT/text plus retained provider
output and a bundle receipt. It has not been compiled, tested, or run with a
model. Checkpoint 2 adds pinned local audio inspection, explicit stream/source
selection, bounded PCM preparation, and retained WAV/probe lineage in the
standalone bundle. Prepared transcript clocks start at audio sample zero;
original-video synchronization is not claimed. Both checkpoints remain unrun.
Checkpoint 3 authors explicit graph/SDK speech policy, one recognition edge
feeding subtitle derivatives, retained evidence artifacts, phase events, and
typed cancellation. Speech and descendants deliberately bypass generic cache/checkpoint
reuse; provider availability stays unobserved until execution. These changes
also remain uncompiled/unrun. Checkpoint 4 adds explicit digest-pinned replay of
portable saved results, including complete retained-evidence checks and SDK
resume assessment. An offline
timing assessment exposes strict-WAV sample clocks and refuses original-media
mapping for prepared audio; it does not shift timestamps. Checkpoint 5 adds an
opt-in speech result index that reuses completely verified historical results
from the existing artifact store. Missing entries permit fresh recognition;
damaged entries fail without native fallback. Generic DAG caches/checkpoints
stay disabled for speech, and no fresh-inference reproducibility is claimed.
Checkpoint 6 separately scopes [#440](https://github.com/egohygiene/renderflow/issues/440):
an explicit pinned time map can translate whole cues from a verified prepared-media
result into original-media subtitle candidates. The offline operation records
operator-supplied timing in a separate envelope and new output bundle; it refuses
unmapped or boundary-crossing cues and does not infer synchronization. Original
recognition artifacts and timeline assessment remain unchanged. Compilation,
runtime qualification, and measured synchronization evidence remain deferred.
Resume from
[the #428 handoff](docs/work/428/HANDOFF.md).

On 2026-10-12, PR #438 and PR #439 merged in dependency order with explicit
maintainer authorization. The implementation is on `main`; #427, #428, and
#440 remain open for their deferred qualification work. Merge does not establish
acceptance or release readiness.

The #429 checkpoint supplies the optional `creative-publishing` doctor profile,
independent tool observations, separate route/configuration status, and a JSON
observation fingerprint. Its ten-provider inventory includes #424/#425 and
#426 capabilities, with pinned Poppler extraction/rasterization and a virtual
recovery provider. Node and afterwriting remain unprobed by generic
discovery: their PDF route requires explicit pinned configuration and its own
preflight. Doctor reports the implemented screenplay routes as authored and
unqualified. Recovery execution owns its exact configured tool/data pins; global
discovery does not attest to them. Real fixture/release versions, platform
qualification, and execution checks remain outstanding; #429 is not complete. Resume
from [the #429 handoff](docs/work/429/HANDOFF.md) and
[profile guide](docs/user-guide/creative-toolchain.md),
[setup guide](docs/user-guide/creative-toolchain-setup.md), and
[qualification record](docs/work/429/QUALIFICATION.md).

The exhaustive planner must materialize each eligible target identity at most
once, suppress source-format regeneration unless explicitly requested, avoid
repeated format nodes inside a path, and retain provider/fidelity/provenance
evidence. This makes reversible edges such as Fountain ↔ FDX safe without
weakening the general transformation graph.

This lane should feed Flow only through a released/versioned Renderflow
contract. Flow owns project-level orchestration and resumability; Renderflow
continues to own static conversion/extraction/provider semantics.

<!-- BEGIN ROADMAP EXECUTION SNAPSHOT -->
<!-- roadmap-manifest
schema: hygiene.roadmap/v1alpha1
repository: egohygiene/renderflow
visibility: public
publication: composed
route: /roadmap/
updated: 2026-09-25
-->
## 2026-08-24 execution snapshot

> This evidence-reconciled snapshot is the issue-generation and visual-roadmap handoff. The longer-horizon strategy below remains canonical context; generated HTML, JSON, progress, issue plans, and commit lists are projections.

**Lifecycle:** functional alpha  
**Current gate:** Restore the full CI and documentation publication matrix, including pnpm setup ordering and Snapcraft schema compatibility.  
**North-star outcome:** A spec-driven, extensible Rust rendering engine with stable plugin boundaries and reproducible multi-format output.

### Visual roadmap publication

**Mode:** `composed`  
**Route:** `/roadmap/`  
**Current publication evidence:** GitHub Pages documentation and configured package/release channels; latest docs and overall CI are red, and no GitHub release was observed.

Compose dist/roadmap/ into the repository's existing final site artifact at /roadmap/. The current Pages workflow remains the only deployer.

### Quest line

<!-- roadmap-step
id: REN-Q01
status: complete
depends_on: []
issues: []
-->
#### REN-Q01 — Build the modular rendering engine

**State:** `complete`  
**Depends on:** None

**Outcome:** Core, CLI, and plugin-SDK crates provide a substantial rendering implementation.

**Exit criteria:**

- [x] Core rendering and plugin boundaries exist as separate crates.
- [x] Representative builds and tests pass.

**Current evidence:**

- PR #335 merged at c03a370012dd18795fcc8c3437b6bd61f82c566b on 2026-07-31.
- PR #339 merged at 1ca651f1a691e07cebacc082a410226abacffcd1 on 2026-08-01.

<!-- roadmap-step
id: REN-Q02
status: blocked
depends_on: [REN-Q01]
issues: []
-->
#### REN-Q02 — Recover CI and documentation publication

**State:** `blocked`  
**Depends on:** `REN-Q01`

**Outcome:** All supported Rust, web, docs, packaging, and benchmark workflows report truthfully and pass.

**Exit criteria:**

- [ ] pnpm is enabled before use and the web job passes.
- [ ] Snapcraft uses an accepted schema and the latest docs deployment is green.

**Current evidence:**

- Rust build/test and the 2026-08-24 scheduled benchmark were green.
- Overall CI was red because pnpm was used before enablement and Snapcraft rejected override-install.

<!-- roadmap-step
id: REN-Q03
status: planned
depends_on: [REN-Q02]
issues: []
-->
#### REN-Q03 — Publish the first verified release

**State:** `planned`  
**Depends on:** `REN-Q02`

**Outcome:** README release promises resolve to an immutable, tested engine and CLI release.

**Exit criteria:**

- [ ] A tagged GitHub release contains checksums and supported artifacts.
- [ ] Installation and smoke tests succeed from release artifacts.

**Current evidence:**

- README links release and package channels, but no GitHub release was observed.

<!-- roadmap-step
id: REN-Q04
status: ready
depends_on: [REN-Q02]
issues: [344, 345]
-->
#### REN-Q04 — Harden derivative media adapters

**State:** `ready`  
**Depends on:** `REN-Q02`

**Outcome:** EPUB/KEPUB and HandBrake/Aniflow work use explicit adapter contracts rather than core coupling.

**Exit criteria:**

- [ ] Issue #344 passes EPUB/KEPUB fixtures.
- [ ] Issue #345 proves the HandBrake/Aniflow boundary with integration tests.

**Current evidence:**

- Issues #344 and #345 opened on 2026-08-19.

<!-- roadmap-step
id: REN-Q05
status: planned
depends_on: [REN-Q03, REN-Q04]
issues: []
-->
#### REN-Q05 — Stabilize the plugin SDK and integrate Flow

**State:** `planned`  
**Depends on:** `REN-Q03`, `REN-Q04`

**Outcome:** Third-party plugins and Flow orchestration rely on a versioned, compatibility-tested SDK.

**Exit criteria:**

- [ ] SDK compatibility policy and fixtures cover supported versions.
- [ ] A Flow-driven render can resume and link its output evidence.

**Current evidence:**

- Architecture PR #346 merged at 9534c2fe536210107f6c26de0087e2c9cdd1be7a on 2026-08-20.
- No stable plugin-SDK release or Flow integration proof was observed.

### Roadmap-to-issue handoff

- A step is complete only when its exit criteria and required evidence are satisfied; commit count never determines progress.
- Ready steps without an issue are candidates for the private, duplicate-aware roadmap.issue-plan.json dry run. Planned steps remain preview-only unless a reviewer explicitly opts them in with issue_policy: propose.
- Issue creation or reconciliation requires human approval or an explicitly authorized Pace operation and returns issue references through a reviewable roadmap pull request.
- Pull requests and commits should include Roadmap-Step: <ID>; historical evidence may be linked through existing issue and pull-request relationships.
- Public rendering uses only allowlisted build-time evidence and never places a GitHub token or private issue plan in the browser artifact.

<!-- END ROADMAP EXECUTION SNAPSHOT -->

## Strategic context

This roadmap describes capability evolution, not promised dates or an issue queue. Sequence follows architecture dependencies and may change when evidence or risk changes.

## Phase 1: Consolidate stable contracts

**Outcome:** A bounded capability advances from documented intent to validated, independently usable behavior.

**Exit signals:**

- The owning contract and acceptance criteria are versioned.
- Implementation and documentation agree.
- Relevant tests and safety checks pass.
- Downstream consumers and migration impact are understood.
- Remaining uncertainty is visible.

## Phase 2: Harden transforms and evidence

**Outcome:** A bounded capability advances from documented intent to validated, independently usable behavior.

**Exit signals:**

- The owning contract and acceptance criteria are versioned.
- Implementation and documentation agree.
- Relevant tests and safety checks pass.
- Downstream consumers and migration impact are understood.
- Remaining uncertainty is visible.

## Phase 3: Expand plugin and SDK boundaries

**Outcome:** A bounded capability advances from documented intent to validated, independently usable behavior.

**Exit signals:**

- The owning contract and acceptance criteria are versioned.
- Implementation and documentation agree.
- Relevant tests and safety checks pass.
- Downstream consumers and migration impact are understood.
- Remaining uncertainty is visible.

## Phase 4: Integrate with the wider Flow suite

**Outcome:** A bounded capability advances from documented intent to validated, independently usable behavior.

**Exit signals:**

- The owning contract and acceptance criteria are versioned.
- Implementation and documentation agree.
- Relevant tests and safety checks pass.
- Downstream consumers and migration impact are understood.
- Remaining uncertainty is visible.

## Cross-cutting tracks

- Security, privacy, accessibility, licensing, and provenance.
- Documentation, architecture portals, examples, and onboarding.
- Packaging, release, compatibility, and self-hosting.
- Organization integration through explicit contracts.
- Observatory evidence and Pace conformance when those systems exist.

## Deferred direction

Optional managed services, enterprise controls, marketplaces, and the conversational organization compiler remain later architecture work. Current choices should preserve portability and avoid foreclosing them.

## Evidence and uncertainty

- **Observed:** The repository README and checked-in implementation establish a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts.
- **Decided for this draft:** The repository owns the bounded concern described here and participates through versioned contracts.
- **Proposed:** Target systems and later roadmap phases remain proposals until accepted and implemented.
- **Open question:** Which parts of this draft should become active in the first independently versioned release?
