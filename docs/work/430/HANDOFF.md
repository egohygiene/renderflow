# #430 — multi-family forest integration checkpoint

## Selectable demonstration checkpoint — October 28, 2026

The maintainer switched back to the artifact forest after the suite merge and
roadmap refresh. Branch `feat/430-selectable-forest-demo` starts at main
`c1d1bebfd48369d18e7c46295859d76e3f831499`.

- Repeatable `--job ID` selects existing fixture jobs in declared order.
  Describe, plan and execute resolve the same subset before job work. The
  retained request and its digest describe that exact subset.
- Unknown/repeated IDs, negative jobs without explicit opt-in and unused native
  configuration keys are refused before output creation/child execution.
  Native negative cases retain their existing shared configuration aliases.
- Default execution retains the nonnegative suite. Static describe now mirrors
  execution selection; add `--include-negatives` to list the entire catalog.
  Omitted jobs have no observed result. Selected unconfigured native jobs still
  report `skipped_unconfigured`, not an observed unavailable provider.
- The first walkthrough selects `original-body`: five built-in exports from
  preserved synthetic Fountain, including the canonical JSON companion.
  The Flow handoff corrects stale unsupported-route claims and documents native
  body evidence retention and the native-only bundle portability limitation.

No provider, general workflow engine, schema or dependency is added. Regression
cases are authored for later execution; source review and Git identity are the
current evidence. No tests, compiler/build, lint/format, schema validators,
fixture/CLI/native/provider execution or CI are run for this checkpoint. Keep
the PR draft and #430/#421 open. The remaining integration qualification steps
below still apply; this usability improvement does not complete them.

## Current integration state — October 27, 2026

PR #455 merged at `0842c16cb50530f37a35b18fb6b0211f3ffc2386` with maintainer
authorization after source review. Main subsequently reached
`8d7a71edd4033a60801c663fec242a2195a2562e` through documentation PR #461 and
schema-integer/Snap-selection PR #462. The source below is now integrated;
its older draft-state and pause instructions describe the historical checkpoint.
No tests, builds, lint, schema/CLI/native/provider execution or CI ran during
this integration. #430/#421 and all executable qualification remain open.
No release, application fidelity or downstream Flow consumption is established.

## Historical export-gap checkpoint (2026-10-16)

Branch: `feat/renderflow-430-original-exports`, based on current main
`0427749a2cccececae2d69aa9fa0a02779603509`. The maintainer resumed this bounded
#430 checkpoint and requested the Trelby script artifact too. The preceding
implementation below remains history; its two missing-direction statements are
superseded by authored source, not by passing runtime evidence.

- [Original export design](ORIGINAL_EXPORTS.md) records the policy, architecture,
  artifact trust contract and exact upstream Trelby format pin.
- New `execution.screenplay_body: {allow_unqualified: true}` authors direct
  original strict Fountain → canonical JSON/FDX/Fade In/OSF/Trelby. Shared body
  codecs are separated from the recovery wrapper without weakening its source,
  loss or review contract. Scripttool reverse directions remain rejected.
  Auxiliary canonical bytes count in intermediate inventory/budgets and remain
  in the local artifact store; select `screenplay_json` to publish a companion.
  Native-only copied bundles require that store to resolve the canonical ref.
- `.trelby` is a distinct recognized native artifact. Its independently authored
  version 3 codec accepts only a minimal bounded six-kind body envelope. General
  GUI saves, pagination, editing and Unicode display are unqualified.
- Canonical transcript → text is authored as a terminal under existing subtitle
  and explicit speech policies. Evidence retains canonical/source identity and
  timing/cue/metadata losses; no reverse timing or alignment claim is added.
- Authored regressions cover semantic preservation, unsupported input, Trelby
  framing, source/output/canonical tampering, policy isolation, terminal
  planning and speech/transcript loss admission. No execution count is a pass.
- The existing creative fixture gains an isolated provider-free original body
  job and a transcript text target. The launcher and new scenarios remain unrun.

Permitted execution so far: `python3 scripts/generate_creative_capabilities.py`
(exit 0, 14 authored unqualified routes; data rendering only), source reads and
Git inspection. Tests, compiler/build, formatter/linter, schema validation,
native execution/probes, installers/downloads, fixture launcher and CI have not
run for this candidate. No passing validation or application-opening evidence
is claimed. Use disabled Git hooks and `[skip ci]`; keep the PR draft.

The next authorized qualification session must compile and run the authored
coverage, exercise original Fountain and transcript targets through CLI/SDK,
open native exports in their actual applications (including Trelby), inspect
portable canonical/loss evidence and verify immutable source bytes. #430/#421
stay open; #424/#427/#429/#419 retain their existing ownership. Do not start
another issue or merge this draft until the maintainer's next instruction.

## Previous forest integration checkpoint

