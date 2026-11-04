# #430 — portable retained inventory

Authored November 3, 2026 (America/New_York).
Branch: `codex/430-portable-forest-bundles`.
Parent: draft #473 at `a175323f6156b76aaf3d9be11cfcc1109374dc83`.

The user authorized the publishing merges and continued artifact-forest source
work while retaining the executable-qualification deferral. This is the second
bounded #430 checkpoint, reusing pinned receipts and streaming file observations.
It does not authorize a new main merge or release.

## Implemented source

- Add a portable index over an unchanged native run receipt and content-addressed
  copies of retained inventory. Every included origin is freshly observed;
  same-content destinations keep native identities while sharing copied objects.
- Include all non-source records and required canonical/source companions.
  Remaining originals are optional and explicitly external by default. Known
  v1 body/recovery/subtitle/speech/lyrics refs and parent IDs must be in inventory;
  opaque extension strings never become host paths to follow.
- Require explicit old bundle/store roots. CAS locators must match their digest;
  missing/contradictory/unsupported dependencies fail rather than invent bytes.
- Add `bundle export` and `verify-portable`, sharing SDK code and I/O limits/
  cancellation. New directory reservation and completion-last output preserve
  existing files. Only owned staging copies are linked, not originals.
- Verify moved snapshots from pinned index/native receipt/data alone. Public
  views retain original target/native metadata and copied-payload mappings;
  run/validation/fidelity/review states remain unchanged.
- Keep native/Flow schemas intact; the index is an additive sidecar. Root/index
  object-shape checks and a grouped expected-file argument keep the shared
  observer explicit without dependencies or weakened gates.

## Review and deferred acceptance

Source review corrected required-canonical-source exclusion, repeated digest
lookup work, inline reference lineage/kind checks, metadata-vs-data byte bounds,
receipt/index shape admission, destination receipt re-observation, phase deadline
handling and an initially mis-scoped missing-store regression. No compiler or
formatter was used; no build or lint success is claimed.

Eight meaningful cases are authored and unrun. One uses the ordinary built-in
Fountain-body SDK path when qualification resumes; it is not native-application
fidelity evidence. Source review and exact Git tree identities are the evidence.
No tests, compiler/build/Cargo resolution, lint/format/schema/docs validators,
product CLI/provider/native/media/model execution or CI ran. No dependencies,
lockfiles, workflows, tags or releases changed.

The package is an internal evidence handoff with native metadata/production
content, not automatic sanitization or remote publication. Hard-link/filesystem
support, CLI/SDK parity, real family fixtures, cancellation/interruption and
reproducible/moved-bundle behavior remain unobserved. Complete native receipt
semantics, unknown extension refs, hostile concurrent filesystem safety and
publisher/creative approval are outside this scoped transport proof.

#430/#421 and #458/#419 retain existing acceptance/release gates. No speculative
issue was created. Next: wire verification/export into the selected multi-family
demo, preserving actual observed versus unattempted states.
