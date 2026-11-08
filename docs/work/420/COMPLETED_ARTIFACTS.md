# Opt-in completed publication recovery

Source checkpoint for #420, November 7, 2026. Add
`retain_completed_artifacts: true` alongside an existing private
`persistence_directory` in the operator configuration. Omission/false preserves
the status/upload-only journal and its original catalog binding. Enabling or
disabling retention changes the binding: preserve and reconcile prior state
before changing policy. It is not a client upload or request option.

Only complete Execute results with actual native published-target joins are
eligible. Core verifies each original bundle-relative file while copying it
into `artifacts/job-<id>/bundle/`; native result JSON and a versioned descriptor
are synced privately, with `cache.json` written last. The journal is committed
after the copy. Original native result fields, locators, hashes, validation,
fidelity and historical processing paths remain unchanged. Source packages,
models, intermediate workspace state and workers are not restored.

Restart binds the descriptor to the recorded native result, checks the exact
private regular-file membership and recopies through core's publication
verifier into a fresh bounded download-only workspace. Bytes are checked again
on each download. A recovered job reports `recovered_after_restart: true`,
`workspace_retained: false`, `artifacts_retained: true`. This restores download
access, not rendering/resume authority or content approval. No retention or
expiry lifetime resets. Interrupted jobs still fail `service_interrupted`;
plan/inspect/dry-run/partial/failed jobs are not cached.

The cache, journal, upload blobs, temporary copies and delivery reservations
share existing storage admission. Files are limited to 4,096 publications,
64 path levels, existing per-job bytes and a cooperative copy deadline; recursive
inventories are bounded to 65,536 nodes. Copy admission includes metadata and
directory overhead. These observed limits are not an OS quota or protection
against a hostile concurrent filesystem writer. Private native results may
include local source text/paths. Keep the state private.

Forget/expiry first journal an owned retired-cache descriptor, then verify its
current bytes and delete that cache. Pending garbage survives restart. Already
prepared download snapshots hold their own file/workspace leases, so removing
the durable cache does not invalidate an admitted stream. IO/corruption stops
admission and refuses durable success. Unknown/partial/orphan directories remain
preserved and charged; a missing commit marker is never adopted or replayed.

Restart, unchanged-result/download, cache tamper, lease-after-forget, opt-in and
non-publication cases are authored, uncompiled/unrun. No tests/builds, dependency
resolution, validators/generators, product/native/listener execution or CI ran.
Power-loss, socket, filesystem/platform and provider qualification remain open
under #420 and existing validation/release gates.
