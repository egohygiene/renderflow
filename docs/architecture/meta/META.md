---
title: Meta — Renderflow Architecture
version: 1.0
date_created: 2026-07-21
last_updated: 2026-07-21
owner: Renderflow Maintainers
tags:
  - architecture
  - meta
  - knowledge-base
---

# META

## Introduction

This document explains how Renderflow's architecture documentation is organized,
how the documents relate to one another, and how the knowledge base should
remain coherent over time.

## Conformance

This document is authored in conformance with:

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/meta/meta.spec.md`

## Purpose & Scope

This document describes the architecture framework itself.

It covers architectural layers, document relationships, canonical ownership,
reading order, and evolution guidance. It does not restate the content of each
document or replace project architecture.

## Definitions

- **Architecture framework**: the full collection of architecture documents.
- **Layer**: a logical grouping of documents with similar concern type.
- **Canonical owner**: the one document responsible for a specific concern.
- **Dependency graph**: the directed relationships among architecture documents.

## Architecture Framework

### Layer model

Renderflow's architecture knowledge base is organized into six layers:

1. **Identity** — why Renderflow exists and the beliefs guiding it.
2. **Foundation** — how Renderflow works conceptually and how it should evolve structurally.
3. **Domain** — the vocabulary and human model needed to reason about the project.
4. **Governance** — the memory of important accepted decisions.
5. **Experience** — the desired user experience and the reusable language that expresses it.
6. **Meta** — governance for AI participation, knowledge evaluation, and the architecture framework itself.

### Canonical ownership

- `PURPOSE.md` owns reason for existence.
- `VISION.md` owns the future state.
- `MANIFESTO.md` owns beliefs.
- `PRINCIPLES.md` owns decision rules.
- `PILLARS.md` owns enduring capability areas.
- `METHODOLOGY.md` owns how work should be performed.
- `FOUNDATIONS.md` owns stable architectural assumptions.
- `ONTOLOGY.md` owns vocabulary.
- `PERSONAL_MODEL.md` owns assumptions about people.
- `SYSTEM.md` owns major systems and capability boundaries.
- `ARCHITECTURE.md` owns structural organization and dependency direction.
- `DESIGN.md` owns design philosophy.
- `DESIGN_SYSTEM.md` owns reusable design language.
- `ROADMAP.md` owns strategic evolution.
- `DECISIONS.md` owns accepted historical decisions.
- `AI_CONSTITUTION.md` owns AI governance.
- `EPISTEMOLOGY.md` owns knowledge philosophy.
- `META.md` owns the architecture framework itself.

### Reading order

The intended dependency flow is:

```text
PURPOSE
    ↓
VISION
    ↓
MANIFESTO
    ↓
PRINCIPLES
    ↓
PILLARS
    ↓
METHODOLOGY
    ↓
FOUNDATIONS
    ↓
ONTOLOGY
    ↓
SYSTEM
    ↓
ARCHITECTURE
    ↓
DESIGN
    ↓
DESIGN_SYSTEM
    ↓
ROADMAP

DECISIONS, AI_CONSTITUTION, and EPISTEMOLOGY are cross-cutting governance
views that depend on the upstream architecture while informing future work.
```

### Evolution rules

- Add a new architecture document only when an existing canonical owner cannot responsibly contain the concern.
- Update dependencies when document relationships change.
- Prefer refining one canonical document over duplicating the same concept elsewhere.
- Keep implementation views separate from canonical architecture knowledge.

## Requirements, Constraints & Guidelines

### Requirements

- Layers and document relationships must be explicit.
- Canonical ownership must remain clear.
- The dependency graph must be understandable to humans and AI systems.

### Constraints

- This document must not duplicate the content of downstream documents.
- Implementation details must stay out of the architecture framework description.
- Project-specific engineering guidance must not replace architectural navigation.

### Guidelines

- Explain relationships rather than content.
- Favor conceptual clarity over exhaustive repetition.
- Treat the architecture itself as a maintained system.

## Authoring Contract

### Purpose

Own the organization and maintenance model of the architecture knowledge base.

### Responsibilities

This document owns:

- architecture layering,
- document relationships,
- canonical ownership,
- navigation guidance,
- evolution rules.

### Non-Responsibilities

This document does not own:

- project purpose,
- system design,
- methodology detail,
- implementation guidance.

### Inputs

- every architecture document
- the architecture specifications
- dependency relationships among documents

### Outputs

- contributor onboarding
- AI architecture generation
- architecture maintenance practice
- future architecture evolution

### AI Generation Rules

AI systems should map the architecture framework, preserve canonical ownership,
and avoid restating document contents.

### Validation

A new contributor should be able to understand how to navigate the architecture
before reading every individual file.

## Acceptance Criteria

- Architecture layers are documented.
- Document relationships and ownership are explicit.
- Dependency flow is understandable.
- The framework can evolve without losing coherence.

## AI Authoring Strategy

AI systems should:

1. read every architecture document,
2. identify layers and ownership,
3. map dependency flow,
4. explain navigation and evolution,
5. avoid duplicating individual documents.

## Rationale & Context

Renderflow now contains a deliberate architecture knowledge base rather than a
small set of isolated implementation notes. `META.md` prevents that knowledge
base from becoming another undocumented system by making ownership, layering,
and reading order explicit.

## Dependencies & External Integrations

### Upstream Dependencies

- every architecture document

### Downstream Dependencies

- contributor onboarding
- AI architecture generation
- documentation maintenance
- architecture evolution

## Examples & Edge Cases

### Example

A contributor looking for the meaning of "Execution Plan" should go to
`ONTOLOGY.md`, while a contributor asking how plans relate to layers should go to
`ARCHITECTURE.md`.

### Edge Case

If a new document about policy or reliability is added, `META.md` must be
updated to explain where it sits in the architecture framework and what concern
it canonically owns.

## Validation Criteria

This document is valid when contributors can navigate the architecture system
without guessing where a concept belongs.

## Related Specifications

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/meta/meta.spec.md`
