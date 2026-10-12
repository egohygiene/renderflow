# Renderflow #428 / #440 — speech and explicit alignment checkpoint 6

## Authority and branch

- Issue: https://github.com/egohygiene/renderflow/issues/428 under #421.
- Separately scoped original-media time maps:
  https://github.com/egohygiene/renderflow/issues/440. This preserves #428's
  non-goal boundary for advanced temporal editing unless separately scoped.
- Draft PR: https://github.com/egohygiene/renderflow/pull/439, head branch
  `feat/renderflow-428-local-speech`, stacked on subtitle draft PR #438 / branch
  `feat/renderflow-427-timed-text`.
- The user explicitly deferred tests and authorized continued implementation.
  The latest instruction approved the next piece after checkpoint 5. Tests, builds,
  schema validation, format/lint, native smoke/inference, and GitHub Actions
  remain deferred. No merge or release was requested.
- Read-only source/Git review, authored coverage, and draft publication are
  authorized. Keep `[skip ci]` commits and disabled hooks; do not dispatch or
  poll CI. Do not bootstrap a toolchain under this budget.
- Checkpoint 1 published head: `5f4cfccfe9f5e314542fdb5a71e578862d4eed63`, tree
  `d3d9c4d72fcd237477568e7197cfb360c26d89a7`, matching local commit
  `698511a69efc037cd5e849fa9a1ceddf1d0b8915`.
- Checkpoint 2 published head: `0c52d18ed2b5b861a7b85f874e85b628f2d65192`, tree
  `a964a21f915eae54d2f0a07c31a207454e80202b`, matching local commit
  `e179356d56bc4ae64a45c603a0c85bacb7c2f61d`.
- Checkpoint 3 published head: `471b7ea10e95d26a16b7f357e0d5d2224883bbf2`, tree
  `50f08430a6d409dc31280d38d43894055269e83a`, matching local commit
  `a2a14774bd00d61f17186c20b1fbc1787589a5f6`.
- Checkpoint 4 published head: `6c80bd84765b74a41dee9e3286a41eb6969f2a24`, tree
  `c1b5e2a90a7cc195d1808d9a6383043710921b5f`, matching local commit
  `2be49123e056e634c0795ee8c030fdb12c7e88f0`.
- Checkpoint 5 published head: `5229f134ac8a89b7b132e695101cce8d7d8d39a2`, tree
  `7d8fbb186e7cd7117d869726aa573927b62c0d0a`, matching local commit
  `49e6bd0c4be1c79ea962d2f0a8d9179c3a75d14d`.
- Stack base: published `14ebc90f35c90c90942a3b9c086ed58b232b662b`, tree
  `53a9d02adc673058ce94c3c5bfa779c1f112eabd`, matching local
  `e72f26ecade13a1cd0b57dc5c0a9c41042f407a2`.
- Both PRs remain stacked drafts. If #438 is merged,
  reconcile and retarget the stack before later publication. This handoff
  cannot contain the hash of its own commit; use the PR head and body.

## Authored path

1. Direct WAV input continues to use `speech transcribe` with explicitly pinned
   local whisper.cpp executable/model, selected language, and bounded options.
   The model is never downloaded or selected implicitly.
2. `speech inspect` accepts local media and a pinned FFprobe executable. Its
   JSON inventory identifies original media bytes, tool/version, forced demuxer,
   raw probe digest, and global audio indexes with available metadata. No track
   is selected automatically, even when only one exists.
3. `speech transcribe --audio-stream N` additionally requires the original
   digest from inspection and pinned FFprobe/FFmpeg paths. The source pin binds
   the index choice to those exact bytes. Preparation re-probes, maps `0:N`,
   downmixes/resamples to mono 16 kHz PCM16, and inspects the completed RIFF/WAV.
4. The existing recognition adapter consumes that exact WAV once. The speech
   envelope attaches original media, selected stream, probe/extraction tools,
   conversion policy, and prepared WAV evidence. Configuration/reuse identities
   include preparation; generic DAG result caching remains disabled.
