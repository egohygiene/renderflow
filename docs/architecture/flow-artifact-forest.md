# Artifact forest handoff to Flow

This is the authored #430 integration contract, with the #406 composition seam
described below. It is not a released or
runtime-qualified creative stack. Flow owns project orchestration, scheduling,
checkpoint policy and approval; Renderflow owns each bounded conversion and
its native artifact evidence. Aniflow retains temporal segmentation and
reconstruction ownership.

Consume a pinned, verified Renderflow release and serialized contracts. Do not
import sibling repository source or infer release support from a merged PR.
[Release qualification](../release-candidate.md) remains owned by #419. The
creative fixture uses public CLI commands; its Python launcher is repository
qualification tooling, not an additional runtime dependency for Flow.

## Contract layers

| Layer | Contract | Consumer meaning |
| --- | --- | --- |
| Request | `renderflow/v2` | One original artifact or one explicitly ordered collection, exact targets and route-specific policy. Separate source families use separate requests. |
| Plan | `renderflow/artifact-forest-v1` within the canonical execution plan | Stable target identities, selected simple paths, declared providers/loss and unavailable/unsupported/excluded targets. No produced files are implied. |
| Run | `renderflow.run/v1` | Actual run state, plan/spec digests, executed steps, diagnostics and selected toolchain evidence. |
| Publications | `renderflow/forest-execution-v1` in `artifact_forest.execution` | One outcome per planned target, with references to the exact artifact records actually published. |
| Artifact inventory | `renderflow.artifact-manifest/v1` | Sources, intermediates and terminal records, byte digests, locators, producers, validation, lineage and native metadata. Inventory membership alone is not publication. |
| Interchange | `flow.artifact/v1` | Minimal compatibility projection. Pair it with the native run and retained evidence. It does not carry the complete publication/review contract. |
| Fixture suite | `renderflow.creative-fixture/v1` / `renderflow.creative-fixture-result/v1` | Repository fixture requests and observations linking independent CLI jobs. It does not replace a Renderflow run manifest or supply a Flow execution engine. |

The forest execution field is additive and optional for older records. If it
is missing, a consumer requiring this contract must report unsupported evidence
rather than infer success from the planned branches. The forest identity binds
the frozen plan; adding observed outcomes does not recompute that identity or
turn it into a digest of the final output tree.

## Select actual deliverables

Read a target's planning branch and execution record together. The original
planning state remains unchanged after execution:

| Execution outcome | Meaning |
| --- | --- |
| `not_selected` | No route was selected. Read the planning reason for excluded, unsupported, unavailable or budget-pruned intent. |
| `planned` | Dry-run evidence only; no publication reference is permitted. |
| `published` | The exact terminal record has a published bundle locator. It can still carry losses, skipped validation or mandatory review. |
| `not_published` | A selected target has no published terminal record. Consult run/step diagnostics; no specific failure cause is inferred. |

For `published`, use `artifact_evidence_index` in that same run's
`artifact_manifest.artifacts`. Require agreement with the row's `artifact_id`
and `locator`, and retain `producing_step_id` when present. The indexed record
provides its digest, size, producer, sources, fidelity, validator evidence and
native metadata. This exact reference matters when two target identities
publish equal bytes to different destinations. An artifact ID alone does not
uniquely identify a publication record.

`artifact_forest.produced_artifacts` lists unique published derivative IDs. It
omits sources, unpublished candidates and intermediates. An explicitly requested
unchanged source can have a `published` row with `source_identity: true`, but it
does not count as a newly produced derivative. Recover the complete output tree
from the artifact manifest and required sidecars, not this abbreviated ID list.

For each selected deliverable, resolve `bundle:` within the corresponding run's
bundle root. Refuse traversal, links escaping the bundle or a missing payload;
verify the recorded byte size and digest before copying or handing it to another
provider. Preserve the native manifest and the evidence files it references.
Do not treat `artifact-store:` locators as files already published into the
portable output bundle.

## Preserve losses and review state

