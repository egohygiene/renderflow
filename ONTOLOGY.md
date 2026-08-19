---
schema: aether.architecture-document/v1
id: renderflow-ontology
title: Renderflow Ontology
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-19
updated: 2026-08-19
governed_by:
  - architecture-ontology
depends_on:
  - renderflow-purpose
  - renderflow-vision
  - renderflow-principles
  - renderflow-epistemology
related:
  - renderflow-pillars
  - renderflow-manifesto
  - renderflow-ai-constitution
  - renderflow-personal-model
supersedes: []
---

# Renderflow Ontology

## Domain scope

Renderflow models the concepts needed for make complex, multi-format document and media rendering reproducible, inspectable, and easy to operate. The ontology names conceptual entities and relationships; it is not a source-code class model, API schema, or database design.

## Canonical concepts

| Concept | Meaning |
| --- | --- |
| Source artifact | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |
| Render specification | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |
| Transform | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |
| Graph | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |
| Executor | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |
| Intermediate artifact | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |
| Output artifact | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |
| Cache entry | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |
| Diagnostic | A canonical concept in the Renderflow domain whose exact fields belong to specifications or schemas, not this ontology. |

## Core relationships

- A repository or person provides source context to one or more domain artifacts.
- A specification constrains how an artifact is interpreted or produced.
- A plan separates proposed action from execution.
- Evidence supports a claim; a decision authorizes a durable direction.
- Provenance connects derived artifacts to their inputs and processing context.
- A consumer integrates through an explicit interface rather than internal structure.

## Boundaries

- Conceptual identity is distinct from filesystem path, database identifier, or display label.
- Observed state is distinct from desired state.
- Proposed relationships are not accepted facts.
- Neighboring repositories retain ownership of their domain concepts.

## Evidence and uncertainty

- **Observed:** The repository README and checked-in implementation establish a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts.
- **Decided for this draft:** The repository owns the bounded concern described here and participates through versioned contracts.
- **Proposed:** Target systems and later roadmap phases remain proposals until accepted and implemented.
- **Open question:** Which parts of this draft should become active in the first independently versioned release?
