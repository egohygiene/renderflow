# Renderflow #428 — local speech checkpoint 3

## Authority and branch

- Issue: https://github.com/egohygiene/renderflow/issues/428 under #421.
- Draft PR: https://github.com/egohygiene/renderflow/pull/439, head branch
  `feat/renderflow-428-local-speech`, stacked on subtitle draft PR #438 / branch
  `feat/renderflow-427-timed-text`.
- The user explicitly deferred tests and authorized continued implementation.
  The latest instruction was to keep going after checkpoint 2. Tests, builds,
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
- Stack base: published `14ebc90f35c90c90942a3b9c086ed58b232b662b`, tree
  `53a9d02adc673058ce94c3c5bfa779c1f112eabd`, matching local
  `e72f26ecade13a1cd0b57dc5c0a9c41042f407a2`.
- Both PRs were still open drafts at checkpoint 3 start. If #438 is merged,
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
   include preparation; actual cache lookup/reuse remains unimplemented.
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
   diagnostic. SDK assessment explicitly returns
   `speech.checkpoint_reuse_disabled` and recomputation.

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
- `src/planning.rs`, `src/spec.rs`, and `schemas/renderflow-v2.schema.json`:
  policy agreement, source pinning, restricted speech graph, deferred provider
  observation, retained evidence publication/budget accounting, and manifest
  records. `src/sdk.rs` adapts phase events and refuses checkpoint reuse;
  `src/graph/dag_executor.rs` retains typed speech cancellation and error codes.
- `src/commands/speech.rs`, `src/cli.rs`, and `src/app.rs`: standalone inspect
  and transcription commands, explicit preparation flags, cancellation, and
  whole-bundle publication. Public `speech` APIs and the existing Engine SDK
  request methods expose the implementation; no separate Flow wire is invented.
- `schemas/renderflow-audio-inventory-v1.schema.json`: inventory plus shared
  preparation definitions. The speech schema adds optional preparation evidence;
  the bundle schema accepts exactly six ordinary files or eight prepared-media
  files. Receipt itself is not listed or self-hashed. The new
  `renderflow-speech-artifact-v1.schema.json` describes namespaced graph evidence.
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
All fifty-two #428 tests remain **unrun**. No compiler, schema validator,
formatter, linter, FFmpeg/FFprobe, whisper inference, model download, or CI job
was executed for any #428 checkpoint. No user media was processed. Source
review and matching Git publication establish no runtime qualification.
Earlier #427 partial smoke/schema evidence does not qualify this code.

## Next checkpoints

1. Decide actual cache/checkpoint persistence and reuse behavior. Keys exist,
   but an effective build/sidecar identity profile must precede enabling reuse.
   The current graph deliberately recomputes speech and its exports. Define
   original-media timeline mapping/refusal before adding any mux/alignment
   route; prepared sample times cannot silently become video timestamps.
2. When the user returns to testing, run focused #427/#428 compilation, Rust
   tests, schema/CLI agreement, and a licensed spoken-audio native fixture with
   pinned binaries/model. Cover multiple tracks, changed source, missing/wrong
   duration, nonzero starts/gaps/edit lists, cancellation, errors, limits,
   no-overwrite publication, and platform/build loading behavior.

#427/#428 acceptance and release remain open. Optiflow #111/PR #122 and ACL
qualification remain paused; Aniflow #36 remains parked.