5. The bundle retains provider JSON, full speech envelope, canonical timed
   transcript, SRT, WebVTT, and text. Prepared-media runs also retain exact
   `audio-probe.json` and `prepared.wav`. A hash/size/loss receipt is published
   last. Existing paths are refused; interrupted publication can leave an
   incomplete new directory, which is not a successful bundle.
6. `execution.speech` now opts the canonical build/planner/Engine SDK into one
   original-source recognition edge followed by canonical JSON/SRT/WebVTT
   fan-out. The explicit policy requires the original source digest, absolute
   provider/model paths and hashes, and local AI permission. Native tools are
   not launched during planning, and provider availability remains unobserved
   until execution. Speech and subtitle policies cannot be combined.
7. Graph execution consumes the artifact-store source directly, retains raw
   recognition output/full envelope and optional audio probe/WAV as typed
   artifact records, and commits those records through canonical lineage.
   Derivatives retain the speech metadata. The native manifest includes the
   retained records; Flow's closed artifact projection remains unchanged.
   Supporting files and canonical JSON publish under `speech/<envelope-sha256>/`,
   including canonical JSON for SRT-only requests. Publication verifies existing
   identical files and refuses conflicting bytes/symlinks. Evidence failure
   withholds selected targets; budget accounting includes retained records and
   published copies. The original source remains in the artifact store.
8. SDK progress reporters receive live coarse speech phases, and typed speech
   cancellation/failures become step diagnostics. Recognition and subtitle
   descendants bypass cache/checkpoint reuse without opening shared state files.
   Late evidence-publication cancellation also produces a cancelled run and
   diagnostic. Without an explicit saved result or opt-in index, SDK assessment
   returns `speech.checkpoint_reuse_disabled` and recomputation.
9. Optional `execution.speech.recovery` selects an absolute receipt path and
   expected SHA-256. The receipt binds the original source, original execution
   configuration, canonical record, and retained evidence. Replay validates
   every retained payload and reconstructs normalization from raw provider JSON
   before restoring the result. It never opens provider/model paths, invokes
   native tools, or silently falls back to new inference. Configuration must
   match with replay-selection controls excluded; execution checks the current
   source again. This is replay of selected historical output, not a claim of
   native determinism or complete effective-environment attestation.
10. Graph publication writes `saved-result.json` after canonical/supporting
    files, with a separate native manifest artifact and budget accounting. Its
    parent is the canonical artifact; canonical lineage never points back at
    the receipt. It records a completed speech result, not successful publication
    of every requested target. Keep the entire supporting-file directory.
    Existing standalone and checkpoint-3 bundles have no such receipt.
11. `speech timeline --receipt PATH --receipt-sha256 SHA` verifies a saved bundle
    and reports its clock. Strict WAV uses the recorded input WAV sample clock;
    prepared audio always requires original-media mapping. Stream start time is
    informational even when zero. `--require-original-media` emits the report
    then refuses missing mapping. No cues are shifted and no mux route is added.
12. `execution.speech.saved_results: reuse_if_available` opts into a dedicated
    historical-result index in the existing artifact store. Default `disabled`
    skips index reads/writes, and combining this mode with explicit `recovery`
    is refused. Keys bind source digest/format, recognition configuration, and
    replay profile while excluding output targets and replay selectors. Default
    omission preserves checkpoint-4 receipt v1 identities.
13. Index lookup verifies the receipt and every referenced CAS payload with the
    same semantic verifier as portable replay. Missing entries permit native
    execution; existing invalid entries or broken evidence fail without native
    fallback. SDK assessment performs the same lookup read-only, with no store
    creation. Planning/dry-run do not access the index. Execution records actual
    hit/miss selection and receipt evidence without changing canonical metadata.
