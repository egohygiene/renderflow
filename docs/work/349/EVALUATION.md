# Briefing formats: bounded evaluation

Authored November 3, 2026 (America/New_York), under existing #349.
These are implementation choices for an unqualified prototype, not findings
from a listening study, provider evaluation or published consumer feed.

| Format | Disposition | Reason and first boundary |
| --- | --- | --- |
| Short repository briefing | Adopt | One authored script over selected immutable text gives a small useful transcript/HTML package with explicit citations, revision and freshness. It does not require a voice or model provider. |
| Release-update feed | Adapt | The same episode can project RSS 2.0 only when its owner supplies explicit channel/item URLs and metadata. No automatic schedule, fabricated release date or feed publication occurs. |
| Narrated documentation | Defer automatic narration | Supplied, consented recordings can accompany a transcript now. Long-form TTS, pronunciation, audio fidelity, chapter synchronization and player support need a separately selected, prepared and qualified voice route. |
| True podcast / conversational show | Reject as a default repository template | Multiple hosts, invented dialogue and mandatory feeds are unnecessary for conveying repository facts. A future editorial product would need real creative intent, content/voice consent and a separately bounded implementation. |

Reading List remains an experience reference from the issue, not a dependency,
integration, copied implementation, endorsed provider or measured comparison.
No service/library/vendor was selected or contacted for this checkpoint.

## Prototype and reuse

The selected `briefing-local-v1` profile owns bounded source capture, an authored
script, transcript/HTML, source-linked chapters, optional supplied PCM narration
and optional RSS candidate bytes. CLI and library use the same implementation.
The existing native PCM inspector validates the admitted ten-minute mono/16 kHz/
PCM16 subset; it does not recognize speech or prove script/audio alignment.
Local capture and receipt-last output reuse the shared observed-file service.
No replacement planner, job engine, AI runtime or voice framework is introduced.

This is a standalone publication SDK/CLI surface. It does not advertise an
ordinary v2 graph selector, a registered TTS transform, a general podcast player,
released Flow support, an RSS hosting service or a remotely current repository.
Optional cross-repo guidance can be consumed later without blocking the local
prototype or moving canonical source authority into Renderflow.

## Approval and freshness

Review references are author declarations bound to selected source/script/audio
digests. Their authenticity remains external. Missing declarations require an
explicit draft build; even declared review never grants publication approval.
Changed source bytes yield a stale plan and refuse a build. Changed chapter or
source/revision metadata invalidates the script-review binding. Supplied audio
has a separate script digest; the plan exposes mismatch and a build refuses it.

The source path is an explicit relative local snapshot. Revision labels and
`as_of` remain owner assertions; no remote repository, clock or model is queried.
Outputs retain that boundary visibly, so an old snapshot is not labeled live.
The episode GUID includes the exact input digest. Canonical documents remain
the authority; narration and feed bytes are candidate derivatives.

## Later evidence

Nine cases are authored/unrun, including a synthetic PCM envelope that is clearly
not recorded speech. Runtime, schema, accessibility, listening, timing, feed/
player, platform, source-preservation and native acceptance remain unobserved.
No tests/build/Cargo/lint/format/schema/docs validators, product/voice/model
execution or CI ran. #349 retains its acceptance; #458/#419 retain qualification
and release ownership. #405's ADR backfill scheduling gate is unchanged.
