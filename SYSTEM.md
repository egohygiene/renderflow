---
schema: aether.architecture-document/v1
id: renderflow-system
title: Renderflow System
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-19
updated: 2026-08-19
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

This document identifies Renderflow's logical systems and responsibilities. It answers what the major systems do; [ARCHITECTURE.md](ARCHITECTURE.md) owns their structural organization and dependency rules.

## System inventory

| System | State | Responsibility |
| --- | --- | --- |
| Specification loader | Current | Owns its bounded portion of a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts; exposes explicit inputs, outputs, failure states, and evidence. |
| Transform registry | Current | Owns its bounded portion of a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts; exposes explicit inputs, outputs, failure states, and evidence. |
| Graph planner | Current | Owns its bounded portion of a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts; exposes explicit inputs, outputs, failure states, and evidence. |
| Execution engine | Current | Owns its bounded portion of a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts; exposes explicit inputs, outputs, failure states, and evidence. |
| Artifact and cache manager | Current or evolving | Owns its bounded portion of a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts; exposes explicit inputs, outputs, failure states, and evidence. |
| CLI and Rust library | Current or evolving | Owns its bounded portion of a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts; exposes explicit inputs, outputs, failure states, and evidence. |
| Documentation and release surface | Current or evolving | Owns its bounded portion of a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts; exposes explicit inputs, outputs, failure states, and evidence. |

## External systems

- Beacon document projects
- Reflector publications
- Flow orchestration
- Pandoc, FFmpeg, Tera, and optional AI providers

External systems are integrations, not hidden implementation units. Each requires version, authentication, availability, data, error, and replacement boundaries appropriate to its risk.

## System interactions

Inputs enter through an adapter or validated contract, move through domain systems, produce artifacts and diagnostics, and leave through a stable interface. Evidence flows back to validation, review, and future decisions.

## Failure model

Systems fail closed at destructive, publication, privacy, and security boundaries. Partial results identify coverage and remain distinguishable from complete success.

## Evidence and uncertainty

- **Observed:** The repository README and checked-in implementation establish a specification-driven Rust rendering engine that plans and executes reusable transformation graphs for publication-ready artifacts.
- **Decided for this draft:** The repository owns the bounded concern described here and participates through versioned contracts.
- **Proposed:** Target systems and later roadmap phases remain proposals until accepted and implemented.
- **Open question:** Which parts of this draft should become active in the first independently versioned release?
