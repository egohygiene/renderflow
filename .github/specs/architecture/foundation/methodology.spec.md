---
title: Document Specification — METHODOLOGY.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - methodology
  - engineering
  - workflow
---

## Introduction

This specification defines how the `METHODOLOGY.md` architecture document shall
be authored, maintained, and validated.

`METHODOLOGY.md` defines the project's preferred methods of working. It
describes the engineering practices, collaboration models, workflows, and
development approaches that guide how the project evolves over time.

Where `PRINCIPLES.md` explains **how decisions should be made**,
`METHODOLOGY.md` explains **how work should be performed**.

The methodology should establish repeatable engineering practices without
prescribing implementation details.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `METHODOLOGY.md` is to define the project's operating model.

This document answers the question:

> **How should work be performed within this project?**

This specification covers:

- engineering methodology
- development workflow
- collaboration practices
- documentation strategy
- AI-assisted engineering
- specification-first development
- architectural process
- iterative refinement
- validation philosophy

This specification does **not** cover:

- implementation
- project planning
- coding standards
- deployment
- infrastructure
- repository structure
- architectural organization

---

### 2. Definitions

- **Methodology**: A repeatable approach for performing work.
- **Workflow**: A sequence of activities used to complete work.
- **Specification-First Development**: Designing and documenting behavior before
  implementation.
- **Iterative Refinement**: Improving systems through successive revisions.
- **AI-Assisted Engineering**: The deliberate use of AI systems to augment
  engineering workflows.
- **Validation**: Verifying that work satisfies defined expectations.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `METHODOLOGY.md` shall define the project's preferred methods of
  working.
- **REQ-002**: Development practices shall be repeatable.
- **REQ-003**: Methodology shall align with the project's principles.
- **REQ-004**: Engineering workflows shall prioritize maintainability.
- **REQ-005**: Validation shall be incorporated throughout the methodology.
- **REQ-006**: Methodology shall remain largely implementation-independent.

#### Constraints

- **CON-001**: Methodology shall not prescribe specific technologies unless
  required by the architecture.
- **CON-002**: Temporary tooling choices shall not become methodology.
- **CON-003**: Product-specific implementation shall not appear.
- **CON-004**: Methodology shall not duplicate project planning.

#### Guidelines

- **GUD-001**: Prefer deterministic workflows.
- **GUD-002**: Prefer specification before implementation.
- **GUD-003**: Prefer automation over manual repetition.
- **GUD-004**: Prefer incremental improvement.
- **GUD-005**: Design workflows that humans and AI can both follow.
- **GUD-006**: Treat documentation as part of the engineering process.

---

### 4. Authoring Contract

#### Purpose

Describe how work should be performed throughout the project.

#### Responsibilities

`METHODOLOGY.md` owns:

- engineering methodology
- development workflow
- collaboration practices
- architectural workflow
- documentation workflow
- AI engineering workflow
- validation strategy

#### Non-Responsibilities

`METHODOLOGY.md` does not own:

- implementation
- architecture
- system organization
- project roadmap
- deployment
- coding conventions
- repository structure

#### Inputs

Authoring should consider:

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- engineering experience
- team practices
- AI capabilities

#### Outputs

The methodology informs:

- engineering processes
- contribution guidelines
- AI workflows
- automation
- documentation
- quality assurance

#### AI Generation Rules

When generating a methodology:

- focus on repeatable practices
- distinguish process from philosophy
- avoid implementation details
- prioritize deterministic workflows
- support collaboration between humans and AI
- preserve consistency with upstream documents

#### Validation

The resulting methodology should enable contributors to perform engineering work
consistently regardless of project size or implementation technology.

---

### 5. Acceptance Criteria

- **AC-001**: Engineering workflows are clearly defined.
- **AC-002**: Development practices are repeatable.
- **AC-003**: Validation is integrated into the methodology.
- **AC-004**: The methodology aligns with project principles.
- **AC-005**: Implementation details are absent.
- **AC-006**: AI-assisted workflows are explicitly supported where appropriate.

---

### 6. AI Authoring Strategy

AI systems should construct the methodology by:

1. Reading upstream architecture documents.
2. Identifying repeatable engineering practices.
3. Separating methodology from implementation.
4. Defining workflows that humans and AI can both follow.
5. Favoring deterministic and observable engineering processes.
6. Producing guidance that remains useful as technologies evolve.

The resulting methodology should describe how engineering work is performed
rather than how software is implemented.

---

### 7. Rationale & Context

Without an explicit methodology, engineering practices gradually become
inconsistent, undocumented, and difficult to reproduce.

By documenting methodology separately from principles and architecture, the
project establishes a repeatable operating model that improves onboarding,
automation, AI collaboration, and long-term maintainability.

Methodology describes how the project evolves rather than what the project
contains.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`

#### Downstream Dependencies

- `FOUNDATIONS.md`
- `SYSTEM.md`
- engineering workflows
- contribution guidelines
- automation
- AI engineering agents

---

### 9. Examples & Edge Cases

```text
Example

Methodology:

Every feature begins with a specification before implementation.

Engineering Impact:

Architecture, acceptance criteria, and validation are established before source
code is written.
```

```text
Edge Case

A contributor bypasses the documented engineering workflow.

Expected:

Evaluate whether the workflow should evolve before making the exception the new
standard practice.
```

---

### 10. Validation Criteria

The completed methodology should satisfy the following:

- Engineering workflows are explicit.
- Practices are repeatable.
- Validation is integrated.
- Automation opportunities are identified.
- Implementation details are absent.
- Humans and AI can consistently follow the methodology.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/foundation/foundations.spec.md`
- `.github/specs/architecture/foundation/system.spec.md`
- `.github/specs/architecture/identity/principles.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`
