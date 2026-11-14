# Selected source and CLI results — November 14, 2026

Relates to [Renderflow #367](https://github.com/egohygiene/renderflow/issues/367).
These observed checks follow the held subtitle-input and checked audio-profile
source checkpoints. They do not close the broader format/provider matrix.

The combined source revision is `c8f2b6896ec82c12e75ad6b8bcf7e4b319602db2`,
tree `e3a7b4ab29930dd55154a2b9a71e0f445454a08d`, based on draft #517 at
`de21aff8ddd6e6d9d1a4762551567c6c96812147`. Rust 1.94.0's locked release CLI
build passed with two jobs. Its SHA-256 is
`e6a182eb2d1381301312f6f37a725620a988dc761cb972a0a9b19c5d7d6cbe82`.

## Executed focused source cases

```sh
cargo +1.94.0 test --locked --offline --jobs 2 --package renderflow \
  --test subtitle_input_admission --test timed_text
```

All ten cases passed: five exact owning subtitle command cases and five
existing public timed-text cases. The owning-source integration harness
includes the real command module; it does not copy its capture implementation.
The selected cases cover deterministic exchanged symlink/FIFO/different-inode
trials, early oversized rejection, exact output publication, existing-output
preservation, Unicode timing, malformed-input refusal and JSON loss metadata.
Linux executed the Unix-specific races; other native platforms remain unrun.

The earlier `--lib commands::subtitles::tests` attempt stopped at compilation:
two preexisting E0061 calls in `planning/forest_execution_tests.rs` (208, 326)
still pass twelve arguments to `artifact_evidence`, which now takes a context;
an E0716 at `graph/capability.rs:1806` borrows a temporary registry. No library
unit tests ran from that attempt, including the new audio unit cases. This
sprint does not repair that unrelated test debt. The final CLI and selected
integration build emitted only the existing unread `PcmWavInspection.frame_count`
warning. Strict Clippy and broad tests were not run.

## Executed separate CLI processes

Seventeen bounded command probes passed. Authored CRLF Unicode SRT converted
to canonical JSON, then WebVTT and plain text. IDs 7/9 and times 1,250–3,750 /
5,000–6,125 ms were checked; the text export explicitly discloses loss of
provenance, overlap policy, timing and IDs. Reusing a destination or the source
as output refuses and preserves bytes. Direct symlink, FIFO with no writer and
sparse 16 MiB + 1 input refuse without an output; the deterministic race cases
above test exchanges between observation and open.

An entirely generated WAV exercised canonical audio planning. Explicit typo,
empty and whitespace profiles refuse both dry-run and real selection with the
bounded `unknown audio quality profile` error. Known `hq_320k`, alias `HQ-320K`
and absent profile each produce a valid MP3 plan in dry-run. No output directory
is created, and source/executable hashes remain unchanged. This checks actual
canonical admission; the retained permissive `build()` compatibility behavior
is source-covered but its unit cases remain blocked by the library test debt.

Valid plans performed provider discovery/version probes, without conversion.
The observed host FFmpeg was `7.1.5-0+deb13u1`. The registry declares minimum
4.0.0; this planning observation does not qualify codec execution, a pinned
native-provider matrix, loudness or perceptual quality.

[The executed receipt](evidence/20261114/selected-checks.json) records the
selected test executables, command argv/status/output hashes and input/output
hashes. Committed argv operands use portable fixture placeholders; the original
private CLI/combined receipt hashes are retained. Raw logs, generated files and test
executables remain in the private task directory. Only the inactive task-owned
test build cache was removed after preserving both executables and hashes.
Release/publication authority, human timing/audio review and broader native
qualification remain unchanged.
