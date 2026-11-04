# #420 — bounded local service application checkpoint

Authored source only. The existing public SDK now has an optional sibling
`renderflow-api` workspace crate providing a transport application over captured
operator pipelines. This first checkpoint adds no listener, hosted service or
new registry dependency. A following transport checkpoint owns HTTP framing,
authentication, routes and OpenAPI. Neither checkpoint constitutes a verified
release or full #420 acceptance.

## Selected profile

The first profile is the existing native page-composition build. Operators name
a dedicated package root, flat v2 config filename and explicit relative file
allowlist. Startup captures bounded nonsymlink regular bytes and a stable package
digest. Original source is never rewritten. Each job receives a new private
copy; only its effective config's output root changes to `bundle`. The effective
config digest is recorded separately from original package identity.

Core spec loading rejects unsupported composition policies. The adapter adds
confined source paths and refuses an external/shared cache. It does not choose
providers, translate content, repair assets, replace fonts, approve publication
or implement another composition model. Captured packages never become consumer
canonical content. Client requests select a registered ID and an optional dry
run; they cannot submit host paths, a model endpoint, executable or config.

General graph profiles, arbitrary uploads/configs, external provider jobs and
durable cross-tool orchestration remain outside this first service profile.
The full SDK and CLI retain their existing capabilities. Later profile admission
needs explicit package/provider/resource semantics under the same #420 issue.

## Job and artifact contracts

All operations are queued: inspect, plan and execute. Fixed workers and a bounded
queue hold concurrency until the SDK actually returns. Each execution resolves
once and passes its private frozen build directly to the SDK executor. Native
planned/complete/partial/failed/cancelled run states survive in both service state
and unchanged structured native output. A cancellation request is recorded
separately; a later native success is not relabeled cancelled.

SDK `ProgressEvent` values enter a finite count/byte ring with monotonic IDs,
eviction counts and replay-gap indicators. Events retain the SDK's actual coarse
phase/step semantics. They do not invent recognition percentages or prove that
queued/planned work published. Result JSON is bounded and retained in memory;
oversized results fail explicitly instead of claiming a successful API response.

Artifacts come only from native `published_targets()` joins after terminal
execution. Listings preserve IDs/digests/validation/fidelity but identify bytes
as unobserved. Downloads select a job-local opaque artifact index and use the
shared `verification::read_publication` SDK function. It copies bytes during
the same confined size/hash observation, with a 32 MiB SDK buffer ceiling and a
smaller service cap. It does not follow receipt host-root declarations or grant
creative/release approval. Large streaming delivery remains future scope.

The shared JSON object decoder exposes the existing duplicate-key visitor to
adapters with an explicit byte cap. It refuses nested duplicates and root arrays
before decoding original bytes into the caller's type; it is not schema or
semantic qualification.

## Resource and lifecycle boundaries

Defaults: 2 workers, 4 queued and 32 retained jobs; 256 files/32 MiB per package,
64 MiB total captured packages, 2 MiB result JSON, 64 events/64 KiB progress,
8 MiB download, 256 MiB per-job and 512 MiB whole-workspace observations,
120-second job deadline and 15-minute terminal retention. Hard upper bounds are
declared in `Limits::validate`.

Admission, queue/result/event/download sizes are direct bounds. Deadline/storage
monitoring requests cooperative SDK cancellation between observations; it is
not a hard abort of in-flight shaping, rasterization or filesystem I/O, nor an
OS disk/memory quota or hostile concurrent-filesystem sandbox. Native per-profile
bounds still apply. Failed storage observations cancel conservatively; no new
work is admitted while the workspace cannot fit its configured budget.

Active jobs cannot be forgotten, expired or evicted to free a concurrency slot.
Terminal forget and cancellation are separate operations. Terminal retention
releases private data; an in-flight download keeps its workspace handle alive.
Shutdown refuses new jobs and requests cancellation. Process restart loses this
transport queue and job IDs; it does not infer SDK resume/recovery or replace
Flow's durable cross-tool state. Release builds use panic=abort, so native panics
remain process failures even though compatible unwind builds catch worker panics.

## Qualification

Nine service cases and three shared SDK cases are authored/unrun: same-workspace
SDK plan parity, immutable capture, actual publication/binary delivery,
dry-run/partial state, cancellation/forget, retained capacity/shutdown, package
refusal, tamper, event eviction/cursor and terminal expiry; shared buffered
delivery/limits and strict JSON decoding retain their own cases.

No tests, compiler/build/Cargo resolution, lint/format/schema/docs validator,
generator, listener, SDK/native/provider/font/media/model execution or CI ran.
Cargo membership/lock metadata and the comic pack's changed lock pin are authored
source changes, not a successful locked build. Original #420 acceptance remains
open for broader profiles, transport/runtime/security/platform/release proof.
