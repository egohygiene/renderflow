---
title: Document Specification — EPISTEMOLOGY.md
version: 1.0
date_created: 2026-07-18
last_updated: 2026-07-18
owner: Ego Hygiene
tags:
  - architecture
  - specification
  - epistemology
  - governance
  - knowledge
---

## Introduction

This specification defines how the `EPISTEMOLOGY.md` architecture document
shall be authored, maintained, and validated.

`EPISTEMOLOGY.md` defines how the project understands knowledge, evidence,
uncertainty, confidence, and truth.

It establishes the project's philosophy for evaluating information, resolving
conflicting claims, preserving provenance, and determining what should be
considered trustworthy.

Where `DECISIONS.md` records **what the project has concluded**,
`EPISTEMOLOGY.md` defines **how those conclusions should be reached**.

The epistemology should guide both human contributors and AI systems whenever
knowledge is created, evaluated, or transformed.

---

### Conformance

This specification conforms to:

- `.github/specs/architecture/document.spec.md`

Any deviations from the document specification standard shall be explicitly
documented.

---

### 1. Purpose & Scope

The purpose of `EPISTEMOLOGY.md` is to establish the project's philosophy of
knowledge.

This document answers the question:

> **How does this project determine what is true, trustworthy, or sufficiently
> justified?**

This specification covers:

- evidence
- provenance
- confidence
- uncertainty
- knowledge quality
- reasoning
- truth claims
- conflicting information
- source evaluation

This specification does **not** cover:

- implementation
- extraction algorithms
- AI prompts
- architecture
- scientific conclusions
- project decisions
- documentation style

---

### 2. Definitions

- **Epistemology**: The philosophy governing how knowledge is acquired,
  evaluated, and justified.
- **Evidence**: Information supporting or challenging a claim.
- **Provenance**: The origin and history of information.
- **Confidence**: The project's assessment of how strongly a claim is supported.
- **Uncertainty**: Known limitations in available knowledge.
- **Canonical Knowledge**: Information accepted as authoritative within the
  project.
- **Claim**: A statement that may be evaluated for truthfulness.

---

### 3. Requirements, Constraints & Guidelines

#### Requirements

- **REQ-001**: `EPISTEMOLOGY.md` shall define the project's philosophy of
  knowledge.
- **REQ-002**: Knowledge evaluation criteria shall be explicitly documented.
- **REQ-003**: Provenance expectations shall be defined.
- **REQ-004**: Confidence and uncertainty shall be treated explicitly.
- **REQ-005**: Conflicting information shall have defined resolution guidance.
- **REQ-006**: The epistemology shall remain applicable to both humans and AI.

#### Constraints

- **CON-001**: The document shall not prescribe implementation.
- **CON-002**: Provider-specific AI behavior shall not appear.
- **CON-003**: Individual research conclusions shall not define the
  epistemology.
- **CON-004**: Knowledge sources shall not be assumed equally reliable.
- **CON-005**: The document shall distinguish evidence from opinion.

#### Guidelines

- **GUD-001**: Preserve provenance whenever practical.
- **GUD-002**: Make uncertainty explicit.
- **GUD-003**: Prefer evidence over authority.
- **GUD-004**: Separate observations from interpretations.
- **GUD-005**: Allow knowledge to evolve as new evidence emerges.
- **GUD-006**: Favor transparency over unwarranted certainty.

---

### 4. Authoring Contract

#### Purpose

Describe how the project evaluates, justifies, and evolves knowledge.

#### Responsibilities

`EPISTEMOLOGY.md` owns:

- knowledge philosophy
- evidence evaluation
- provenance
- confidence
- uncertainty
- truth evaluation
- knowledge governance

#### Non-Responsibilities

`EPISTEMOLOGY.md` does not own:

- implementation
- architecture
- AI prompts
- engineering methodology
- project decisions
- research databases

#### Inputs

Authoring should consider:

- `PURPOSE.md`
- `PRINCIPLES.md`
- `AI_CONSTITUTION.md`
- relevant research methodology
- domain expertise

#### Outputs

The epistemology informs:

- AI reasoning
- knowledge extraction
- research workflows
- documentation
- decision making
- information governance

#### AI Generation Rules

When generating an epistemology:

- distinguish evidence from conclusions
- preserve provenance
- treat uncertainty explicitly
- avoid overstating confidence
- support evolving knowledge
- remain implementation-independent

#### Validation

The resulting epistemology should enable both humans and AI systems to evaluate
knowledge consistently and transparently.

---

### 5. Acceptance Criteria

- **AC-001**: Knowledge evaluation philosophy is explicitly documented.
- **AC-002**: Provenance expectations are defined.
- **AC-003**: Confidence and uncertainty are addressed.
- **AC-004**: Knowledge governance remains implementation-independent.
- **AC-005**: AI and human reasoning are guided consistently.
- **AC-006**: The document supports future knowledge evolution.

---

### 6. AI Authoring Strategy

AI systems should construct the epistemology by:

1. Reading the project's governance documents.
2. Defining how knowledge should be evaluated.
3. Explaining confidence and uncertainty.
4. Defining provenance expectations.
5. Preserving transparency.
6. Producing guidance that remains applicable regardless of implementation.

The resulting document should govern reasoning rather than conclusions.

---

### 7. Rationale & Context

Knowledge systems are only as trustworthy as the methods they use to evaluate
knowledge.

Without an explicit epistemology, contributors and AI systems gradually adopt
inconsistent assumptions about evidence, confidence, and truth.

Documenting epistemology provides a shared framework for evaluating information,
preserving provenance, and improving reasoning across the project.

---

### 8. Dependencies & External Integrations

#### Upstream Dependencies

- `PURPOSE.md`
- `PRINCIPLES.md`
- `AI_CONSTITUTION.md`

#### Downstream Dependencies

- knowledge systems
- research workflows
- AI agents
- documentation
- decision records
- information governance

---

### 9. Examples & Edge Cases

```text
Example

Claim

A particular technique improves long-term memory retention.

Evidence

Peer-reviewed research, replicated studies, and documented provenance.

Confidence

Moderate.

Architectural Impact

AI systems present the claim with supporting evidence and an explicit confidence
assessment rather than treating it as unquestionable fact.
```

```text
Edge Case

Two authoritative sources reach conflicting conclusions.

Expected

Preserve both perspectives, document the disagreement, record provenance, and
avoid manufacturing false certainty.
```

---

### 10. Validation Criteria

The completed epistemology should satisfy the following:

- Knowledge evaluation principles are explicit.
- Provenance is emphasized.
- Confidence and uncertainty are addressed.
- Evidence is distinguished from opinion.
- The document remains implementation-independent.
- Both humans and AI can apply the guidance consistently.

---

### 11. Related Specifications / Further Reading

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/governance/ai-constitution.spec.md`
- `.github/specs/architecture/governance/decisions.spec.md`
- `.github/specs/architecture/identity/principles.spec.md`
- `.github/specs/knowledge-extract.spec.md`
