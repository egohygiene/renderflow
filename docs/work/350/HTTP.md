# Captured presentation service profile

The #350/#420 source checkpoint adds operator-selected `profile: presentation`
to the existing local HTTP/SDK service. Register exactly the flat v2 config,
renderer-neutral presentation model and all referenced local PNG/JPEG images.
The service freezes that closure before SDK reads, refusing missing/extra assets,
traversal, client uploads and unadmitted execution overrides. Citations and
repository/revision/path metadata stay author declarations; no repository or
remote image is fetched to establish their truth.

The profile requires explicit `allow_local_processes: true` because native
Slidev targets can execute operator-selected tools. This permission is required
even for a source-only registration; it advertises possible effects, not a
claim that a process ran. Capture and plan do not launch Slidev. Native web/PDF
execution additionally requires the core's `allow_unqualified` opt-in, exact
prepared Node/Slidev installation pins and a browser pin for PDF. No install,
browser/model download, provider auto-selection or hosted service is added.

Exact primary targets are `presentation_source` (complete source ZIP),
`presentation_web` (complete web ZIP) and `pdf`. Existing core presentation
planning/build, note inclusion policy, native receipt and artifact-forest joins
remain authoritative. Normal job status/events/cancellation and verified buffered
or streamed downloads use the same service owners. Explicit completed-artifact
retention can recover verified downloads after restart; interrupted jobs are
not replayed. Source-only output remains usable without installed native tools.

Speaker notes remain omitted unless explicitly selected. Original captured
model bytes can still contain private content in process-local storage; primary
target selection is not a general privacy/redaction policy. Declared review,
rights and freshness are not publication approval. Native process/network/
resource behavior retains the existing standalone adapter's limits; the service
is not an OS quota/sandbox or a tenant boundary.

Four focused regressions are authored, uncompiled/unrun, alongside the earlier
briefing cases. No tests/builds, validators, dependency resolution, SDK/native/
Slidev/browser/listener/container execution or CI ran. Native/export/accessibility,
socket/resource and release qualification remain #350/#420/#458/#419 work.