| Family | Evidence that Flow must retain |
| --- | --- |
| Original screenplay body | `metadata["renderflow.screenplay_body"]`, exact original Fountain and retained canonical screenplay references/digests, output digest and policy/profile identity. The strict six-kind body profile retains `qualification: authored_unqualified`, `runtime_qualified: false` and `review_required: true`; independent semantic reparse is not native-application or creative approval. |
| Screenplay recovery | `metadata["renderflow.screenplay_recovery"]`, the canonical candidate, extraction evidence and native export evidence. `lossy: true` and `review_required: true` survive every derivative. Ordinal heuristic confidence is not measured accuracy or approval. |
| Timed text | `metadata["renderflow.subtitle"]`, canonical transcript references, prior/export losses, overlap and language policy. SRT/VTT bytes omit some metadata held in the canonical record. |
| Local speech | `metadata["renderflow.speech"]`, raw provider result, transcription, any preparation evidence and the retained saved-result bundle. `human_reviewed: false` cannot become true through successful export. |
| Screenplay PDF | Configured Node/package identity, fixed formatting profile, provider evidence and independent PDF inspection. A complete PDF is not visual or native-application qualification. |
| Publication hygiene | Embedded hygiene decisions and findings. A `review_required` artifact needs explicit downstream approval; publication alone does not provide it. |

An original-body native export keeps its canonical companion in the local
artifact store unless `screenplay_json` is also selected for publication.
Copying a native-only bundle does not make an `artifact-store:` reference
portable. Preserve the referenced store, or select the canonical target and
retain its actual publication together with the original source and native run
evidence when moving the outputs. The creative fixture's `original-body` job
selects that canonical companion explicitly.

Prepared-audio timestamps do not establish original-video synchronization.
The explicit saved-result time-map exporter supplies a separate assertion and
evidence bundle; Flow must preserve that distinction. Neither source-format
suppression nor a successful native reparse establishes lossless round trips.

## Failure, retry and toolchain identity

Retain a child manifest even when its process exits unsuccessfully. `partial`,
`failed` and `cancelled` are distinct from `complete`; a published target in a
partial run does not make every requested target successful. Early planning or
configuration failures can occur before a run manifest exists. Record the
failed invocation and diagnostics without fabricating an empty successful run.

Each family's explicit policies remain authoritative. A missing or incompatible
provider must remain visible in that family's plan or failed invocation; it
must not cause the orchestrator to mislabel another family's successful run.
The fixture launcher isolates these jobs and retains configured-child failures.
Skipped optional fixture jobs are unattempted, not passing conversions.

The doctor fingerprint (`provider-discovery-observations/v1`) is separate from
the selected plan toolchain snapshot and from observed adapter execution pins.
Preserve their respective scopes. Configured recovery/speech tools can remain
unobserved in planning; Node/Afterwriting are intentionally unprobed by generic
doctor. Hashes of selected executables do not attest all shared libraries,
fonts, language data or model licensing.

Do not infer generic cache/checkpoint reuse for screenplay imports, PDF,
recovery or speech. Their adapter handoffs define the actual restrictions.
Speech's explicit verified historical-result recovery is a separate mechanism;
a saved result is not an observation of a newly installed native toolchain.

## Localized composition handoff

Composition uses a frozen, captured package and its scoped executor. It does
not register the page manifest as ordinary JSON input or add locales to the
format-keyed conversion DAG. `planning::resolve_composition` captures the input
once and returns `ResolvedCompositionExecution`; `planning::execute_composition`
consumes that snapshot and plan. Existing `composition` CLI commands retain
their native v5 stdout contracts.

| Bundle evidence | Meaning |
| --- | --- |
| `composition-plan.json` | Native `renderflow.composition-plan/v5`, including exact locale/output identities, input pins, typography and raster/PDF policy |
| `execution-plan.json` | `renderflow.composition-execution-plan/v1` wrapper with the native plan, common `ExecutionPlan`/artifact forest and identified shared-stage dependencies |
| `composition-run.json` | Native `renderflow.composition-run/v5`, actual per-branch outcomes/times, shared-stage observations and the complete artifact inventory |
| `run-manifest.json` | Common `renderflow.run/v1` projection with the same run ID, state, inventory and exact publication references |

The wrapper is planning evidence, not a serialized executable snapshot.
Replaying deserialized plans is not supported. Its format nodes are a
compatibility view; the forest target IDs and stage IDs preserve the locale
identity that format nodes alone cannot represent. Dependency waves do not
claim concurrent execution. Structural costs and unqualified quality values
are bookkeeping, not observed performance or fidelity measurements.

