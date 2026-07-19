---
title: Document Specification — SYSTEM.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - systems
  - decomposition
  - engineering
---

## Introduction

This specification defines how the `SYSTEM.md` architecture document shall be
authored, maintained, and validated.

`SYSTEM.md` defines the major systems that comprise the project. It establishes
the project's highest-level decomposition into cohesive systems, each with a
well-defined purpose and responsibility.

Where `ONTOLOGY.md` defines **what exists** within the project domain,
`SYSTEM.md` defines **what systems exist** to fulfill the project's purpose.

Where `ARCHITECTURE.md` explains how those systems are organized and interact,
`SYSTEM.md` identifies the systems themselves.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `SYSTEM.md` is to identify the project's major systems and define
their responsibilities.

This document answers the question:

> **What systems make up this project?**

This specification covers:

- system identification
- system responsibilities
- system boundaries
- system ownership
- system purpose
- system capabilities
- conceptual decomposition

This specification does **not** cover:

- implementation
- architecture
- APIs
- deployment
- infrastructure
- module organization
- workflows
- repository structure

---

### 2. Definitions

- **System**: A cohesive collection of capabilities that fulfills a distinct
  responsibility.
- **Capability**: A high-level function provided by a system.
- **Responsibility**: The architectural concern owned by a system.
- **Boundary**: The separation between systems and their responsibilities.
- **System Decomposition**: The process of dividing the project into cohesive,
  loosely coupled systems.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `SYSTEM.md` shall identify every major system within the project.
- **REQ-002**: Every system shall have a clearly defined purpose.
- **REQ-003**: Every system shall have explicitly defined responsibilities.
- **REQ-004**: System boundaries shall be documented.
- **REQ-005**: Capabilities shall be assigned to exactly one primary system.
- **REQ-006**: System descriptions shall remain implementation-independent.
- **REQ-007**: The collection of systems shall collectively support the
  project's purpose and vision.

#### Constraints

- **CON-001**: Source code shall not appear.
- **CON-002**: Module organization shall not replace system definitions.
- **CON-003**: Framework-specific concepts shall not define systems.
- **CON-004**: Temporary implementation decisions shall not determine system
  boundaries.
- **CON-005**: Individual GitHub repositories shall not automatically imply
  independent systems.

#### Guidelines

- **GUD-001**: Prefer cohesive systems with clear ownership.
- **GUD-002**: Minimize overlapping responsibilities.
- **GUD-003**: Prefer conceptual decomposition over technical decomposition.
- **GUD-004**: Define systems according to purpose rather than technology.
- **GUD-005**: Keep system responsibilities stable as implementation evolves.

---

### 4. Authoring Contract

#### Purpose

Describe the project's major systems and the responsibilities owned by each.

#### Responsibilities

`SYSTEM.md` owns:

- system inventory
- system purpose
- system responsibilities
- capability ownership
- conceptual decomposition
- system boundaries

#### Non-Responsibilities

`SYSTEM.md` does not own:

- implementation
- architecture
- APIs
- deployment
- infrastructure
- repository organization
- engineering workflow

#### Inputs

Authoring should consider:

- `PURPOSE.md`
- `VISION.md`
- `FOUNDATIONS.md`
- `ONTOLOGY.md`
- `PERSONAL_MODEL.md`

#### Outputs

The system model informs:

- `ARCHITECTURE.md`
- implementation planning
- repository decomposition
- documentation
- AI reasoning
- engineering decisions

#### AI Generation Rules

When generating a system model:

- identify cohesive systems before architectural layers
- define responsibilities before interactions
- avoid implementation details
- minimize overlapping ownership
- preserve consistency with upstream documents
- think conceptually rather than technologically

#### Validation

The resulting system model should allow readers to understand the project's
major systems without requiring knowledge of implementation.

---

### 5. Acceptance Criteria

- **AC-001**: Every major system is identified.
- **AC-002**: Every system has a clearly defined purpose.
- **AC-003**: Responsibilities are explicitly assigned.
- **AC-004**: System boundaries are documented.
- **AC-005**: Capability ownership is unambiguous.
- **AC-006**: Implementation details are absent.

---

### 6. AI Authoring Strategy

AI systems should construct the system model by:

1. Reading all upstream architecture documents.
2. Identifying the project's major capabilities.
3. Grouping related capabilities into cohesive systems.
4. Defining responsibilities for each system.
5. Establishing clear conceptual boundaries.
6. Producing a stable system decomposition independent of implementation.

The resulting document should describe *what systems exist*, not how they are
implemented.

---

### 7. Rationale & Context

Projects naturally grow in complexity over time.

Without an explicit system model, responsibilities gradually become fragmented,
duplicated, or tightly coupled.

By defining systems first, the project establishes a stable conceptual
decomposition that enables clearer architecture, implementation, ownership, and
future evolution.

The system model provides the bridge between domain understanding and software
architecture.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`
- `FOUNDATIONS.md`
- `ONTOLOGY.md`
- `PERSONAL_MODEL.md`

#### Downstream Dependencies

- `ARCHITECTURE.md`
- repository organization
- engineering decomposition
- implementation planning
- documentation
- AI engineering workflows

---

### 9. Examples & Edge Cases

```text
Example

System:

Knowledge System

Purpose:

Manage the ingestion, organization, retrieval, and publication of knowledge.

Responsibilities:

- knowledge ingestion
- knowledge storage
- semantic organization
- knowledge publishing

Architectural Impact:

Other systems interact with the Knowledge System rather than implementing their
own independent knowledge management.
```

```text
Edge Case

A new capability appears that overlaps two existing systems.

Expected:

Reevaluate the system decomposition before assigning shared ownership.

Every capability should have one primary system responsible for it.
```

---

### 10. Validation Criteria

The completed system model should satisfy the following:

- Systems are cohesive.
- Responsibilities are unique.
- Boundaries are explicit.
- Capability ownership is clear.
- Implementation details are absent.
- The document remains understandable independently of source code.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/domain/ontology.spec.md`
- `.github/specs/architecture/domain/personal-model.spec.md`
- `.github/specs/architecture/foundation/foundations.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`
