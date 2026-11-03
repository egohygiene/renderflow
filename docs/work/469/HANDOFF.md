# #469 — manual timed-lyrics derivatives

Date: November 3, 2026 (UTC). Branch: `codex/469-manual-timed-lyrics`.
Base: #379's editorial branch at `c5ee9b0e2bed8f6ec00bbf0b88a6b6ab3f0a98cb`;
its bundled-profile loader and reviewed SDK-test setup correction are reused. This is
separate from #378's agent and #350's standalone presentation implementation.

The maintainer approved the Renderflow batch and clarified this issue's current
scope: manually prepare/review SRT or another supported timed file, then derive
the other eligible representations. Automatic detection/transcription/alignment
and a speculative future stub/issue are excluded. Testing/CI remains deferred.
This does not authorize a merge, release, upload or provider submission.

## Delivered source

- `LyricsPolicy` binds explicit classification, declared review, optional
  language/revision and cue-section labels to exact source bytes. The canonical
  wrapper uses the existing `TimedTranscript`; parsers and SRT/WebVTT/text
  writers are reused unchanged. Plain input and start-only LRC do not supply
  cue ends and are not admitted as inputs to this bounded first path.
- The ordinary `renderflow/v2` spec, bundled `lyrics` profile, graph/forest
  planner, CLI build and SDK all use one normalization hub. No second executor
  or timing engine, recognition provider or audio/model dependency is added.
- JSON/YAML retain the modeled timing/context; caption/text/Markdown/LRC/TTML
  outputs disclose losses. LRC emits supplied millisecond line starts without
  inferring ends; TTML uses supplied begin/end times and explicit generated
  XML IDs. Missing score/chord support is an unsupported eligibility diagnostic.
- `lyrics inspect` and the public eligibility API expose the same implemented
  output set. Planned eligibility is distinct from actual manifest publications.
- New semantic JSON/LRC format names and MIME/detection descriptors are explicit.
  Runtime/published spec schemas and composition exclusions carry the policy.
- Artifact transforms use the existing store/process cancellation signal and
  disable generic cache reuse. Mandatory publication admission freshly derives
  expected bytes from the bound original and verifies exact policy, native
  metadata, immediate lineage and canonical identity, including restored records.
- An original synthetic French SRT exercises a repeat, multiline cues and
  non-centisecond timestamps. Its source digest was mechanically recorded.

## Review and evidence

Source review covered policy/codec/graph boundaries, default unreviewed state,
source equivalent exports, metadata/format losses, phase cancellation and the
normal publisher's mandatory admission. It corrected a copied artifact storage
enum label, an unsupported example planning-mode key, a quadratic section lookup,
section byte bounds, declared-language binding, and output media-type admission.

Ten regression cases are authored: four semantic/codec cases, two artifact
lineage/cancellation cases, one graph topology case, and three ordinary
SDK/spec/schema cases. They include exact timing/words/repeats, structured
round trips, independent TTML parsing, digest/section rejection, missing
timing/audio rejection, score diagnostics, source/policy drift and detached
evidence. **All are unrun.** No generated lyric output or successful build is
claimed. Compiler and integration behavior still require qualification.

No tests, compiler/build/Cargo resolution, formatter/lint/schema/docs validators,
product CLI/native/provider/media/model execution or CI ran. No dependencies,
lockfiles, workflows, tags or releases changed. Source/Git identities are the
current evidence; no issue is closed by this checkpoint.

## Remaining acceptance and boundaries

Keep #469 for actual CLI/SDK/forest execution, all profile targets and unchanged
source suppression, malformed/Unicode/repeated/overlapping cases, parser/player
compatibility, failure/retry/idempotence and public provenance/loss inspection.
The inherited strict plain-text subset refuses markup/entities/literal `&` and
cue settings rather than altering content. LRC/TTML are export-only and are not
retailer or broadcast qualifications. There is no audio-time-map or measured
sync claim for a manually supplied timed file. The native manifest and canonical
companion retain evidence; no upload or publishing authority follows.

#427/#440 retain their broader contracts, #430 the whole-forest integration
proof, and #458/#419 the deferred baseline/release gates. No new speculative
backlog was created. See [the guide](../../user-guide/manual-lyrics.md).
