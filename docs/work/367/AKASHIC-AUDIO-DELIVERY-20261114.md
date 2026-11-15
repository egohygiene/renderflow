# Authored WAV audio delivery candidate — November 14, 2026

This checkpoint selects existing canonical audio strategies and explicit quality
profiles. It uses two authored integer PCM fixtures, each three seconds, stereo
44.1 kHz: 24-bit WAV for FLAC/MP3/AAC-in-M4A and 16-bit WAV for ALAC-in-M4A.
`alac_cd` explicitly selects 16-bit/44.1 kHz; this does not promise preservation
of a 24-bit/44.1 kHz original through that profile.

## Native failure and scoped repair

Actual pinned FFmpeg 8.0.1 runs completed the FLAC (`flac_max`), MP3 (`hq_320k`)
and AAC-in-M4A (`aac_256k`) routes. The declared `m4a_alac` / `alac_cd` branch
failed with native FFmpeg exit 234 because the strategy staged a file called
`output.m4a_alac`; FFmpeg could not infer its physical container.
The failed native run receipt and original stderr are retained separately.

The existing strategy adapter now asks `AudioFormat::file_extension()` for audio
staging suffixes; the ALAC canonical bundle suffix also uses that owner's `.m4a`.
Graph format remains `m4a_alac`, selected output type remains ALAC, and its
explicit profile still selects codec `alac`. No AAC-format/profile substitution
or extension-only validation workaround is used.

Two focused Rust regression tests passed: physical staging/container suffix and
explicit codec identity, plus canonical ALAC suffix while its graph key remains
distinct. Final actual ALAC execution and all selected audio observations passed
on public draft source `848984ab363058b2b765f4056ed01bde9f2b54d6`
(tree `87cc8cc9e1dd55095603376ba523a52bf6f4df9a`). The clean-source Linux
release build completed in 5m17s with Rust 1.94.0, locked offline inputs and two
jobs, reusing the existing private target cache. Binary SHA-256:
`61ed02b8a0b5396ea1685c3131ae29f75f943a355668a98106e93fda0e28b942`.
Compiled contract SHA-256 remains
`9ff99a410bcf98a82801eeadf3e35fdc40539c36d192ce066365769c98e9aa74`.
This is the current Debian-host candidate, without Ubuntu/native-Mac qualification.

## Actual selected delivery results

Two plans published five artifacts across four formats. Each source contains
132,300 stereo frames at 44.1 kHz (three seconds). The second plan deliberately
selects AAC and ALAC together: both files use `.m4a`, with distinct graph formats,
branches, roles, artifact identities, codecs and output paths.

| Source | Declared target / profile | Actual codec | Decoded frames | Exact authored samples |
| --- | --- | --- | ---: | --- |
| 24-bit WAV | FLAC / `flac_max` | FLAC, 24-bit | 132,300 | Yes |
| 24-bit WAV | M4A / `aac_256k` | AAC | 133,120 | No; lossy |
| 24-bit WAV | MP3 / `hq_320k` | MP3 | 132,300 | No; lossy |
| 16-bit WAV | M4A / `aac_256k` | AAC | 133,120 | No; lossy |
| 16-bit WAV | `m4a_alac` / `alac_cd` | ALAC, 16-bit | 132,300 | Yes |

FLAC and ALAC preserve exact decoded sample bytes, channels, rate, bit depth,
and zero-to-three-second stream/packet coverage for these matching profiles.
This does not imply that a different profile which resamples or changes precision
preserves the original samples.

MP3 packets cover approximately 3.030204 seconds. The actual leading skip is
1,105 samples and trailing discard is 227; declared gapless presentation and
decoded frame count reconcile to the original three seconds. AAC/M4A declares
1,024 priming samples and three-second presentation. Decoding retains an 820-frame
padded tail (less than one 1,024-sample AAC frame); the observer does not silently
trim or pad files to manufacture equality. Raw stream duration, packet extent,
skip/discard, presentation and decoded sample count are recorded separately.

