# Pinned native audio validation

Select `execution.validation.audio` to require fresh native checks of the
supported direct WAV audio derivatives. This checks actual immutable source and
output payloads before publication; cached encoder results receive the same
check again. Without this option, existing generic validation behavior remains.

```yaml
execution:
  validation:
    audio:
      profile: pcm-delivery-ffmpeg-8.0.1-v1
      ffmpeg:
        path: /absolute/path/to/ffmpeg
        sha256: <exact-lowercase-64-character-sha256>
      ffprobe:
        path: /absolute/path/to/ffprobe
        sha256: <exact-lowercase-64-character-sha256>
```

The closed policy pins **verification** executables and requires their observed
version to be 8.0.1. It does not download tools, discover them on PATH, or attest
the encoder executable: the existing strategy/toolchain still owns encoding.
Both verifiers must also satisfy the existing tool allow/deny and execution
requirements; a disallowed verification tool is not read or launched.
Executable hashes do not attest linked native libraries or provide an OS sandbox.

Before media operations, each verifier advertises the exact required demuxer and
decoder IDs. The FFmpeg verifier must also advertise the selected raw PCM encoder
and muxer. Queries are bounded and hashed, comma-separated aliases are normalized,
and independently pinned FFprobe capabilities are checked separately. Missing,
unreadable or unrecognized feature evidence is unavailable; a version string alone
does not grant support. This is a fixed-profile prerequisite check, without a new
global registry or quality claim.

The supported initial profile starts with an original, native integer PCM16/24
RIFF/WAVE source. Its direct built-in output routes are:

| Target format | Native scope |
| --- | --- |
| `flac` | FLAC with native 16/24-bit decoded PCM |
| `m4a_alac` | ALAC in M4A with native 16/24-bit decoded PCM |
| `mp3` | MPEG-1 sample clock: 32, 44.1 or 48 kHz |
| `m4a` | AAC LC in M4A with a 1,024-sample frame clock |

Unsupported source, route, precision, provider version or resource bounds remain
`unavailable`. Malformed payloads, wrong codecs/profile layouts/clocks and
mismatched samples under a source-preserving lossless profile are `invalid`.
Selecting this option requires the native check: either state blocks that target,
including with generic `required: false` or `allow_unavailable: true`.

Custom validators still run. With an empty/default list, nonempty and the actual
native check replace the signature-only default. An explicit custom signature
validator keeps its own warning. Requesting the contextual native validator ID
`validator.audio.pinned_pcm_delivery` executes that configured check once.

## Fidelity of each actual artifact

`Lossless` requires a lossless codec **and** matching original precision, rate,
channels, frame count and every decoded PCM sample byte. This describes decoded
PCM essence and clocks; container/ancillary metadata equality is not asserted.
An ALAC/FLAC label, extension, requested profile or cached metadata cannot grant
that result. The native proof is joined to the concrete target identity, profile,
artifact/digest and producing step/configuration.

A selected CD-precision, resampling or channel-change profile stays `Lossy` for
its declared PCM changes. AAC and MP3 stay `Lossy`, including a rare decoded
sample equality. Native checks do not attest encoded bitrate/compression quality,
perceptual equivalence, mastering, loudness, playback or content approval.
Planned route scores and catalog support tiers remain conservative.

Only direct original-source routes are admitted. Earlier transform lineage or a
multihop route is unavailable in this bounded profile; a later local lossless
encode cannot erase earlier loss. Unknown history before the explicitly supplied
source is not reconstructed or qualified.

## Clocks and bounds

The evidence records raw stream duration, packet extent, declared skip/discard,
reconciled presentation and decoded frame count separately in integer samples.
Equal-rate and integral resampling clocks must match exactly; a fractional
resampling boundary may use only its exact floor/ceiling sample count.

MP3 requires 1,152-sample packets and reconciles bounded leading/trailing skip.
AAC LC requires all interior 1,024-sample packets, its declared 1,024-sample
priming block, and the exact decoded frame-rounded extent. Its nonpresented tail
is recorded rather than silently trimmed or padded to force equality.

The source and encoded output each have a 32 MiB cap, decoded PCM a 64 MiB cap,
and presentation a 600-second cap. Native processes use the existing bounded
ProcessExecutor: 30-second probe/decode timeouts, 4 MiB probe capture, small error
captures, one codec/filter thread, file-only protocols, forced demuxers and disabled
MOV external references. A decoder byte sentinel is a refusal guard; clipped
output is never accepted as a successful equivalence proof. These restrictions
are not an OS memory/disk/network sandbox or a platform qualification matrix.

`renderflow.audio.native_validation` in each terminal artifact carries observed
facts and hashes. Raw decoder temporary files are removed; original/CAS output
lineage remains, so an explicitly selected verifier can reobserve the result.
Use the current run's accepted target outcomes: old files can remain after a
refused rerun, and file existence alone is not a new accepted publication.
