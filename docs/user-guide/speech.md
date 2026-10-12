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
  temporary replacement. Cache/checkpoint reuse remains disabled/unimplemented.

The envelope records configuration and prospective reuse identities containing
source, provider, model, options, and adapter policy. No graph edge or active
speech cache is registered in this checkpoint. See [the handoff](../work/428/HANDOFF.md)
for remaining SDK, caching, original-media synchronization, and qualification work.
