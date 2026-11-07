# Bounded local status and upload recovery

November 7, 2026 source checkpoint, based on main
`4d887e95fbb43dc86167b7029e49595bb6704295`. #420 remains open.

The service may opt into `persistence_directory`, an existing owner-only local
directory separate from source packages and job storage. Omission retains the
existing process-local behavior. The new path is operator configuration, never
a client request. Initial implementation requires Unix-style permissions and
filesystem locking; native/platform qualification is unobserved.

For example, add `persistence_directory: /private/renderflow-state` alongside
`workspace_parent` in the service configuration after preparing that directory
with owner-only permissions. Keep it separate from the ephemeral job directory.
Do not modify it while its writer lock is held. Catalog or limit changes require
manual preservation/reconciliation of the prior journal before a new state set.

One lifetime OS writer lock protects a versioned local journal. Each admitted
upload is an immutable private blob with exact hash/size/expiry. Job admission,
running/workspace identity, terminal result, explicit cancellation, forget and
retention changes are journaled before their API acknowledgement, SDK work or
later status exposure. Writes sync a private temporary file, rename the journal
atomically, then sync its directory. New upload bytes are synced before the
journal can reference them. Orphan cleanup follows the committed snapshot;
orphan blobs never become admitted uploads. Read/save fail closed on unsafe
paths/files, unknown entries, excessive counts/bytes, catalog/limit changes,
corrupt records or changed upload bytes. Existing state is preserved for review.

The journal and upload blobs share `workspace_storage_bytes` with observed
process-local workspace usage, including space for the next atomic snapshot.
Each snapshot is also limited to half that ceiling. Result/upload count and
byte limits still apply. These are cooperative observations, not a reserved
filesystem quota, preemptible IO, bounded total host RSS or an adversarial host
filesystem sandbox. Synchronous bounded journal/hash IO can delay other local
requests. Private state can contain source text and native private paths; it is
not a public provenance document, signed attestation or tamper-proof database.

Restart binds the stored catalog and limits to freshly captured packages, checks
unexpired blob bytes, and restores only the remaining upload lifetime. Expired
uploads are not usable. Active/queued/cancelling records become terminal failed
status with error code `service_interrupted`; no native result, cancellation
success, resume, retry or expensive rerun is inferred. An interruption after
native completion but before durable acknowledgement is still interrupted.

Completed status/results remain historical. `recovered_after_restart: true`
and `workspace_retained: false` state that the old binary workspace was not
retained. Artifact access returns 409 `workspace_not_retained`, rather than
advertising guessed paths or rebuilding files. Binary workspace recovery is
outside this checkpoint. Remaining terminal retention is bounded and does not
reset at restart. Progress payloads are not journaled continuously; recovered
events expose a gap, and consumers must reset stale cursors after recovery.

Journal failure stops new admission, cooperatively cancels current work and
returns a storage error; further job status reads refuse a durable-success
claim. Single-writer locking stays held until workers release the service book,
so a concurrent replacement process cannot silently take over still-live work.
This is local transport bookkeeping. Core owns plans/native evidence and Flow
owns cross-tool execution/recovery. No scheduler or automatic replay is added.

Six focused journal regressions are authored, uncompiled/unrun: interrupted
recovery, exact upload restoration, changed/corrupt state refusal, exclusive
writer/catalog binding, expiry/orphan cleanup, and failed-commit admission stop.
No compiler/build/Cargo/test/lint/format/schema/docs/OpenAPI validator,
SDK/native/provider/model/listener/container execution, dependency install or
CI ran. Power-loss/process-fault/socket/platform and native qualification,
broader profile support and immutable release gates remain with #420/#458/#419.
