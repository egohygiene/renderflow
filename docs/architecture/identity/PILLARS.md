---
title: Pillars — Renderflow
version: 1.0
date_created: 2026-07-21
last_updated: 2026-07-21
owner: Renderflow Maintainers
tags:
  - architecture
  - identity
  - pillars
---

# PILLARS

## Introduction

The pillars are Renderflow's enduring strategic capabilities.

## Conformance

This document is authored in conformance with:

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/identity/pillars.spec.md`

## Purpose & Scope

This document defines the long-lived capability areas Renderflow must keep
strengthening in order to fulfill its purpose and vision.

It does not define implementation projects or release milestones.

## Definitions

- **Pillar**: an enduring capability area that sustains the project over time.
- **Strategic capability**: a broad competency required to fulfill the vision.
- **Capability investment**: work that strengthens a pillar without redefining it.

## Renderflow Pillars

### 1. Specification-First Workflow

Renderflow must remain centered on explicit specifications as the primary way to
describe build intent, variables, targets, and transformation paths.

### 2. Universal Transformation Planning

Renderflow must continue treating transformation as a graph-aware planning
problem so that branching, optimization, and reuse remain first-class.

### 3. Deterministic and Reproducible Execution

Renderflow must preserve trust through predictable execution, inspectable plans,
and cache behavior that rewards stable inputs.

### 4. Extensible Capability Surface

Renderflow must grow through plugins, transform registries, AI backends, and
other explicit contracts rather than by hard-coding every specialized need into
the core.

### 5. Portable Delivery and Adoption

Renderflow must remain practical across local development, CI, documentation,
and packaged distribution so the architecture can travel with the project.

## Requirements, Constraints & Guidelines

### Requirements

- Each pillar must support the purpose and vision.
- Pillars must remain broader and more durable than individual features.
- Strategic capabilities must be distinct from decision principles.

### Constraints

- Roadmap items must not be mistaken for pillars.
- Pillars must not depend on a specific provider, format, or release channel.
- Overlapping ownership between pillars should be minimized.

### Guidelines

- Keep the set small.
- Use pillars to organize long-term investment.
- Prefer capabilities that can absorb future implementation change.

## Authoring Contract

### Purpose

Own the enduring capability structure behind Renderflow's strategy.

### Responsibilities

This document owns:

- the strategic capability areas the project must preserve,
- the bridge between high-level principles and lower-level methodology,
- language for long-term investment decisions.

### Non-Responsibilities

This document does not own:

- tactical initiatives,
- implementation design,
- current system decomposition,
- release planning.

### Inputs

- `PURPOSE.md`
- `VISION.md`
- `MANIFESTO.md`
- `PRINCIPLES.md`

### Outputs

- `METHODOLOGY.md`
- `FOUNDATIONS.md`
- `ROADMAP.md`

### AI Generation Rules

AI systems should define durable capability areas, not task backlogs.

### Validation

A proposed long-term investment should fit clearly under one or more pillars.

## Acceptance Criteria

- Pillars are strategically meaningful and distinct.
- They support the vision without duplicating principles.
- They remain implementation-independent.
- They can organize downstream methodology and roadmap thinking.

## AI Authoring Strategy

AI systems should:

1. read purpose, vision, manifesto, and principles,
2. identify the enduring capabilities those documents require,
3. keep the list small and stable,
4. avoid roadmap phrasing.

## Rationale & Context

Renderflow already spans CLI usage, graph planning, caching, plugins, AI
transforms, packaging, and documentation. The pillars identify the durable
capability areas that let those surfaces evolve without losing coherence.

## Dependencies & External Integrations

### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`
- `MANIFESTO.md`
- `PRINCIPLES.md`

### Downstream Dependencies

- `METHODOLOGY.md`
- `FOUNDATIONS.md`
- `ROADMAP.md`

## Examples & Edge Cases

### Example

Work on better execution-plan visualization strengthens the Universal
Transformation Planning pillar even if the underlying renderer changes.

### Edge Case

A one-off format adapter may be useful, but it is not a pillar unless it
strengthens one of the enduring capability areas above.

## Validation Criteria

This document is valid when Renderflow's long-term investments can be organized
without adding feature-specific pseudo-pillars.

## Related Specifications

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/identity/pillars.spec.md`
