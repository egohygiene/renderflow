---
schema: aether.architecture-document/v1
id: renderflow-architecture
title: Renderflow Architecture
kind: architecture-document
version: 0.1.2
status: draft
owners:
  - egohygiene
created: 2026-08-19
updated: 2026-11-10
governed_by:
  - architecture-architecture
depends_on:
  - renderflow-foundations
  - renderflow-system
related:
  - renderflow-purpose
  - renderflow-vision
  - renderflow-principles
  - renderflow-pillars
supersedes: []
---

# Renderflow Architecture

## Purpose and scope

**Start here for the current architecture.** This document owns structural
boundaries, dependency direction and integration rules. [SYSTEM.md](SYSTEM.md)
owns logical responsibilities. The [implementation view](docs/architecture/overview.md)
maps those responsibilities to current source; the directory layout does not
define the conceptual architecture.

The root architecture documents use the repository's existing
`aether.architecture-document/v1` convention. Their `draft` status is retained;
this navigation correction does not ratify a new organization policy. The older
foundation documents under `docs/architecture/foundation/` remain historical
reference, with [current destinations](docs/architecture/overview.md#historical-foundation-documents)
and original contents preserved.

## Saved Sonic evidence readback

The existing Sonic owner also reads a retained bundle against an externally
supplied receipt digest. Bounded exact file/CAS membership, artifact lineage,
DNA/source/intent bindings and candidate-only semantics are checked without
creating an artifact store or invoking extraction/inference. Optional current
recipe observation distinguishes current, stale and unavailable sources. The
readback establishes retained byte relationships only; it does not authenticate
providers, refresh model readiness, approve content or publish a release.

## Layer model

The existing Sonic evidence boundary projects selected source-clock musical
events and explicitly selected timed-text structural context through its current
DNA/hygiene owner. Aniflow remains the analysis producer; no analyzer, sibling
dependency or second media runtime is added. Provider/review declarations remain
unauthenticated, source frame ranges stay exact, and companion words remain
outside model input. See [Sonic evidence](docs/user-guide/sonic-dna.md).

Sonic owns bounded native music WAV byte inspection and original sample-frame
clocks without conversion. Speech retains its stricter source profile. Larger
Sonic source policies explicitly select bounded shared capture limits; other
publication callers and outgoing bundles retain their current limits. Existing
DNA, hygiene, artifact store and model runtime owners remain unchanged.

Explicit Sonic analysis contexts project bounded inferred-note and selected-stem
relationship fields from pinned public Aniflow reports. Exact source clocks,
unknown musical confidence and declared original-mix lineage stay separate;
raw native acceptance is not replayed. Unselected semantic/MIDI payloads remain
uninterpreted. No sibling source dependency or second analyzer runtime is added.

1. **Intent and contracts** — identity, policy, specifications, schemas, and accepted decisions.
2. **Domain** — canonical concepts and pure domain behavior.
3. **Application** — planning, orchestration, use cases, and state transitions.
4. **Adapters** — filesystems, providers, frameworks, renderers, and external tools.
5. **Interfaces** — CLI, library, site, reports, generated artifacts, and automation contracts.
6. **Evidence** — tests, diagnostics, provenance, manifests, and health projections.

Dependencies point inward toward stable contracts and domain behavior. External details do not become canonical domain truth.

## Structural view

```mermaid
flowchart TD
  I[CLI and library] --> R[Validated intent]
  R --> P[Planning]
  C[Capability contracts] --> P
  P --> E[Execution]
  C --> E
  E --> A[Artifacts and run evidence]
  A --> V[Validation and review]
```

This is a conceptual interaction view, not a call graph or proof that a route
is available. Interfaces share application behavior. Planning selects bounded
capabilities and records expected work; execution records what actually
happened. Artifact checks and human review remain distinct from plan intent,
provider success and telemetry.

The current ordinary build boundary dispatches to the existing format graph,
layered composition, frozen presentation or frozen local briefing owner. These
engines retain their own
execution state and target identities. A locale/output branch is not reduced
to a format name, and a requested target is not a published derivative. The
[composition integration decision](docs/work/406/PLANNER_INTEGRATION.md) and
[artifact-forest contract](docs/architecture/derivative-forest.md) describe the
specific representations without introducing a second orchestration system.

## Dependency rules

- Sibling domain capabilities integrate through versioned public contracts, not direct access to internals.
- Generated artifacts never become the canonical source unless an accepted decision explicitly changes ownership.
- Provider and platform adapters depend on application ports; core behavior does not depend on a provider implementation.
- Read, plan, apply, verify, publish, and recover remain separate authority boundaries when consequential.
- Cross-repository references use releases, immutable commits, schemas, packages, or documented APIs rather than mutable default-branch assumptions.

## Ecosystem interfaces

Consumer projects own content, rights, translations and publication approval.
Renderflow owns generic static-artifact planning, rendering and evidence. Flow
owns cross-tool orchestration through released public interfaces; its
[Renderflow adapter](https://github.com/egohygiene/flow/issues/52) remains separate
work gated on a qualified immutable release. A source-tree import or a handoff
document is not that integration.

Provider adapters expose only declared directions and bounded capabilities.
Tool installation, model/font/assets, network policy, version identity and
validation belong to the selected route. An installed tool does not establish
every conversion it could theoretically perform. The
[provider contract](docs/provider-contract.md) and
[toolchain qualification record](docs/work/429/QUALIFICATION.md) make those
boundaries concrete.

## Deployment and portability

The architecture favors independently usable local and self-hosted operation. Optional managed services may add availability, collaboration, support, and hosted infrastructure without becoming the canonical holder of portable state.

## Evidence and evolution

The #397 native WAV and explicitly selected MIDI/stem projections share the
current retained Sonic bundle reader. Bundle integrity and current-source
comparison remain separate: readback verifies the selected receipt, inventory,
CAS and candidate relationships before optionally recapturing the exact recipe
with its declared finite source limit. It never invokes an analyzer or model,
replays a provider, infers sample alignment, or grants publication approval.
Normalizer revision 4 is intent-bound; an older saved candidate can remain
internally intact while its current-source plan is stale under newer mappings.
This integration is source work awaiting compilation and qualification.

- **Current source:** the workspace exposes reusable planning, execution,
  composition, provider and SDK contracts. Merged implementation is not a
  claim of passing runtime, visual, native-provider or platform qualification.
- **Merged source checkpoint:** [PR #455](https://github.com/egohygiene/renderflow/pull/455)
  adds original-Fountain exports and transcript terminal Text. It merged on
  October 27, 2026; native fidelity, runtime and released-consumer evidence
  remain outstanding under #430 and #419.
- **Proposed release:** [#419](https://github.com/egohygiene/renderflow/issues/419)
  and the [release guide](docs/release-candidate.md) own the bounded first
  immutable publication candidate. A Cargo version does not establish a
  published or supported distribution.
- **Scheduled governance:** existing [decisions](DECISIONS.md) keep their
  four-digit identifiers. [#405](https://github.com/egohygiene/renderflow/issues/405)
  retains its shared-ADR-system scheduling gate;
  [#409](https://github.com/egohygiene/renderflow/issues/409) retains the later
  post-roadmap audit. Neither is activated by these documentation corrections.

Use the [live roadmap](https://github.com/egohygiene/renderflow/issues/367) for
current ordering and issue-specific qualification gaps. Historical checkpoints
in [ROADMAP.md](ROADMAP.md) retain their original dates and evidence limits.
