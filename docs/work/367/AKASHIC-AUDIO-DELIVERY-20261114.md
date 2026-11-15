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
distinct. Final actual ALAC execution and the full source/profile/tool/decoded
sample proof remain pending at this source-preparation checkpoint.

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

The final observer will compare decoded FLAC/ALAC samples exactly, separate
lossy raw stream/packet/presentation/decode clocks and priming/padding, recheck
original bytes, and bind native source/forest/profile/provider evidence. Native
signature-validation warnings remain visible; independent decoding does not
change product support tiers. There is no mastering, loudness, perceptual,
physical playback, native-platform/full-provider matrix, signing or public
release approval from these local synthetic operations.
