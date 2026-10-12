# Local speech transcription

This #428 checkpoint is an **uncompiled, unrun implementation draft**. The user
has deferred tests, linting, and CI while implementation moves forward. No
spoken-audio fixture or model has qualified this interface yet.

The authored `speech transcribe` command runs one local whisper.cpp inference
and derives all outputs from one normalized timed transcript. It requires an
already-local executable and model; Renderflow installs or downloads neither.
Direct input is mono 16 kHz signed PCM16 RIFF/WAV, up to ten minutes. An optional
preparation route inspects and decodes one explicitly selected audio stream
from a local supported container into that profile.

## Request a transcript bundle

Use the real executable file path, resolving a final symlink before invoking
the command. Source, executable, and model must be regular files. The provider
and model SHA-256 values are required content pins; replace the illustrative
placeholders below with the digests of the files you selected. `model-id` is
your declared label; the model digest establishes its byte identity.

```bash
renderflow speech transcribe \
  --input "/absolute/path/to/voice.wav" \
  --provider "/absolute/path/to/real/whisper-cli" \
  --provider-sha256 "<64-character-lowercase-executable-sha256>" \
  --model "/absolute/path/to/ggml-model.bin" \
  --model-id "my-local-model" \
  --model-sha256 "<64-character-lowercase-model-sha256>" \
  --language "en" \
  --threads 4 \
  --timeout-seconds 900 \
  --output-dir "/absolute/path/to/new-transcript-bundle"
```

The output parent must exist and the bundle directory must be new. English is
the default language; `--language "auto"` requests detection. The provider's
reported language must agree with the request. Translation and speaker
diarization are not enabled. Ctrl-C requests cancellation. CLI messages show
the operation phase; percentage progress and machine progress events are not
part of this checkpoint.

## Select audio from a video or another audio file

First list the audio streams. Inspection returns JSON with each **global** stream
index, codec, channel count, sample rate, available language/title/timing, and
the source digest. It performs no recognition and selects no default track.

```bash
renderflow speech inspect \
  --input "/absolute/path/to/video.mp4" \
  --ffprobe "/absolute/path/to/real/ffprobe" \
  --ffprobe-sha256 "<64-character-lowercase-ffprobe-sha256>"
```

Then choose an audio index from that inventory and copy its `source_sha256`.
The required input digest prevents a changed file from silently reusing the
track choice. For example, if the desired audio track has global index `1`:

```bash
renderflow speech transcribe \
  --input "/absolute/path/to/video.mp4" \
  --input-sha256 "<source_sha256-from-inspection>" \
  --audio-stream 1 \
  --ffprobe "/absolute/path/to/real/ffprobe" \
  --ffprobe-sha256 "<64-character-lowercase-ffprobe-sha256>" \
  --ffmpeg "/absolute/path/to/real/ffmpeg" \
  --ffmpeg-sha256 "<64-character-lowercase-ffmpeg-sha256>" \
  --audio-timeout-seconds 300 \
  --provider "/absolute/path/to/real/whisper-cli" \
  --provider-sha256 "<64-character-lowercase-executable-sha256>" \
  --model "/absolute/path/to/ggml-model.bin" \
  --model-id "my-local-model" \
  --model-sha256 "<64-character-lowercase-model-sha256>" \
  --language "en" \
  --threads 4 \
  --timeout-seconds 900 \
  --output-dir "/absolute/path/to/new-video-transcript"
```

Omitting `--audio-stream` keeps the direct strict WAV route. Supplying it
requires both pinned FFmpeg tools and the source digest; even a single-track
file requires a choice. The current container profile uses file extensions to
force supported demuxers: MKV/MKA/WebM, MP4/M4V/M4A/MOV, AVI, WAV, FLAC, MP3,
Ogg/OGA/Opus, and AAC.
Mislabeled files are not automatically retried with another demuxer. Playlists,
URLs, directories, and final-component symlinks are refused.

