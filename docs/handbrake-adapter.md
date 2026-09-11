# Bounded HandBrake video adapter

Renderflow exposes `video.transcode.whole_file` through
`adapter.media.handbrake`. The adapter creates whole-file MP4 delivery or
mezzanine derivatives. It does not select time ranges, split media, order
segments, or reconstruct segments.

## Ownership boundary

| Concern | Owner | Contract |
| --- | --- | --- |
| Whole-file delivery transcode | Renderflow | `video.transcode.whole_file` |
| Temporal decomposition and boundary accuracy | Aniflow | `media.video.segment/v1` |
| Ordered segment manifest and reconstruction | Aniflow | `media.video.reconstruct/v1` |
| Cross-provider sequencing and artifact routing | Flow | `flow.artifact/v1` |

Aniflow may send Renderflow either the original whole video before splitting or
a validated reconstructed whole video after joining. Renderflow never accepts
an Aniflow plan or segment manifest as a substitute for a video input. Flow
must treat the returned `flow_artifact` as a new lossy derivative and retain
the source relationship included in that record.

## Typed presets

| Renderflow ID | HandBrake preset | Intended output |
| --- | --- | --- |
| `fast_720p30` | `Fast 720p30` | Compact compatible MP4 |
| `fast_1080p30` | `Fast 1080p30` | Default compatible MP4 |
| `creator_1080p60` | `Creator 1080p60` | High-quality creator upload master |
| `production_standard` | `Production Standard` | Editing or mezzanine handoff |

These stable IDs map to an allowlist of official preset names. Arbitrary
HandBrake arguments are not accepted. The registry requires HandBrake 1.6.0 or
newer because the Creator preset naming was standardized in that release.

Inspect the contract or plan a transform without running HandBrake:

```bash
renderflow video capabilities --format json

renderflow video plan \
  --input "source.mp4" \
  --output "delivery.mp4" \
  --preset "fast-1080p30" \
  --format json
```

Execute the planned shape:

```bash
renderflow video transcode \
  --input "source.mp4" \
  --output "delivery.mp4" \
  --preset "creator-1080p60" \
  --timeout-seconds 7200 \
  --capture-limit-bytes 262144 \
  --progress-interval-ms 1000 \
  --maximum-output-bytes 21474836480 \
  --format json
```

## Process and artifact safety

`HandBrakeCLI` runs directly through `renderflow.process/v1`; no shell parses
the command. The request declares a wall-clock timeout, bounded stdout and
stderr capture, a progress heartbeat interval, denied network intent, and a
maximum accepted output size. Each adapter request starts only one child
process. Ctrl+C uses the same cancellation token as SDK callers and terminates
the process tree.

HandBrake writes to a randomized temporary MP4 in the destination directory.
Renderflow requires a non-empty file, an MP4 `ftyp` box, and the configured
size bound before atomically publishing it. Existing output and provenance
files are never replaced. A successful run writes `<output>.renderflow.json`
with provider/tool versions, preset identity, input/output SHA-256 digests,
argv digest, timing, bounded-output evidence, validation, and a
`flow.artifact/v1` projection.

Progress uses `renderflow.progress/v1`. Heartbeats report elapsed execution
time without claiming an unreliable percentage; the durable transform report
is authoritative.

The machine-readable surfaces are published as the
[`renderflow.handbrake-capability/v1` schema](https://github.com/egohygiene/renderflow/blob/main/schemas/renderflow-handbrake-capability-v1.schema.json)
and the
[`renderflow.handbrake-transform/v1` schema](https://github.com/egohygiene/renderflow/blob/main/schemas/renderflow-handbrake-transform-v1.schema.json).

## Sources

- [HandBrake command-line reference](https://handbrake.fr/docs/en/latest/cli/command-line-reference.html)
- [HandBrake official presets](https://handbrake.fr/docs/en/latest/technical/official-presets.html)
