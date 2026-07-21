---
title: Manifesto — Renderflow
version: 1.0
date_created: 2026-07-21
last_updated: 2026-07-21
owner: Renderflow Maintainers
tags:
  - architecture
  - identity
  - manifesto
---

# MANIFESTO

## Introduction

Renderflow's manifesto states the beliefs that justify its architecture and the
culture expected around it.

## Conformance

This document is authored in conformance with:

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/identity/manifesto.spec.md`

## Purpose & Scope

This document expresses what Renderflow believes about transformation systems,
automation, and maintainable publishing workflows.

It is not a feature list, implementation guide, or roadmap.

## Definitions

- **Belief**: a durable idea that shapes decisions.
- **Conviction**: a principle held strongly enough to influence trade-offs.
- **Culture**: the pattern of choices a project repeatedly rewards.

## Renderflow Manifesto

Renderflow believes that:

1. **Specifications are better memory than scripts.** Intent should survive people, machines, and time.
2. **Planning deserves first-class treatment.** Systems should explain what will happen before they do it.
3. **Transformation is a graph problem whenever work can branch, merge, or be reused.** Linear pipelines are useful, but they are not the full truth.
4. **Determinism is a form of respect.** Contributors deserve builds they can reason about.
5. **Extensibility should not require forking the core.** Plugins and explicit contracts are healthier than hidden patch points.
6. **AI must remain supervised, optional, and inspectable.** Intelligence is useful only when it stays inside clear operational boundaries.
7. **Portability matters more than local convenience.** A workflow that only works on one machine is not yet a durable workflow.
8. **Documentation is part of the product.** A system that cannot explain itself cannot scale safely to new contributors or AI agents.

## Requirements, Constraints & Guidelines

### Requirements

- The manifesto must express beliefs that can guide architecture over time.
- Beliefs must align with Renderflow's purpose and vision.
- The document must explain cultural identity, not operational detail.

### Constraints

- Temporary implementation choices must not appear as convictions.
- Marketing language must not replace clear architectural beliefs.
- Tactical work must not be presented as philosophy.

### Guidelines

- Write with conviction.
- Prefer memorable statements.
- Prefer beliefs that explain why Renderflow's architecture takes its current shape.

## Authoring Contract

### Purpose

Own the project's core beliefs.

### Responsibilities

This document owns:

- Renderflow's cultural identity,
- the beliefs that justify declarative workflows,
- the philosophy behind planning, reproducibility, and extensibility.

### Non-Responsibilities

This document does not own:

- system decomposition,
- implementation details,
- design-system primitives,
- roadmap phases.

### Inputs

- `PURPOSE.md`
- `VISION.md`
- observed repository emphasis on graph planning, reproducibility, plugins, and documentation

### Outputs

- `PRINCIPLES.md`
- `PILLARS.md`
- `AI_CONSTITUTION.md`

### AI Generation Rules

AI systems should write beliefs strong enough to shape trade-offs, while keeping
them independent of current tooling.

### Validation

If a decision can cite this manifesto as its rationale, the document is working.

## Acceptance Criteria

- Renderflow's core beliefs are explicit.
- The manifesto aligns with purpose and vision.
- The statements are timeless enough to survive implementation change.
- The document inspires without becoming promotional.

## AI Authoring Strategy

AI systems should:

1. read `PURPOSE.md` and `VISION.md`,
2. infer the beliefs required to support them,
3. express those beliefs with clarity and conviction,
4. avoid implementation- or release-specific wording.

## Rationale & Context

Renderflow already encodes strong beliefs in practice: it uses declared specs,
can inspect graph plans, distinguishes planning from execution, and treats AI as
optional configuration. The manifesto makes those beliefs explicit so future work
does not accidentally drift away from them.

## Dependencies & External Integrations

### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`

### Downstream Dependencies

- `PRINCIPLES.md`
- `PILLARS.md`
- `AI_CONSTITUTION.md`
- `METHODOLOGY.md`

## Examples & Edge Cases

### Example

When deciding whether to add a new transformation pathway, the manifesto favors
an explicit graph-visible capability over opaque automation.

### Edge Case

A technically useful shortcut that hides planning from users may still be
rejected if it violates the belief that planning deserves first-class treatment.

## Validation Criteria

This document is valid when contributors can use it to explain why Renderflow
values inspectability, determinism, and extensibility.

## Related Specifications

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/identity/manifesto.spec.md`
