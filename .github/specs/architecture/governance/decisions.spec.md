---
title: Document Specification — DECISIONS.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - decisions
  - governance
  - adr
---

## Introduction

This specification defines how the `DECISIONS.md` architecture document shall
be authored, maintained, and validated.

`DECISIONS.md` records the project's significant architectural, engineering,
product, and governance decisions together with the rationale that led to each
decision.

Where `PRINCIPLES.md` defines **how decisions should be made**,
`DECISIONS.md` documents **which important decisions have actually been made**
and preserves the reasoning behind them.

The document serves as the project's long-term architectural memory.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `DECISIONS.md` is to preserve important project decisions and
their rationale.

This document answers the question:

> **Why is the project the way it is?**

This specification covers:

- architectural decisions
- engineering decisions
- governance decisions
- major technology selections
- accepted trade-offs
- documented rationale
- historical context
- superseded decisions

This specification does **not** cover:

- implementation tasks
- GitHub issues
- meeting notes
- sprint planning
- brainstorming
- feature requests
- temporary experiments

---

### 2. Definitions

- **Decision**: A deliberate architectural or engineering choice.
- **Decision Record**: A documented explanation of a decision and its rationale.
- **Trade-off**: A compromise accepted in exchange for desired benefits.
- **Alternative**: A solution considered but not selected.
- **Superseded Decision**: A decision intentionally replaced by a newer one.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `DECISIONS.md` shall record significant project decisions.
- **REQ-002**: Every decision shall include its rationale.
- **REQ-003**: Significant alternatives shall be documented when practical.
- **REQ-004**: Decisions shall reference related architecture documents.
- **REQ-005**: Superseded decisions shall remain documented.
- **REQ-006**: Decisions shall be written so future contributors can understand
  the original reasoning.

#### Constraints

- **CON-001**: Temporary implementation details shall not become decision
  records.
- **CON-002**: GitHub issues shall not replace architectural decisions.
- **CON-003**: Meeting notes shall not appear.
- **CON-004**: Decisions shall not duplicate principles or methodology.
- **CON-005**: Decisions shall remain factual rather than argumentative.

#### Guidelines

- **GUD-001**: Record decisions as close to their adoption as practical.
- **GUD-002**: Explain *why*, not only *what*.
- **GUD-003**: Prefer concise rationale.
- **GUD-004**: Document trade-offs honestly.
- **GUD-005**: Preserve historical context even after decisions evolve.

---

### 4. Authoring Contract

#### Purpose

Describe the project's significant architectural and engineering decisions.

#### Responsibilities

`DECISIONS.md` owns:

- accepted decisions
- decision rationale
- architectural trade-offs
- historical reasoning
- superseded decisions
- governance history

#### Non-Responsibilities

`DECISIONS.md` does not own:

- principles
- methodology
- implementation
- architecture
- project planning
- issue tracking
- meeting minutes

#### Inputs

Authoring should consider:

- `PRINCIPLES.md`
- `FOUNDATIONS.md`
- `SYSTEM.md`
- `ARCHITECTURE.md`
- accepted proposals
- engineering discussions

#### Outputs

The decision log informs:

- future contributors
- architectural reviews
- implementation decisions
- onboarding
- AI reasoning
- future decision making

#### AI Generation Rules

When documenting a decision:

- explain the context
- explain the rationale
- identify alternatives when appropriate
- document accepted trade-offs
- preserve historical accuracy
- avoid rewriting history

#### Validation

Every decision should remain understandable years after it was originally
made.

---

### 5. Acceptance Criteria

- **AC-001**: Significant decisions are documented.
- **AC-002**: Every decision includes rationale.
- **AC-003**: Trade-offs are documented where applicable.
- **AC-004**: Historical context is preserved.
- **AC-005**: Superseded decisions remain traceable.
- **AC-006**: Future contributors can understand why decisions were made.

---

### 6. AI Authoring Strategy

AI systems should construct the decision log by:

1. Reading the current architecture.
2. Identifying major architectural and engineering decisions.
3. Explaining the rationale behind each decision.
4. Recording important alternatives when known.
5. Preserving historical context.
6. Avoiding speculation about undocumented reasoning.

The resulting document should become the project's institutional memory.

---

### 7. Rationale & Context

Projects accumulate decisions over time.

Without documenting them, contributors repeatedly revisit the same questions,
lose valuable context, and unknowingly reverse important architectural choices.

A maintained decision log reduces repeated debates, accelerates onboarding, and
helps AI systems understand the intent behind the architecture rather than only
its current implementation.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PRINCIPLES.md`
- `FOUNDATIONS.md`
- `SYSTEM.md`
- `ARCHITECTURE.md`

#### Downstream Dependencies

- onboarding
- architecture reviews
- implementation
- documentation
- AI engineering agents

---

### 9. Examples & Edge Cases

```text
Example

Decision:

Use specification-first development.

Rationale:

Improves architectural clarity, enables AI-assisted engineering, and separates
design from implementation.

Trade-off:

Slightly higher upfront effort in exchange for greater long-term consistency.
```

```text
Edge Case

A previous decision is replaced.

Expected:

Retain the original decision, mark it as superseded, reference the replacement,
and preserve the historical rationale.
```

---

### 10. Validation Criteria

The completed decision log should satisfy the following:

- Significant decisions are documented.
- Rationale accompanies every decision.
- Historical context is preserved.
- Trade-offs are explicitly acknowledged.
- Superseded decisions remain discoverable.
- The document remains useful independently of implementation.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/identity/principles.spec.md`
- `.github/specs/architecture/foundation/foundations.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`
- `.github/specs/architecture/foundation/methodology.spec.md`