The final observer passed 18 selected commands: two provider version observations,
two Render plans and two builds, two source probes, and five output probe/decode pairs.
It binds native source hashes/lineage, forest branch/profile/role/identity,
provider selection under the controlled PATH, every published artifact, probes,
decoded PCM and retained command streams. Source WAV/config and selected
executable/observer hashes are checked again afterward. Output captures are
bounded during execution (4 MiB, 60 seconds); owned process groups are cleaned up.

Native validation remains `valid_with_warnings` / declared-signature-only.
Generic audio graph edges also conservatively report native fidelity `lossy`,
including these sample-equal FLAC/ALAC results. Both facts remain visible in the
receipt; independent actual decoder observations do not rewrite product tiers.
The same final candidate also passed the 21-command manual-lyrics observer,
including all eight formats, 17 exact refusals, and retained-store portable
export/verification (nine records/eight publications).

The [audio observation](akashic-audio-delivery-observed-20261114.json),
[tool preparation](audio-tool-preparation-observed-20261114.json), and
[same-candidate lyrics regression](akashic-lyrics-after-audio-observed-20261114.json)
are portable projections with original private report/log/content hashes.
The immutable build capture truthfully said runtime pending at capture time;
these separately bound observations occurred afterward.

## Provider preparation

A separate task-owned audio-only FFmpeg/FFprobe 8.0.1 pair was built from the
verified official 8.0.1 source archive. Official source archive SHA-256:
`05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41`.
Its detached signature was verified with FFmpeg key fingerprint
`FCF986EA15E6E293A5644F10B4322F04D67658D8`.

The private build links Debian LAME 3.100-6+b3 static development objects;
package hashes were checked against the Packages index bound to a verified
Debian trixie InRelease. No system package installation was performed. The
preserved video-capable Ani FFmpeg pair is unchanged; this audio-only build
must be selected per Render invocation, while Ani keeps its explicit video tools.

Actual audio FFmpeg SHA-256:
`94dbc5c9d7602f044b0ec9089cea2085d0f2adc3a2b1612eeec84f8badc6949d`.
FFprobe SHA-256:
`40d68a7e8fc61e8656be62b12179142124ebd61eea5ed8afefda6e2d68b97a1d`.
The build exposes native FLAC/ALAC/AAC and libmp3lame encoders, matching decoders,
PCM raw output and required WAV/FLAC/MP3/MP4 muxers. Configure reports LGPL 2.1
or later; exact source/dependency notices and preparation receipts are retained.

The observer compares decoded FLAC/ALAC samples exactly, separates
lossy raw stream/packet/presentation/decode clocks and priming/padding, rechecks
original bytes, and binds native source/forest/profile/provider evidence. Native
signature-validation warnings remain visible; independent decoding does not
change product support tiers. There is no mastering, loudness, perceptual,
physical playback, native-platform/full-provider matrix, signing or public
release approval from these local synthetic operations.

## Reproduce the bounded proof

Select the captured CLI and the separately prepared audio-only tools. The work
parent must exist and the selected work directory must be new. Python 3 uses
only its standard library and generates both authored WAV fixtures:

```sh
python3 scripts/observations/check_audio_delivery_candidate.py \
  --binary /path/to/renderflow \
  --audio-tool-directory /path/to/audio-tools/bin \
  --work-directory /path/to/existing-parent/new-audio-proof
```

The observer emits actual native configs in `inputs/`, accepted artifacts and
run manifests in `outputs/`, and retained probe/decode/command evidence in
`observed/`. Its selected source/rate/precision/profile constraints are deliberate;
change them only with a new scoped observation rather than retaining these results.

To prepare the provider elsewhere, verify the exact official source archive and
detached signature, verify/extract the two pinned Debian LAME packages privately,
and configure FFmpeg with the portable argv in the preparation receipt, followed
by `make -j2` and `make install` under a private prefix. The receipt retains the
exact source/index/package/notice/log hashes and actual compiled feature set.
It records the observed build and does not promise cross-host binary reproducibility.
