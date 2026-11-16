# Requested ALAC precision checkpoint — November 15, 2026

This checkpoint repairs the existing audio profile argument owner. Explicit ALAC
16-bit profiles select FFmpeg's `s16p` input format; 24-bit profiles select its
`s32p` input format, which the ALAC encoder stores at 24-bit precision. Other
codec arguments, defaults without an explicit depth and native acceptance rules
remain with their existing owners. No new converter or provider is added.

The previous native-audio checkpoint found that a 24-bit source requested as
`alac_cd` retained 24-bit precision, and correctly refused it as
`audio.native.profile.layout`. Its original failed observation remains history.
Requested sample conversion must meet its profile and remain `Lossy`; only
matching original precision/rate/channels/count/sample bytes can earn observed
PCM `Lossless`. Native acceptance must continue to reject a producer that fails
the requested layout.

## Source preparation status

The source change is based on actual main
`7f02a0e357ef3ffcdaabeb805d7eff708eb4667d`, tree
`8ab4e527eebb86fad73dceb008cf3f5127b97781`. The separately preserved pinned
FFmpeg 8.0.1 encoder advertises both `s16p` and `s32p`. The new public-source
CLI build, native precision/PCM proof and retained refusal controls are pending;
their actual source, executable, contract and observation identities will be
appended to this same draft. Authored source is not a passed runtime claim.

This is a bounded local candidate checkpoint under #367. Native platform/release,
mastering, loudness, perceptual and creative approval remain separate. No hosted
CI, private media/model, native Mac/physical volume, signing, release/content or
Pages publication is selected. Existing immutable candidates and original proof
bytes remain unchanged; Flow #88 remains unchanged and scheduled last.
