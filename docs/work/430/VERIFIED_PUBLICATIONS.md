# #430 — observed publication files

Authored November 3, 2026 (America/New_York).
Branch: `codex/430-verified-publications`.
Base: `0514685f424bf16e76bd0b01b02f0254dc4e5789`, after explicitly authorized
merges #468, #470, #471 and #472. The final main tree matched the saved combined
publishing source exactly. No tests/CI or qualification accompanied those merges.

The maintainer authorized continued artifact-forest source work. Existing
#422–#429 implementation is mostly merged; its remaining execution acceptance
stays open. This checkpoint addresses #430's missing common consumer-side
publication file verification, without rebuilding those adapters.

## Implemented

- Add `RunManifest::verify_published_targets`, `PinnedRunReceipt` and sealed
  observation views over the existing `published_targets()` structural join.
- Add `bundle verify` CLI adaptation to that shared SDK service. The caller pins
  exact native receipt bytes and supplies the bundle root explicitly.
- Validate logical bundle paths, observed real directory/file components,
  regular payload size and SHA-256, metadata/root stability, cancellation and
  aggregate work bounds. Separate same-content destinations are each observed.
- Preserve original run/validation/fidelity/loss/review states. An empty planned
  result stays empty/planned; no approval or successful workflow is inferred.
- The shared streaming observation primitive can copy into a new caller-owned
  staging sink for the following portability checkpoint; it publishes nothing.

## Evidence and scope

Source/API/documentation inspection only. Seven authored filesystem cases cover
real bytes with partial/skipped/review state preservation, distinct destinations
and tampering, traversal/alternate/encoded/directory paths, cancellation/budgets/
algorithms, pinned receipt identity and ignored host output paths, planned empty
results and observed Unix symlink leaf/directory rejection. All are unrun.

No tests, compiler/build/Cargo resolution, lint/format/schema/docs validators,
product CLI/provider/native/media/model execution or CI ran. No dependencies,
lockfiles, workflows, tags, releases or native run/Flow wire schemas changed.
The result report is additive; no complete receipt-authentication or sandbox
claim is supplied. Quiescent filesystem observations are not protection against
a hostile concurrent writer, and future bytes require fresh use-time checking.

This remains draft. #430/#421 stay open for observed CLI/SDK/native/portability
and released Flow consumer acceptance. Next: retain canonical/sidecar evidence
in a portable bundle under the same existing issue; no speculative issue added.
