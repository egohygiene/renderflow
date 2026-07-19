---
title: Document Specification — ROADMAP.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - roadmap
  - planning
  - strategy
---

## Introduction

This specification defines how the `ROADMAP.md` architecture document shall
be authored, maintained, and validated.

`ROADMAP.md` defines the long-term strategic evolution of the project.

It describes how the project is expected to mature over time through major
initiatives, architectural milestones, and capability growth rather than
individual implementation tasks.

Where `VISION.md` defines **where the project ultimately wants to go**,
`ROADMAP.md` defines **the strategic path toward that vision**.

The roadmap should communicate direction rather than deadlines.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `ROADMAP.md` is to communicate the strategic evolution of the
project.

This document answers the question:

> **How should this project evolve over time?**

This specification covers:

- strategic initiatives
- architectural evolution
- capability growth
- long-term priorities
- milestone sequencing
- future architectural direction

This specification does **not** cover:

- GitHub issues
- sprint planning
- implementation tasks
- release schedules
- deadlines
- project boards
- staffing
- resource allocation

---

### 2. Definitions

- **Roadmap**: A strategic plan describing the intended evolution of the
  project.
- **Initiative**: A significant body of work advancing the project's vision.
- **Milestone**: A meaningful architectural or product achievement.
- **Capability**: A high-level feature or competency provided by the project.
- **Phase**: A logical stage of long-term project evolution.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `ROADMAP.md` shall define the long-term strategic direction of
  the project.
- **REQ-002**: Major initiatives shall be described.
- **REQ-003**: Roadmap items shall align with the project's vision.
- **REQ-004**: Architectural evolution shall be explicitly represented.
- **REQ-005**: Capability growth shall be prioritized over implementation
  detail.
- **REQ-006**: The roadmap shall remain implementation-independent.

#### Constraints

- **CON-001**: GitHub issues shall not appear.
- **CON-002**: Sprint planning shall not appear.
- **CON-003**: Temporary implementation work shall not define the roadmap.
- **CON-004**: Calendar dates shall be avoided unless architecturally
  significant.
- **CON-005**: Technology choices shall not drive roadmap organization.

#### Guidelines

- **GUD-001**: Organize the roadmap around capabilities rather than tasks.
- **GUD-002**: Prefer phases over deadlines.
- **GUD-003**: Describe outcomes rather than implementation.
- **GUD-004**: Keep initiatives independent when possible.
- **GUD-005**: Preserve flexibility for future architectural change.

---

### 4. Authoring Contract

#### Purpose

Describe how the project should evolve over time.

#### Responsibilities

`ROADMAP.md` owns:

- strategic initiatives
- architectural milestones
- capability evolution
- long-term priorities
- future architectural direction

#### Non-Responsibilities

`ROADMAP.md` does not own:

- implementation tasks
- issue tracking
- sprint planning
- release planning
- engineering workflow
- project management

#### Inputs

Authoring should consider:

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- `FOUNDATIONS.md`
- `SYSTEM.md`
- `ARCHITECTURE.md`

#### Outputs

The roadmap informs:

- milestone planning
- GitHub issues
- implementation priorities
- engineering initiatives
- documentation evolution

#### AI Generation Rules

When generating a roadmap:

- begin with the project's vision
- think in capabilities rather than features
- organize work into logical phases
- avoid implementation detail
- preserve architectural flexibility
- prioritize strategic outcomes over schedules

#### Validation

The resulting roadmap should describe the project's future direction without
becoming an implementation plan.

---

### 5. Acceptance Criteria

- **AC-001**: Strategic initiatives are clearly defined.
- **AC-002**: Roadmap phases are logically ordered.
- **AC-003**: Architectural evolution is visible.
- **AC-004**: Implementation tasks are absent.
- **AC-005**: Roadmap items align with the project's vision.
- **AC-006**: The roadmap remains adaptable as implementation evolves.

---

### 6. AI Authoring Strategy

AI systems should construct the roadmap by:

1. Reading all upstream architecture documents.
2. Understanding the project's long-term vision.
3. Identifying major capabilities required to achieve that vision.
4. Organizing capabilities into strategic phases.
5. Avoiding implementation planning.
6. Producing a roadmap that can evolve without invalidating existing work.

The generated roadmap should communicate direction rather than execution.

---

### 7. Rationale & Context

Without a strategic roadmap, projects often become collections of unrelated
implementation tasks.

A roadmap provides architectural continuity by connecting today's work to the
project's long-term vision while allowing implementation details to change over
time.

The roadmap should evolve as understanding improves without becoming a project
management artifact.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- `FOUNDATIONS.md`
- `SYSTEM.md`
- `ARCHITECTURE.md`

#### Downstream Dependencies

- GitHub milestones
- GitHub issues
- implementation planning
- engineering priorities
- documentation strategy

---

### 9. Examples & Edge Cases

```text
Example

Phase 1

Establish reusable engineering foundations.

Capabilities

- Specification-first development
- Automated validation
- AI-assisted engineering

Architectural Impact

Future initiatives build upon a stable engineering platform.
```

```text
Edge Case

A high-priority implementation task appears that does not align with the
documented roadmap.

Expected

Evaluate whether the roadmap should evolve before allowing tactical work to
redefine long-term strategy.
```

---

### 10. Validation Criteria

The completed roadmap should satisfy the following:

- Strategic direction is explicit.
- Capabilities are prioritized.
- Phases are logically ordered.
- Architectural evolution is visible.
- Implementation details are absent.
- The roadmap remains understandable independently of project management
  tooling.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/identity/vision.spec.md`
- `.github/specs/architecture/foundation/foundations.spec.md`
- `.github/specs/architecture/foundation/system.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`
