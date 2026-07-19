---
title: Document Specification — ONTOLOGY.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - ontology
  - domain-model
  - ubiquitous-language
---

## Introduction

This specification defines how the `ONTOLOGY.md` architecture document shall be authored, maintained, and validated.

`ONTOLOGY.md` defines the project's canonical domain language. It establishes the concepts, entities, relationships, terminology, and boundaries that describe what exists within the project's problem space.

All architectural documents, implementation artifacts, and AI-generated content should use the ontology as the authoritative vocabulary for the project.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly documented.

---

### 1. Purpose & Scope

The purpose of `ONTOLOGY.md` is to establish a shared conceptual model for the project.

This document answers the question:

> **What exists within the project's domain?**

This specification covers:

- canonical concepts
- domain entities
- conceptual relationships
- ubiquitous language
- terminology
- domain boundaries
- conceptual evolution

This specification does **not** cover:

- software architecture
- implementation details
- APIs
- data models
- persistence
- user interface design
- workflows

---

### 2. Definitions

- **Ontology**: The formal conceptual description of the project's domain.
- **Concept**: A meaningful abstraction within the domain.
- **Entity**: A concept possessing an identifiable identity.
- **Relationship**: A defined association between two concepts.
- **Canonical Term**: The preferred vocabulary used throughout the project.
- **Domain Boundary**: The distinction between concepts that belong inside or outside the project's scope.
- **Ubiquitous Language**: Shared terminology consistently used by humans and AI across the project.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `ONTOLOGY.md` shall define the canonical vocabulary for the project.
- **REQ-002**: Every major domain concept shall have a single authoritative definition.
- **REQ-003**: Relationships between major concepts shall be explicitly documented.
- **REQ-004**: Domain terminology shall remain implementation-independent.
- **REQ-005**: Concepts shall be described from a domain perspective rather than a technical perspective.
- **REQ-006**: Domain boundaries shall be explicitly identified.
- **REQ-007**: Canonical terminology shall be used consistently throughout the document.

#### Constraints

- **CON-001**: Software architecture shall not be described.
- **CON-002**: APIs shall not be defined.
- **CON-003**: Database schemas shall not be included.
- **CON-004**: Source code shall not appear.
- **CON-005**: Product implementation details shall not influence conceptual definitions.

#### Guidelines

- **GUD-001**: Prefer one canonical name for each concept.
- **GUD-002**: Avoid synonyms unless they are explicitly documented.
- **GUD-003**: Keep definitions concise and precise.
- **GUD-004**: Describe concepts independently of technology choices.
- **GUD-005**: Prefer conceptual relationships over implementation relationships.

---

### 4. Authoring Contract

#### Purpose

Describe the conceptual world of the project.

#### Responsibilities

`ONTOLOGY.md` owns:

- concepts
- entities
- terminology
- relationships
- domain boundaries
- ubiquitous language

#### Non-Responsibilities

`ONTOLOGY.md` does not own:

- architecture
- implementation
- infrastructure
- APIs
- storage
- workflows
- design systems
- project planning

#### Inputs

Authoring should consider:

- `PURPOSE.md`
- `VISION.md`
- domain research
- subject matter expertise

#### Outputs

The ontology informs:

- `PERSONAL_MODEL.md`
- `SYSTEM.md`
- `ARCHITECTURE.md`
- `DESIGN.md`
- implementation naming
- documentation
- AI prompts

#### AI Generation Rules

When generating an ontology:

- identify concepts before implementations
- establish canonical terminology
- define relationships explicitly
- avoid ambiguous vocabulary
- preserve consistency with upstream documents
- avoid inventing unnecessary concepts

#### Validation

The resulting ontology should serve as the authoritative vocabulary for every other architecture document.

---

### 5. Acceptance Criteria

- **AC-001**: Every major concept has a canonical definition.
- **AC-002**: Every relationship is explicitly described.
- **AC-003**: Canonical terminology is internally consistent.
- **AC-004**: Domain boundaries are clearly defined.
- **AC-005**: No implementation details appear.
- **AC-006**: Other architecture documents can reference concepts without redefining them.

---

### 6. AI Authoring Strategy

AI systems should construct the ontology by:

1. Reading all upstream architecture documents.
2. Identifying the project's conceptual entities.
3. Establishing canonical terminology.
4. Defining conceptual relationships.
5. Eliminating duplicate or conflicting definitions.
6. Producing a stable conceptual model suitable for long-term evolution.

AI should avoid creating concepts that duplicate existing terminology or contradict established architectural principles.

---

### 7. Rationale & Context

A shared ontology enables consistent communication across architecture, implementation, documentation, and AI systems.

Without a canonical ontology, terminology gradually diverges, resulting in duplicated concepts, inconsistent naming, conflicting documentation, and reduced maintainability.

By centralizing domain language, the ontology becomes the conceptual foundation for every subsequent architectural decision.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`

#### Downstream Dependencies

- `PERSONAL_MODEL.md`
- `SYSTEM.md`
- `ARCHITECTURE.md`
- `DESIGN.md`
- source code naming
- documentation
- AI prompts

---

### 9. Examples & Edge Cases

```text
Example

Concept:
Garden

Definition:
A curated collection of interconnected knowledge artifacts.

Relationship:
Garden contains Knowledge Artifacts.
```

```text
Edge Case

Two concepts appear to overlap.

Expected:
Merge them into a single canonical concept or explicitly distinguish their responsibilities.
```

---

### 10. Validation Criteria

The completed ontology should satisfy the following:

- Every concept has one authoritative definition.
- Relationships are documented.
- Canonical terminology is consistent.
- Domain boundaries are explicit.
- Implementation details are absent.
- The document can be understood independently of source code.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/domain/personal-model.spec.md`
- `.github/specs/architecture/foundation/system.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`
