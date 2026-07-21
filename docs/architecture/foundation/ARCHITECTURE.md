---
title: Architecture — Renderflow
version: 1.0
date_created: 2026-07-21
last_updated: 2026-07-21
owner: Renderflow Maintainers
tags:
  - architecture
  - foundation
  - architecture
---

# ARCHITECTURE

## Introduction

Renderflow's architecture describes how its major systems are organized to turn
declared intent into trustworthy outputs.

## Conformance

This document is authored in conformance with:

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`

## Purpose & Scope

This document defines Renderflow's structural organization, dependency
direction, and architectural boundaries.

It does not define code-level modules, APIs, deployment layouts, or current
framework choices.

## Definitions

- **Layer**: a grouping of responsibilities with a common architectural role.
- **Dependency direction**: the permitted flow of reliance among layers.
- **Architectural boundary**: a structural separation that preserves clarity and change isolation.

## Renderflow Architecture

### Layer 1: Intent Layer

The Intent Layer captures what the operator wants.

It contains the normalized meaning of source artifacts, output declarations,
variables, and transform definitions. Its responsibility is to make requested
work explicit before the system reasons about execution.

### Layer 2: Planning Layer

The Planning Layer reasons about possible work.

It owns transform-graph construction, optimization strategy, path selection,
execution-plan generation, multi-target DAG shaping, and explanatory plan views.
Its responsibility is to decide how intent can be fulfilled.

### Layer 3: Capability Layer

The Capability Layer provides the bounded actions the planner may rely on.

It includes built-in transforms, command-defined transforms, plugin-backed
transforms, AI transforms, and render strategies. Its responsibility is to make
valid transformations available without collapsing planning into execution.

### Layer 4: Execution Layer

The Execution Layer performs work.

It owns pipeline execution, DAG wave execution, cache-aware reuse, output
materialization, and concurrency. Its responsibility is to carry out selected
plans repeatably and efficiently.

### Layer 5: Trust Layer

The Trust Layer makes behavior understandable and governable.

It owns diagnostics, dry runs, dependency awareness, incremental correctness,
documentation-facing explanation, AI governance, CI validation, and release
confidence mechanisms. Its responsibility is to preserve confidence in both the
process and the result.

## Dependency Direction

Renderflow follows these dependency rules:

- The **Planning Layer** depends on the Intent Layer's declared meaning.
- The **Capability Layer** exposes bounded actions that the Planning Layer can reason about.
- The **Execution Layer** depends on planning output and capability contracts.
- The **Trust Layer** observes and governs every layer but does not replace their responsibilities.
- Lower layers must not redefine the meaning of higher-level intent.
- Capability implementations may evolve as long as the planner-facing contract remains stable.

## Communication Patterns

- **Intent → Planning**: specifications become normalized requests and available graph space.
- **Planning → Execution**: execution plans and DAGs describe ordered work.
- **Capability ↔ Execution**: execution invokes bounded transforms and renderers.
- **Execution → Trust**: produced artifacts, cache decisions, and failures emit explanations.
- **Trust → Operator**: diagnostics, plans, docs, and workflows preserve inspectability.

## Architectural Constraints

- Planning must remain visible independently of execution.
- Extensibility must preserve architectural contracts.
- Shared intermediates must be representable without duplicating work.
- AI participation must remain optional, governed, and subordinate to intent.
- Packaging and documentation should extend the architecture's reach without redefining its core.

## Requirements, Constraints & Guidelines

### Requirements

- Major layers must be explicit.
- Dependency direction must be understandable.
- Boundaries must support evolution, portability, and maintainability.

### Constraints

- Source code structure must not define the architecture.
- Implementation details must not replace architectural description.
- Temporary integrations must not distort durable layer boundaries.

### Guidelines

- Prefer stable abstractions.
- Minimize cross-layer leakage.
- Keep responsibility boundaries explicit.

## Authoring Contract

### Purpose

Own Renderflow's structural organization.

### Responsibilities

This document owns:

- architectural layers,
- boundary definitions,
- dependency direction,
- communication patterns,
- structural constraints.

### Non-Responsibilities

This document does not own:

- detailed system inventory,
- implementation modules,
- design-system language,
- roadmap sequencing.

### Inputs

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- `SYSTEM.md`
- `ONTOLOGY.md`
- `DESIGN.md`

### Outputs

- implementation decomposition,
- documentation structure,
- engineering review,
- AI contributor understanding

### AI Generation Rules

AI systems should begin with system ownership, then organize those systems into
clear layers with explicit dependency direction.

### Validation

The architecture should explain how Renderflow is organized without requiring
source-code inspection.

## Acceptance Criteria

- Architectural layers are clearly defined.
- Boundaries and dependency direction are explicit.
- The architecture aligns with Renderflow's principles.
- Implementation detail is absent.

## AI Authoring Strategy

AI systems should:

1. read upstream documents,
2. map major systems into layers,
3. document dependency direction,
4. preserve stable abstractions,
5. avoid file-tree descriptions.

## Rationale & Context

Renderflow currently presents itself through a config-driven interface, a graph
planner, a transform capability surface, DAG execution, diagnostics, and
cross-platform delivery. The architecture explains why those concerns are
separate and how they work together without reducing the project to any one
implementation path.

## Dependencies & External Integrations

### Upstream Dependencies

- `PURPOSE.md`
- `VISION.md`
- `PRINCIPLES.md`
- `SYSTEM.md`
- `ONTOLOGY.md`
- `DESIGN.md`

### Downstream Dependencies

- implementation modules
- repository organization
- documentation structure
- AI engineering workflows

## Examples & Edge Cases

### Example

A graph plan rendered as Mermaid is a Trust-Layer view over Planning-Layer
output; it is not itself the architecture.

### Edge Case

A standard single-output build may use less of the Planning Layer than a
multi-target graph build, but it still fits the same architecture because intent,
capability, execution, and trust remain distinct.

## Validation Criteria

This document is valid when contributors can explain how Renderflow turns
specifications into outputs using layered responsibilities instead of
module-specific knowledge.

## Related Specifications

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/foundation/architecture.spec.md`