Preparation downmixes to mono, resamples to 16 kHz, and encodes signed PCM16.
These operations discard source audio detail. **Transcript times refer to
decoded sample order in `prepared.wav`, starting at zero.** Original timestamp
offsets and gaps are not preserved; the result is not automatically aligned for
muxing captions into the original video. Observed start time is provenance, not
a universal synchronization offset. No trimming or segmented transcription is
offered in this checkpoint.

Known overlong streams are refused. Missing duration remains unknown at
inspection; the decoded WAV must still fit ten minutes. A 601-second native
output guard is a refusal sentinel: output beyond 600 seconds fails and is not
published as a successful truncated transcript.

## Publication

No output is published until recognition and every derivative have been
prepared. Publication reserves a new directory, writes files without replacing
existing ones, and writes `bundle.json` last. An I/O error or cancellation may
leave a partial directory for inspection. A complete receipt plus matching
listed files and hashes establishes bundle publication; a directory's existence
alone does not. Output/stdout failures can happen after files were published.

## Retain the whole bundle

| File | Contents |
| --- | --- |
| `provider-output.json` | Exact bounded whisper.cpp full JSON, including token details the normalized model does not represent. |
| `transcription.json` | `renderflow.speech-transcription/v1`: normalized transcript plus provider/model/configuration/source identity and limitations. |
| `transcript.timed-transcript.json` | Existing `renderflow.timed-transcript/v1` payload for ordinary subtitle conversion. |
| `transcript.srt` | SubRip export; numeric cue IDs replace speech segment IDs. |
| `transcript.vtt` | WebVTT export retaining speech segment IDs. |
| `transcript.txt` | Plain text with timing and cue IDs omitted. |
| `audio-probe.json` | Prepared-media runs only: exact bounded FFprobe JSON used for stream selection. |
| `prepared.wav` | Prepared-media runs only: exact WAV bytes consumed by recognition. |
| `bundle.json` | Exact hashes/sizes and export losses for six files, or eight for prepared-media input; also emitted to stdout. |

The canonical timed-transcript contract is unchanged. Full speech provenance
lives in `transcription.json`, so keep it and the raw provider output alongside
derivatives. Copying just SRT, WebVTT, text, or canonical JSON does not preserve
the complete speech evidence. The existing [subtitle commands](subtitles.md)
can render additional derivatives from the canonical file without rerunning ASR.

For prepared-media runs, the canonical transcript source digest names the WAV.
The speech envelope's `evidence.audio_preparation` links it to the original
media digest, chosen stream, pinned FFprobe/FFmpeg identities, conversion policy,
raw probe digest, and preparation limits. Keep the entire bundle for that chain.

Segment offsets come from the provider in integer milliseconds. Segment-edge
whitespace is trimmed and that normalization is recorded. The strict #427 text
profile rejects markup, literal `<`, `>`, `&`, empty/invalid cues, and unsupported
control characters rather than silently rewriting them. Empty recognition is
an explicit failure. The text is **unreviewed**; token probabilities are not
reported as calibrated confidence. Lyrics and singing need human review.

## Bounds and current limits

- WAV input: 32 MiB maximum and ten minutes of one mono 16 kHz PCM16 data stream.
- Preparation source: 4 GiB maximum. Probe output: 1 MiB maximum; FFprobe timeout
  30 seconds. Preparation timeout: 1–3600 seconds, default 300. FFmpeg output
  is inspected after exit and source/tools are rechecked around work. Retaining
  the prepared WAV adds up to 32 MiB to the published bundle.
- Model: 4 GiB maximum; executable: 256 MiB maximum. Both digests are checked
  before use, and source/model/executable identities are rechecked around work.
- Native stdout/stderr capture: 64 KiB each. Accepted provider JSON and each
  published report/derivative are capped at 16 MiB. The native output file is
  size-checked after the process; temporary disk and RAM are not OS-capped.
- Threads: 1–64; one processor; native timeout: 1–3600 seconds, default 900.
  Hash/read operations have 120-second budgets checked between reads, which
  cannot interrupt a blocked filesystem read.
