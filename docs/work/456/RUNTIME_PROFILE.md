# #456 checkpoint 1 — explicit runtime binding

The maintainer authorized three separate source checkpoints and a sync after
all three: runtime mapping, source-grounded candidates, then review/export.
This first checkpoint follows draft #484. No merge, install, model preparation,
model invocation or qualification is claimed.

## Reused contracts and selected seam

| Existing owner | Mapping for this consumer |
| --- | --- |
| `AiModelCatalog` and resolver | One named provider/model entry; absent/unverified stays unavailable. No ranking across alternative models or local-to-remote fallback. |
| Ollama/OpenAI-compatible adapters | Explicit base origin and optional credential environment handle. No plaintext key field or implicit global endpoint/key lookup in the new profile. |
| `AiSkillSpec`/registry/runtime | Existing schema validation, generation settings, hygiene, candidate state, artifact store and native execution evidence. |
| Existing runtime retry/deadline ports | Profile fixes one attempt, bounds input/output/tokens/HTTP response and transport time; cancellation checked before/after provider work and during bounded response reads. |
| Existing execution/cache identity | Profile/effective settings and endpoint hash bind new calls. Legacy calls without context retain their former cache identity shape. Timing/usage remain operational, not cache inputs. |
| Flow #80 | Renderflow-owned proposed mapping only. No Flow source import, shared crate, gateway, provider-library replacement or ratified interoperability claim. |

The existing adapters suffice for one bounded textual consumer. No new provider
library/gateway/dependency is selected. Broader Flow #80 library comparisons and
shared profile ratification remain Flow-owned work; this checkpoint does not
declare them completed. Existing generic summarization and metadata extraction
remain unchanged domain capabilities.

`renderflow.ai-runtime-profile/v1` separates processing policy from deployment
origin/credential handles. Disabled is default. Local mode requires a literal
loopback IP/port and an explicit assertion that the service's own cloud/egress
features are disabled. That assertion is not observed firewall or offline proof.
Remote mode requires explicit HTTPS selection and later source/privacy permission.
No constructor/inspection/probe installs, downloads, starts or invokes anything.

Local declared readiness requires catalog availability, recorded weight digest,
runtime revision and a preparation reference. These are operator declarations,
not attestations that the running endpoint serves those exact weights. Selected
model features and explicit byte/output-token limits must support requested
settings and structured JSON; unknown or unsupported limits/options are refused.
The existing catalog's stable model ID constraint remains; tag/alias dispatch
is not silently rewritten. Prepared catalog/model declarations are a separate
operator action and cannot be invented by the consumer.

## Identity, budgets and privacy

Optional allowed server-reported aliases are explicit. Requested catalog model,
server-reported model and unknown native weight/runtime evidence stay separate.
Unexpected reported names are refused before candidate admission. Missing server
identity stays unknown, never authenticated. New cache identity includes exact
profile/endpoint/settings; no durable cache is newly enabled (`not_requested`).

Profile-controlled transports refuse HTTP redirects and disable inherited
proxy discovery for literal loopback endpoints; remote transports preserve their
normal environment/proxy behavior and TLS verification. No executor proxy
configuration is changed. The existing ureq 2.12.1 agent API was inspected from
its tagged source; no dependency was installed or resolved.

HTTP envelopes are read incrementally to a finite byte cap, with generic errors
that do not echo source/model payloads. Legacy adapters now share an 8 MiB
default response ceiling; profile calls choose a smaller explicit ceiling.
Existing timeout/cancellation remains cooperative during blocking native/HTTP
work. There is no hard interrupt/OS sandbox guarantee.

Cost is unknown when not reported. Unsupported monetary caps are refused before
inference rather than treated as enforced. Remote operation additionally requires
explicit permission to accept unknown cost; the example refuses it by default.
No paid or local provider request occurred. Profile inspection never reads secret
values, and the selected existing adapter resolves its declared environment
handle only at explicit execution.

Legacy transform configuration (`AiTransformBuilder`), old CLI provider/doctor
commands and skill/catalog selection remain separate existing surfaces. The new
profile uses explicit JSON/CLI paths, not undeclared environment overrides. In
particular, `ai profile` is declaration inspection; `ai doctor` can probe a live
endpoint and is not run by this workflow.

## Evidence and next checkpoint

Nine cases are authored/unrun: disabled/unprepared; endpoint/egress/credential
boundaries; no fallback; capability/cost/budget refusal; profile identity changes;
pre-cancel/unexpected reported model; runtime cache identity; bounded response
reading; cancelled reader with no input read. Fake providers are in-process test
data only. Shared model evidence schema and existing struct-literal fixtures
are updated for the optional reported identity.

No compiler/build/Cargo resolution, tests, lint/format, schema/docs/OpenAPI
validators, generators/harnesses, SDK/native/provider/model/listener/container
execution or CI ran. No output, passing report, observed model readiness or
runtime quality result exists. Original #456 acceptance and #429/#458/#419
qualification remain open. #405/#409 gates are unchanged.

Next: a source-grounded publication skill over approved immutable text slices,
task-specific evidence-reference validation and an explicit review-required
candidate. Full shared-profile, hardware/model-quality and release-consumer
proof remain outstanding.
