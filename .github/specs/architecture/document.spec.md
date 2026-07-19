---
title: Document Specification Standard — Architecture Documents
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - standard
  - authoring
  - documentation
---

## Introduction

This specification defines the canonical standard for authoring architecture documents throughout the Ego Hygiene ecosystem.

Rather than prescribing the contents of any specific architecture document, this specification defines the common structure, conventions, terminology, validation rules, and authoring contracts that all architecture documents shall follow.

Every architecture document specification should extend this standard.

---

### 1. Purpose & Scope

This specification establishes a consistent architecture language across repositories, teams, and AI agents.

The goals are to:

- improve consistency
- reduce ambiguity
- simplify onboarding
- enable AI-assisted architecture generation
- make architectural decisions easier to understand and maintain

This specification applies to all architecture document specifications located beneath:

```text
.github/specs/architecture/
```

Examples include:

- PURPOSE.md
- VISION.md
- PRINCIPLES.md
- SYSTEM.md
- ARCHITECTURE.md
- ONTOLOGY.md
- DESIGN.md

This specification does **not** define the content of those documents.

Instead, it defines how their specifications should be written.

---

### 2. Definitions

- **Architecture Document**: A canonical markdown document describing one aspect of a project's architecture.
- **Architecture Specification**: A specification describing how an architecture document should be authored.
- **Authoring Contract**: Rules governing how humans and AI agents produce architecture documents.
- **Canonical Document**: The authoritative source for a specific architectural concern.
- **Upstream Dependency**: A document that should be read before authoring another.
- **Downstream Dependency**: A document that consumes concepts from another document.
- **Architecture Graph**: The dependency graph connecting architecture documents.
- **Implementation Artifact**: Source code, infrastructure configuration, or runtime behavior.

---

### 3. Architecture Philosophy

Every architecture document should satisfy the following principles.

#### Single Responsibility

Each document should own exactly one architectural concern.

Information should not be duplicated across documents.

---

#### Stable Knowledge

Architecture documents should change slowly.

Implementation changes frequently.

Architecture changes deliberately.

---

#### Progressive Refinement

Higher-level documents establish philosophy.

Lower-level documents refine those ideas into implementation guidance.

---

#### Canonical Ownership

Every concept should have one authoritative home.

Other documents should reference rather than duplicate it.

---

#### Human & AI Readability

Architecture documents should be understandable by:

- engineers
- architects
- technical writers
- AI coding assistants
- future maintainers

---

### 4. Standard Document Structure

Every architecture specification should contain the following sections.

| Section | Required |
| -------- | -------- |
| Frontmatter | Yes |
| Introduction | Yes |
| Purpose & Scope | Yes |
| Definitions | Yes |
| Requirements, Constraints & Guidelines | Yes |
| Authoring Contract | Yes |
| Acceptance Criteria | Yes |
| AI Authoring Strategy | Yes |
| Rationale & Context | Yes |
| Dependencies | Yes |
| Examples & Edge Cases | Yes |
| Validation Criteria | Yes |
| Related Specifications | Yes |

Additional sections may be added when justified.

---

### 5. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: Every architecture specification shall follow this document structure.
- **REQ-002**: Every specification shall define exactly one architectural concern.
- **REQ-003**: Every specification shall identify upstream and downstream dependencies.
- **REQ-004**: Every specification shall define measurable acceptance criteria.
- **REQ-005**: Every specification shall define validation criteria.

#### Constraints

- **CON-001**: Specifications shall remain implementation-independent unless implementation ownership is explicit.
- **CON-002**: Specifications shall avoid duplicate responsibilities.
- **CON-003**: Specifications shall reference related documents instead of copying them.

#### Guidelines

- **GUD-001**: Prefer timeless language.
- **GUD-002**: Prefer precise terminology.
- **GUD-003**: Prefer explicit responsibilities.
- **GUD-004**: Prefer deterministic structure.

---

### 6. Authoring Contract

Every architecture specification shall define:

#### Purpose

Why the document exists.

---

#### Responsibilities

What the document owns.

---

#### Non-Responsibilities

What belongs elsewhere.

---

#### Inputs

Documents or knowledge required before authoring.

---

#### Outputs

Documents influenced by this one.

---

#### AI Generation Rules

Instructions for AI systems producing the document.

---

#### Validation

Objective success criteria.

---

### 7. AI Authoring Strategy

AI systems generating architecture documents should:

1. Read upstream documents first.
2. Preserve canonical terminology.
3. Avoid duplication.
4. Respect document ownership boundaries.
5. Prefer extending existing concepts.
6. Produce deterministic output where possible.
7. Explain assumptions when required information is unavailable.

AI should never invent contradictory architecture.

---

### 8. Dependency Model

Architecture specifications form a directed dependency graph.

Example:

```text
PURPOSE
    │
    ▼
VISION
    │
    ▼
PRINCIPLES
    │
    ▼
SYSTEM
    │
    ▼
ARCHITECTURE
```

Specifications should declare:

- upstream dependencies
- downstream dependencies

to support automated generation.

---

### 9. Validation Criteria

Every specification should be reviewable.

Validation should confirm:

- responsibilities are unique
- terminology is consistent
- dependencies are correct
- acceptance criteria are measurable
- implementation details are appropriately scoped

---

### 10. Markdown Standards

Architecture specifications shall:

- use valid UTF-8
- follow Markdownlint recommendations
- contain exactly one H1 heading
- use incremental heading levels
- prefer tables over deeply nested lists
- wrap long prose at a consistent width
- avoid HTML unless necessary
- use fenced code blocks with language identifiers
- use kebab-case filenames
- maintain stable section ordering

---

### 11. Versioning

Specifications should evolve through semantic versioning.

Minor revisions:

- clarification
- wording improvements
- examples

Major revisions:

- structural changes
- new required sections
- breaking authoring requirements

Specifications should preserve backwards compatibility whenever practical.

---

### 12. Related Specifications

This document serves as the parent specification for all documents within:

```text
.github/specs/architecture/
```

All architecture specifications should explicitly state that they conform to this standard.

Future revisions may introduce additional specialized standards for:

- process specifications
- workflow specifications
- interface specifications
- API specifications
- data contract specifications
