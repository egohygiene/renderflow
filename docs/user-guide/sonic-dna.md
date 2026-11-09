# Sonic DNA from supplied audio evidence

The `sonic-evidence-v1` profile turns a bounded audio snapshot and optional
supplied analyzer artifacts into Sonic DNA, evidence descriptions and prompt
guidance. It reuses the existing DNA engine, artifact store, hygiene and optional
AI skill runtime. Aniflow owns temporal extraction; this operation does not run
stem, tempo, key, lyric, transcription, alignment or MIDI analyzers.

This #397 checkpoint is authored and **unqualified**. Commands, source recipes,
schemas and cases below have not been executed as product qualification.

## Try the synthetic source recipe later

The [example](../../examples/sonic-dna/README.md) contains original synthetic
contract shapes and an explicit recipe for 100 ms of silence. Tempo/key values
are authored test assertions, not observations measured from that silence.

For the later execution session, with a compatible built binary:

```bash
python3 examples/sonic-dna/materialize.py \
  --output "examples/sonic-dna/source.wav"

renderflow dna sonic plan --input "examples/sonic-dna/sonic.json"

renderflow dna sonic build --input "examples/sonic-dna/sonic.json" \
  --output-dir "sonic-candidate"
```

The materializer writes only a new supplied output path and uses no tool/model.
`inspect` aliases `plan`. Planning captures exact inputs and declared capability
compatibility without creating an artifact store/output, probing a service or
invoking a provider. A build writes a new directory whose parent already exists.
Existing outputs are refused before any model invocation.

## Input contract and authority

The [request schema](../../schemas/renderflow-sonic-request-v1.schema.json) uses
`renderflow.sonic-request/v1`:

- `audio` pins one local relative path and exact SHA-256. This first profile
  admits native little-endian RIFF/WAVE PCM16/24/32 or finite IEEE float32,
  1..8 interleaved channels and 8..192 kHz, up to ten minutes. Sample frames
  count simultaneous channel frames. Compressed/extensible WAVE, RF64, multiple
  data streams and implicit conversion are refused. Prepare other audio separately.
- `analyses` optionally pins `aniflow.audio-analysis/v1` JSON and every declared
  evidence file. Source hash, size, stream, zero-origin sample clock and exact
  frame count must match the selected WAV. Provider/capability/evidence IDs,
  scopes, supported kinds/units and confidence classes are cross-checked.
