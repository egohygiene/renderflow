# Additional native audio layouts — November 15, 2026

The exact [ALAC precision candidate](ALAC-PRECISION-20261115.md) now has actual
48-kHz mono24 and 96-kHz stereo24 observations. These are new authored input
layouts beyond the earlier 44.1-kHz stereo proof. This checkpoint adds an
observer, portable receipts and documentation only; it does not add a codec,
provider, converter, runtime or new product build.

## Measured source and observations

The CLI remains SHA-256
`d7b2f03670ed1262c9dfab67c9cf64a0314c7c9fb190d0ffc4a14e87ca96ca54`,
20,151,576 bytes, from actual published source
`4fb2539fa686e966143be69b711ffeeccc59d3b4`, tree
`22d8809c8386050bd1734280c69561562aef8d11`. Its compiled release contract remains
`199bf790b2d78e7706b1e24b3e0fdb189b8286eb231f8be1c8da503d82922f58`,
372,295 bytes. The warm-cache Rust 1.94 release build and its immutable record
are retained with the preceding checkpoint. No independent rebuild, package or
supported-host qualification is inferred from the later proof tip.

- [Source/capture/observer bindings](native-audio-layout-bindings-20261115.json).
- [Final actual CLI observation](native-audio-layout-observed-20261115.json):
  seven canonical builds and 27 independent feature/probe/decode commands,
  34 commands total. All 151 input/output/observation files and 68 command
  streams are pinned. Each of seven current raw manifests has a separately
  retained exact byte copy; no positive baseline manifest was overwritten.

## Accepted input/profile combinations

| Original three-second WAV | Selected matching profiles | Exact decoded outcome |
| --- | --- | --- |
| 48 kHz, mono, signed PCM24, 144,000 frames | `flac_broadcast`, `alac_broadcast` | Every original PCM byte, precision, rate, channel and frame count matches; observed lossless |
| 96 kHz, stereo, signed PCM24, 288,000 frames | `flac_hires`, `alac_hires` | Every original PCM byte, precision, rate, channel and frame count matches; observed lossless |

The existing broadcast/hi-res profiles leave channels unspecified; the actual
mono source stays mono and stereo stays stereo. Their explicitly requested
rates/depths match these authored sources. The ALAC hi-res case exercises the
repaired explicit `s32p` path with exact PCM rather than treating a resampling
case as source-preserving.

Both source layouts also produce AAC LC with `aac_256k` at the original rate,
and MP3 with `hq_320k` at its explicitly declared 44.1-kHz rate. All four are
lossy. Native source/target/profile/artifact/producing-step joins, independent
stream/packet/skip/discard clocks and decoded frames agree:

| Source | AAC presented / decoded frames | MP3 presented / decoded frames |
| --- | ---: | ---: |
| 48-kHz mono | 144,000 / 144,384 | 132,300 / 132,300 at 44.1 kHz |
| 96-kHz stereo | 288,000 / 288,768 | 132,300 / 132,300 at 44.1 kHz |

AAC's declared priming and frame-rounded decoded extent remain separate from
presentation. MP3's selected rate conversion stays a declared change; neither
codec gains lossless status from a codec label, container or rounded clock.

## Required refusal controls

Each producer fault follows the real accepted 96-kHz stereo24 `alac_hires`
source/profile baseline and runs in a new isolated CAS parent. The observer
requires exactly one changed producer invocation, independent observed media,
the exact native state/code, structured CLI exit 1 and no accepted outputs.
The test-only shim changes encoder arguments; the absolute pinned real verifiers
remain unchanged and are not represented as attesting the producer executable.

| Controlled condition | Observed native result |
| --- | --- |
| Producer emits 16-bit ALAC for requested 24-bit | Invalid `audio.native.profile.layout` |
| Producer emits 48 kHz for requested 96 kHz | Invalid `audio.native.profile.layout` |
| Producer removes exactly one presented sample frame, preserving rate/channels/depth | Invalid `audio.native.presentation_mismatch` |
| Producer quantizes samples through 16-bit but still emits 24-bit ALAC with unchanged rate/channels/count | Invalid `audio.native.pcm_mismatch` |
| Original 96-kHz stereo integer PCM32 | Required Unavailable `audio.native.source_precision_unsupported` |

The PCM control uses the already prepared `aformat` filter, then the selected
ALAC `s32p` encoder. An independent raw24 decode must equal every original
signed integer reduced to its high 16 bits and promoted back to 24 bits:
`(value >> 8) << 8`. Original nonzero low bytes make the decoded bytes differ
while the admitted layout/clock remain the same. A lossless codec therefore
cannot hide changed samples.

An earlier observer attempted a `volume` filter absent from the selected tool
build. Its source/log/raw failure remain explicitly superseded: the encoder
failed before producing a native-validator subject, so that attempt is never
counted as a passed native PCM refusal. No tools, product source or binary were
changed to repair the observation.

## Reproduce and limits

Use the same captured CLI and pinned audio tool pair with a fresh work directory
whose canonical parent exists. The observer reuses the preceding precision
observer and existing authored-audio capture helper byte-for-byte:

```sh
python3 scripts/observations/check_audio_layout_candidate.py \
  --binary /absolute/path/to/captured/renderflow \
  --audio-tool-directory /absolute/path/to/pinned/audio/bin \
  --work-directory /absolute/path/to/new-layout-observation
```

Only these short authored direct original-source profiles are exercised. Source
bytes and previous captures/proofs remain unchanged. PCM fidelity excludes
container metadata, bitrate, perceptual/listening quality, mastering, loudness,
unknown upstream history and human approval. No hosted CI, complete runtime or
strict Clippy matrix, native Mac/physical volume, private media/model, signing,
release/content or Pages publication occurred. Broader #367/#419 acceptance stays
open; Flow #88 remains unchanged and scheduled last.
