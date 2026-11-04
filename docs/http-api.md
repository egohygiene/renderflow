# Local HTTP and job service

The optional `renderflow-api` executable adapts the same public SDK used by the
CLI. This is an **authored, uncompiled and unrun #420 checkpoint**, available in
source only. No listener, container, output or passing API/OpenAPI report was
started or produced during this sprint. It is not a published distribution.

The first admitted profile is native page composition. A trusted local operator
registers a dedicated package root, flat v2 config and explicit relative file
allowlist. Startup captures bytes and an original-package identity; each job
gets a private copy and a separately identified effective config, with output
placed under its own `bundle`. A request selects a registered ID and optional
dry run. It cannot provide a host path, plugin, provider executable, AI endpoint,
arbitrary configuration or uploaded source. General SDK profiles can be admitted
later with explicit package/provider policies under the existing issue.

## Local startup

The [sample configuration](../examples/http-api/service.json) uses the original
mixed-asset composition fixture and its retained DejaVu font/license. It emits
the native default SVG/HTML/transcript candidates for both declared locales.
All examples below are **unrun commands for the later qualification session**.

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
connection. POST uses explicit Content-Length and UTF-8 `application/json`;
encoded/chunked bodies, `Expect: 100-continue`, duplicate headers, root arrays,
nested duplicate JSON keys and unknown request fields are refused. Routes use
literal IDs, without percent-encoded paths or arbitrary filesystem locators.

```bash
curl --noproxy 127.0.0.1 http://127.0.0.1:8787/health

curl --noproxy 127.0.0.1 \
  -H "Authorization: Bearer $RENDERFLOW_API_TOKEN" \
  -H "Content-Type: application/json" \
  --data '{"pipeline_id":"sample-page","dry_run":true}' \
  http://127.0.0.1:8787/v1/execute
```

These examples do not create a job until explicitly run. All inspect, plan and
execute requests return `202` plus a `Location` job path. Poll that path for the
structured result. The SDK executes one captured resolved build per execution;
no terminal output is parsed and no CLI subprocess is involved.

| Endpoint | Behavior |
| --- | --- |
| `GET /health`, `GET /ready` | Process/configured admission state, without provider probes |
| `GET /v1/pipelines` | Registered package IDs, capture sizes/digests and profile |
| `POST /v1/inspect`, `/v1/plan`, `/v1/execute` | Queue the corresponding shared SDK operation |
| `GET /v1/jobs/{job_id}` | Current state, cancellation intent, bounded native result/error and progress cursor |
| `DELETE /v1/jobs/{job_id}` | Request cancellation; keep worker/workspace until the SDK returns |
| `GET /v1/jobs/{job_id}/events` | Finite replay of retained native SDK progress as SSE |
| `GET /v1/jobs/{job_id}/artifacts` | Actual native publication joins; file bytes remain unobserved in the listing |
| `GET /v1/jobs/{job_id}/artifacts/{artifact_id}` | Bounded binary attachment copied during shared size/hash observation |
| `DELETE /v1/jobs/{job_id}/retention` | Forget terminal data; active jobs cannot be deleted |
| `GET /v1/formats`, `/v1/transforms`, `/v1/capabilities` | Admitted service declarations; broader SDK inventory is separately labelled |
| `GET /openapi.json` | Maintained [OpenAPI 3.1 envelope document](http-api.openapi.json) |

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

## Progress, artifacts and retention

SSE sends retained `event: progress` records with native `ProgressEvent` payloads,
then `event: job` metadata and closes. Reconnect using `?after=N` **or**
`Last-Event-ID`. Count/byte eviction, gaps (including dropped tail events) and
the latest cursor are explicit. This is finite replay, with the SDK's actual
coarse phase/step notifications; no percentages or always-open stream is inferred.

Artifact IDs are job-local references to exact native publication joins.
Downloads ignore receipt-declared host roots, use the private bundle, refuse
unsafe/changed/missing files and return observed bytes with `X-Content-SHA256`.
HTML/SVG are attachments with restrictive response policy. The default buffer
cap is 8 MiB; a larger native publication receives 413. Streaming large outputs
remains future work. Verified bytes grant no creative, print, publisher or release
approval. Native receipts and review state retain that distinction.

Terminal data expires after 15 minutes by default, or can be explicitly
forgotten. In-flight download handles retain their workspace until observation
finishes. Captured operator packages remain in memory until shutdown. Restart
loses API job IDs and queue state; it does not infer SDK checkpoint resume or
replace Flow's cross-tool orchestration. Stop requests cancellation and allows
a short grace period. Forced exit/native abort can leave private workspace data
and incomplete bundles; no interrupted run is promoted to success.

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
specifies actual API envelopes/requests; native plan/manifest objects remain
SDK-owned opaque nested schemas here. Full nested native/OpenAPI/schema/runtime
qualification has not run.

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
#420 remains open for broader profiles/uploads/large streaming, recovery and all
deferred compilation, API/OpenAPI, process, fault, platform and release evidence.
