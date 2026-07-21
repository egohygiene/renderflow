---
title: Design System — Renderflow
version: 1.0
date_created: 2026-07-21
last_updated: 2026-07-21
owner: Renderflow Maintainers
tags:
  - architecture
  - experience
  - design-system
---

# DESIGN_SYSTEM

## Introduction

Renderflow's design system defines the reusable design language that should make
its experience consistent across interfaces and implementations.

## Conformance

This document is authored in conformance with:

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/experience/design-system.spec.md`

## Purpose & Scope

This document defines the canonical design language for Renderflow.

It covers reusable communication patterns, accessibility expectations, visual
philosophy, and interaction primitives. It does not define CSS, component code,
or product-specific layouts.

## Definitions

- **Design language**: the shared vocabulary through which the experience is expressed.
- **Interaction pattern**: a reusable user-facing behavior.
- **Accessibility**: the practice of making the experience understandable and usable by diverse audiences.
- **State language**: the consistent way progress, success, warning, failure, and uncertainty are communicated.

## Renderflow Design Language

### Visual philosophy

- Favor structured clarity over decorative density.
- Present transformations as flows, plans, or states when that improves understanding.
- Use hierarchy to distinguish intent, plan, execution, and result.
- Treat documentation and CLI output as first-class design surfaces.

### Communication patterns

- Prefer canonical terms from `ONTOLOGY.md`.
- Prefer tables, diagrams, and ordered flows when they reduce ambiguity.
- Prefer explicit status language: planned, executing, cached, produced, warning, failed.
- Distinguish observations from advice so users can tell what happened versus what to do next.

### Interaction patterns

- The shortest path should solve the common case.
- Advanced inspection should be available without requiring hidden knowledge.
- Commands and docs should support progressive disclosure from overview to detail.
- Extension points should reveal capabilities and limits, not appear as opaque hooks.

### Accessibility expectations

- Explanations should not depend solely on color or visual styling.
- Structured text output should remain understandable in plain terminals and static documentation.
- Diagnostics should be scannable and actionable.
- Examples should be concrete enough for copy, adaptation, and learning.

### Motion and feedback philosophy

- Feedback should emphasize state transition rather than spectacle.
- Long-running or multi-step work should communicate progress in a way that preserves operator confidence.
- Repeated work skipped by caches should still be visible as a meaningful outcome.

## Requirements, Constraints & Guidelines

### Requirements

- The design language must align with `DESIGN.md` and `PERSONAL_MODEL.md`.
- Reusable interaction and communication patterns must be explicit.
- Accessibility expectations must be stated clearly.

### Constraints

- No implementation code or framework-specific components.
- No product-specific layouts.
- Temporary aesthetic trends must not redefine the design system.

### Guidelines

- Prioritize consistency over novelty.
- Favor reusable patterns over one-off rules.
- Express the architecture through recognizable states and transitions.

## Authoring Contract

### Purpose

Own the reusable language through which Renderflow's design philosophy is
expressed.

### Responsibilities

This document owns:

- visual and communication philosophy,
- interaction patterns,
- accessibility expectations,
- reusable design conventions.

### Non-Responsibilities

This document does not own:

- implementation code,
- component libraries,
- screen layouts,
- workflow-specific product copy.

### Inputs

- `PURPOSE.md`
- `PRINCIPLES.md`
- `PERSONAL_MODEL.md`
- `DESIGN.md`

### Outputs

- user-facing docs
- CLI presentation conventions
- future component libraries or style guides

### AI Generation Rules

AI systems should describe reusable patterns and state language, not framework
artifacts.

### Validation

Independent interfaces should be able to feel recognizably like Renderflow by
following this design language.

## Acceptance Criteria

- The design language is explicit.
- Reusable interaction patterns are documented.
- Accessibility expectations are visible.
- Implementation details are absent.

## AI Authoring Strategy

AI systems should:

1. read `DESIGN.md` and `PERSONAL_MODEL.md`,
2. derive reusable communication and interaction patterns,
3. preserve implementation independence,
4. keep accessibility explicit.

## Rationale & Context

Renderflow already teaches through its docs, graph visualizations, CLI command
structure, and diagnostics. A design system is still useful even for a CLI-first
project because consistency of explanation, state language, and interaction
progression is part of the user experience.

## Dependencies & External Integrations

### Upstream Dependencies

- `PURPOSE.md`
- `PRINCIPLES.md`
- `PERSONAL_MODEL.md`
- `DESIGN.md`

### Downstream Dependencies

- documentation presentation
- CLI conventions
- future UI or embedding surfaces

## Examples & Edge Cases

### Example

A plan view that labels nodes as source, intermediate, and output follows the
design-system goal of expressing workflow state with canonical, reusable terms.

### Edge Case

A future graphical interface can diverge visually from the CLI, but it should
still preserve the same state language, interaction clarity, and accessibility
expectations.

## Validation Criteria

This document is valid when multiple Renderflow interfaces can remain coherent
without sharing a single implementation stack.

## Related Specifications

- `.github/specs/architecture/document.spec.md`
- `.github/specs/architecture/experience/design-system.spec.md`
