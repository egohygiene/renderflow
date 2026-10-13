# #426 screenplay recovery checkpoint

This checkpoint authors a deterministic, explicitly lossy recovery path from
one immutable screenplay-shaped PDF or UTF-8 text file into
`renderflow.screenplay/v1`, followed by optional Fountain, FDX, Fade In, and OSF
body exports. The preceding derivative-forest PR #445 was merged before this
work, at `e631e1fcfe99eefe5376441c6051107bb7a95f96`.

## Authored behavior

- `renderflow screenplay recover` publishes a new evidence bundle, with its
  completion receipt written last. Canonical JSON and full extracted page text
  are retained even when native derivatives are requested.
- `execution.screenplay_recovery` enables the same model/providers/exports in
  the canonical CLI/SDK build lifecycle. Its graph is isolated from unrelated
  converters, has no PDF output edge, and disables cache/checkpoint reuse.
- Each PDF page is inspected through pinned `pdftotext` before optional pinned
  `pdftoppm` and Tesseract OCR. Only pages without alphabetic embedded text take
  the OCR fallback. This routing heuristic does not establish page completeness.
- Source/page identities, exact extracted text, line spans, inferred element
  kinds, ordinal confidence, ambiguity reasons, and loss records survive into
  recovery evidence. Confidence labels are not measured probabilities.
- Canonical validation runs before export. Native body writers reparse their
  output independently and refuse unsupported representations. Fountain may
  join adjacent dialogue lines, with explicit join evidence.
- Terminal publication repeats source, canonical, output, configuration, and
  evidence checks even when general artifact validation is disabled.
- Every candidate and derivative stays lossy and review-required. No AI route,
  review approval operation, native application acceptance claim, or toolchain
  qualification is supplied by this checkpoint.

## Deliberately deferred

Per the sprint instruction, no tests, compiler/build, formatter/linter, schema
validator, native version probe or conversion, installation/download, or CI
was run. Source review and Git identity checks are the only verification in
this checkpoint. Test source includes deterministic recovery, fake extraction
runners with synthetic PDFs, export round trips, evidence tampering, and graph
policy coverage; none of those cases has been executed.

Keep #426 open until the later qualification session supplies observed results.
That session should check text-bearing and image-only synthetic PDFs, mixed
pages, malformed/encrypted input, unavailable or mismatched tool/data pins,
timeouts/cancellation, all four native exports, source preservation, and full
CLI/SDK manifests with general validation disabled. Native application opening
and human review of recovered content remain distinct from structural checks.

## Entry points

- User instructions: [Screenplay recovery](../../screenplay-recovery.md).
- Shared implementation: `crates/renderflow-core/src/screenplay/recovery/`.
- CLI front door: `crates/renderflow-core/src/commands/screenplay_recovery.rs`.
- Planning bindings: `crates/renderflow-core/src/planning/screenplay_recovery.rs`.
- Policy schema: `schemas/renderflow-v2.schema.json`.

Bounds and strict subset refusals are intentional and documented. This is a
reviewable recovery draft, not exact reconstruction of arbitrary screenplay
layout, revision history, dual dialogue, typography, or missing OCR content.
