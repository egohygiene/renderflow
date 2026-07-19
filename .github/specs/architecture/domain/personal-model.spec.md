---
title: Document Specification — PERSONAL_MODEL.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - personal-model
  - human-centered-design
  - domain-model
---

## Introduction

This specification defines how the `PERSONAL_MODEL.md` architecture document
shall be authored, maintained, and validated.

`PERSONAL_MODEL.md` defines the project's conceptual model of a person. It
describes the assumptions, principles, and human-centered concepts that guide
how the project understands people within its domain.

Where `ONTOLOGY.md` defines **what exists**, `PERSONAL_MODEL.md` defines **how
people exist** within that conceptual world.

Every user experience, feature, workflow, AI interaction, and architectural
decision involving people should remain consistent with this model.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `PERSONAL_MODEL.md` is to establish the project's canonical model
of a person.

This document answers the question:

> **How does this project understand the people it serves?**

This specification covers:

- human assumptions
- identity
- agency
- autonomy
- motivations
- needs
- growth
- relationships
- well-being
- decision-making
- interaction with the surrounding domain

This specification does **not** cover:

- implementation
- software architecture
- user interface design
- product requirements
- workflows
- APIs
- psychology research
- medical guidance

---

### 2. Definitions

- **Personal Model**: The conceptual representation of a person within the
  project's domain.
- **Agency**: A person's ability to make meaningful choices.
- **Autonomy**: The ability to direct one's own behavior and decisions.
- **Identity**: The characteristics through which a person understands and
  expresses themselves.
- **Growth**: The process through which a person changes over time.
- **Need**: A fundamental human requirement recognized by the project.
- **Well-Being**: The holistic state the project seeks to support without
  prescribing.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `PERSONAL_MODEL.md` shall define the project's conceptual model
  of a person.
- **REQ-002**: Human assumptions shall be explicitly documented.
- **REQ-003**: The model shall remain consistent with the project's purpose and
  principles.
- **REQ-004**: Human agency and autonomy shall be explicitly addressed.
- **REQ-005**: The document shall distinguish enduring principles from temporary
  product decisions.
- **REQ-006**: The model shall support consistent architectural and design
  decisions.

#### Constraints

- **CON-001**: The document shall not prescribe implementation.
- **CON-002**: The document shall not define software architecture.
- **CON-003**: The document shall not make unsupported scientific or medical
  claims.
- **CON-004**: Product-specific behavior shall not define the personal model.
- **CON-005**: The model shall not assume every person is identical.

#### Guidelines

- **GUD-001**: Prefer human-centered language.
- **GUD-002**: Distinguish assumptions from facts.
- **GUD-003**: Prefer timeless principles over contemporary trends.
- **GUD-004**: Describe people conceptually rather than behaviorally.
- **GUD-005**: Recognize diversity without fragmenting the conceptual model.

---

### 4. Authoring Contract

#### Purpose

Describe the project's conceptual understanding of people.

#### Responsibilities

`PERSONAL_MODEL.md` owns:

- assumptions about people
- agency
- autonomy
- motivations
- needs
- identity
- relationships
- personal growth
- interaction with the domain

#### Non-Responsibilities

`PERSONAL_MODEL.md` does not own:

- ontology
- architecture
- implementation
- APIs
- user interface design
- workflows
- infrastructure
- roadmap

#### Inputs

Authoring should consider:

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- `ONTOLOGY.md`
- relevant domain research
- subject matter expertise

#### Outputs

The personal model informs:

- `DESIGN.md`
- `DESIGN_SYSTEM.md`
- `SYSTEM.md`
- `ARCHITECTURE.md`
- AI behavior
- interaction design
- user experience
- feature prioritization

#### AI Generation Rules

When generating a personal model:

- begin with the ontology
- model people rather than software
- prefer enduring principles
- distinguish assumptions from observations
- preserve consistency with upstream documents
- avoid implementation details
- avoid normative judgments unless explicitly defined by the project's
  principles

#### Validation

The resulting personal model should provide a coherent conceptual framework for
reasoning about people throughout the project.

---

### 5. Acceptance Criteria

- **AC-001**: The project's assumptions about people are explicitly documented.
- **AC-002**: Human agency and autonomy are clearly addressed.
- **AC-003**: The model aligns with the ontology.
- **AC-004**: The model aligns with the project's principles.
- **AC-005**: No implementation details appear.
- **AC-006**: The document can guide future architectural and design decisions.

---

### 6. AI Authoring Strategy

AI systems should construct the personal model by:

1. Reading all upstream architecture documents.
2. Understanding the ontology before modeling people.
3. Identifying the assumptions the project makes about individuals.
4. Describing those assumptions in implementation-independent language.
5. Preserving consistency across the architecture.
6. Avoiding unsupported claims or unnecessary complexity.

The generated document should remain stable as implementation evolves.

---

### 7. Rationale & Context

Every software system implicitly assumes a model of the people it serves.

By documenting that model explicitly, architectural decisions become more
consistent, explainable, and ethically grounded.

Rather than allowing assumptions to emerge accidentally through implementation,
the project intentionally defines them as part of its architecture.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- `ONTOLOGY.md`

#### Downstream Dependencies

- `DESIGN.md`
- `DESIGN_SYSTEM.md`
- `SYSTEM.md`
- `ARCHITECTURE.md`
- AI prompts
- interaction design
- documentation

---

### 9. Examples & Edge Cases

```text
Example

Assumption:

People are active participants in their own growth rather than passive
recipients of recommendations.

Architectural Impact:

The system emphasizes reflection, autonomy, and informed decision-making rather
than prescriptive automation.
```

```text
Edge Case

A proposed feature assumes users always optimize for efficiency.

Expected:

Evaluate whether that assumption aligns with the project's personal model before
accepting the architectural change.
```

---

### 10. Validation Criteria

The completed personal model should satisfy the following:

- Human assumptions are explicit.
- Agency and autonomy are addressed.
- Terminology aligns with the ontology.
- No implementation details appear.
- Architectural decisions can reference the model without redefining it.
- The document remains understandable without reading source code.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/domain/ontology.spec.md`
- `.github/specs/architecture/identity/principles.spec.md`
- `.github/specs/architecture/foundation/system.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`
