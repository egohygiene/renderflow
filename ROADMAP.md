---
schema: aether.architecture-document/v1
id: renderflow-roadmap
title: Renderflow Roadmap
kind: architecture-document
version: 0.1.2
status: draft
owners:
  - egohygiene
created: 2026-08-19
updated: 2026-09-25
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
