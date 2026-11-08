# Local HTTP and job service

The optional `renderflow-api` executable adapts the same public SDK used by the
CLI. This is an **authored, uncompiled and unrun #420 checkpoint**, available in
source only. No listener, container, output or passing API/OpenAPI report was
started or produced during this sprint. It is not a published distribution.

Registered packages now select a finite service profile. A trusted local
operator chooses a dedicated root, flat v2 config and explicit relative file
allowlist. Startup captures bytes and an identity including profile/effect
permission. Each job gets a private copy, an effective-config digest and its own
`bundle`. Clients select a registered ID, optional dry run and optional immutable
source-upload ID. Uploads require explicit operator permission for a built-in
text profile; clients cannot choose host paths, formats, providers or configs.

| Operator `profile` | Existing SDK behavior | Process effects |
| --- | --- | --- |
| `composition` (default) | Native layered/localized page targets | Built-in |
| `manual_lyrics` | Explicitly supplied SRT/WebVTT/timed JSON through the manual lyrics policy | Built-in; no recognition/alignment/model |
| `timed_text` | Standalone SRT/WebVTT/timed JSON conversion and terminal text | Built-in; no embedded-media extraction |
| `fountain_body` | Original Fountain to the existing five body-export formats | Built-in; explicit unqualified policy retained |
| `editorial` | Bundled editorial Markdown to HTML/PDF/DOCX/EPUB and original-source identity | Requires `allow_local_processes: true`; local Pandoc and optionally Tectonic |

The new graph profiles clamp tool/transform allowlists, deny network/AI requests,
refuse custom registries/profiles/templates/variables/validators and require
captured source membership. Only unchanged selected core bundled profiles may
be expanded. Core validates and executes each native policy. Target-oriented
`all_reachable` forests stay within those same finite edges; this is not an
unrestricted generic graph endpoint. Missing tools/unsupported branches and
native partial results remain explicit. Inspection/planning editorial jobs can
perform provider discovery as part of the existing SDK resolver.

The default composition configuration remains compatible. Library/core native
plan/run/artifact objects pass through unchanged. API job/pipeline views add
`profile`, `local_processes`, upload capability and the job's optional source
upload reference; the maintained draft OpenAPI describes them.

## Local startup

The [sample configuration](../examples/http-api/service.json) uses the original
mixed-asset composition fixture and its retained DejaVu font/license. It emits
the native default SVG/HTML/transcript candidates for both declared locales.
For built-in multi-profile operation, select [local-profiles.json](../examples/http-api/local-profiles.json).
[editorial-opt-in.json](../examples/http-api/editorial-opt-in.json) additionally
authorizes local provider discovery/execution. All examples below are **unrun
commands for the later qualification session**.

1. Review the operator configuration. Roots resolve relative to its directory;
   `workspace_parent` must already exist and contain no symlink components.
   The sample uses Linux `/tmp`; choose an appropriate real directory elsewhere.
2. Set `RENDERFLOW_API_TOKEN` to a fresh random value with 32–256 ASCII
   non-whitespace bytes. Keep it outside the JSON file and source control.
3. Build/start explicitly from this source checkout:

```bash
cargo run --locked --package renderflow-api --bin renderflow-api -- \
  --config examples/http-api/service.json
```

The default listener is `127.0.0.1:8787`. Only loopback IPs are accepted; no hosted
service, public bind, TLS, cloud identity or browser session is configured.
The executable's credential comes from the environment; embedding hosts can
provide another implementation of the `Authorizer` port. The credential grants
access to all registered packages and jobs in this one trusted local process.
This is not a multi-user tenant boundary or OS sandbox.

Health reports process liveness. Readiness reports configured admission/shutdown
state, with provider readiness `not_probed`. A healthy listener is not provider,
rendering, toolchain or release qualification; individual queue/storage limits
can still refuse a submission.

## Requests and results

Except for `GET /health`, endpoints require `Authorization: Bearer ...`.
Host must identify the exact loopback listener or `localhost` on its port.
Browser `Origin` requests are refused; CORS, cookies, upgrades and proxy identity
headers do not create a session. Only one HTTP/1.1 request is served per
connection. POST uses explicit Content-Length and UTF-8 `application/json`, except
the explicitly enabled raw binary upload endpoint;
encoded/chunked bodies, `Expect: 100-continue`, duplicate headers, root arrays,
nested duplicate JSON keys and unknown request fields are refused. Routes use
literal IDs, without percent-encoded paths or arbitrary filesystem locators.

