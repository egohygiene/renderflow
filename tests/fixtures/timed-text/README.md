# Synthetic timed-text fixtures

SPDX-License-Identifier: MIT

These tiny subtitle examples are original Renderflow test material, authored
for issue #427. They contain no private media, scraped captions, recordings,
or externally licensed dialogue. They are distributed under the repository's
MIT license.

- `plain-unicode.srt` contains two non-overlapping cues with deliberately
  nonconsecutive numeric identifiers, multiline Unicode text, and millisecond
  timings. Coverage also exercises these bytes with CRLF line endings.
- `identified.vtt` contains the same cues with meaningful WebVTT identifiers.

The pure integration tests derive malformed cases in memory and write only to
temporary directories. They require no FFmpeg installation, speech model,
network access, or source media.

The separate ignored `timed_text_native` target authors a local qualification
run using these original text fixtures, tiny generated silent audio, and the
negative fixture at
`crates/renderflow-core/tests/fixtures/timed-text/native/unsupported.ass`.
It requires explicitly selected FFmpeg and
FFprobe version lines and a fresh output directory. When deliberately run, it
retains generated containers and per-case evidence for review. It does not
download media or install providers. See the
[native qualification guide](../../../docs/work/427/NATIVE_QUALIFICATION.md).

The Rust tests remain authored coverage: the completion pass was authorized,
but its host lacked Rust/Cargo. Independent FFmpeg command smoke using these
fixtures passed; it does not establish Rust adapter or SDK qualification.
See the [validation record](../../../docs/work/427/VALIDATION.md).
