# Checked audio profile configuration

Relates to [Renderflow #367](https://github.com/egohygiene/renderflow/issues/367).
This checkpoint starts from the current local-CI draft #517 at
`de21aff8ddd6e6d9d1a4762551567c6c96812147`.

The audio argument builder previously converted any unrecognised explicit
profile into format defaults. A misspelling such as `hq_320kk` therefore did
not select the quality the configuration appeared to request. Canonical
planning also admitted that profile because adapter construction did not
validate it; dry-run never calls the transform's render method.

The change adds a checked argument-builder entrypoint and one shared profile
resolver. The canonical audio adapter validates the selection during
construction, and the audio strategy uses checked arguments before either its
direct dry-run or provider path. Unknown and empty explicit selections produce
a bounded configuration error; the invalid value is not echoed into diagnostics.
No external conversion runs for that rejected selection.

Omitting the profile retains existing format defaults. Valid identifiers,
case/hyphen aliases and resulting FFmpeg argument vectors remain unchanged.
The existing public `FfmpegAudioArgs::build() -> Vec<String>` retains its
historical permissive behavior for SDK compatibility; the new
`try_build() -> Result<Vec<String>>` is the checked configuration path. No
profile-family compatibility redesign, codec preset change, loudness DSP or
new provider is introduced.

Five new focused cases cover rejected explicit typo/empty selections, valid/absent
argument equality, strategy rejection before a
provider for real/direct-dry runs, valid provider-free dry-run and canonical
adapter admission. The old legacy-fallback case remains in place alongside
those new cases.

## Checks and limits

The owning Rustfmt 1.94 source check and Git whitespace check pass in the
isolated authoring worktree. No Rust compilation/test or provider process was
run by the authoring subtask. The parent checkpoint owner will run selected
tests and the canonical CLI after combining the source fixes; this document
does not claim those results in advance.

Suggested focused selectors under the existing locked package contract:

- `cargo +1.94.0 test --locked --package renderflow --lib audio::ffmpeg::tests`
- `cargo +1.94.0 test --locked --package renderflow --lib audio::strategy::tests`
- `cargo +1.94.0 test --locked --package renderflow --lib adapters::strategy::tests`

The canonical CLI probe should reject an explicit typo on both dry-run and
real selection before conversion, preserve absent/default behavior and retain
the exact known-profile configuration. Full Rust/provider/native/release
qualification, human audio review and publication remain separate. This
checkpoint does not establish loudness or perceptual-quality compliance.
