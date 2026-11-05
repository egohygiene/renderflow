# #420 — finite local service profiles

November 4, 2026 (America/New_York). The maintainer authorized continuing comic
publication gaps and API coverage. This source checkpoint is stacked after the
comic publication draft #483, which itself follows #481 → #482. Existing drafts
remain unmerged pending review. No release or issue closure is claimed.

## Admission and shared ownership

`PipelineConfig.profile` selects composition (default), manual_lyrics,
timed_text, fountain_body or editorial. The first four use existing built-in
SDK routes. Editorial requires explicit operator `allow_local_processes: true`
and uses the existing Pandoc/Tectonic strategy, including its bundled assets.
That permission is refused for built-in profiles. HTTP clients still supply
only a registered ID/dry-run choice, with no upload/path/config/provider/model
or network permission. Source root/config/files remain operator-owned captures.

Graph profiles require one explicit captured artifact, deny network/AI requests,
refuse custom registries/profiles/templates/variables/validators/publication/
overwrite policies and reject embedded-media extraction. Native policy must
match the selected profile. Finite allowed provider/transform sets are applied
to the effective spec before SDK resolution; operator denies and narrower
allows stay intact. Selected SDK plan edges are checked again before execution.
All-reachable planning operates inside those same native edges, with source
suppression, alias/cycle/partial/loss state still owned by core.

The existing bundled-profile loader is exposed as a read-only public core port
so the adapter compares definitions to exactly the loader's own data. It does
not parse a second domain profile grammar or reimplement planning/rendering.
No new dependency, gateway, CLI subprocess or orchestration framework is added.
Flow retains durable cross-tool orchestration.

Package identity now binds profile/effect declarations plus original file
identities. The effective private config has a separate digest; besides output
placement it records graph admission ceilings. Job and pipeline views add
profile/local_processes. Existing composition configs omit these fields and
retain default composition behavior. Downloads use the native composition
output extension or shared graph descriptor, without using arbitrary paths or
receipt names as attachment authority. Unknown MIME strings get a binary
fallback. Existing confined size/hash observation remains mandatory.

## Effects and limits

Capture/catalog/health/readiness do not launch provider discovery. Explicitly
submitted editorial inspect/plan/execute jobs may discover providers through
the public SDK. Provider versions/cache/preparation remain the operator's
separate responsibility. No install/download/fallback occurs automatically.
Only the core bundled editorial font/template configuration is admitted.

Existing strategy limitations remain: API time/storage bounds request
cooperative cancellation; they cannot forcibly interrupt every in-flight
Pandoc/filesystem call. Slots/workspaces remain held until native return, and
late completion/partial/failure stays separate from cancellation intent. No OS
sandbox, hard quota/kill, firewall, tenant isolation, durable recovery, hosted
service, full native schema, print/creative/provider/release approval or
qualification is inferred. Larger/multi-source/upload/provider graphs and
streaming remain existing #420 scope.

## Evidence and next sweep

Eight focused cases are authored/unrun: exact manual SRT words/millisecond
timing through a lyrics forest, timed-text native artifacts/downloads, original
Fountain body exports, effect refusal before planning, capture/profile/template
escape refusal, explicit editorial process permission with no submitted job,
profile defaults/package identity, and graph dry-run publication boundaries.
Existing HTTP contract cases now expect the added job fields.

Operator examples are `examples/http-api/local-profiles.json` (built-ins) and
`editorial-opt-in.json` (explicit local processes). New flat specs reference
existing synthetic creative-forest sources. No provider fixture or generated
output is copied. OpenAPI/operator schema and guides are source-maintained.

Manual source review only. No tests, compiler/build/Cargo resolution, lint/
format/schema/docs/OpenAPI validators, repository generators/harnesses, SDK/
native/provider/font/media/model/listener/container execution or CI ran.
No passing response/report, rendered output, model inference, installed provider,
image/release or downstream integration is claimed. All examples are unrun.

#420 remains open for original API/runtime/contract acceptance; #429/#458/#419
retain exact toolchain/repository/release qualification. #405/#409 gates remain
unchanged. #456's runtime-profile coordination remains later work; no AI model
or sibling dependency is needed to author these deterministic local profiles.
