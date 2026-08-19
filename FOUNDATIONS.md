---
schema: aether.architecture-document/v1
id: renderflow-foundations
title: Renderflow Foundations
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-19
updated: 2026-08-19
governed_by:
  - architecture-foundations
depends_on:
  - renderflow-purpose
  - renderflow-principles
  - renderflow-epistemology
related:
  - renderflow-vision
  - renderflow-pillars
  - renderflow-manifesto
  - renderflow-ai-constitution
supersedes: []
---

# Renderflow Foundations

## Foundational assumptions

- The repository owns one bounded concern and integrates through explicit contracts.
- Human authority, privacy, safety, provenance, and accessibility are architectural constraints.
- Canonical source is distinguishable from generated output and transient state.
- Observed, desired, proposed, and accepted states remain distinguishable.
- Validation evidence must be reproducible closely enough to support review.
- Standalone usefulness is preserved even when the wider organization adds value.
- Self-hosting and portability are supported without pretending external compute, storage, domains, or providers are free.

## Enduring constraints

- Do not make mutable default branches or unpublished internal APIs cross-repository dependencies.
- Do not put secrets, private source material, or provider credentials in generated architecture or distribution artifacts.
- Do not let convenience erase approval, rollback, or provenance at consequential boundaries.
- Do not claim cross-platform, self-hosted, or production support beyond verified evidence.

## Trust boundaries

Repository source, generated artifacts, local state, external providers, organization automation, and user-controlled infrastructure are distinct trust zones. Every crossing requires an explicit data, authority, and failure contract.

## Success properties

The foundation is healthy when Declarative rendering contracts, Graph planning and execution, Extensible transform ecosystem, Publication-grade evidence remain independently testable and their ownership is clear.

## Evidence and uncertainty

- **Observed:** The repository README and checked-in implementation establish a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts.
- **Decided for this draft:** The repository owns the bounded concern described here and participates through versioned contracts.
- **Proposed:** Target systems and later roadmap phases remain proposals until accepted and implemented.
- **Open question:** Which parts of this draft should become active in the first independently versioned release?
