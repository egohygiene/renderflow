# #430 — selectable portable forest demonstration

Authored November 3, 2026 (America/New_York).
Branch: `codex/430-portable-forest-demo`.
Parent: draft #474 at `02f1103d40b17cbb6d7b8825f12c840c10905039`.

The approved publishing stack #468/#470/#471/#472 is merged. Continued forest
work remains in separate draft branches for the next user sync; it is not a new
main merge or release authorization. Draft #473 adds shared file observations;
#474 adds portable retained-inventory export and verification.

## Implemented source

- Add a provider-free `manual-lyrics` fixture using original synthetic SRT, an
  explicit digest-bound unreviewed policy and the eight-target bundled profile.
  Profile identity hashes remain authored declarations, not generated receipts.
  Preserve the canonical companion, source-equivalent SRT distinction and LRC
  timing/grouping losses. The launcher does not duplicate the Rust codecs.
- Add opt-in `--portable` execute behavior. Use the supplied Renderflow binary's
  public `bundle verify`, `export --include-sources` and `verify-portable` commands.
  Pin the native receipt and export index, rename the handoff, and verify with
  only the moved root/index digest. Keep all original sources/caches intact.
  Selection, parsing and later packaging share the same exact native receipt
  digest, so a later metadata change cannot silently select a new receipt.
- Retain separate bounded process/capture evidence for each operation, copied
  index/receipt references and independent bundle status. Packaging can preserve
  observed partial-run publications without making the job complete. Refusals,
  standalone jobs and unconfigured native jobs fabricate no handoff evidence.
- Static describe records the opt-in in the effective request/digest; plan mode
  refuses it before output or child creation. Extend the existing request/result
  schema with optional envelope fields, without changing native or Flow schemas.

## Review and remaining acceptance

Six new meaningful regression cases are authored/unrun: manual-source binding
and explicit profile scope; static opt-in describe; plan refusal before output;
partial/skipped status preservation; failed packaging retaining its actual logs;
and moved-verification pin/root selection without promoting original run state.
The last cases use clearly synthetic CLI envelopes and are not Rust verifier,
native conversion or file portability qualification.

No tests, compiler/build/Cargo resolution, lint/format/schema/docs validators,
product CLI/provider/native/media/model execution or CI ran. Source inspection
and exact Git tree identities are the checkpoint evidence. No dependencies,
workflows, tags, releases or speculative issues were added. Existing gates are
unchanged. #430/#421 own real multi-family/SDK/CLI/portability acceptance, #429
owns toolchain qualification, and #458/#419 own baseline/released-consumer work.

The launcher retains originals rather than deleting them to simulate an offline
host. Disconnected-host behavior, actual CLI/native receipts, platform/filesystem
compatibility, interruption/cancellation, format/player fidelity and released Flow
consumption remain unobserved. The handoff is internal evidence containing
originals/metadata; transport success does not sanitize it or grant approval.

Next sync: review the three source drafts in order, then schedule the deferred
qualification sweep using the existing issues. Do not close roadmap acceptance
for authored cases or merge these drafts without the next user instruction.
