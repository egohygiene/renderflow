---
title: Ontology — Renderflow
version: 1.0
date_created: 2026-07-21
last_updated: 2026-07-21
owner: Renderflow Maintainers
tags:
  - architecture
  - domain
  - ontology
---

# ONTOLOGY

## Introduction

Renderflow's ontology defines the canonical vocabulary used throughout its
architecture.

## Conformance

This document is authored in conformance with:

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/domain/ontology.spec.md`

## Purpose & Scope

This document defines the concepts, entities, and relationships that describe
Renderflow's domain.

It does not define implementation modules, storage models, or APIs.

## Definitions

### Core concepts

- **Source Artifact**: the original content or media from which work begins.
- **Specification**: the declared intent describing inputs, outputs, variables, and execution context.
- **Output**: a desired end artifact produced by Renderflow.
- **Format**: a representation such as Markdown, HTML, PDF, DOCX, image, or audio.
- **Transform**: a conversion or mutation step that moves content from one meaningful state to another.
- **Transform Graph**: the navigable space of possible transformations among formats.
- **Execution Plan**: a presentation-independent description of the chosen path or DAG.
- **DAG**: the dependency structure that orders shared work without cycles.
- **Wave**: a set of DAG work items that can execute concurrently because their dependencies are satisfied.
- **Optimization Mode**: the strategy used to choose among possible paths, such as speed, quality, balanced, or pareto-oriented trade-offs.
- **Renderer**: a capability that materializes content into a target output form.
- **Registry**: a bounded catalog of named capabilities.
- **Plugin**: an externally supplied capability that enters Renderflow through a registry-backed contract.
- **AI Transform**: a transform whose behavior depends on a configured AI provider while remaining subject to Renderflow governance.
- **Cache**: persisted evidence that prior work may be safely reused for the same declared context.
- **Diagnostic**: an explanation of plan shape, validity, health, or failure state.

## Domain Relationships

- A **Specification** declares one or more desired **Outputs** for a **Source Artifact**.
- A **Transform** moves a **Format** toward another **Format**.
- The collection of available transforms forms a **Transform Graph**.
- Planning selects part of that graph and produces an **Execution Plan**.
- Multi-target planning may merge selected paths into a **DAG**.
- A **Wave** is derived from the DAG's ready work.
- A **Renderer**, **Plugin**, or **AI Transform** may execute a transform step.
- A **Cache** can satisfy or skip work only when its provenance matches the relevant declared context.
- A **Diagnostic** may describe the specification, graph, plan, execution, or capability surface.

## Requirements, Constraints & Guidelines

### Requirements

- Canonical terms must be defined once and used consistently.
- Relationships between concepts must be explicit.
- The ontology must support both human and AI understanding.

### Constraints

- Implementation names must not replace conceptual language.
- Synonyms that blur ownership should be avoided.
- This document must not become a system architecture description.

### Guidelines

- Prefer one primary name per concept.
- Prefer concise definitions.
- Prefer terms that remain valid across implementation change.

## Authoring Contract

### Purpose

Own Renderflow's canonical domain vocabulary.

### Responsibilities

This document owns:

- definitions,
- conceptual relationships,
- shared language for every downstream architecture document.

### Non-Responsibilities

This document does not own:

- system boundaries,
- implementation details,
- engineering process,
- design philosophy.

### Inputs

- `FOUNDATIONS.md`
- repository docs describing graph planning, DAG execution, plugins, caching, and AI

### Outputs

- `SYSTEM.md`
- `ARCHITECTURE.md`
- `DESIGN.md`
- `DECISIONS.md`
- `META.md`

### AI Generation Rules

AI systems should normalize terminology and eliminate ambiguous synonyms.

### Validation

A contributor should be able to read this document and interpret downstream
architecture without inventing new terms.

## Acceptance Criteria

- Core domain concepts are defined.
- Relationships are explicit.
- Terminology is stable and canonical.
- Implementation detail is absent.

## AI Authoring Strategy

AI systems should:

1. inspect the repository's recurring concepts,
2. normalize them into one vocabulary,
3. define their relationships,
4. ensure downstream documents can reference this ontology.

## Rationale & Context

Renderflow spans standard pipelines, graph planning, DAG execution, plugins,
AI-assisted transforms, and multiple artifact types. Without a canonical
ontology, contributors will describe the same idea with different words and lose
architectural clarity.

## Dependencies & External Integrations

### Upstream Dependencies

- `FOUNDATIONS.md`

### Downstream Dependencies

- `SYSTEM.md`
- `ARCHITECTURE.md`
- `DESIGN.md`
- `DECISIONS.md`
- `META.md`

## Examples & Edge Cases

### Example

A specification that produces HTML and PDF from one Markdown source may use one
Transform Graph, produce one multi-target Execution Plan, and execute shared
work through a DAG.

### Edge Case

An AI transform is still a transform in the ontology. Its special governance does
not justify a separate vocabulary that disconnects it from planning and
execution.

## Validation Criteria

This document is valid when downstream architecture can refer to concepts such as
Transform Graph, Execution Plan, Wave, and Cache without redefining them.

## Related Specifications

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/domain/ontology.spec.md`
