# Subtitle completion pass — checkpoint 4

**Partial validation; Rust completion is environment-blocked.** The user
authorized focused local checks on 2026-10-12 UTC (2026-10-11 America/New_York).
This record replaces the earlier blanket deferral for those checks. Lint,
formatting, full-suite checks, GitHub Actions, merge, and release remain outside
this checkpoint.

The source under inspection was published commit
`c77e26c6b911e2b87585848022ccfdb7d7bb6565`, tree
`396518c863f30a2cd01eda5fec241377cb8c10d4`, matching local commit
`a920b97608e42e1554cd05b218d057d8c01e59ac`. This checkpoint changes only
documentation and retained validation material, not product code or schemas.

## Executed results

| Scope | Observed result | What remains unproven |
| --- | --- | --- |
| Focused Cargo command | Unavailable: shell exit 127, `cargo: command not found` | Compilation and all 61 authored Rust test functions |
| Independent FFmpeg/FFprobe command smoke | 12/12 checks passed on Linux x86_64 | Rust process adapter, typed errors, limits, cancellation, and SDK behavior |
| Five JSON schemas | Draft 2020-12 self-validation passed | Agreement with actual Rust-serialized output |
| Authored schema samples | 66/66 expected accept/refuse assertions passed | Runtime model validation and graph execution |
| Source review | No concrete compiler/API or provider defect identified | Compiler/runtime correctness |

No product changes were justified by these results. No user media was used.
No FFmpeg/provider/model installation, repository dependency change, lockfile
change, remote CI run, merge, or release occurred.

## Native command smoke

The installed provider version lines were:

```text
ffmpeg version 6.1.1-3ubuntu5 Copyright (c) 2000-2023 the FFmpeg developers
ffprobe version 6.1.1-3ubuntu5 Copyright (c) 2007-2023 the FFmpeg developers
```

A one-off Python subprocess driver exercised the argument recipes in
`timed_text_native.rs` and `timed_text/provider.rs`. Its twelve checks were:

- Four warning-free synthetic mux recipes: mixed SubRip/WebVTT Matroska,
  audio-only Matroska, ASS Matroska, and an MP4 `mov_text` refusal fixture.
- Four exact subtitle inventories, including nonzero absolute indices,
  languages, unsupported codec identities, and an empty subtitle inventory.
- Two stream-copy extractions preserving expected timing and Unicode text;
  WebVTT cue IDs were also retained. SubRip numbering is regenerated.
- Two provider-version stability checks before/after those operations.

FFmpeg normalizes this WebVTT fixture's timestamps from `HH:MM:SS.mmm` to
`MM:SS.mmm`. The first byte comparison exposed this representation difference;
semantic comparison of retained IDs, integral millisecond times, and text
passed. Both forms are explicitly handled by the authored Rust parser, which
has not yet executed. The trace records the byte mismatch and semantic result.

[Native command trace](evidence/checkpoint-4/native-provider-smoke.json) contains
the actual argv, exit codes, stdout/stderr, observations, and artifact hashes.
Its SHA-256 is
`17693ea0a14f6d76a8b4d095b394f8b815f0f43273959dcd0c7e98c5e211e8f9`.
Absolute paths identify the temporary execution directory. Generated media
remains transient; binaries are not committed. This is a diagnostic smoke
record, not a complete retained Rust qualification bundle.

**These twelve checks are not the Rust harness's ten qualification cases.**
Python subprocess execution does not validate `ProcessExecutor`, the codecs,
the SDK, typed refusals, or canonical provenance serialization. No
`renderflow.subtitle-qualification/v1` passing report was produced.

## Schema evidence

An isolated scratch installation of `jsonschema==4.25.1` checked the transcript,
inspection, export, qualification, and `renderflow/v2` schemas. Nested schema
references were resolved locally. Positive and negative authored samples
exercise transcript fields, strict text constraints, export nesting,
inspection shapes, qualification prerequisites/case order, and subtitle policy.

[Schema results](evidence/checkpoint-4/schema-validation.json) retain schema
hashes and each assertion. The [executed script](evidence/checkpoint-4/validate_schemas.py)
is retained unchanged. Its synthetic passing-report sample tests the schema
shape; it is explicitly not evidence of a successful native run.

On a host with `jsonschema==4.25.1` installed, reproduce from the repository root
using a fresh evidence output path:

```bash
python3 "docs/work/427/evidence/checkpoint-4/validate_schemas.py" --repo "." --evidence-json "/absolute/path/to/new-schema-results.json"
```

After the real Rust native kit runs, add
`--native-dir "/absolute/path/to/retained-native-run"` to check the actual
qualification report and extracted/SDK transcript JSON as well. This optional
mode has not run yet. Schema acceptance alone does not assert that a report's
qualification status is `passed`.

## Toolchain blocker and exact resume point

The attempted command was:

```bash
cargo test --locked --package "renderflow" --lib timed_text
```

It exited 127 before any project code could run. Neither `cargo`, `rustc`, nor
`rustup` is installed in the available runtime. An ordinary request to the
official Rust 1.94 distribution endpoint timed out after 15 seconds. The default
package-manager bootstrap also failed because this environment cannot perform
the required user/group process operations. No access controls were changed.

Resume in this branch on a host with the repository's Rust 1.94 baseline:

```bash
cargo test --locked --package "renderflow" --lib timed_text
cargo test --locked --package "renderflow" --lib subtitle
cargo test --locked --package "renderflow" --test "timed_text" --test "timed_text_graph" --test "timed_text_native"
cargo run --locked --package "renderflow-cli" --bin "renderflow" -- subtitles --help
```

The third command runs the native target's report-helper tests while leaving
the native qualification entry point ignored. Next execute the explicit pinned
command in [NATIVE_QUALIFICATION.md](NATIVE_QUALIFICATION.md), retain its whole
output directory, validate its actual JSON with the script above, and inspect
the CLI's conversion/refusal behavior. Fix concrete failures before updating
acceptance status. General lint/full-suite/release work remains deferred.

#427 and draft PR #438 remain open. This checkpoint did not prove the shared
transcript/SDK contract. The user's subsequent decision, recorded in the
[handoff](HANDOFF.md), permits #428 draft implementation while those tests stay
deferred; it does not convert this partial evidence into a completed gate.
