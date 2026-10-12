# Renderflow #428 — local speech checkpoint 1

## Authority and starting point

- Parent issue: https://github.com/egohygiene/renderflow/issues/428 under #421;
  dependent subtitle work remains in draft PR #438 for #427.
- The user explicitly chose to skip remaining tests and continue implementation
  after #427's environment-blocked completion pass. Do not attempt another
  toolchain bootstrap or native qualification under this budget.
- Strategy: implementation-first checkpoints with durable evidence boundaries.
  Tests, compilation, lint/format, schemas, local/native smoke, and GitHub
  Actions are deferred. Read-only source/Git inspection and draft publication
  are authorized. No merge/release was requested in this continuation.
- Branch: `feat/renderflow-428-local-speech`, targeting the existing subtitle
  branch `feat/renderflow-427-timed-text` as a stacked draft.
- Published base: `14ebc90f35c90c90942a3b9c086ed58b232b662b`, tree
  `53a9d02adc673058ce94c3c5bfa779c1f112eabd`, matching local commit
  `e72f26ecade13a1cd0b57dc5c0a9c41042f407a2`.
- That base includes a source-found correction to two #427 integration-test
  imports: the SDK types are publicly re-exported at the crate root; its `sdk`
  module is private. The correction remains uncompiled.

Find the current PR by this branch; this handoff cannot contain its own commit
SHA. If the subtitle PR is merged, reconcile/retarget the stack before further
publication. Do not merge either draft merely because source review is clean.

## Authored interfaces

- `src/speech/mod.rs` in `crates/renderflow-core` owns provider-neutral options,
  identity, speech evidence, and the `renderflow.speech-transcription/v1`
  envelope. `normalize_whisper_json` maps one bounded provider result into the
  existing `TimedTranscript`. It rejects duplicate JSON keys, translation,
  language substitution, missing/invalid segments, and unsupported strict text.
- The canonical #427 timed-transcript model/schema is unchanged. Speech
  metadata remains in the separate envelope, which must accompany derivatives.
  Token probabilities do not become calibrated confidence. All text is marked
  unreviewed, segment-edge trimming is disclosed, and speaker IDs are absent.
- `src/speech/provider.rs` accepts `SpeechRequest` with local input,
  executable/model paths and expected SHA-256 digests, model ID, and options.
  `transcribe` returns `SpeechRun` containing both `SpeechTranscription` and
  the exact bounded raw provider JSON. It uses the shared `ProcessExecutor`.
- Input is one mono 16 kHz signed PCM16 RIFF/WAV file, at most ten minutes and
  32 MiB. The parser refuses compressed/extensible/RF64, malformed boundaries,
  duplicate format/data chunks, and implicit conversion. Source, model, and
  executable are read-only and rechecked around native work.
- The provider version is observed with `--version`; executable/model byte
  identity is explicitly pinned. Model <=4 GiB; executable <=256 MiB; captures
  <=64 KiB each; accepted JSON <=16 MiB; bounded hashing/read budgets and native
  timeout/cancellation. Fixed decoding arguments are shared with the
  configuration hash to keep argv and identity aligned.
- `renderflow speech transcribe` exposes the standalone flow. It runs inference
  once, generates canonical JSON/SRT/WebVTT/text, retains `provider-output.json`
  and `transcription.json`, and publishes a hash/size/loss receipt last. Existing
  destinations are refused. Publication failures may leave a partial new
  directory; consumers need the receipt and matching files, not existence alone.
- Tool/adapter registries add `tool.whisper_cpp`,
  `adapter.speech.whisper_cpp`, and `speech.transcribe` as experimental optional
  capability metadata. No planner graph edge is advertised.
- [User guide](../../user-guide/speech.md) describes the authored invocation,
  bundle layout, provenance preservation, and limits. All examples are unrun.

## Source review and evidence boundary

[Provider evaluation](PROVIDER_EVALUATION.md) pins whisper.cpp source to
`7e26d3b0d5bced6c8217ef09710ec2f969001241` and records the exact flags, JSON
fields, model/version behavior, licensing, and backend caveats. Upstream can
exit zero on invalid CLI arguments; required output/model/language checks must
remain in place. Routine provider stderr is not automatically a failure.

Source review caught and corrected an initial identity gap: fixed decoding
flags now participate in the configuration hash and are consumed from the same
constant by the argv builder. The provider preserves raw output bytes for the
bundle instead of retaining only their digest. CLI publication checks the raw
output digest before writing and uses new-file publication for every member.

Fourteen test functions are authored and **unrun**: six normalization/identity
tests, six native-adapter helper/refusal tests, and two publication tests. The
original synthetic provider-JSON fixture is not generated ASR output. No
spoken-audio fixture, model, provider binary, compiler, test, schema validator,
lint, formatter, or CI job was executed/installed for #428.

Earlier #427 provider smoke/schema results do not qualify #428. No passing
speech evidence, platform compatibility, lyric accuracy, or release readiness
is claimed. Use `[skip ci]` commits and disabled hooks; do not dispatch/poll CI.

## Next implementation checkpoints

1. Add explicit audio stream discovery/selection and bounded local extraction
   or preparation for container inputs. Preserve original media digest,
   selected stream, extraction tool identity, and derived WAV identity. Never
   select a stream silently or bypass the known WAV input profile.
2. Wire an explicit speech policy through planner/SDK and native artifact
   lineage. Keep full speech evidence and raw provider output inspectable;
   retain the closed Flow artifact envelope. Add structured progress/cancellation
   events without inventing recognition percentages.
3. Decide the actual cache/checkpoint contract. The current configuration and
   reuse keys include provider/model/source/options/adapter policy, but no
   speech lookup, persistence, or reuse is implemented. Shared libraries,
   compiled accelerators, and model sidecars are not covered by the two file
   hashes; qualify an effective execution profile before enabling reuse.
4. When the user returns to testing, run the focused Rust tests and schema
   agreement checks, CLI no-overwrite/refusal cases, and a licensed spoken-audio
   fixture with pinned binary/model. Retain raw output and derivatives; prove
   timeout, cancellation, source stability, and platform behavior. Inspect
   Windows/macOS environment/loading requirements and English-only model
   language behavior. General lint/full-suite/release work stays separate.

## Limits that must remain visible

There is no video/audio normalization, graph route, diarization, translation,
word-alignment guarantee, human-approved copy, active cache reuse, or model
download. CLI progress currently reports phases only. Native stdout/stderr are
bounded; on-disk provider output is checked after exit, without OS disk/RAM caps.
Network deny is process-policy intent, not an OS sandbox. GPU-disabled settings
do not guarantee CPU-only execution. Hashing can observe persistent mutation,
not hostile temporary path replacement or blocked filesystem reads. The final
component of input/model/executable paths must not be a symlink; ancestor links
are canonicalized. Local model licensing remains an operator responsibility.

#427/#428 acceptance and release remain open. Optiflow #111/PR #122 and its ACL
qualification remain the other paused workstream; Aniflow #36 remains parked.
