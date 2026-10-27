---
schema: aether.architecture-document/v1
id: renderflow-system
title: Renderflow System
kind: architecture-document
version: 0.1.1
status: draft
owners:
  - egohygiene
created: 2026-08-19
updated: 2026-10-26
governed_by:
  - architecture-system
depends_on:
  - renderflow-foundations
  - renderflow-ontology
related:
  - renderflow-purpose
  - renderflow-vision
  - renderflow-principles
  - renderflow-pillars
supersedes: []
---

# Renderflow System

## Purpose and scope

This document identifies Renderflow's logical systems and responsibilities.
Start with [ARCHITECTURE.md](ARCHITECTURE.md) for structural boundaries and
dependency rules; use the [source map](docs/architecture/overview.md#source-map)
for implementation locations. These are responsibility boundaries, not seven
separately deployed services.

## System inventory

| System | Responsibility and boundary | Contract or evidence pointer |
| --- | --- | --- |
| Specification loader | Load versioned intent, normalize supported legacy input, resolve configuration paths, and reject invalid source/target/policy combinations. Validation of intent does not execute or approve outputs. | [Spec v2 reference](docs/user-guide/spec-v2-reference.md) |
| Transform registry | Describe available directions, provider/tool requirements, loss and determinism declarations; bind selected capabilities to adapters. Tool discovery is separate from route qualification, and reverse routes are not inferred. | [Tool registry](docs/user-guide/tool-registry.md), [creative capability matrix](docs/user-guide/creative-capabilities.md) |
| Graph planner and build selection | Resolve a build once, select the graph or composition engine, preserve target membership and identity, and explain unsupported branches. Graph routes share eligible intermediates; composition retains locale/output identity in its own plan and artifact forest. | [Derivative forest](docs/architecture/derivative-forest.md), [composition integration](docs/work/406/PLANNER_INTEGRATION.md) |
| Execution engine | Execute captured plans through selected adapters with bounded resource, cancellation and policy behavior. Preserve per-branch failure and partial outcomes. Native composition owns its typography/rendering, not a duplicate CLI renderer. | [Process execution](docs/process-execution.md), [page composition](docs/user-guide/page-composition.md), [execution evidence](docs/execution-evidence.md) |
| Artifact and cache manager | Retain immutable bytes, IDs, lineage, manifests and permitted reuse; distinguish sources, intermediates, rejected candidates and published outputs. Reuse requires its own verification and never inherits creative approval. | [Artifact kernel](docs/artifact-kernel.md), [provider/checkpoint contract](docs/provider-contract.md), [validation](docs/user-guide/artifact-validation.md) |
| CLI and Rust library | Adapt user/library requests to shared planning and execution; return structured plans, diagnostics, progress and results. The standalone executable remains thin; library callers use the public SDK without invoking a second domain implementation. | [CLI build](docs/cli-reference/build.md), [public SDK exports](crates/renderflow-core/src/lib.rs), [Flow handoff](docs/architecture/flow-artifact-forest.md) |
| Documentation and release surface | Explain supported and unsupported behavior, retain qualification evidence, and publish only the declared verified artifact scope. Documentation and source versions do not create a released capability. | [Live roadmap](https://github.com/egohygiene/renderflow/issues/367), [release gate](docs/release-candidate.md) |

All seven systems have current source or documentation surfaces. This describes
ownership, not a blanket qualification status: the creative/composition
checkpoints retain deferred tests and native/visual/platform evidence, and the
first integration candidate remains proposed. The plugin SDK is an alpha
contract. See each linked contract and issue for its actual limits.

## External systems

- Consumer projects, including document and publication projects, own canonical
  content, assets, rights and approval.
- Flow owns cross-tool orchestration; its Renderflow adapter and released
  consumer proof remain tracked in [Flow #52](https://github.com/egohygiene/flow/issues/52).
- Local tools and optional AI services supply only the selected adapter's
  capabilities. Renderflow retains provider identity, policy and loss evidence.

External systems are integrations, not hidden implementation units. Each requires version, authentication, availability, data, error, and replacement boundaries appropriate to its risk.

## System interactions

Inputs enter through a validated contract. Planning captures intent and source
state; execution records observed outcomes. Output publication joins exact
target identities to artifact records rather than counting requested paths as
produced files. Domain manifests support downstream decisions only after their
paths, bytes and relevant evidence are checked. Logs and progress do not replace
that verification or human review.

## Failure model

Systems fail closed at destructive, publication, privacy, and security boundaries. Partial results identify coverage and remain distinguishable from complete success.

## Evidence and uncertainty

This responsibility map was reconciled against source at
`9dac5428df38691b2922e09ea02d51b02a7f43eb` for
[#459](https://github.com/egohygiene/renderflow/issues/459). It adds no provider,
runtime validation or release evidence. [PR #455](https://github.com/egohygiene/renderflow/pull/455)
remains draft-only at this checkpoint; its original-Fountain and transcript
export additions are not main or released capabilities. The
[architecture entry](ARCHITECTURE.md#evidence-and-evolution) keeps current,
draft, proposed and published states distinct.
