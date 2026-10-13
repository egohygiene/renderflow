# #430 — multi-family forest integration checkpoint

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
| Fountain to FDX/Fade In/OSF/canonical | General original-Fountain export adapters are still absent. #424's authored native import direction does not imply its inverse. |
| PDF/text recovery to canonical and native body formats | Authored, explicitly lossy and review-required; native PDF/OCR execution still needs exact tool/data qualification. |
| Embedded subtitles and local speech | Separate explicit-policy jobs; speech is not a fallback inferred from missing captions. |
| Canonical transcript to SRT/VTT/text | SRT/VTT graph export is authored; plain text remains a standalone export surface. |
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

1. Reconcile the missing original-Fountain export and graph text-export
   directions with #424/#430 scope before claiming the original full diagram.
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