Base: merged #447 at `1853824691041491bc6284c9e67c59c6ecda7a48`.
Branch: `feat/renderflow-430-artifact-forest-handoff`. Obtain the checkpoint's
eventual merge SHA from its PR. The maintainer authorized #430 and continued the
implementation sprint with tests, builds, lint, native execution and CI deferred.

## Implementation

- `renderflow/forest-execution-v1` adds deterministic per-target outcomes to
  native run evidence. It keeps planning branches intact and links a published
  target to its exact inventory index, ID, locator and producing step where
  present. Same-content publications retain separate target/destination records.
- Target evidence preserves each candidate's native metadata even when artifact
  IDs match. Hygiene reuse includes the complete candidate record, and its
  evidence and publication blocking remain specific to each target.
- `produced_artifacts` now contains unique actually published terminal
  derivative IDs. It excludes sources, retained intermediates, rejected
  candidates and unchanged source exports. An explicitly published source is
  marked `source_identity: true` instead.
- The creative fixture launcher uses existing public CLI interfaces for
  isolated source-family jobs, retains child plan/run/standalone evidence and
  failures, and reports optional native jobs as unattempted when unconfigured.
  Synthetic inputs and negative scenarios are authored; no successful run
  receipts or installed toolchain observations were fabricated.
- The shared creative route catalog supplies both doctor route declarations
  and generated Markdown/JSON capability documentation. Authored routes remain
  unqualified; unsupported directions are not inferred from tool capabilities.
- [Flow handoff](../../architecture/flow-artifact-forest.md),
  [fixture guide](../../user-guide/creative-forest.md), README, evidence docs
  and ROADMAP now distinguish intent, publication, review and release status.

## Capability reconciliation with #338 and #421

The closed #338 capability sweep is historical scope, not proof that every
arrow in the later #421/#430 diagrams is executable. This checkpoint preserves
that issue's history and reconciles the creative subset explicitly:

| Requested scenario | Current boundary |
| --- | --- |
| Fountain to PDF and analysis | Bounded pinned PDF route and separate deterministic analysis command are authored. |
| Fountain to FDX/Fade In/OSF/canonical | Absent at this historical checkpoint; the current original-body checkpoint above authors a separate built-in direction. #424's native imports do not imply their inverse. |
| PDF/text recovery to canonical and native body formats | Authored, explicitly lossy and review-required; native PDF/OCR execution still needs exact tool/data qualification. |
| Embedded subtitles and local speech | Separate explicit-policy jobs; speech is not a fallback inferred from missing captions. |
| Canonical transcript to SRT/VTT/text | SRT/VTT were authored here; the current checkpoint above authors the previously missing terminal text graph edge. |
| Combined forest | One launcher combines isolated job evidence. It is not a new general multi-source engine or Flow implementation. |

Do not disguise Fountain as unstructured text to satisfy a missing native export
direction. Do not present silence or handwritten provider JSON as a successful
recognition fixture. A skipped native job is not an observed missing provider.

## Verification boundary

Source review and Git content identity checks are permitted. The capability
documentation generator may run because it renders data without invoking
Renderflow or providers. Tests and regression fixtures can be authored but not
executed during this sprint. Do not run compiler/build, formatter/linter,
schema validators, native probes/conversions, installers/downloads, the fixture
launcher or CI. Use disabled commit hooks and `[skip ci]`; do not dispatch or
poll Actions. Keep all unobserved acceptance criteria unchecked.

## Resume and complete the issue

1. Qualify the authored original-Fountain export and graph text-export
   directions with #424/#427/#430 scope before claiming the original full diagram.
   Retain existing issue ownership; create follow-up audit issues only from the
   later test session's actual findings.
2. When execution is authorized, compile and run the focused forest publication,
   catalog and launcher regression coverage; validate actual native run/forest
   schemas and exercise failed, partial, cancelled and duplicate-content cases.
3. Run the provider-free fixture, then explicitly configured real native jobs
   with exact installation/model/font records from the #429 qualification plan.
   Qualify text-bearing/scanned/mixed PDFs, native application opening, captions
   and a redistribution-safe spoken source. Retain all process and artifact
   evidence, including failures and unattempted combinations.
4. Compare CLI and SDK plans/runs, exact target membership/output trees and
   source preservation across repeat runs. Confirm missing-provider isolation,
   bounded cycles, source suppression and preservation of losses/review state.
5. After #419 supplies a verified immutable release with these contracts,
   demonstrate Flow consuming its downloaded binary and native evidence without
   a sibling-source dependency. The existing minimal `flow.artifact/v1`
   projection alone is insufficient for publication/review decisions.

#430 and parent #421 remain open. #406 is the next planned implementation lane;
#412/#413 and the existing adapter/toolchain/release issues retain deferred
qualification. No merge supplies passing evidence or approval of creative output.