- The mapping consumes sample-rate/channel-count observations, tempo/key
  candidates and selected `aniflow/audio-musical-structure` timelines. Musical
  events retain exact half-open source sample ranges, labels, timeline kind,
  original uncertainty and provider/evidence pointers. Markers occupy one sample;
  ordering, distinct IDs, source extent and explicit overlap policy are checked.
  Each selected timeline has at most 256 events and the existing total-fact bound
  still applies. Other timeline families, semantic-artifact, excerpt and extension
  payloads remain explicitly counted as unprojected. This is a bounded projection of the
  [pinned public contract](https://github.com/egohygiene/aniflow/blob/8edb41e3a81ea0015e725144757f6efcb7d4fb0c/docs/contracts/audio-analysis-v1.schema.json),
  not Aniflow's complete native validator or proof that a provider was executed.
- `companions` are exact byte-bound references with explicit kinds and authority
  declarations. Reviewed lyrics remain distinct from transcript candidates;
  artist-MIDI remains distinct from inferred MIDI. By default their contents are
  not decoded, promoted to canonical material or sent to a model. An explicit
  `timed_text_context` on a reviewed-lyrics or transcript-candidate reference can
  select the existing plain SRT/WebVTT parser for structural context only:

  ```json
  "timed_text_context": {"format": "srt", "maximum_cues": 128}
  ```

  This lane admits at most 1 MiB and 256 cues, bounds all cues to the exact selected
  audio sample clock, and emits only declared role, count, first/last time, overlap
  and source sample extent. It does not expose cue words/IDs/speakers to DNA or
  model input, infer lyric accuracy/alignment, authenticate the supplied review,
  or interpret MIDI. Syntax does not establish musical meaning. Raw words remain
  private captured input, with no implicit companion-role fallback. A declared
  reference is not authenticated authorship, review, alignment or semantic proof.
- `corrections` can append declared human tempo/key corrections to existing fact
  IDs shown in plan/DNA. Each keeps its original estimate, a separate origin,
  review reference and unknown confidence. Structural audio facts cannot be
  rewritten through this route. No correction approves the artifact.

Paths stay confined to the selected recipe directory; no arbitrary metadata
string becomes a filesystem path. Mismatched, missing, duplicate or unobserved
references fail before model work. Native source status remains in the output:
failed/unavailable analysis is not a successful analyzer observation.

The normalizer revision `sonic-evidence-mappings/4` participates in intent
identity, and extractor version 4 distinguishes native music WAV and selected note/stem admission
from the earlier 16 kHz mono projection. Existing JSON requests may omit context. Rust
callers constructing `Companion` literals supply `timed_text_context: None` for
the prior byte-binding behavior. Existing core Artifact DNA is unchanged; the
selected context/facts remain in its namespaced Sonic extension.

Six focused temporal/companion/privacy/refusal cases are authored, uncompiled
and unrun. Native/provider/model quality and broader media/semantic/companion
mapping remain #397 scope; final execution is deferred to flow #88. No actual
analyzer, model, lyric review or media-processing run is claimed.

## Confidence and evidence descriptions

The core `renderflow.artifact-dna/v1` contract stays unchanged. Actual PCM
structural observations use its normal observation fields. The namespaced
`provider_extensions["renderflow.sonic"]` retains normalized musical facts,
original confidence tags, analyzer declarations and exact manifest pointers.

`calibrated`, `uncalibrated`, `unavailable` and `not_applicable` keep their
meaning. An unknown musical confidence is never converted to zero or one, and
an uncalibrated score is not called an accuracy probability. Original declared
providers/tools/models/licenses are hash-bound to the supplied manifest, not
authenticated. Conflicts and corrections stay separate facts.

Without AI, `audio-description.json` renders known evidence in English, with
fact references and explicit uncertainty. It does not infer instrumentation,
vocal identity, mood, lyrical meaning or score semantics. At most 32 facts are
displayed in the short description; the complete sanitized fact list remains
in DNA. Importance weights in prompt guidance are separate from confidence.

## Optional structured-evidence interpretation

The bundled `skill.sonic.describe-evidence@1.0.0` consumes **sanitized structured
facts** through the existing model catalog/runtime. It never receives audio,
lyrics, MIDI or companion payloads. A text-only model is not represented as
hearing audio. Direct-audio inference is explicitly unsupported by this profile.

Semantic configuration is disabled by default. Enabling it requires both
`policy.allow_ai: true` and `semantic.source_approved_for_ai: true`, plus an
explicit `optional` or `required` mode. These are caller declarations; the
software does not grant permission itself. Request a prepared compatible model
through the existing runtime; no model is downloaded or service started.

CLI model use additionally requires both an explicitly prepared local
Ollama-only catalog and a loopback base URL, for example:

```bash
renderflow dna sonic build --input "local-sonic.json" \
  --output-dir "sonic-semantic-candidate" \
  --ai-catalog "prepared-local-catalog.json" \
  --ollama-endpoint "http://127.0.0.1:11434"
```

Keep the recipe beside its declared inputs or update the confined relative
paths. The example paths are placeholders, not prepared runtime evidence.
Verify the selected runtime/model/assets and availability separately. A catalog
entry marked available remains a declaration; loopback alone does not prove
that a service's cloud features or outbound access are disabled.

The SDK may receive an existing explicitly configured `AiSkillRuntime`; remote
selection additionally requires the existing policy/skill permissions. No failed
invocation silently switches providers. This skill has one attempt and a
60-second provider budget. Cancellation is checked before/after the synchronous
call; immediate in-flight abort is not claimed.

The profile checks exact skill/input/output identity, strict JSON, known fact
citations and domain hygiene. Source-like instructions remain data. The same
domain privacy/protected-reference policy is applied before model input and
after output. Citation validity does not certify semantic accuracy. Prose,
corrections and prompt guidance remain review-required candidates.

`disabled`, `blocked`, `not_configured`, `unavailable`, `ready`, `produced`,
`failed` and `invalid` remain separate semantic states. Optional failure retains
baseline DNA and deterministic descriptions. Required semantic failure retains
that evidence with `state: partial`; CLI emits the report and returns nonzero.
Model usage/cost unknowns stay unknown in the existing execution evidence.

## Output and reproduction

The new local bundle contains:

- `sonic-dna.json`: existing DNA contract with sanitized Sonic evidence;
- `audio-description.json` and `.md`: cited structured candidate and a literal
  Markdown projection;
- `prompt-guidance.json`: selected descriptive components/tags, explicit
  importance weights and avoidance guidance for related original assets;
- `lineage.json` and produced CAS objects: exact parent/artifact references;
- `sonic-run.json` last: plan, actual semantic status, normalization diagnostics
  and observed file hashes/sizes.

Original audio, analyzer/companion payloads and raw provider candidate bytes are
not copied into this outgoing bundle. Lineage marks selected inputs external;
retain the original recipe/files privately for reproduction. Opaque hashes bind
them without turning them into authenticated source or approval records.
The outgoing envelope is local candidate evidence, not remote publication.

Identity includes exact request/input digests, mapping/profile/engine revisions,
the pinned upstream contract, policy/corrections/weights and semantic skill.
Existing AI evidence also records provider/model/runtime/settings/schema/cache
identity. Automatic Sonic caching/resume is not introduced; new source/review or
policy needs a new run. Reviewed promotion remains outside this operation.

This is a standalone DNA SDK/CLI operation, with
`dna::sonic::{plan,build}`. It does not advertise an ordinary v2 graph transform,
direct audio model adapter or released Flow/Dreamscape integration. Consumers
can use versioned candidate artifacts without importing sibling source.

## Bounds and remaining evidence

Recipe/native JSON is capped at 2 MiB per file. There are at most 8 analyses,
32 companions, 64 corrections, 256 selected input files and 500 imported/corrected
facts; expanded facts are capped at 2 MiB. Source admission is selected by `policy.max_source_bytes` (at most 256 MiB).
For policies above the earlier 32 MiB ceiling, Sonic explicitly selects source
budget plus 64 MiB of supporting capture (at most 320 MiB); smaller policies
retain 64 MiB total capture. Outgoing bundles and other local publication
callers retain 64 MiB. Cooperative 120-second I/O phases and per-evidence bounds
remain. Copies for capture/CAS/DNA can require more memory than the byte budget. Native opaque provider declarations
are hash-bound once rather than multiplied into every fact. These are not hard
RSS/syscall limits or a hostile concurrent filesystem sandbox.

Sixteen regression cases are authored/unrun, including clearly synthetic model
responses. No tests, compiler/build/Cargo resolution, lint/format/schema/docs
validators, product/native/provider/audio/model execution or CI ran. Full native
analysis validation, broader audio/musical/timeline/companion interpretation,
direct-audio models, measured grounding/quality, live local/remote operation and
platform/release proof remain under existing #397 and #458/#419 acceptance.

## Native music WAV source checkpoint

The `native_music_wav/v1` structural profile preserves exact original sample
rate, channels, frame count, bit depth and encoding. Nothing resamples, downmixes,
quantizes, clips or repairs data. Float samples must be finite; finite headroom
outside [-1,1] is retained. RIFF lengths, unique format/data chunks, block/byte
rates, padding, partial frames and optional fact counts are checked. Ancillary
chunks remain hash-bound source bytes without exposing their text. Speech's
existing 16 kHz mono PCM16 admission is unchanged.

Analyzer source clocks and ascending, unique, in-range channel scopes match the
actual selected source. Timed-text structure maps millisecond half-open bounds
onto that original sample grid with exact integer ceiling, including 44.1 kHz;
this does not prove text/audio alignment. Native plan fields expose selected
source/capture limits. Set an explicit adequate `policy.max_source_bytes` for a
full music master; this is finite admission, not a streaming or RSS guarantee.
Historical technical schemas remain readable without promoting old outputs.

Six new synthetic format/clock/channel/privacy/refusal cases are authored,
uncompiled/unrun. No tests, compiler/builds, dependencies, format/schema checks,
product/native/media/model execution or CI ran. Broader #397 and release #419
acceptance remain open; deep suite qualification stays flow #88, scheduled last.

## Explicit note and selected-stem context

An analysis may opt into these bounded contexts while retaining every declared
evidence file and its exact digest:

```json
"midi_context": {"artifact_id": "midi", "maximum_notes": 64},
"stem_context": {"lineage_artifact_id": "stem_lineage"}
```

Omit unneeded contexts. Existing JSON keeps prior byte-binding/unprojected
behavior; Rust `AnalysisInput` literals supply `midi_context: None` and
`stem_context: None`. A source that declares a stem requires explicit matching
stem context. Every analysis in one recipe still describes the exact selected
WAV, rather than silently combining unrelated source clocks.

MIDI selection follows one matching `candidate_midi` semantic reference into
the retained `aniflow.audio-midi/v1` JSON report. It checks source/clock, exact
provider and upstream evidence bindings, current fixed method, native/normalized
note mapping, rational microsecond times, tick serialization, pitch/velocity
bounds, ordering and same-pitch overlap. The initial native Aniflow candidate
profile remains PCM16 mono 22.05 kHz, at most 120 seconds/8 MiB. Broader music
WAV admission does not make this analyzer available on every music master.
Nothing resamples or rebinds a different source to satisfy it. Explicit limits
of 1..128 notes refuse excess notes without truncation. Unselected semantic
families remain counted as unprojected; raw artist/inferred MIDI companions
remain opaque. No MIDI binary parser or analyzer invocation is added.

Each `sonic.score.inferred_note` keeps exact rational start/end, pitch, velocity,
activation, original unavailable confidence and report pointer. Activation and
velocity are not confidence. The 960 PPQ / 500000 microsecond-quarter tick clock
is serialization, not a detected tempo. Candidates are unreviewed, with no
instrument/voice interpretation or score authorship/review authority. Embedded
raw-note mappings are checked; separately referenced raw observation bytes are
not claimed observed. Native runtime/probe/method authenticity is unestablished.

Stem selection reads the retained `aniflow.audio-stem-lineage/v1` report and
exact root stem identity, provider, upstream, separation and 7..14 retained
authority artifact references. Whole-stem scope and declared zero-origin
duration tolerance (at most 20 ms) are checked with integer clock arithmetic.
The selected stem bytes are observed; original-mix bytes remain unobserved
declarations. The relationship fact keeps each source's own hash, rate, channel
count and frame count and explicitly denies established sample alignment. Raw
plan/checkpoint/provider-lock digests are retained metadata, not replayed native
acceptance, separator authentication or an inferred instrument/stem role.

These are bounded projections of public contracts pinned at Aniflow commit
`8edb41e3a81ea0015e725144757f6efcb7d4fb0c`, without sibling source imports. Exact
selected payload snapshots are borrowed within existing capture; raw settings,
commands, authority file payloads and companion words do not enter facts/model
input. Existing DNA/hygiene/artifact/model owners and total-fact/byte caps remain.
Normalizer/extractor revision4 is intent-bound. Five meaningful synthetic
projection/privacy/clock/refusal cases are authored, uncompiled/unrun. Tests,
builds/dependencies, lint/format/schema/docs/native/provider/media/model execution
and CI remain deferred. #397 keeps full analysis/semantic/direct-audio/quality
acceptance; flow #88 owns the final deep pass, scheduled last.
