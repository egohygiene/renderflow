---
title: Document Specification — PILLARS.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - pillars
  - strategy
  - identity
---

## Introduction

This specification defines how the `PILLARS.md` architecture document shall be
authored, maintained, and validated.

`PILLARS.md` defines the enduring strategic themes that support the project's
purpose, vision, and long-term success.

Each pillar represents a capability or area of sustained investment that should
remain important throughout the lifetime of the project.

Where `PRINCIPLES.md` explains **how decisions are made**, `PILLARS.md`
identifies **what areas the project continually invests in to fulfill its
mission**.

The pillars should provide long-term strategic focus without prescribing
implementation.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `PILLARS.md` is to define the project's enduring strategic
pillars.

This document answers the question:

> **What enduring capabilities must remain strong for the project to succeed?**

This specification covers:

- strategic pillars
- long-term investment areas
- enduring capabilities
- organizational priorities
- strategic focus
- project identity

This specification does **not** cover:

- implementation
- architecture
- systems
- project planning
- GitHub issues
- milestones
- engineering workflow
- product roadmap

---

### 2. Definitions

- **Pillar**: A long-lived strategic capability that supports the project's
  mission.
- **Strategic Capability**: A capability requiring continuous investment over
  time.
- **Strategic Focus**: A recurring area of attention that influences long-term
  decisions.
- **Investment Area**: A domain in which the project intentionally commits
  ongoing effort.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `PILLARS.md` shall define the project's strategic pillars.
- **REQ-002**: Each pillar shall support the project's purpose and vision.
- **REQ-003**: Pillars shall represent enduring capabilities rather than
  temporary initiatives.
- **REQ-004**: Pillars shall remain implementation-independent.
- **REQ-005**: Pillars shall be mutually distinguishable.
- **REQ-006**: Every major initiative should reinforce one or more pillars.

#### Constraints

- **CON-001**: Pillars shall not describe implementation.
- **CON-002**: Temporary projects shall not become pillars.
- **CON-003**: Technology choices shall not define pillars.
- **CON-004**: Pillars shall not duplicate principles.
- **CON-005**: Pillars shall not become roadmap items.

#### Guidelines

- **GUD-001**: Prefer a small number of enduring pillars.
- **GUD-002**: Define pillars in human language.
- **GUD-003**: Prefer capabilities over features.
- **GUD-004**: Ensure every pillar supports the project's vision.
- **GUD-005**: Keep pillars stable as implementation evolves.

---

### 4. Authoring Contract

#### Purpose

Describe the strategic capabilities that the project continually strengthens.

#### Responsibilities

`PILLARS.md` owns:

- strategic pillars
- enduring capabilities
- long-term investment areas
- strategic focus
- capability priorities

#### Non-Responsibilities

`PILLARS.md` does not own:

- implementation
- architecture
- systems
- engineering methodology
- project roadmap
- GitHub issues
- milestones

#### Inputs

Authoring should consider:

- `PURPOSE.md`
- `VISION.md`
- `MANIFESTO.md`
- `PRINCIPLES.md`

#### Outputs

The pillars inform:

- `ROADMAP.md`
- `SYSTEM.md`
- strategic initiatives
- investment decisions
- architectural priorities
- product direction

#### AI Generation Rules

When generating project pillars:

- begin with the project's purpose and vision
- identify enduring strategic capabilities
- avoid implementation details
- distinguish pillars from principles
- avoid creating pillars for temporary initiatives
- ensure each pillar supports long-term project success

#### Validation

The resulting pillars should provide strategic direction while remaining stable
across multiple generations of implementation.

---

### 5. Acceptance Criteria

- **AC-001**: Strategic pillars are clearly defined.
- **AC-002**: Every pillar supports the project's vision.
- **AC-003**: Pillars remain implementation-independent.
- **AC-004**: Pillars are distinct from principles and systems.
- **AC-005**: The collection of pillars represents the project's long-term
  strategic focus.
- **AC-006**: Future initiatives can be aligned to one or more pillars.

---

### 6. AI Authoring Strategy

AI systems should construct the project pillars by:

1. Reading all upstream identity documents.
2. Understanding the project's purpose and vision.
3. Identifying the enduring capabilities required for success.
4. Eliminating temporary initiatives.
5. Distinguishing strategic capabilities from implementation.
6. Producing a stable collection of long-term pillars.

The generated document should communicate enduring strategic priorities rather
than execution plans.

---

### 7. Rationale & Context

Projects often lose focus as new opportunities, technologies, and priorities
emerge.

Clearly defined pillars provide continuity by identifying the strategic
capabilities that deserve sustained investment regardless of changing
implementation details.

They help align architecture, roadmaps, engineering decisions, and future
initiatives around a shared long-term direction.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`
- `MANIFESTO.md`
- `PRINCIPLES.md`

#### Downstream Dependencies

- `ROADMAP.md`
- `SYSTEM.md`
- strategic planning
- architecture
- investment priorities

---

### 9. Examples & Edge Cases

```text
Example

Pillar:

Open Knowledge

Purpose:

Ensure knowledge remains portable, transparent, and accessible across tools.

Strategic Impact:

Future systems, architecture, and roadmaps consistently reinforce open
standards and user ownership.
```

```text
Edge Case

A proposed pillar reflects a temporary technology trend.

Expected:

Do not adopt it.

Pillars should represent enduring strategic capabilities rather than current
implementation priorities.
```

---

### 10. Validation Criteria

The completed pillars should satisfy the following:

- Strategic capabilities are explicit.
- Pillars align with the project's vision.
- Pillars remain implementation-independent.
- Every pillar is distinct.
- Future initiatives can be mapped to one or more pillars.
- The document remains stable as technologies evolve.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/identity/purpose.spec.md`
- `.github/specs/architecture/identity/vision.spec.md`
- `.github/specs/architecture/identity/manifesto.spec.md`
- `.github/specs/architecture/identity/principles.spec.md`
- `.github/specs/architecture/foundation/roadmap.spec.md`
