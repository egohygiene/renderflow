---
title: Document Specification — FOUNDATIONS.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - foundations
  - assumptions
  - axioms
---

## Introduction

This specification defines how the `FOUNDATIONS.md` architecture document shall
be authored, maintained, and validated.

`FOUNDATIONS.md` establishes the project's foundational assumptions and
architectural truths.

These are the concepts that every other architectural document may safely assume
without repeatedly redefining them.

Where `PRINCIPLES.md` explains **how decisions should be made**,
`FOUNDATIONS.md` defines **what is assumed to already be true**.

Every architectural decision should build upon these foundations.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `FOUNDATIONS.md` is to establish the project's architectural
axioms.

This document answers the question:

> **What truths does the architecture assume?**

This specification covers:

- foundational assumptions
- architectural axioms
- enduring truths
- baseline constraints
- invariant concepts
- core mental models

This specification does **not** cover:

- implementation
- architecture
- systems
- APIs
- workflows
- planning
- project goals

---

### 2. Definitions

- **Foundation**: A fundamental assumption upon which the architecture depends.
- **Architectural Axiom**: A statement assumed to be true without requiring
  repeated justification.
- **Invariant**: A property expected to remain true as the system evolves.
- **Mental Model**: A conceptual framework used to reason about the project.
- **Assumption**: A condition treated as true during architectural reasoning.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `FOUNDATIONS.md` shall define the project's architectural
  assumptions.
- **REQ-002**: Foundations shall be stable over time.
- **REQ-003**: Every foundation shall be implementation-independent.
- **REQ-004**: Foundations shall support the project's principles.
- **REQ-005**: Invariants shall be explicitly identified.
- **REQ-006**: Foundations shall be understandable without reference to source
  code.

#### Constraints

- **CON-001**: Product implementation shall not appear.
- **CON-002**: Temporary engineering decisions shall not become foundations.
- **CON-003**: Framework choices shall not become architectural truths.
- **CON-004**: Project planning shall not appear.

#### Guidelines

- **GUD-001**: Prefer timeless assumptions.
- **GUD-002**: Minimize the number of foundational statements.
- **GUD-003**: Prefer conceptual truths over implementation constraints.
- **GUD-004**: Document invariants explicitly.
- **GUD-005**: Avoid assumptions that may frequently change.

---

### 4. Authoring Contract

#### Purpose

Describe the assumptions and invariants that underpin the project's
architecture.

#### Responsibilities

`FOUNDATIONS.md` owns:

- architectural assumptions
- invariants
- mental models
- enduring truths
- baseline constraints

#### Non-Responsibilities

`FOUNDATIONS.md` does not own:

- implementation
- system organization
- architecture
- workflows
- APIs
- deployment
- planning

#### Inputs

Authoring should consider:

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- `METHODOLOGY.md`

#### Outputs

The foundations inform:

- `SYSTEM.md`
- `ARCHITECTURE.md`
- `DESIGN.md`
- engineering decisions
- AI reasoning

#### AI Generation Rules

When generating foundations:

- identify assumptions that should remain true throughout the project's life
- avoid implementation details
- avoid temporary decisions
- distinguish assumptions from principles
- preserve consistency with upstream documents

#### Validation

The resulting foundations should provide a stable conceptual base upon which the
remaining architecture can safely build.

---

### 5. Acceptance Criteria

- **AC-001**: Foundational assumptions are explicitly documented.
- **AC-002**: Architectural invariants are identified.
- **AC-003**: Foundations align with the project's principles.
- **AC-004**: No implementation details appear.
- **AC-005**: Downstream documents can reference foundations without redefining
  them.

---

### 6. AI Authoring Strategy

AI systems should construct the foundations by:

1. Reading all upstream architecture documents.
2. Identifying assumptions that remain true regardless of implementation.
3. Separating enduring truths from temporary decisions.
4. Defining architectural invariants.
5. Producing a stable conceptual foundation for the remainder of the
   architecture.

The resulting document should evolve slowly and only when the project's core
understanding changes.

---

### 7. Rationale & Context

Strong architectures are built upon explicit assumptions rather than implicit
beliefs.

By documenting foundational truths separately from architecture, systems,
implementation, and process, the project creates a stable reasoning framework
that remains valuable as technology evolves.

Foundations should change only when the project's understanding of itself
fundamentally changes.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- `METHODOLOGY.md`

#### Downstream Dependencies

- `SYSTEM.md`
- `ARCHITECTURE.md`
- `DESIGN.md`
- engineering standards
- AI architecture generation

---

### 9. Examples & Edge Cases

```text
Example

Foundation:

Knowledge should remain portable across tools.

Architectural Impact:

Every system is designed around open formats, deterministic exports, and
vendor-independent representations.
```

```text
Edge Case

An implementation framework requires violating a foundational assumption.

Expected:

Reevaluate the implementation before modifying the foundation.

Foundations should change only when the project's underlying architectural
beliefs evolve.
```

---

### 10. Validation Criteria

The completed foundations should satisfy the following:

- Assumptions are explicit.
- Invariants are documented.
- Foundations are stable.
- Implementation details are absent.
- Downstream architecture can reference them consistently.
- The document remains understandable without reading source code.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/foundation/methodology.spec.md`
- `.github/specs/architecture/foundation/system.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`
- `.github/specs/architecture/identity/principles.spec.md`
