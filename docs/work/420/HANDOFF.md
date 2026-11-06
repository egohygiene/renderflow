# #420 — local HTTP/job source checkpoint

The maintainer authorized merging #479/#480 and continuing the oldest local
lane. They are merged at `f8d104b0b3a8f33835bc4b3bd0764e00fcfcae61` and
`750054b950ae342f052153d4af4cf5173e74348a`, with exact reviewed trees preserved.
The next feature is the optional HTTP/job adapter. #419's release and #458's
locked baseline still need the deferred qualification sweep.

## Two source checkpoints

1. [Draft #481](https://github.com/egohygiene/renderflow/pull/481) adds an optional
   `renderflow-api` workspace library, captured native-composition packages,
   bounded process-local jobs, shared SDK progress/cancellation/results and
   verified binary reads. [SERVICE.md](SERVICE.md) records its detailed contract.
2. This stacked transport checkpoint adds loopback-only sockets, bearer auth
   port/default, fixed connection workers, exact request/body/framing/response
   bounds, read/write budgets, versioned routes, finite SSE replay and safe
   attachments. A thin executable, maintained OpenAPI/operator schema, sample
   source configuration and unrun local/container recipe complete the surface.

Core remains the owner of spec validation, planning/frozen execution, locale/
geometry/typography/rendering, caches, native evidence, partial outcomes and
review requirements. The adapter admits native composition only in this first
profile. No client upload/config/provider executable/model endpoint, generic
graph admission, public bind, browser session, hosted service or cross-tool job
framework is added. The broader SDK inventory is labelled separately.

## Evidence and tests

Source review compared public SDK types/methods, native profile admission,
package paths, cancellation/expiry/queue lifetime, shared JSON/observed-file
ports, HTTP framing/auth ordering, request/response/SSE schema fields, socket
admission and signal/container assumptions. A replay fix now reports dropped
tail events as gaps. No caller-controlled native receipt/path is trusted.

Draft #481 authors nine service and three SDK cases. This checkpoint adds seven
transport/contract cases and one dropped-tail replay case: **20 authored cases
total, all uncompiled/unrun**. Cursor-based transport cases do not themselves
start sockets. Existing real socket, overload, shutdown, process-fault, native,
OS/memory/disk, platform and model/provider qualification remains unobserved.

No tests, compiler/build/Cargo resolution, lint/format, schema/docs/OpenAPI
validator, generator, SDK/native/provider/font/media/model/listener/container
execution or CI ran. No passing API response/report, image, release, deployment
or consumer integration is claimed. New registry dependency httparse 1.10.1 is
source-pinned from published metadata; existing packages are reused. Lock data
is authored, not a successful locked build. Corpus lock pins are maintained
explicitly, not silently changed golden outputs.

## Remaining #420 acceptance

- Broader SDK profiles, explicit safe uploads, provider/process policies and
  large streaming retrieval need finite admission contracts.
- The OpenAPI document covers API envelopes/requests. Nested native plan/run/
  artifact models remain SDK-owned opaque objects here; full nested schema and
  generated/validated documentation evidence remains outstanding.
- Service jobs/IDs are process-local. Cancellation is a request until the SDK
  returns. Active state/workspace slots cannot be deleted to pretend completion;
  native partial/success/failure and cancellation intent stay separate.
- Deadlines/storage monitoring are cooperative observations/cancellation, not
  an OS sandbox or hard interrupt of in-flight native CPU/I/O. Native panic=
  abort and forced shutdown remain process/interruption failures, not recovery.
- A bearer grants one trusted local operator scope, with no tenant isolation,
  TLS, CORS or cloud identity guarantee. Container use is a Linux local example
  and must receive clean build/license/platform verification before release.
- Provider/creative/print/publisher approval is never inferred. #420 stays open;
  #429/#458/#419 retain toolchain/repository/release evidence. No new issue is
  needed for these existing requirements.

See [API guide](../../http-api.md), [OpenAPI](../../http-api.openapi.json),
[operator schema](../../../schemas/renderflow-api-service-v1.schema.json),
[sample](../../../examples/http-api/README.md) and
[dependency decision](HTTP_DEPENDENCY.md).