14. Index publication follows successful selected-output materialization,
    result verification, and persistence of the completed run manifest. The first
    valid entry for a key is retained; concurrent
    writers validate the existing winner instead of replacing it. Index entries
    point to the receipt already held in CAS, adding no permanent media copy.
    Entries at `speech-results/v1/<lookup-sha256>.json` under the artifact root
    are limited to 4 KiB, reserved in the execution storage budget. Interruption
    after atomic publication can leave a verified entry reusable even when the
    final run reports cancellation or its updated manifest cannot be persisted.
    This selects historical output and does not attest current native libraries,
    accelerators, model sidecars, or reproducibility of another inference.
15. Separately scoped #440 adds offline `speech align` and the public
    `speech::alignment::align_saved_result` API. The input is an opaque fully
    verified saved result plus a digest-pinned operator-supplied time map.
    The map binds receipt, original source, recognized WAV, and canonical
    transcript digests. Prepared-media results only; strict original WAV input
    retains its own sample-clock identity and is not shifted by this operation.
16. The first mapping profile applies ordered one-to-one millisecond translations
    with no overlapping source or destination intervals. Every complete cue must
    fit within one interval. Missing coverage, boundary-crossing cues, stale
    identities, unknown/duplicate fields, invalid intervals, and overflow fail.
    Cue text, IDs, speakers, language, overlap policy, and saved artifacts are
    preserved. No times are inferred from stream start metadata.
17. Alignment publishes a separate new-directory bundle containing exact mapping
    bytes, a derived alignment envelope, mapped JSON/SRT/WebVTT/text, and a
    completion receipt last. The envelope states operator-supplied timing,
    original media not reobserved, and recognition not human-reviewed. All
    derivatives require the mapping evidence, including bare canonical JSON.
    The original saved speech bundle remains external supporting evidence;
    aligned bundles cannot masquerade as native saved speech receipts. No
    graph/Engine policy or recognition cache integration is added for alignment.

## Source locations and interfaces

- `crates/renderflow-core/src/speech/mod.rs`: provider-neutral speech envelope,
  strict whisper JSON normalization, optional audio preparation evidence,
  inventory and tool/stream contracts, preparation policy hashing, and
  `attach_audio_preparation`. Attachment verifies the WAV identity, cue bounds,
  policy and prior recognition identities; identical reattachment is idempotent
  and replacing evidence is refused.
- `src/speech/provider.rs`: original bounded WAV recognition adapter, now sharing
  local file snapshots, bounded reads, process setup, and WAV inspection with
  its sibling audio adapter. `SpeechRequest` and `SpeechRun` remain unchanged.
- `src/speech/audio.rs`: `AudioTool`, `AudioInspectRequest`,
  `AudioPrepareRequest`, `AudioInspection`, and `PreparedAudio`. Preparation
  keeps its private temporary directory alive and provides bounded WAV reads
  plus source/tool stability rechecks after recognition.
- `src/speech/policy.rs`: object-only opt-in policy and structural validation.
  `src/speech/graph.rs`: artifact-native transform, typed retained evidence,
  phase observer, record validation, and fixed role-to-filename mapping.
  Stored-source provider entry points verify expected bytes before launching
  any native process; media preparation takes an explicit graph format instead
  of staging a file with a suffix.
- `src/speech/recovery.rs`: bounded portable receipt, complete retained-byte
  and semantic checks, opaque verified saved result, and artifact restoration.
  `src/speech/timeline.rs`: assessment and explicit original-clock refusal for
  verified saved results. Offline inspection checks retained files and recorded
  source identity, not the absent original media. Strict-WAV duration is recorded
  in the pinned receipt; graph execution verifies the current source separately.
- `src/speech/cache.rs`: bounded index entries, deterministic recognition key,
  read-only lookup, full CAS receipt verification, and no-clobber registration.
  Recovery inspection supports either a portable bundle or CAS origin and
  rechecks that same receipt origin before restoration.
- `src/speech/alignment.rs`: strict bounded time-map parsing, receipt/digest
  binding, whole-cue interval translation, derived transcript validation, and
  a separate operator-assertion envelope. Public entry points require the
  recovery verifier's opaque result. Existing timeline readiness stays unchanged.
