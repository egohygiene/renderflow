# #350 — presentation publication profile

Date: November 3, 2026 (UTC). Branch: `codex/350-presentation-profile`.
Base: `1a193eb3924ad20ee5add77c2c495bb32e9bb900` (main).

The maintainer authorized the Renderflow feature batch on separate draft
branches and retained the implementation-first testing/CI deferral. This PR is
independent of #379/#378. No merge, tag, release or deployment is performed.

## Implementation

- Renderer-neutral `renderflow.presentation/v1`: ordered stable slide IDs,
  sections, literal text, citations, local digest-pinned PNG/JPEG media,
  captions/alt text, notes, two original themes, static geometry/no transitions,
  source revision/digest/freshness, rights and declared review state.
- Shared public SDK and CLI inspect/plan/build. The standalone profile does not
  add another job engine or claim ordinary v2 forest integration.
- Source ZIP, linear HTML companion, local CSS/media and receipt-last output
  publication. Notes are omitted by default. Fixed ZIP metadata and ordered
  members make assembly deterministic; native byte reproducibility is unproven.
- Local Slidev 53.0.0 adapter: explicit Node/program/installation/browser pins,
  before/after persistent-drift observations, fixed no-theme/no-remote-font
  generation, loopback PDF export and shared process cancellation/timeouts.
  Reuses the existing public installed-package fingerprint API currently
  exposed by `screenplay::afterwriting`; no copied snapshot implementation.
- Independent output observations require a bounded regular web tree/HTML index
  or a parsed PDF with the expected page count and 16:9 MediaBoxes. Neither
  provider exit status nor a path alone becomes successful publication.
- Model schema, synthetic deck and source-pinned Renderflow overview draft.
  The README digest was mechanically observed; the overview still needs human
  content approval. No reviewed Identity assets or product approval are invented.

## Source review and qualification

Source review covered literal-content isolation, note removal, local path/media
capture, provider pin admission, shared process controls, output inventory,
no-overwrite publication and deterministic archive metadata. It corrected the
night theme's actual CSS identity, literal newlines that could split Slidev
slides, and the web-tree aggregate entry bound before checkpointing.

Seven authored regression cases cover deterministic source packaging/default
note exclusion; literal HTML/Vue content and language; cancellation/overwrite;
native opt-in/provider admission; provider pins/Node version; duplicate/unknown
model data/transitions; and path/URL boundaries. All are unrun.

No tests, compiler/build/Cargo resolution, formatter/lint/schema/docs validators,
product CLI/source packaging, native/provider/browser/media/PDF execution or CI
ran. No dependencies or lockfiles changed. Source review and Git identities are
the current evidence. #350 stays open for native/offline/two-run/browser,
visual/accessibility, source/API/CLI compatibility and reviewed product-example
acceptance. #458/#419 retain baseline and immutable release ownership.

## Practical limits

No OS/network/RSS/scratch sandbox; network intent is declarative and PDF needs
loopback. Package fingerprints attest supplied regular files, not publisher
trust or dependency closure. Native font metrics/metadata can vary. Input and
final-output bounds are explicit; running native scratch is not quota-enforced.
The initial literal-text model omits arbitrary Markdown/components, animations,
video, remote media and general forest dispatch. Publication approval and remote
hosting remain separate. Partial I/O publication has no completion receipt.

See [the guide](../../user-guide/presentations.md) for exact contracts, limits,
tool preparation, preview/inspection, export, manual deployment and rollback.
