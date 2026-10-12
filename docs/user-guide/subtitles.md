# Subtitles and timed transcripts

This #427 checkpoint is an **unvalidated draft**. The commands below describe
the authored interface; they have not been executed or released. Qualification
and remaining integration work are recorded in [the handoff](../work/427/HANDOFF.md).

## Convert a subtitle file

The local text converter accepts the strict plain-text SRT/WebVTT profile and
`renderflow.timed-transcript/v1` JSON. It needs no external media tool.

```bash
renderflow subtitles convert --input "captions.srt" --input-format "srt" --output "captions.timed-transcript.json" --output-format "json"
renderflow subtitles convert --input "captions.timed-transcript.json" --input-format "json" --output "captions.vtt" --output-format "vtt"
```

Use `--output-format "text"` for plain transcript text. Timing and cue metadata
losses are recorded in the JSON report on stdout. Preserve that report when
exporting a format that cannot carry the full source/provenance information:

```bash
renderflow subtitles convert --input "captions.srt" --input-format "srt" --output "captions.txt" --output-format "text" > "captions-export.json"
```

The output parent directory must already exist. The command publishes a new
file and refuses an existing destination, including the input itself. The
source is read only. Publication uses a temporary file beside the destination;
this is not an in-place or zero-additional-storage operation. If stdout cannot
be written after publication, the output file may already exist.

## Inspect and extract existing subtitle streams

Inspection uses the optional local FFprobe executable. Extraction additionally
requires FFmpeg. No tool or model is downloaded automatically.

```bash
renderflow subtitles inspect --input "clip.mkv"
renderflow subtitles extract --input "clip.mkv" --stream-index 2 --output "captions.timed-transcript.json" --output-format "json"
```

Choose the **absolute stream index returned by inspection**; `2` is only an
example, never a default choice. A container with no subtitles returns an empty
stream list. Extraction never substitutes a different stream or invents cues.

The initial adapter accepts local Matroska/WebM and MOV/MP4-family containers
and only extracts SubRip/WebVTT subtitle codecs. It forces the corresponding
demuxer, restricts protocols, disables external MOV data references, and uses
the shared bounded process executor. Unsupported codecs remain visible as
diagnostics; they are not silently converted. Media contents are hashed for
provenance, so inspection still reads the source bytes even though it does not
decode the whole audio/video track.

## Preservation and limits

- The canonical JSON stores source SHA-256, cue IDs, millisecond timestamps,
  text, optional speaker/language, overlap policy, and losses.
- Extracted JSON also records stream index/codec/language, FFprobe and FFmpeg
  version evidence, and the intermediate subtitle digest.
- Input must be UTF-8. Text input is capped at 16 MiB, cue count at 100,000,
  and each cue's text at 64 KiB.
- Cue start times must be ordered, and each end must follow its start.
  Overlaps are explicit: the initial parsed profile preserves them; JSON can
  request rejection.
- Styled markup, WebVTT positioning/styles/regions, and unsupported constructs
  are rejected. This profile does not flatten rich captions while claiming
  fidelity.
- ASS/SSA/TTML recognition is not proof of executable conversion support.
- The initial text profile also refuses literal `<`, `>`, and `&`; it does not
  yet distinguish literal punctuation from subtitle markup/entities.
- Plain subtitle exports cannot carry every canonical metadata field. Keep the
  canonical JSON or stdout export report when provenance matters.

Provider limits are fixed in this checkpoint: 64 GiB source size, 256 KiB
inventory capture, 8 MiB extracted subtitle capture, 64 KiB diagnostic capture,
30-second inspection processes, and 300-second extraction processes. Hashing
checks cancellation and a 120-second budget between reads; it cannot interrupt
a blocked filesystem read. Native warnings and errors refuse partial results.
Source rechecks detect changes but do not defend against hostile concurrent
path replacement. Use stable local source files. Ctrl-C requests cancellation
through the shared process executor.

Wire shapes are documented in the [transcript schema](../../schemas/renderflow-timed-transcript-v1.schema.json),
[inspection schema](../../schemas/renderflow-subtitle-inspection-v1.schema.json),
and [export schema](../../schemas/renderflow-subtitle-export-v1.schema.json).
The export report hashes the immediate input as well as retaining the original
source metadata carried by a supplied transcript; supplied provenance is not
independently attested by JSON validation.

These are standalone CLI and Rust module operations. They do not yet add
subtitle execution edges to `build`, the exhaustive forest, or Flow's released
provider contract. Speech recognition is #428; temporal editing belongs to
Aniflow.
