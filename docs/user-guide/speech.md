# Local speech transcription

This #428 checkpoint is an **uncompiled, unrun implementation draft**. The user
has deferred tests, linting, and CI while implementation moves forward. No
spoken-audio fixture or model has qualified this interface yet.

The authored `speech transcribe` command runs one local whisper.cpp inference
and derives all outputs from one normalized timed transcript. It requires an
already-local executable and model; Renderflow installs or downloads neither.
The initial input profile is mono 16 kHz signed PCM16 RIFF/WAV, up to ten minutes.
Video/container audio extraction and resampling are follow-up work.

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
| `bundle.json` | Exact hashes/sizes and export losses for the six files above; also emitted to stdout. |

The canonical timed-transcript contract is unchanged. Full speech provenance
lives in `transcription.json`, so keep it and the raw provider output alongside
derivatives. Copying just SRT, WebVTT, text, or canonical JSON does not preserve
the complete speech evidence. The existing [subtitle commands](subtitles.md)
can render additional derivatives from the canonical file without rerunning ASR.

Segment offsets come from the provider in integer milliseconds. Segment-edge
whitespace is trimmed and that normalization is recorded. The strict #427 text
profile rejects markup, literal `<`, `>`, `&`, empty/invalid cues, and unsupported
control characters rather than silently rewriting them. Empty recognition is
an explicit failure. The text is **unreviewed**; token probabilities are not
reported as calibrated confidence. Lyrics and singing need human review.

## Bounds and current limits

- WAV input: 32 MiB maximum and ten minutes of one mono 16 kHz PCM16 data stream.
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
  temporary replacement. Cache/checkpoint reuse remains disabled/unimplemented.

The envelope records configuration and prospective reuse identities containing
source, provider, model, options, and adapter policy. No graph edge or active
speech cache is registered in this checkpoint. See [the handoff](../work/428/HANDOFF.md)
for remaining audio extraction, SDK, caching, and qualification work.
