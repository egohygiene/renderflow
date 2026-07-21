---
title: Epistemology — Renderflow
version: 1.0
date_created: 2026-07-21
last_updated: 2026-07-21
owner: Renderflow Maintainers
tags:
  - architecture
  - meta
  - epistemology
---

# EPISTEMOLOGY

## Introduction

Renderflow's epistemology defines how the project evaluates truth, evidence,
confidence, and uncertainty.

## Conformance

This document is authored in conformance with:

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/meta/epistemology.spec.md`

## Purpose & Scope

This document establishes how Renderflow decides what is trustworthy when
reasoning about architecture, implementation, behavior, and documentation.

It covers evidence, provenance, uncertainty, and conflict resolution. It does
not define particular conclusions, implementation algorithms, or provider
prompts.

## Definitions

- **Evidence**: information that supports or challenges a claim.
- **Provenance**: the origin and traceability of information.
- **Confidence**: the degree to which a claim is justified by available evidence.
- **Uncertainty**: known limits in evidence, interpretation, or current understanding.
- **Canonical knowledge**: information the project treats as authoritative for a specific concern.

## Renderflow Epistemology

### Sources of truth

Renderflow evaluates claims using a hierarchy of canonical knowledge:

1. purpose, principles, and architecture documents for durable intent,
2. specifications and user-facing contracts for declared behavior,
3. source code and tests for implemented behavior,
4. documentation and examples for taught behavior,
5. runtime diagnostics and CI results for observed behavior.

No single source replaces all others; the relevant canonical source depends on
the type of claim being evaluated.

### Knowledge rules

- Prefer direct evidence over inference.
- Prefer reproducible observation over anecdote.
- Preserve provenance when summarizing findings.
- Distinguish current implementation from intended architecture.
- Distinguish observations from interpretations and recommendations.
- Treat uncertainty as information, not as failure.

### Resolving conflicting claims

When claims conflict:

1. identify the type of claim being made,
2. find the canonical owner of that concern,
3. compare the claim with direct evidence,
4. document uncertainty when evidence remains incomplete,
5. escalate for human judgment when authority or ambiguity remains high.

## Requirements, Constraints & Guidelines

### Requirements

- Knowledge evaluation criteria must be explicit.
- Provenance expectations must be preserved.
- Confidence and uncertainty must be treated directly.
- The epistemology must guide both humans and AI.

### Constraints

- This document must not prescribe implementation.
- Authority alone must not replace evidence.
- Opinions and observations must remain distinguishable.

### Guidelines

- Favor transparency over unwarranted certainty.
- Prefer evidence that can be reproduced or inspected.
- Allow conclusions to evolve when better evidence appears.

## Authoring Contract

### Purpose

Own how Renderflow evaluates and justifies knowledge.

### Responsibilities

This document owns:

- evidence philosophy,
- provenance expectations,
- confidence and uncertainty language,
- conflict-resolution guidance.

### Non-Responsibilities

This document does not own:

- specific project decisions,
- implementation details,
- AI prompts,
- system architecture.

### Inputs

- `PURPOSE.md`
- `PRINCIPLES.md`
- `AI_CONSTITUTION.md`
- repository documentation, tests, and workflows

### Outputs

- `DECISIONS.md`
- AI reasoning
- documentation maintenance
- contributor research and review practice

### AI Generation Rules

AI systems should preserve provenance, separate evidence from inference, and
make uncertainty explicit.

### Validation

The epistemology should let different contributors evaluate the same claim in a
consistent way.

## Acceptance Criteria

- The project's knowledge philosophy is explicit.
- Provenance and confidence expectations are clear.
- Conflicting information has a resolution model.
- The document remains applicable to both humans and AI.

## AI Authoring Strategy

AI systems should:

1. read the governance and identity documents,
2. define how evidence should be weighed,
3. explain confidence and uncertainty,
4. avoid turning epistemology into project-specific conclusions.

## Rationale & Context

Renderflow is documented, tested, and increasingly AI-assisted. Without a shared
way to judge evidence, contributors may confuse intended architecture with
accidental implementation, or treat confident guesses as facts. This document
provides a common reasoning discipline.

## Dependencies & External Integrations

### Upstream Dependencies

- `PURPOSE.md`
- `PRINCIPLES.md`
- `AI_CONSTITUTION.md`

### Downstream Dependencies

- `DECISIONS.md`
- AI reasoning workflows
- review practice
- documentation governance

## Examples & Edge Cases

### Example

A claim about Renderflow's graph planner should be checked first against the
canonical architecture and relevant docs, then against source and tests, rather
than inferred only from one code path.

### Edge Case

If documentation and implementation disagree, the epistemology does not assume
one is automatically correct; it requires checking the canonical owner of the
claim and preserving the uncertainty until resolved.

## Validation Criteria

This document is valid when contributors can explain not only what Renderflow
believes, but how Renderflow decides whether a belief is justified.

## Related Specifications

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/meta/epistemology.spec.md`