```bash
curl --noproxy 127.0.0.1 http://127.0.0.1:8787/health

curl --noproxy 127.0.0.1 \
  --header @/path/to/private-auth-header \
  -H "Content-Type: application/json" \
  --data '{"pipeline_id":"sample-page","dry_run":true}' \
  http://127.0.0.1:8787/v1/execute
```

These examples do not create a job until explicitly run. All inspect, plan and
execute requests return `202` plus a `Location` job path. Poll that path for the
structured result. The SDK executes one captured resolved build per execution;
no terminal output is parsed and no CLI subprocess is involved.
The private header file contains the authorization header, is operator-owned
with restrictive permissions, and stays outside source control. This keeps the
credential out of curl's command arguments; no token/header file was created here.

| Endpoint | Behavior |
| --- | --- |
| `GET /health`, `GET /ready` | Process/configured admission state, without provider probes |
| `GET /v1/pipelines` | Registered package IDs, capture sizes/digests and profile |
| `POST /v1/pipelines/{pipeline_id}/uploads` | Capture one bounded immutable source for an enabled built-in text profile; no job starts |
| `GET`, `DELETE /v1/uploads/{upload_id}` | Metadata before expiry, or forget only when no retained job references it |
| `POST /v1/inspect`, `/v1/plan`, `/v1/execute` | Queue the corresponding shared SDK operation |
| `GET /v1/jobs/{job_id}` | Current state, cancellation intent, bounded native result/error and progress cursor |
| `DELETE /v1/jobs/{job_id}` | Request cancellation; keep worker/workspace until the SDK returns |
| `GET /v1/jobs/{job_id}/events` | Finite replay of retained native SDK progress as SSE |
| `GET /v1/jobs/{job_id}/artifacts` | Actual native publication joins; file bytes remain unobserved in the listing |
| `GET`, `HEAD /v1/jobs/{job_id}/artifacts/{artifact_id}` | Full verified private snapshot; fixed length/hash headers and bounded binary delivery |
| `DELETE /v1/jobs/{job_id}/retention` | Forget terminal data; active jobs cannot be deleted |
| `GET /v1/formats`, `/v1/transforms`, `/v1/capabilities` | Admitted service declarations; broader SDK inventory is separately labelled |
| `GET /openapi.json` | Maintained [OpenAPI 3.1 envelope document](http-api.openapi.json) |
| `GET /schemas/renderflow-sdk-api-v1.schema.json` | Authenticated, self-contained core SDK result/progress schema document |

Job states are queued/running/cancelling/planned/complete/partial/failed/cancelled.
Cancellation intent is separate from native outcome. A late native success stays
successful, and a partial native run keeps its actual published siblings. A dry
run stays planned and provides no downloadable publication. Inspection/planning
completion does not imply a render. Native validation, loss, provenance and
human-review requirements remain in the unchanged SDK result.

Every nonbinary, non-SSE response uses a schema/request-ID envelope. Errors have
stable adapter codes and HTTP status. Native execution can return a `200` job
resource with failed/partial state; consumers must inspect that state and native
evidence. Core planning/execution errors without a native result use generic
structured errors, avoiding exposure of raw operator paths or source content.
Native SDK models still contain their own private-job paths and are available to
the authenticated local operator; those paths never become download authority.

## Optional immutable source uploads

The [source-uploads example](../examples/http-api/source-uploads.json) explicitly
enables one source slot for each built-in manual-lyrics, timed-text and Fountain
profile. Other examples keep uploads off. Composition/editorial config and asset
uploads are not admitted. Read the [upload contract](work/420/UPLOADS.md) before
enabling it. The format, source path, graph and effects stay fixed by the captured
pipeline; only new source bytes may be supplied.

The following are **unrun examples** for after qualification. Replace the hash
with `sha256sum` of the exact file. The private header file holds the bearer
header outside source control and process arguments:

```bash
curl --noproxy 127.0.0.1 \
  --header @/path/to/private-auth-header \
  --header 'Content-Type: application/octet-stream' \
  --header 'X-Content-SHA256: REPLACE_WITH_EXACT_FILE_SHA256' \
  --data-binary @examples/lyrics/manual.srt \
  http://127.0.0.1:8787/v1/pipelines/manual-lyrics/uploads
```

