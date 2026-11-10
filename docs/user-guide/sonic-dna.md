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
  admits the existing strict mono 16,000 Hz signed PCM16 RIFF/WAVE subset, up to
  ten minutes. Compressed/extensible WAVE, containers, multiple streams and
  implicit conversion are refused. Prepare other audio separately.
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
  [pinned public contract](https://github.com/egohygiene/aniflow/blob/8008e0176f45b34d2a9e74d8a26d9b3fd374b81f/docs/contracts/audio-analysis-v1.schema.json),
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

The normalizer revision `sonic-evidence-mappings/2` participates in intent
identity, and extractor version 2 distinguishes the extended mapping from the
earlier scalar-only projection. Existing JSON requests may omit context. Rust
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
facts; expanded facts are capped at 2 MiB. Source capture/output uses the shared
64 MiB budget and cooperative 120-second I/O phases. Selected PCM/evidence-file
caps and stricter native limits also apply. Native opaque provider declarations
are hash-bound once rather than multiplied into every fact. These are not hard
RSS/syscall limits or a hostile concurrent filesystem sandbox.

Sixteen regression cases are authored/unrun, including clearly synthetic model
responses. No tests, compiler/build/Cargo resolution, lint/format/schema/docs
validators, product/native/provider/audio/model execution or CI ran. Full native
analysis validation, broader audio/musical/timeline/companion interpretation,
direct-audio models, measured grounding/quality, live local/remote operation and
platform/release proof remain under existing #397 and #458/#419 acceptance.

## Inspect a retained candidate bundle

`dna::sonic::readback::inspect` and `dna sonic status` read a saved v1 bundle
against the actual `sonic-run.json` SHA-256 retained separately by the caller:

```console
renderflow dna sonic status --directory sonic-candidate --receipt-sha256 EXTERNAL_RECEIPT_SHA256 --input sonic.json
```

The optional current recipe is recaptured through the existing bounded source
reader only. Status never creates a store, runs extraction, resolves a model or
launches inference. It checks exact files/CAS membership and bytes, record IDs,
ordered lineage, DNA/source/intent bindings, description citations/Markdown and
candidate-only semantics. `complete`/`partial` retain the saved semantic outcome;
provider readiness is explicitly not reobserved. Source state is `current`,
`stale`, `unavailable`, `unavailable_or_changed`, or `not_supplied`. These are
byte/evidence relationships, never authenticated provider execution or musical
accuracy. Missing/malformed receipts and unknown or symbolic members refuse;
no partial directory is adopted. Changed source leaves retained bytes untouched.

Closed request/report schemas are `renderflow.sonic-readback-request/v1` and
`renderflow.sonic-readback/v1`. Existing plan's `inspect` alias keeps its meaning.
Five focused SDK cases and one CLI parser case are authored, uncompiled/unrun.
All standing checks/native/model/CI deferrals persist; flow #88 remains last.
