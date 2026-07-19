---
title: Document Specification — META.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - meta
  - architecture-framework
  - governance
---

## Introduction

This specification defines how the `META.md` architecture document shall be
authored, maintained, and validated.

`META.md` describes the architecture documentation system itself.

Rather than describing the project, it explains how the project's architecture
is organized, how its documents relate to one another, and how the architecture
should evolve over time.

Where individual architecture documents describe specific concerns,
`META.md` describes the architecture as a coherent knowledge system.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `META.md` is to describe the architecture framework itself.

This document answers the question:

> **How should this architecture be understood, navigated, and maintained?**

This specification covers:

- architecture organization
- document relationships
- dependency graph
- architectural layering
- authoring philosophy
- architecture evolution
- documentation governance

This specification does **not** cover:

- implementation
- system architecture
- engineering methodology
- project roadmap
- individual architectural concerns

---

### 2. Definitions

- **Architecture Framework**: The complete collection of architecture documents.
- **Architecture Layer**: A logical grouping of architecture documents.
- **Dependency Graph**: The relationships between architecture documents.
- **Canonical Document**: The authoritative owner of a specific concern.
- **Architecture Evolution**: The process by which the architecture itself
  changes over time.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `META.md` shall describe the architecture documentation
  framework.
- **REQ-002**: Architectural layers shall be documented.
- **REQ-003**: Relationships between documents shall be described.
- **REQ-004**: Canonical ownership shall be explicit.
- **REQ-005**: The architecture dependency graph shall be documented.
- **REQ-006**: Guidance for evolving the architecture shall be included.

#### Constraints

- **CON-001**: `META.md` shall not duplicate individual architecture documents.
- **CON-002**: Implementation details shall not appear.
- **CON-003**: Project-specific engineering guidance shall not replace
  architectural guidance.
- **CON-004**: Architectural responsibilities shall remain clearly separated.

#### Guidelines

- **GUD-001**: Explain relationships rather than content.
- **GUD-002**: Preserve a single source of truth for every concern.
- **GUD-003**: Keep architectural layering explicit.
- **GUD-004**: Favor conceptual clarity over completeness.
- **GUD-005**: Treat the architecture itself as an evolving system.

---

### 4. Authoring Contract

#### Purpose

Describe how the architecture documentation system is organized.

#### Responsibilities

`META.md` owns:

- architecture overview
- document relationships
- architectural layering
- dependency graph
- navigation guidance
- architecture evolution

#### Non-Responsibilities

`META.md` does not own:

- project purpose
- vision
- architecture
- systems
- methodology
- implementation
- governance decisions

#### Inputs

Authoring should consider:

- every architecture document
- document specifications
- dependency relationships
- architecture evolution

#### Outputs

The meta document informs:

- architecture onboarding
- contributor understanding
- AI architecture generation
- documentation maintenance
- future architecture evolution

#### AI Generation Rules

When generating `META.md`:

- describe relationships rather than repeating content
- preserve canonical ownership
- explain architectural layers
- document dependency flow
- avoid duplicating downstream documents

#### Validation

The resulting document should enable a new contributor to understand the
architecture before reading any individual document.

---

### 5. Acceptance Criteria

- **AC-001**: Architecture layers are documented.
- **AC-002**: Document relationships are explicit.
- **AC-003**: Dependency flow is understandable.
- **AC-004**: Canonical ownership is preserved.
- **AC-005**: Navigation guidance is provided.
- **AC-006**: The architecture framework can evolve without losing coherence.

---

### 6. AI Authoring Strategy

AI systems should construct `META.md` by:

1. Reading every architecture document.
2. Identifying architectural layers.
3. Mapping dependency relationships.
4. Explaining the purpose of each layer.
5. Avoiding duplication of document content.
6. Producing an architectural overview suitable for both humans and AI.

The resulting document should describe the architecture framework rather than
the project itself.

---

### 7. Rationale & Context

Large architecture systems eventually become architectures in their own right.

Without documentation describing how architecture is organized, contributors
must reverse engineer document relationships and ownership.

`META.md` provides an explicit conceptual map that improves onboarding,
navigation, maintainability, and AI-assisted architecture generation.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- Every architecture document.

#### Downstream Dependencies

- contributor onboarding
- architecture maintenance
- AI architecture agents
- architecture generation
- documentation evolution

---

### 9. Examples & Edge Cases

```text
Example

Identity Layer

Purpose
Vision
Manifesto
Principles
Pillars

Purpose

Defines the project's strategic identity.
```

```text
Edge Case

A new architecture document is introduced.

Expected

Update the architectural dependency graph and document ownership rather than
allowing the new document to exist without a defined relationship to the rest
of the architecture.
```

---

### 10. Validation Criteria

The completed meta document should satisfy the following:

- Architectural layers are documented.
- Relationships are understandable.
- Dependency flow is explicit.
- Canonical ownership is preserved.
- Navigation is intuitive.
- The document remains implementation-independent.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- Every architecture specification beneath
  `.github/specs/architecture/`