A successful 201 returns metadata and `Location: /v1/uploads/upload-...`, not a
job. Submit that exact ID in a normal JSON request, for example:

```json
{"pipeline_id":"manual-lyrics","upload_id":"REPLACE_WITH_ACTUAL_UPLOAD_ID"}
```

Plan/inspect it before explicitly selecting execution. The upload ID must belong
to the selected pipeline and be unexpired. Job metadata records the source hash,
size, upload ID and an explicit `review_inherited: false`. Manual lyric uploads
start unreviewed and clear old language/revision/section annotations. This does
not infer approval or timing; the supplied file remains the only timing source.
Invalid content can still fail core parsing/planning. The operator's original
files and registered base package do not change.

Defaults are 16 uploads, 8 MiB per file, 32 MiB total and 15-minute expiry. The
effective body bound also respects HTTP and package limits. Expiry prevents new
jobs; retained jobs keep their exact source lease charged to the pool. Finish or
cancel a job, wait for terminal state, then forget it before deleting its upload.
DELETE of an upload in use returns 409. By default uploads are process-local.
An explicit private `persistence_directory` retains bounded, digest-checked
unexpired uploads and historical job records across restart; see
[local recovery](work/420/RECOVERY.md). No upload starts a provider, model,
installation or job by itself.

## Progress, artifacts and retention

SSE sends retained `event: progress` records with native `ProgressEvent` payloads,
then `event: job` metadata and closes. Reconnect using `?after=N` **or**
`Last-Event-ID`. Count/byte eviction, gaps (including dropped tail events) and
the latest cursor are explicit. This is finite replay, with the SDK's actual
coarse phase/step notifications; no percentages or always-open stream is inferred.

Artifact IDs are job-local references to exact native publication joins.
Downloads ignore receipt-declared host roots and observe the actual private
bundle through core's existing confined size/hash observer. Bytes first go into
a bounded private temporary file. A successful observation and snapshot length
check must finish before a 200 response is constructed. Failed preparation
returns a JSON error; it cannot send an unchecked successful artifact body.

The HTTP route then streams fixed-size chunks from that snapshot, with exact
Content-Length, `X-Content-SHA256` and a strong digest ETag. HEAD observes the same
full snapshot and sends headers only. Range requests receive 416; partial,
conditional and resumable transfers are not implemented. HTML/SVG remain
attachments with restrictive response policy, and unknown MIME values use a
binary fallback. Native format descriptors still own filename extensions.

Defaults allow two concurrent snapshots, 128 MiB each, 256 MiB total reserved
snapshot bytes, and separate 30-second cooperative preparation/delivery budgets.
Private workspace storage admission also applies. The HTTP write deadline is
independent and may be shorter (five seconds by default). Reservations and job
workspace leases stay held until the response is dropped, including when a job
expires or is forgotten during delivery. Cancelled preparation, corrupt sources,
capacity/size/deadline refusals and shutdown do not yield a usable snapshot.

Client disconnect, write failure, deadline or shutdown after headers closes the
connection without appending JSON to binary data. Check full received length and
SHA-256 before accepting a download; an incomplete body is not a successful
artifact receipt. Delivery rechecks snapshot size/hash, and request telemetry
reports incomplete delivery. Snapshot storage is temporary, not a publication or
durable export. The library's old buffered `download` convenience keeps its 8 MiB
default and 32 MiB ceiling; `prepare_download` is the separate large-file port.
Verified bytes grant no creative, print, publisher or release approval.

Terminal data expires after 15 minutes by default, or can be explicitly
forgotten. In-flight download handles retain their workspace until observation
finishes. In-flight stream handles also retain their snapshot/workspace permits.
Captured operator packages remain in memory until shutdown. Restart
loses API job IDs and queue state; it does not infer SDK checkpoint resume or
replace Flow's cross-tool orchestration. Stop requests cancellation and allows
a short grace period. Forced exit/native abort can leave private workspace data
and incomplete bundles; no interrupted run is promoted to success.

## Editorial effect boundary

