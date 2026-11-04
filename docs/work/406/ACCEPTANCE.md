# #406 — source acceptance reconciliation

Source review: November 3, 2026 (America/New_York). This reconciles the six
implementation checkpoints #449–#454 with the original
[#406](https://github.com/egohygiene/renderflow/issues/406) requirements. It adds
no renderer, adapter, dependency, fixture baseline or approval. **The issue stays
open.** Source presence and authored regression cases do not establish passing
tests, output fidelity, compatibility or release readiness.

## Original acceptance mapped to source

Paths below are relative to `crates/renderflow-core/src/` unless linked otherwise.
“Authored” describes implementation available for later qualification.

| Original requirement | Authored implementation and boundary | Remaining evidence or scope |
| --- | --- | --- |
| One versioned, format-neutral contract | `composition/model.rs`; [composition schema](../../../schemas/renderflow-page-composition-v1.schema.json) and [content schema](../../../schemas/renderflow-page-content-v1.schema.json). Canonical inputs remain v1; native execution/profile/cache identity is v5. | Execute schema/model agreement and compatibility checks; the contract is a bounded scene model, not every editor feature. |
| Replace text by stable content ID without editing art | Pinned per-locale `PageContent` documents, text-layer `content_id`, separate immutable assets, explicit locale overrides; `composition/load.rs`. | Observe source immutability and changed-content output behavior on real executions. |
| Source/default and all declared locales share planning/rendering | `composition/mod.rs`, `planning/composition.rs`, `planning/build_composition.rs` and `planning/build.rs`; exact/subset/all-declared selection, common CLI/SDK build facade. | Execute selector/default/API parity cases. Visual output remains Latin LTR horizontal with authored line breaks; unsupported writing modes are diagnosed. |
| Deterministic, provenance-complete, cacheable, independently validated branches | Captured inputs, settings/locale-aware identities, native cache admission and per-locale shared work in `composition/execution.rs`; native/common publication correlation in `planning/composition_evidence.rs`. | Actual two-run byte/cache/stage evidence, interruption and corrupt-cache behavior, visual checks and independent provider/platform validation are unobserved. Receipt correlation alone does not verify files. |
| Actionable layout/typography/writing-mode failures | `composition/typography.rs`, `typography_provider.rs`, `render.rs`, `pdf_plan.rs`; measured glyph/fallback reports, safe-area/clip/line-box diagnostics and scoped branch failures. | Exercise licensed-font and negative fixtures. Broader script/font/layout features require a concrete supported subset and consumer requirement. |
| Advertise only supported format operations | [Format evaluation](FORMAT_EVALUATION.md) separates detection, inspection, extraction, generation, validation and round-trip; native output planning keeps unavailable targets visible. | No general SVG/editor round-trip or ORA/PSD/PSB/IDML composition adapter is implemented. Add an adapter only after selecting the exact features and invariants it must preserve. |
| No automatic or lossless recovery of flattened PNG layers | Authored package provenance distinguishes `flattened_original`, `textless_original` and `reconstructed`; flattened cross-locale substitution is blocked by `load.rs`; review/loss declarations remain explicit. | The composition entry point requires a manifest. It does not detect/reconstruct arbitrary raster inputs, perform OCR/inpainting, or authenticate an author's provenance declaration. Any future raster import needs its own detection and recovery evidence. |
| Existing document/image publication remains compatible | Build facade dispatches to the existing graph or composition engine; native evidence joins the existing forest contract without a fake format-keyed locale DAG. | Execute legacy v1/v2 graph cases and the existing publication/provider regression sweep. Shared APIs changed in #454; the migration remains documented in [planner integration](PLANNER_INTEGRATION.md). |
| Consumer owns translations, content and approval | Input review declarations, stale-content blocking, retained losses and `review_required`/unqualified execution evidence; no translation or approval service is added. | Human content/rights/visual/publication review remains a consumer responsibility. Renderflow declarations never substitute for it. |

## Current delivery and handoff limits

SVG and HTML candidates retain measured glyph paths with stable layer/content
relationships and transcript companions. PNG uses those captured inputs and
measured geometry. PDF is a single-page flattened image with explicit page
boxes, DPI, opaque background, color and binding intent; it is not editable,
tagged, font-embedded or print-certified. Transcript output can remain available
when a visual sibling is blocked. Defaults remain SVG/HTML/transcript JSON;
PNG and PDF are explicit selections.

The original operation matrix is a historical design evaluation. Its “candidate”
entries describe upstream feasibility, not installed providers or Renderflow
support. The [guide](../../user-guide/page-composition.md) and provider decisions
describe the current authored subset.

The common `run-manifest.json`, or the ordinary-build `renderflow-run.json`,
can be selected for the existing [publication verifier](../../user-guide/bundle-evidence.md)
and [portable inventory handoff](../../user-guide/portable-forest-bundles.md).
Use an explicitly pinned receipt and bundle root; do not infer output from a
planned filename or overall process exit. These tools preserve native state,
loss and metadata and observe selected file bytes. They grant no new runtime,
creative, print or publisher authority.

Portable export copies inventory payloads and exact selected common receipt
bytes. Optional source inclusion does not reconstruct the original relative
package layout or automatically include every native plan/sidecar. Keep the
editable source package and original native bundle for reproducibility. Common
verification is not a visual or full native composition-receipt validator. No
composition-specific portable round-trip has been executed in this checkpoint.

## Existing qualification material

The repository already contains authored cases in:

- `composition/load.rs`: duplicate JSON keys, source pins, frozen input capture,
  inert SVG refusal, stale content and path confinement.
- `composition/tests.rs` and `composition/shared_render_tests.rs`: selector and
  branch identity, scoped PNG/PDF settings, native/common receipt joins,
  shared-stage observations, cache reuse, partial runs and tamper refusal.
- `composition/render_tests.rs` and `composition/raster_tests.rs`: bounded
  visual geometry and raster/PDF behavior within the selected provider subset.
- [Ordinary build cases](../../../crates/renderflow-core/tests/composition_build.rs):
  frozen CLI/SDK planning, dry-run/cancellation/no-overwrite, publication joins,
  unsupported policies, partial siblings and existing graph admission.
- [Structural, measured-font and mixed-asset recipes](../../../examples/page-composition/README.md):
  original art/text, expanded German text and explicit unsupported Arabic/Japanese
  visual cases. The structural WOFF2 stub is not a visual font; the measured
  fixtures retain the DejaVu license and captured font.

These are source references, **not executed results**. No new test is necessary
for this documentation-only reconciliation. No tests, compiler/build/Cargo,
lint/format, schema/docs validators, native/CLI/provider/visual execution or CI
were run.

## Queue decision

No additional unconditional local feature requirement was selected by this
review. Bounded typography, PNG/PDF, common planning and ordinary build admission
are already authored. Historical “next: add those features” notes must not cause
duplicate implementations. Broader typography and native interchange remain
open requirements to scope when a concrete use case selects them; this review
does not remove them from #406 or close any original acceptance checkbox.

Continue the oldest actionable local source queue with existing
[#412](https://github.com/egohygiene/renderflow/issues/412), mapping current
capabilities to finite slices of the existing generator-first corpus, then its
focused synthetic-comic child
[#413](https://github.com/egohygiene/renderflow/issues/413). Author recipes and
expected outcomes before the later execution sweep; do not invent another
fixture harness, executed baselines or CI evidence. Neither needs private
consumer artwork or a completed Flow adapter for its bounded preparation.

Keep #406 open for actual composition acceptance. Existing #429 owns toolchain
qualification and #458/#419 retain repository/release evidence. Flow #52 owns
the cross-repo adapter against a qualified immutable Renderflow release. That
external integration does not block the local corpus work. No new issue is
needed for these already-tracked boundaries.