- `src/planning.rs`, `src/spec.rs`, and `schemas/renderflow-v2.schema.json`:
  policy agreement, source pinning, restricted speech graph, deferred provider
  observation, retained evidence publication/budget accounting, and manifest
  records. `src/sdk.rs` adapts phase events and assesses explicit saved results
  without creating shared state; generic speech checkpoint reuse stays disabled.
  `src/graph/dag_executor.rs` retains typed speech cancellation and error codes.
- `src/commands/speech.rs`, `src/cli.rs`, and `src/app.rs`: standalone inspect
  and transcription commands, offline timeline assessment, preparation flags,
  cancellation, and whole-bundle publication. Public `speech` APIs and the existing Engine SDK
  request methods expose the implementation; no separate Flow wire is invented.
  The new standalone `speech align` command consumes a graph-produced saved
  receipt and writes a distinct derived bundle without invoking native tools.
- `schemas/renderflow-audio-inventory-v1.schema.json`: inventory plus shared
  preparation definitions. The speech schema adds optional preparation evidence;
  the bundle schema accepts exactly six ordinary files or eight prepared-media
  files. Receipt itself is not listed or self-hashed. The new
  `renderflow-speech-artifact-v1.schema.json` describes namespaced graph evidence.
  Saved-result and timeline-assessment schemas describe the new portable
  recovery and read-only clock reports; schema agreement remains unexecuted.
- `renderflow-speech-time-map-v1`, `renderflow-speech-alignment-v1`, and
  `renderflow-speech-aligned-bundle-v1` schemas describe #440's operator map,
  derived envelope, and receipt-last output bundle. They do not change existing
  speech or timed-transcript schema identities.
- Tool/adapter metadata registers experimental `speech.audio.inspect` and
  `speech.audio.prepare` capabilities. Their bounded preparation is used inside
  the explicit speech edge; this does not establish platform qualification.

The #427 `TimedTranscript` model/schema and closed Flow artifact envelope remain
unchanged. Its speech source is the recognized WAV, not the original video.
Keep the separate full speech envelope and raw files to preserve the whole chain.

## Profile and evidence boundaries

- Allowed forced demuxers: Matroska/WebM, MOV/MP4, AVI, WAV, FLAC, MP3, Ogg,
  and AAC, selected from allowlisted file extensions in the standalone path or
  the declared graph source format for artifact-store payloads. No unrestricted
  format fallback or playlist route.
- Local original media <=4 GiB; prepared WAV <=32 MiB and <=600 seconds.
  Provider executables <=256 MiB; model <=4 GiB. Expected SHA-256 values are
  mandatory for tools/model, and original media in preparation requests.
- Probe capture <=1 MiB, other native captures <=64 KiB. Probe timeout 30s;
  preparation timeout 1–3600s (default 300); recognition timeout 1–3600s
  (default 900). Hash/read budgets are checked between reads for up to 120s.
- Missing stream duration stays unknown; container duration is not substituted.
  Known overlong audio is refused. Output `-t 601` is a refusal sentinel:
  the strict WAV parser refuses >600s, so it never represents a successful
  intentionally clipped transcript. No trimming or segmentation is offered.
- Prepared timing is **decoded sample order from zero**. Resampling/downmixing
  loses audio detail; rebasing collapses original timestamp offsets and gaps.
  Recorded start time does not establish a general mapping back to the video.
  Original-video caption synchronization/mux readiness is not claimed.
- Explicit alignment accepts at most 256 KiB and 1024 mapping segments. Source
  intervals stay within verified prepared-WAV duration; destination intervals
  stay within the timed-text 99:59:59.999 limit. Map identity and arithmetic are
  checked, but the operator's timing assertions are not measured against media.
  No speed changes, cue splitting/clipping/dropping, muxing, burn-in, word/lyric
  alignment, or human approval of recognition is introduced.