`allow_local_processes` is operator permission, bound into package identity. It
is mandatory for editorial and refused for built-in profiles. HTTP requests
cannot alter it. Capture/catalog/health do not probe Pandoc; an explicitly
submitted editorial inspect/plan/execute may do so through the SDK. The source
article and declared local assets come from the captured package; fonts/templates
come from the existing bundled editorial strategy. External font registries,
custom templates and client-supplied programs are not admitted.

Prepare provider versions and Tectonic's local cache separately. No installation,
download, remote fallback or job execution occurred in this sprint. Existing
editorial provider/resource/cancellation limitations still apply: the API's
cooperative deadline may mark cancellation intent while an in-flight strategy
process is still returning. The workspace and worker slot remain held, and the
native outcome is preserved. This is one trusted local operator, with no OS
sandbox, hard process-kill, firewall, durable recovery or qualification guarantee.

## Resource limits and deployment

Defaults: four HTTP workers, eight admitted connections, 8 KiB/32 headers,
16 KiB request bodies, 4 MiB JSON responses, one-second read idle, five-second
read/write budgets and five-second shutdown grace. Saturated socket admission
closes before parsing; job/retention capacity returns structured 503. No thread
is created per client connection. A bounded header read can contain coalesced
body bytes before authentication; no additional body reads/decoding or job
admission occurs until host/origin/auth gates pass.

Application defaults and cancellation/storage limits are in the
[service contract](work/420/SERVICE.md). Socket I/O uses per-read/write remaining
budgets. SDK deadlines/storage monitoring request cooperative cancellation;
they cannot interrupt arbitrary in-flight native CPU or filesystem work and do
not impose OS memory/disk quotas. Logging emits generated request IDs, status,
delivery and timing only, without URL/query/header/body/token/native-result logs.

The operator configuration [JSON schema](../schemas/renderflow-api-service-v1.schema.json)
describes both application and HTTP envelopes. Code additionally enforces
cross-field bounds and profile/package constraints. The OpenAPI document
specifies actual API envelopes/requests; native plan/manifest objects reference
core-owned typed schemas. The
[SDK schema source](../schemas/renderflow-sdk-api-v1.schema.json) describes
inspection, provider plans, execution results and progress, including graph
nodes/edges/waves, source profiles/collections, toolchains, run/artifact/step
evidence, validation, hygiene and forests. Existing core run/intake definitions
are imported rather than copied into the adapter. The authenticated schema
endpoint serves a self-contained projection with local references, so API
clients can retain it for offline use. This projection performs no provider,
source or network operation. Native JSON-valued policy/options/metadata remain
explicit extension data; shape alone does not check semantic joins or file bytes.
Schema assembly, structural validation and runtime qualification remain unrun.

An optional [container recipe](../crates/renderflow-api/Dockerfile) builds this
source binary. It is unbuilt and unpublished. Use a reviewed configuration with
package roots under a read-only `/packages` mount and writable private
`workspace_parent: /data/jobs`. The runtime UID is 10001. A Linux local example:

```bash
docker build -f crates/renderflow-api/Dockerfile -t renderflow-api:local .
docker run --rm --network host --read-only \
  --env RENDERFLOW_API_TOKEN \
  --mount type=bind,src=/absolute/config,dst=/config,readonly \
  --mount type=bind,src=/absolute/packages,dst=/packages,readonly \
  --tmpfs /data/jobs:rw,size=536870912,uid=10001,gid=10001,mode=0700 \
  --tmpfs /tmp:rw,size=67108864,mode=1777 \
  renderflow-api:local --config /config/service.json
```

These commands are unrun. Linux host networking keeps the loopback listener
local; ordinary container port publishing does not reach a container-internal
loopback bind. Pin/verify image digests, clean builds, complete license/SBOM
records and supported-platform behavior during #419's release sweep. No image,
port forward, deployment, provider or service was started by this checkpoint.

See [handoff](work/420/HANDOFF.md) and [dependency decision](work/420/HTTP_DEPENDENCY.md).
#420 remains open for broader source/asset profiles, recovery and all
deferred compilation, API/OpenAPI, process, fault, platform and release evidence.

The operator-selected local briefing profile reuses the common publisher with
exact captured references and no new client uploads/processes; see
[briefing HTTP admission](work/349/HTTP.md) and its
[unrun operator sample](../examples/http-api/briefing.json).