- The adapter uses direct argv, a cleared environment, local files, and a deny
  network policy declaration. It does not impose an OS network sandbox on an
  operator-supplied executable. Fixed decoder settings and GPU-disabled flags
  do not guarantee CPU-only execution or identical results across builds.
- Linked libraries, compiled accelerators, and model sidecars are not attested
  by the two file digests. File rechecks detect persistent changes, not hostile
  temporary replacement. Automatic cache/checkpoint reuse remains disabled.

The envelope records configuration and reuse identities containing source,
provider, model, options, and adapter policy. The graph route below also supports
explicit replay of a pinned saved result. This recovers historical output and
does not attest the current native environment or promise identical new inference.
See [the handoff](../work/428/HANDOFF.md) for remaining qualification work.

## Use the planner and Rust SDK

The third draft checkpoint adds an explicit `execution.speech` policy to the
existing build/SDK route. Without that policy, the planner does not add speech
recognition. It requires one source, its exact SHA-256, and local AI permission.
Speech runs select only the declared source-to-transcript route and its subtitle
exports; they do not silently transcode some other audio/video format first.

Save an authored configuration like this beside `voice.wav`, substituting real
absolute tool/model paths and lowercase SHA-256 values:

```yaml
schema: renderflow/v2
sources:
  - id: voice
    path: voice.wav
    format: wav
targets:
  exact:
    - format: timed_transcript_json
      role: transcript
    - format: srt
      role: captions_srt
    - format: vtt
      role: captions_vtt
execution:
  ai: local_only
  network: deny
  speech:
    source_sha256: "<64-character-lowercase-source-sha256>"
    provider:
      path: "/absolute/path/to/real/whisper-cli"
      sha256: "<64-character-lowercase-executable-sha256>"
    model:
      id: "my-local-model"
      path: "/absolute/path/to/ggml-model.bin"
      sha256: "<64-character-lowercase-model-sha256>"
    options:
      language: "en"
      threads: 4
      timeout_seconds: 900
output:
  bundle_root: "dist"
```

```bash
renderflow build --config "speech.yaml"
```

For video or other supported audio, declare the actual source format, use the
digest from `speech inspect`, and add this nested block under `execution.speech`:

```yaml
audio:
  stream_index: 1
  ffprobe:
    path: "/absolute/path/to/real/ffprobe"
    sha256: "<64-character-lowercase-ffprobe-sha256>"
  ffmpeg:
    path: "/absolute/path/to/real/ffmpeg"
    sha256: "<64-character-lowercase-ffmpeg-sha256>"
  timeout_seconds: 300
```

The stream index remains an explicit global index; `1` above is an example,
not a default. Omitting `audio` permits only strict WAV input. Both speech and
embedded-subtitle extraction cannot be enabled in the same run. Plain text
remains a standalone speech/subtitle export rather than a new graph format.

SDK callers use the existing public entry points:

```rust
use renderflow::{EngineBuilder, ExecutionRequest, PlanRequest};

let engine = EngineBuilder::new().build()?;
let plan = engine.plan(PlanRequest::from_path("speech.yaml"))?;
let resolved = engine.resolve_execution(ExecutionRequest::from_path("speech.yaml"))?;
let result = engine.execute_resolved(resolved, false)?;
```

Planning reads the source to establish its identity but does not launch the
pinned speech tools or read a model. Provider availability is explicitly
unobserved until execution; a successful plan is not a successful transcription.
Runtime consumes the imported immutable artifact-store source, with no extra
copy merely to provide a file extension. The normal source import still uses
storage for a source copy, and prepared audio/retained evidence needs additional
storage. This route is not an in-place low-storage optimizer.

The SDK's cancellation token reaches preparation and recognition. A progress
reporter receives coarse live `speech.verify_input`, `speech.prepare_audio`
(when needed), `speech.transcribe`, `speech.retain_artifacts`, and
`speech.complete` states on the existing progress-event contract. The last phase
means speech artifacts were retained; publication and the final run event still
follow. These events do not invent percentages. Typed failures and cancellation
remain visible in final step diagnostics.

