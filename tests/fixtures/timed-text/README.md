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

The integration tests derive malformed cases in memory and write only to
temporary directories. They require no FFmpeg installation, speech model,
network access, or source media.

The tests and fixtures are authored coverage. Execution is deferred under the
current implementation sprint; their presence is not qualification evidence.