- Recognition is unreviewed, with provider segment milliseconds and disclosed
  segment-edge whitespace trimming. No confidence calibration, translation,
  diarization, word alignment, or singing accuracy guarantee is invented.
- Process execution uses direct argv, cleared environment, cancellation,
  timeouts, input protocol restrictions, and disabled MOV external references.
  These are not OS disk/RAM/CPU/network/filesystem confinement. Native output
  files are inspected after exit. Blocked filesystem reads and hostile temporary
  file replacement are not defeated by digest rechecks.
- Source/model/tool final path components must not be symlinks; ancestor links
  are canonicalized. Tool hashes do not attest linked libraries, model sidecars,
  optional accelerators, build-dependent codecs, or all effective configuration.
  Model/build licensing and actual platform support remain unqualified.

## Source review; all execution deferred

[Whisper evaluation](PROVIDER_EVALUATION.md) pins upstream source
`7e26d3b0d5bced6c8217ef09710ec2f969001241`.
[Audio preparation evaluation](AUDIO_PREPARATION.md) pins FFmpeg source
`615b07170e30d79ad5906418ea49f81c3cbd8457` and explains global indexing, demuxer
aliases, metadata copying, duration sentinel, and sample timeline semantics.
Source snapshots describe assumptions, not the origin of a supplied binary.

Checkpoint 1 authored fourteen tests. Checkpoint 2 added eight tests for contract, metadata,
selection/argument, duration, identity, and cancellation coverage for later
execution. Checkpoint 3 adds thirty tests for graph/SDK/policy, evidence
propagation, stored-source demuxer selection, cancellation, and phase events.
Checkpoint 4 adds seventeen tests: six portable recovery tests, seven
policy/planner/SDK cases, and four clock-assessment cases. Source review caught
and corrected graph JSON newline handling and descriptive source metadata
normalization in receipt verification. Checkpoint 5 adds fourteen tests: nine
index/recovery cases and five policy/SDK/graph integration cases. They cover
read-only lookup, missing versus invalid evidence, first-writer preservation,
policy identity, and historical replay without native files. Checkpoint 6 adds
fifteen authored #440 cases: eleven core mapping/reader cases (one Unix-only)
and four command/publication/manifest cases. Source review covers explicit
assertion boundaries, full receipt identity, interval containment, output digest
bindings, export evidence requirements, and receipt-last publication. All
eighty-three #428 and fifteen #440 tests remain **unrun**. No compiler, schema validator,
formatter, linter, FFmpeg/FFprobe, whisper inference, model download, or CI job
was executed for any #428/#440 checkpoint. No user media was processed. Source
review and matching Git publication establish no runtime qualification.
Earlier #427 partial smoke/schema evidence does not qualify this code.

## Next checkpoints

1. When the user returns to testing, run focused #427/#428 compilation, Rust
   tests, schema/CLI agreement, and a licensed spoken-audio native fixture with
   pinned binaries/model. Cover multiple tracks, changed source, missing/wrong
   duration, nonzero starts/gaps/edit lists, cancellation, errors, limits,
   no-overwrite publication, portable saved-result restore into a new store,
   corrupt/missing/mismatched recovery evidence, and platform/build behavior.
2. Execute #440 interval, identity, strict parsing, cancellation, CLI/schema,
   exact-export digest, and no-overwrite/receipt-last coverage when checks resume.
   A supplied map's synchronization truth remains an operator responsibility;
   measured sample-to-media evidence is a separate capability.
3. Reproducible fresh native inference remains a separate qualification decision:
   selecting saved historical results does not attest linked libraries or model
   sidecars. The original timeline API still refuses inferred offsets; #440
   produces distinct candidates from explicit operator assertions and does not
   upgrade the historical speech evidence or timeline readiness.

#427/#428/#440 acceptance and release remain open. Optiflow #111/PR #122 and ACL
qualification remain paused; Aniflow #36 remains parked.
