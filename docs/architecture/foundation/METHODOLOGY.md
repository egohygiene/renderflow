---
title: Methodology — Renderflow
version: 1.0
date_created: 2026-07-21
last_updated: 2026-07-21
owner: Renderflow Maintainers
tags:
  - architecture
  - foundation
  - methodology
---

# METHODOLOGY

## Introduction

Renderflow's methodology describes how work should move from intent to validated
capability.

## Conformance

This document is authored in conformance with:

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/foundation/methodology.spec.md`

## Purpose & Scope

This document defines the preferred way to design, change, validate, and evolve
Renderflow.

It covers engineering workflow, validation philosophy, documentation practice,
and AI-assisted contribution. It does not define roadmap phases or low-level
implementation tactics.

## Definitions

- **Specification-first engineering**: defining intent and contracts before implementation work expands.
- **Validation loop**: the cycle of inspect, change, test, and review.
- **Architectural fit**: consistency between a proposed change and the project's purpose, principles, and foundations.

## Renderflow Methodology

### 1. Start with explicit intent

Work should begin with a specification, architecture document, user-visible
contract, or repository artifact that makes the intended change legible before
implementation spreads it across modules.

### 2. Prefer progressive refinement

Renderflow should evolve from identity to system design to implementation.
Changes should explain themselves at the right architectural level instead of
forcing contributors to infer direction from code alone.

### 3. Treat planning as part of delivery

Because Renderflow exists to make transformation intent explicit, its own
engineering workflow should value design, plan visibility, and pre-execution
reasoning rather than only end-state artifacts.

### 4. Validate early and close to the change

Linting, tests, dry runs, documentation builds, and targeted verification should
happen as soon as a change can be evaluated.

### 5. Extend by contract

New capabilities should prefer schema extensions, transform definitions,
registries, plugins, and reusable abstractions over one-off branching.

### 6. Use AI under supervision

AI may accelerate authoring, analysis, and implementation, but it must remain
reviewable, constrained by architecture, and validated through the same quality
bar as human work.

### 7. Preserve portability in the workflow itself

Documentation, CI, packaging, and release automation should reinforce the same
repeatability that the product expects from transformation pipelines.

## Requirements, Constraints & Guidelines

### Requirements

- Methodology must align with specification-first and deterministic principles.
- Validation must cover code, docs, and user-facing behavior when relevant.
- AI-assisted work must remain reviewable and governed.

### Constraints

- Workflow convenience must not outrank architectural clarity.
- Temporary implementation shortcuts must not become default process.
- Tooling must not replace architectural judgment.

### Guidelines

- Prefer small, reviewable changes.
- Prefer targeted validation before broad validation.
- Prefer documentation and examples that age with the architecture.

## Authoring Contract

### Purpose

Own how Renderflow work should be performed.

### Responsibilities

This document owns:

- engineering workflow philosophy,
- validation expectations,
- the relationship between specifications, code, docs, and AI contribution.

### Non-Responsibilities

This document does not own:

- system decomposition,
- implementation details,
- release phase sequencing,
- domain vocabulary.

### Inputs

- `PURPOSE.md`
- `PRINCIPLES.md`
- `PILLARS.md`
- repository workflows for CI, docs, and release

### Outputs

- `FOUNDATIONS.md`
- `DECISIONS.md`
- `AI_CONSTITUTION.md`
- engineering practices and contribution workflows

### AI Generation Rules

AI systems should describe stable methods of working, not current command syntax.

### Validation

The methodology should help contributors decide how to approach work before they
choose tools.

## Acceptance Criteria

- The methodology defines a repeatable way of working.
- Validation philosophy is explicit.
- AI-assisted contribution is governed.
- The document stays independent of temporary tooling choices.

## AI Authoring Strategy

AI systems should:

1. read upstream identity documents,
2. infer the working methods required to preserve those values,
3. describe workflows and validation loops,
4. avoid turning the document into a tool manual.

## Rationale & Context

Renderflow is itself a system about explicit transformation intent. Its internal
engineering process should therefore resist accidental complexity, undocumented
exceptions, and validation that happens only at the end.

## Dependencies & External Integrations

### Upstream Dependencies

- `PURPOSE.md`
- `PRINCIPLES.md`
- `PILLARS.md`

### Downstream Dependencies

- `FOUNDATIONS.md`
- `DECISIONS.md`
- `AI_CONSTITUTION.md`
- contribution and release workflows

## Examples & Edge Cases

### Example

Adding a new transform type should update the relevant schema, user-facing docs,
and validation paths instead of appearing only as implementation code.

### Edge Case

A one-off emergency fix may bypass parts of the normal workflow, but it should
be reconciled back into the documented methodology once the immediate problem has
passed.

## Validation Criteria

This document is valid when contributors can use it to choose an approach to
work before they choose an implementation.

## Related Specifications

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/foundation/methodology.spec.md`
