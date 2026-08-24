---
schema: aether.architecture-document/v1
id: renderflow-roadmap
title: Renderflow Roadmap
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-19
updated: 2026-08-24
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

<!-- BEGIN ROADMAP EXECUTION SNAPSHOT -->
<!-- roadmap-manifest
schema: hygiene.roadmap/v1alpha1
repository: egohygiene/renderflow
visibility: public
publication: composed
route: /roadmap/
updated: 2026-08-24
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
- Ready or planned steps without an issue are candidates for the private, duplicate-aware roadmap.issue-plan.json dry run.
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
