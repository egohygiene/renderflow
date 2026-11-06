# #420 — verified streaming and native-schema source checkpoint

November 6, 2026 (America/New_York). This third authorized draft follows #488
(large comic) and #489 (bounded immutable uploads). All remain drafts for sync;
no merge, release, model, provider or service execution is part of this sprint.

## Integrity before transport admission

Core `evidence::verification::copy_publication` selects an actual native target
join and reuses the existing bounded file observer with confinement and observed
symlink refusal.
It copies to a caller-owned provisional sink and returns verified evidence only
after expected size/hash, named/open identity, root rechecks and cancellation/
deadline checks succeed. Its sink can contain provisional bytes after failure;
callers must keep them private. The old buffered reader's 32 MiB ceiling remains.

The optional adapter's `prepare_download` reserves a finite snapshot slot and
native-sized byte allowance, checks private workspace capacity, then copies into
a private temporary file. Verification and destination-length checks must pass
before an HTTP success response exists. The handle owns the file and reservation/
workspace leases. Job expiry or explicit terminal forget cannot invalidate an
admitted snapshot. Failed preparation drops both file and permit. Normal native
results/manifests remain unchanged; no second renderer or file verifier is added.

The artifact GET route now streams that private snapshot in 64 KiB chunks with
fixed Content-Length, observed SHA-256, strong digest ETag and attachment policy.
HEAD performs the same observation and sends no body. Ranges receive a typed 416;
no range/resume/conditional transfer is implied. Delivery is single-use, checks
length/hash and cancellation/deadline, and keeps the lease until drop. A socket
failure after headers closes without a JSON suffix; incomplete length/hash is
not successful receipt. Telemetry retains the existing delivered flag without
source/path/token/body logs. Browser/public-bind/tenant policy is unchanged.

Defaults: two snapshots, 128 MiB per snapshot, 256 MiB total reservation and
30-second preparation/delivery budgets, plus existing workspace admission and
the HTTP write deadline (five seconds by default). Snapshot files are private
and temporary, normally unnamed by the OS. Reservation accounting covers them
even where directory scans cannot see open unlinked files. Other workspace
activity and native I/O still have cooperative limits; this is not measured
memory/performance, hard OS quotas or a hostile-filesystem sandbox. The legacy
buffered library API remains separately bounded. Shutdown cancels core copying
and delivery. No dependency, lock or workflow change is added.

## One native schema owner

`renderflow::api_contract::json_schema` projects the existing core run/intake
schemas together with core SDK result definitions. The new source schema covers
ArtifactProfile, ProviderPlan/ExecutionPlan, graph/source/collection/geometry,
selected toolchains, ExecutionResult, diagnostics and ProgressEvent. Native
artifact/step/validation/hygiene/forest models remain imports from their current
core schema. Toolchain shape is connected explicitly. JSON-valued metadata,
policy and options stay declared extension data, not opaque native root models.

OpenAPI replaces its loose native-result placeholders with references to this
core-owned schema. The authenticated raw metadata endpoint serves a self-
contained projection: all native references are local within that document.
It assembles embedded declarations only, with no file/provider/model/network
access or runtime validation. No competing API renderer/schema model or new
schema dependency is added. Serde shape coverage and semantic/runtime/native
join checks remain distinct; no schema validity or output approval is inferred.

Thirteen new cases are authored/unrun: over-32-MiB core copy with bounded writes;
sink/cancellation refusal; snapshot beyond configured buffer cap with immutable
bytes after source change/job forget; corrupt native input cleanup; concurrent
reservations/drop; interrupted writer cleanup; size/deadline refusal; shutdown;
HTTP GET/HEAD/hash/ETag/range behavior; pre-response JSON integrity failure;
authenticated schema metadata; local schema-reference closure; and SDK field
ownership. These include synthetic file receipts and in-process SDK/Cursor
fixtures, not paid, socket, native-provider or model quality evidence.

No compiler/build/Cargo resolution, tests, lint/format/schema/docs/OpenAPI
validators, runtime schema assembly, generator/harness, SDK/native/provider/font/
media/model/listener/container execution, install/download or CI dispatch/polling
ran. No artifact was generated, uploaded or delivered by this source sprint.
All 28 new cases across the three drafts remain uncompiled/unrun. Source review
and exact Git trees are transport evidence only. #420/#413/#412 and #429/#458/#419
retain their full original acceptance/qualification; #405/#409 gates stay intact.
