# #420 — bounded immutable source uploads

November 6, 2026 (America/New_York). This second authorized source checkpoint
follows draft #488. The third will cover large streamed artifact retrieval and
native schema coverage. All drafts remain unmerged for maintainer sync.

## Admission and source identity

`allow_source_uploads` defaults false. An operator can enable it only for the
existing manual_lyrics, timed_text or fountain_body profile. Exactly one captured
artifact path, declared format, original config, file allowlist, finite graph,
network/AI denies and process policy remain operator-owned. Composition and
editorial uploads are refused: asset/manifest dependency capture and native
process source admission are not expanded by this bounded text checkpoint.

An authenticated raw `application/octet-stream` POST to
`/v1/pipelines/{pipeline_id}/uploads` requires Content-Length and exact lowercase
`X-Content-SHA256`. Host/origin/auth/operator permission and the effective body
cap precede additional body reads; a small initial header read can contain
coalesced bytes. Incomplete, compressed/chunked/multipart/oversize or digest-
mismatched requests create no upload record. Complete capture returns an opaque
process-local upload ID plus hash/size/expiry metadata, never source text or a
host locator. Upload alone starts no inspection, job, provider or inference.

Inspect/plan/execute accepts optional `upload_id`, bound to the same registered
pipeline. A per-job immutable overlay replaces only that source and its digest;
the existing profile admission runs again before queueing. Effective package and
config identities bind the new bytes. Base package/source files stay unchanged.
Manual lyric classification is rebound to the new digest, with review reset to
unreviewed and old language/revision/cue-section declarations cleared. A client
cannot inherit old source approval, choose a format or supply executable config.
Malformed source data remains the SDK codec's responsibility and can fail a job;
an upload receipt is not format validity or publication permission.

Disabled-upload pipeline identity preserves the previous identity shape. Enabling
the capability is a distinct package identity. Equivalent source bytes/effective
configuration give equivalent package identities independent of upload IDs or
expiry timestamps. New job/pipeline views and source-maintained schemas expose
only declarations and actual captured references.

## Capacity, expiry and cleanup

Defaults: 16 retained uploads, 8 MiB each, 32 MiB total and 15-minute expiry. Each
HTTP body also obeys its 8 MiB binary cap and the service's package cap; the old
16 KiB JSON cap is unchanged. All limits have finite hard admission ceilings.
Private workspace/source/result/process limits remain separately enforced.
Buffers for admitted HTTP workers, transient normalization and native work are
additional bounded/cooperative work; this is not a measured process-memory or
OS quota guarantee. No new dependency, lockfile or workflow change is needed.

Expiry prevents new submissions. Queued/running/retained terminal jobs hold their
exact upload lease, keeping it charged to the finite pool. A referenced upload
cannot be deleted; finish/cancel and explicitly forget the terminal job first.
Unused uploads can be deleted through the metadata endpoint; expired unused
records are reaped. Restart loses upload IDs and does not reconstruct durable
jobs; Flow retains durable orchestration. If response delivery fails after
capture, a retry can create another pending upload, never an automatic job.

Ten new cases are authored/unrun: new lyric words/timing/review reset; frozen
overlay/base identity; operator/cross-pipeline refusal; digest/size/pool limits;
expiry before submission; retained-job lease accounting; config/path/format/
process injection refusal and malformed source failure; legacy defaults/unknown
options/shutdown; authenticated HTTP metadata; early binary-body/digest/type
refusals. Existing struct-literal and framing fixtures are updated for additive
fields. Fake/Cursor tests are not socket, native-provider or quality evidence.

No compiler/build/Cargo resolution, tests, lint/format/schema/docs/OpenAPI
validators, generator/harness, SDK/native/provider/font/media/model/listener/
container execution, install/download, paid request or CI dispatch/polling ran.
No upload, listening server, passing report, provider approval or publication was
performed. Code is uncompiled; only source review and exact Git transport are
evidence. #420 remains open for its original broad API/native/platform/fault/
resource acceptance, wider source/asset profiles, streaming and nested schemas.
#429/#458/#419 and #405/#409 retain their existing gates. No new issue is needed.
