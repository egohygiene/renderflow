# Renderflow #428 — local speech checkpoint 2

## Authority and branch

- Issue: https://github.com/egohygiene/renderflow/issues/428 under #421.
- Draft PR: https://github.com/egohygiene/renderflow/pull/439, head branch
  `feat/renderflow-428-local-speech`, stacked on subtitle draft PR #438 / branch
  `feat/renderflow-427-timed-text`.
- The user explicitly deferred tests and authorized continued implementation.
  The latest instruction was to keep going after checkpoint 1. Tests, builds,
  schema validation, format/lint, native smoke/inference, and GitHub Actions
  remain deferred. No merge or release was requested.
- Read-only source/Git review, authored coverage, and draft publication are
  authorized. Keep `[skip ci]` commits and disabled hooks; do not dispatch or
  poll CI. Do not bootstrap a toolchain under this budget.
- Checkpoint 1 published head: `5f4cfccfe9f5e314542fdb5a71e578862d4eed63`, tree
  `d3d9c4d72fcd237477568e7197cfb360c26d89a7`, matching local commit
  `698511a69efc037cd5e849fa9a1ceddf1d0b8915`.
- Stack base: published `14ebc90f35c90c90942a3b9c086ed58b232b662b`, tree
  `53a9d02adc673058ce94c3c5bfa779c1f112eabd`, matching local
  `e72f26ecade13a1cd0b57dc5c0a9c41042f407a2`.
- Both PRs were still open drafts at checkpoint 2 start. If #438 is merged,
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
- `src/commands/speech.rs`, `src/cli.rs`, and `src/app.rs`: standalone inspect
  and transcription commands, explicit preparation flags, cancellation, and
  whole-bundle publication. Root-level public `speech` APIs are available to
  callers; planner/Engine SDK speech policy is still absent.
- `schemas/renderflow-audio-inventory-v1.schema.json`: inventory plus shared
  preparation definitions. The speech schema adds optional preparation evidence;
  the bundle schema accepts exactly six ordinary files or eight prepared-media
  files. Receipt itself is not listed or self-hashed.
- Tool/adapter metadata registers experimental `speech.audio.inspect` and
  `speech.audio.prepare` capabilities. These do not advertise executable graph
  edges or platform qualification.

The #427 `TimedTranscript` model/schema and closed Flow artifact envelope remain
unchanged. Its speech source is the recognized WAV, not the original video.
Keep the separate full speech envelope and raw files to preserve the whole chain.

## Profile and evidence boundaries

- Allowed forced demuxers: Matroska/WebM, MOV/MP4, AVI, WAV, FLAC, MP3, Ogg,
  and AAC, selected from
  allowlisted file extensions. No unrestricted format fallback or playlist route.
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

Checkpoint 1 authored fourteen tests. Checkpoint 2 adds eight tests for contract, metadata,
selection/argument, duration, identity, and cancellation coverage for later
execution. All #428 tests remain **unrun**. No compiler, schema validator,
formatter, linter, FFmpeg/FFprobe, whisper inference, model download, or CI job
was executed for either #428 checkpoint. No user media was processed. Source
review and matching Git publication establish no runtime qualification.
Earlier #427 partial smoke/schema evidence does not qualify this code.

## Next checkpoints

1. Add explicit planner/Engine SDK speech policy, artifact lineage, and structured
   phase/cancellation events. Preserve the full evidence and raw files without
   extending the closed Flow wire arbitrarily. Do not silently treat prepared
   sample times as original video times; define mapping/refusal before any mux
   route advertises alignment.
2. Decide actual cache/checkpoint persistence and reuse behavior. Keys exist,
   but an effective build/sidecar identity profile must precede enabling reuse.
3. When the user returns to testing, run focused #427/#428 compilation, Rust
   tests, schema/CLI agreement, and a licensed spoken-audio native fixture with
   pinned binaries/model. Cover multiple tracks, changed source, missing/wrong
   duration, nonzero starts/gaps/edit lists, cancellation, errors, limits,
   no-overwrite publication, and platform/build loading behavior.

#427/#428 acceptance and release remain open. Optiflow #111/PR #122 and ACL
qualification remain paused; Aniflow #36 remains parked.