Graph runs publish supporting files under `speech/<envelope-sha256>/` in the
output bundle: `provider-output.json`, `transcription.json`, and
`transcript.timed-transcript.json`, plus `audio-probe.json` and `prepared.wav`
when preparation is enabled. A `saved-result.json` receipt is published after
these supporting files. The canonical transcript is retained even for an
SRT-only request. Existing identical evidence can be kept; different bytes and
symlinks are refused. Selected outputs are withheld if evidence publication
fails. Cancellation during that publication is reported as a cancelled run;
an incomplete evidence directory may remain. The original source stays in the
artifact store. Budget accounting includes the retained records and published
copies.

The native manifest retains namespaced `renderflow.speech` metadata with the
canonical transcript and retained evidence records. Derivative artifacts carry
the same speech references; the existing subtitle metadata still records their
export losses. Flow's closed artifact projection retains hashes and lineage,
while the detailed speech evidence remains in the native manifest and retained
JSON files. Keep the complete output bundle and its run manifest.

Recognition and its derived subtitle steps bypass automatic result-cache and
checkpoint reuse without opening shared cache/checkpoint files. Without an
explicit saved-result selector, SDK resume assessment requests recomputation.
All examples and this integration remain uncompiled and unrun.

## Replay a saved result

To regenerate selected transcript/subtitle outputs without another recognition
run, keep the entire `speech/<envelope-sha256>/` directory. Add the following
under the original `execution.speech` policy, using the receipt digest recorded
in the native run manifest:

```yaml
recovery:
  receipt_path: "/absolute/path/dist/speech/<envelope-sha256>/saved-result.json"
  receipt_sha256: "<saved-result-sha256-from-run-manifest>"
```

Keep the original source digest, declared format, provider/model configuration,
options, and preparation policy. Select whichever canonical JSON/SRT/WebVTT
targets are needed, then use the ordinary build command or Engine SDK. The
original source is imported and checked again. Provider/model paths remain
recorded configuration; replay does not open them or launch native tools.

Replay verifies the pinned receipt and all retained files, reconstructs the
normalized result from raw provider output, and restores canonical artifacts
into the current store. Receipts are limited to 256 KiB; retained payloads keep
their existing byte limits and cancellation-aware reads. Selected subtitle exports are generated from that
result. A missing, changed, incompatible, or incomplete saved result fails
without starting another recognition run. Remove `recovery` to request a fresh
native run. Progress reports `speech.reuse_saved_result` during replay.

`Engine::assess_resume` verifies an explicit receipt read-only and reports
whether it can be reused. Execution verifies it again. The receipt identifies
a historical result; it is not evidence that installed libraries, accelerators,
or model sidecars would reproduce that result today.

The receipt belongs to the graph output bundle. Existing standalone
`speech transcribe` bundles and older graph bundles have no saved-result
receipt and cannot be selected by this recovery policy.

## Inspect transcript timing

Use a graph receipt to inspect the transcript clock before another project
consumes it:

```bash
renderflow speech timeline \
  --receipt "/absolute/path/dist/speech/<envelope-sha256>/saved-result.json" \
  --receipt-sha256 "<saved-result-sha256-from-run-manifest>"
```

This prints a JSON assessment after checking the pinned receipt and retained
files. It does not read the original media, load a model, run a provider, or
create an artifact store. Source identity and strict-input-WAV duration are
recorded evidence; prepared WAV bytes are present and checked in prepared
bundles.

Strict WAV input uses the input WAV's sample clock. Prepared audio always
reports that original-media mapping is required, including when its recorded
stream start is zero. A stream start time alone cannot prove that decoding
preserved gaps, delay, or edits. The assessment does not shift cues or claim
video mux readiness.

Add `--require-original-media` to print the assessment and then fail when that
mapping is missing. SDK/library consumers can use the corresponding
`speech::timeline::assess` and `require_original_media_timeline` APIs with a
verified saved result.
