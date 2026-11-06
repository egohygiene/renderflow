# #456 checkpoint 2 — source-grounded candidate

This source checkpoint follows runtime-profile draft #485. The maintainer
authorized all three checkpoints in separate drafts, then a sync. Nothing here
starts an inference server, invokes a model, publishes metadata or accepts a
candidate. Flow #80's shared-profile/native proof remains separate.

## Public standalone boundary

`publication::summary::capture` freezes a bounded JSON recipe and exact UTF-8
source bytes through existing confined publication I/O and the artifact store.
Each declared source has its expected digest, role, explicit AI/remote privacy
permission and approval reference. Approval is a declaration, not authenticated
legal or human authority. One source is at most 1 MiB, total 2 MiB, eight sources,
64 selected nonoverlapping ranges and 32 KiB selected text. Ranges use byte
offsets and must lie on UTF-8 boundaries. Unselected bytes are recorded; there
is no silent truncation, OCR, audio, image understanding or automatic timing.

Each evidence ID binds source ID/digest and exact range; captured slice text,
hashes and native source artifact IDs remain in the review candidate. Original
paths are not portable candidate/telemetry data. Plan reports omit slice text.
Changes to originals after capture do not replace the immutable approved
snapshot; fresh capture rejects a stale declared digest.

`plan` reuses the explicit runtime profile. `generate` and the existing-provider
embedding port use the common skill runtime, schema validator, hygiene, store
and native identity evidence. Disabled/unavailable/refused/cancelled paths call
no model. Source/remote permission is checked separately from runtime readiness.
There is one selected inference attempt; errors never reroute or become a paid
fallback. Unknown cost remains unknown. Unsupported finite monetary caps remain
prelaunch refusals under the profile contract.

The new `skill.publication.summary@1.0.0` produces a short list of cited synopsis
statements plus uncertainty/omissions and review_required=true. The runtime
validates structure/hygiene only. The consumer then checks evidence membership,
unique citation lists, slice/hash/source joins, native input/output/skill
identity and bounds. Its separate reference validator and explicit
`references_checked_semantics_unverified` state do not claim factual entailment,
paraphrase accuracy, author approval or publication permission. Human review is
mandatory. The shared runtime's validator list does not falsely claim it ran
the domain reference validator.

Candidate identity binds recipe, sources/slices, body, model/profile/skill/
settings/hygiene/native cache identity and reference/coverage policy. Invocation
IDs, timestamps, usage, duration and optional observer delivery are outside that
identity. Outputs use existing candidate state and intermediate artifact class;
no canonical publication metadata is replaced.

## Prompt/data and operational boundaries

Shared prompt rendering now substitutes reviewed template tokens in one pass.
Inserted source values are opaque and cannot become a second variable. Template-
like source text remains literal data; malformed or undeclared tokens in the
reviewed template are still refused. This is lexical separation, not a guarantee
against model prompt injection. There are no model-controlled tools, approval
actions or file writes.

The optional bounded observer receives invocation/profile hashes, status,
duration, observed/unknown attempt counts and reported/unknown token usage.
It receives no prompt, source text, source paths, endpoints or credentials.
An observer error is recorded separately and does not alter the candidate or
its identity; collector delivery is not inferred from callback success. This
consumes existing observability semantics without a telemetry backend/OTLP
dependency. Actual model weight/runtime identity and output quality remain
unattested. Failed runtime calls can have unknown attempt observations; a
configured one-attempt ceiling is not invented observed success.

## Source evidence

Ten new cases are authored/unrun: frozen candidate/native joins; unknown or
repeated citations; malformed/self-approving output; disabled/unverified model;
source/remote privacy gates; original drift after capture; UTF-8/overlap/omission
coverage; timeout/no retry and late cancellation; observer failure/redaction;
template-like source and candidate tamper refusal. Fake providers are in-process
fixtures, not model quality or real-provider evidence.

No compiler/build/Cargo resolution, tests, lint/format/schema/docs validators,
generators/harnesses, SDK/native/provider/font/media/model/listener/container
execution or CI ran. No synopsis was inferred or validated by a live model; no
passing report, source approval, release or integration is claimed. New schemas,
skill and API code are authored and uncompiled. No new dependency or lock change.

Next checkpoint adds explicit accept/revise/reject records, exact-candidate
review/export, CLI and a small original text example. #456 remains open for
all original full acceptance, Flow #80 compatibility and measured model/hardware
results. #429/#458/#419 and #405/#409 keep their existing scope/gates.
