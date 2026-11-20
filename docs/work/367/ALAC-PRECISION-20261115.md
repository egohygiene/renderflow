# Requested ALAC precision checkpoint — November 15, 2026

The existing profile argument owner now enforces explicitly selected ALAC
precision: `s16p` for 16-bit and `s32p` for ALAC's 24-bit representation. The
actual canonical CLI accepts 24→16-bit `alac_cd` as a declared lossy conversion;
matching 16-bit input remains exact PCM lossless. Codec names, defaults without
an explicit depth, catalog/planning fidelity and native acceptance rules remain
with their existing owners. No new converter or provider is added.

The earlier [native-audio checkpoint](NATIVE-AUDIO-VALIDATION-20261115.md) correctly
refused the then-unenforced precision. Its dated failure, measured binary and
receipts remain unchanged. The [current guide](../../user-guide/native-audio-validation.md)
describes the repaired profile behavior rather than presenting that earlier
failure as the current encoder contract.

## Exact source and candidate

The measured source is actual published draft head
`4fb2539fa686e966143be69b711ffeeccc59d3b4`, tree
`22d8809c8386050bd1734280c69561562aef8d11`, based on merged main
`7f02a0e357ef3ffcdaabeb805d7eff708eb4667d`. After compilation, changes only add
the observer, portable receipts and documentation; later proof tips are not new
compiled identities.

| Captured subject | Bytes | SHA-256 |
| --- | ---: | --- |
| CLI | 20,151,576 | `d7b2f03670ed1262c9dfab67c9cf64a0314c7c9fb190d0ffc4a14e87ca96ca54` |
| Compiled release contract | 372,295 | `199bf790b2d78e7706b1e24b3e0fdb189b8286eb231f8be1c8da503d82922f58` |

The Rust 1.94 release build passed in 307.786 seconds, with the existing private
warm cache, two jobs, locked/offline dependencies and no incremental/debug data.
The source was clean before and after the invocation. Its immutable build record
truthfully captures the then-pending runtime state; the later passed proof is a
separate receipt. No second independent build is claimed. The host is Debian 13,
x86-64 GNU, not a newly qualified Ubuntu/Mac/Windows release target.

- [Source/build/check bindings](alac-precision-bindings-20261115.json).
- [Final actual CLI observation](alac-precision-observed-20261115.json): six
  canonical build invocations and 15 independent feature/probe/decode commands,
  21 commands total. All 87 retained input/output/observation files and 42 command
  streams are pinned. Each of the six current raw manifests also has an exact
  separate retained byte copy; no historical baseline was overwritten.

## Actual outcomes

| Selected case | Observed result |
| --- | --- |
| PCM24, 44.1 kHz stereo → `alac_cd` | Valid 16-bit ALAC; lossy; every decoded sample equals the signed source integer shifted right eight bits |
| Same plan's AAC M4A | Valid lossy AAC, distinct target/artifact/file identities despite the same `.m4a` suffix |
| PCM16, 44.1 kHz stereo → `alac_cd` | Valid, exact precision/rate/channels/count/every decoded byte; lossless |
| PCM16, 44.1 kHz stereo → `alac_broadcast` | Valid requested 24-bit/48-kHz conversion; lossy |
| Existing reject-lossy policy on 24→16 | Native check valid, observed fidelity lossy, no accepted publication |
| Deliberately precision-omitting producer | Actual 24-bit ALAC; required Invalid `audio.native.profile.layout`, no accepted publication |
| Native policy omitted | Existing generic signature warning and conservative lossy declaration retained |

The independent quantization comparison does not compare two conversion paths.
The observer reads every original signed 24-bit integer and writes its signed
high 16 bits as expected PCM. All 529,200 decoded bytes match that projection;
259,308 original sample values have nonzero discarded low bytes. Quantization
is therefore an actual irreversible change, not a codec-label lossless claim.

The producer fault is test-only and follows a real accepted source/profile
baseline. It removes the encoder precision option, records exactly one actual
producer invocation and retains the wrong-depth payload for independent probe.
The selected absolute FFmpeg/FFprobe verifiers remain their separately pinned
real 8.0.1 bytes; their hashes do not attest the producer or confer approval.

An initial observer attempt reused an already correct cached payload, so the
faulty producer never ran. That failed observer assertion, old script and raw
manifest remain explicitly superseded in the bindings. The final observer gives
each producer-fault case a fresh isolated CAS parent; it does not weaken valid
cache acceptance or count an unrelated error as a native refusal.

## Reproduce and scope

Use the exact captured CLI and separately prepared pinned audio tool pair. The
work directory must be new with an existing canonical parent:

```sh
python3 scripts/observations/check_alac_precision_candidate.py \
  --binary /absolute/path/to/captured/renderflow \
  --audio-tool-directory /absolute/path/to/pinned/audio/bin \
  --work-directory /absolute/path/to/new-precision-observation
```

Original files, accepted binaries and old evidence remain unchanged. Native
fidelity covers selected-WAV PCM essence/clocks only, without container metadata,
unknown upstream history, bitrate/compression, mastering, loudness, perceptual
or creative approval. No hosted CI, full runtime/Clippy/native-platform matrix,
private media/model, native Mac/physical volume, signing or release/content/Pages
publication occurred. Broader #367/#419 acceptance remains open; Flow #88 remains
unchanged and scheduled last.