The native run's `plan_sha256` commits to the exact pretty-printed, newline-
terminated `composition-plan.json` bytes. The common `execution_plan_digest`
commits to the same encoding of the whole `execution-plan.json` wrapper.
`source_spec_digest` identifies only the canonical manifest bytes; it is not a
digest of the entire source directory. The full plan retains content, artwork,
font and license input evidence. The common projection has no observed generic
toolchain snapshot; native provider profiles remain in the retained plan and
artifact metadata.

Every native branch has a corresponding common step and forest execution row.
A published cache hit maps to a reused step, blocked/unavailable branches map
to skipped steps, and failure/cancellation remain distinct. Only published rows
carry output artifact references. Publication matching checks exact native
record index, ID, locator, output kind, cache state, lineage, producer and native
identity/review metadata; the output list must agree with those records. These
are receipt-correlation checks, not independent observations of on-disk bytes.
Apply the host checks described above before using any payload.

Successful shared transcript/SVG preparation is retained as
`composition.shared_stage.observed` diagnostics keyed to the planned stage ID.
The digest, size, materialization/reuse counts and actual consumer IDs describe
an in-memory intermediate. They do not create artifact-store records, published
intermediates, timed stage execution records, or runtime/visual qualification.
A recorded consumer can still fail later during rendering or publication. A
terminal cache hit can bypass shared preparation entirely. Do not turn planned
sharing into an observed cache hit.

Composition publications preserve the `renderflow.composition` metadata,
including locale, content revision, exact source/configuration pins, typography
provider evidence, flattening and color limitations, and mandatory review.
The original canonical package remains necessary for editable source layout;
the hashed `inputs/` evidence collection is not a reconstructed source tree.

Flow main was source-inspected at
`4621df7437e0fdb278f025c75e9feb7298eb7bd9`: it has no Renderflow adapter. Its
`flow.artifact/v1` shape remains compatible with the minimal projection, but
that does not establish execution compatibility. A future Flow-owned adapter
must consume a pinned release, preserve the native receipts, and translate
publication locators to confined root-relative bindings. Flow's current
artifact acceptance requires a complete produced/reused result and an exact
declared output set; it must not relabel a partial composition run as complete.
No Flow repository changes or end-to-end runtime qualification are included in
this checkpoint.

## Current capability boundary

The [generated matrix](../user-guide/creative-capabilities.md) and
[fixture guide](../user-guide/creative-forest.md) separate current behavior from
the broader #430 outcome. Fountain PDF/analysis, native-to-Fountain imports,
review-required PDF/text recovery, timed text and speech use separate bounded
interfaces. Merged source adds direct original Fountain body exports to
canonical screenplay JSON, FDX, Fade In, OSF and Trelby through the explicit
`screenplay_body.allow_unqualified` policy. Its strict six-kind body profile
does not support general screenplay metadata/layout conversion or qualify
native-application opening. Scripttool reverse writers remain rejected;
recovery exports retain their separate lossy, review-required contract.
Original Fountain must not be reinterpreted as unstructured text to reach
recovery exports.

Canonical transcript to plain text is also authored as a terminal graph target
under the existing subtitle and explicit speech policies. Its evidence retains
the canonical/source identities and timing, cue-ID and structured-metadata
losses. It does not add a reverse timing edge, automatic recognition or
original-media synchronization. These merged routes remain unqualified until
their runtime and native evidence is recorded.

`RunManifest::flow_artifacts_v1()` continues to project all native inventory
records, including sources and intermediates. It intentionally omits locators,
native provider identifiers, validation, loss/review metadata and toolchain
evidence. Its compatibility fixture pins the earlier Flow contract at
`a7d28ee812f9d6ccd93b6786be924e24c455accd`; that is not a claim about a current
released Flow integration. Use native publication rows to select deliverables
before projecting, and retain the native evidence alongside the projection.

Actual CLI/SDK parity, complete native fixture runs, missing-provider isolation,
repeated-run behavior, platform support and downloaded-release consumption
remain to be demonstrated. #430 and #421 stay open until their evidence and
explicit remaining capability decisions are reconciled.
