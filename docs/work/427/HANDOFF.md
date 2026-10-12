# Renderflow #427 — timed-text checkpoint 4

## 2026-10-12 merged status

The user subsequently authorized merging the stack. PR #438 merged at
`425d0ea3ae499ffe04b27c688d7ea84fa890b8dc`, followed by PR #439 onto `main`.
#427 remains open for deferred Rust/SDK qualification. No additional tests,
builds, lint, schema checks, native runs, or CI were executed as part of merging.
Implementation continues in existing [#429](../429/HANDOFF.md). This status
supersedes the historical draft statements below; earlier checkpoint and
validation records remain preserved.

## Scope and observed state

- Parent: https://github.com/egohygiene/renderflow/issues/427 (under #421).
- Role: implementation, with focused authored coverage and necessary interface notes.
- Strategy: Aether worker-strategy 0.1.0 at
  `db5f4bd339f9266758c74c3435d47c8005f89a8e`, with specfile,
  repository-continuity, and local-validation-evidence dependencies at that ref.
- Observed 2026-10-12 UTC: #427 open; draft PR #438 holds this work.
  Checkpoint 3 began from published head
  `ff10b8cc866ba421e6017e078d83afb27845e0e5`, tree
  `2ca3e2fbedca41b65fdee94430e6120b3970f011`; main remained at the base below.
- Verified base: `8e866f91aa8b20255f25a553031f25fa98aa3856` (`main`).
- Candidate branch: `feat/renderflow-427-timed-text`; obtain its exact head from
  the draft PR. This file cannot contain its own eventual commit SHA.
- Checkpoint 4 began from published head
  `c77e26c6b911e2b87585848022ccfdb7d7bb6565`, tree
  `396518c863f30a2cd01eda5fec241377cb8c10d4` (matching local commit
  `a920b97608e42e1554cd05b218d057d8c01e59ac`). The user authorized the focused
  local completion pass; lint and GitHub Actions remain deferred.
- Existing SRT/WebVTT format declarations were present, but no subtitle parser
  or extraction adapter existed. No root AGENTS.md or CONTINUITY.md was present.

## Checkpoint outcome and interfaces

Checkpoint 1 authored a standalone subtitle workflow: strict SRT/WebVTT
parsing, timed-transcript JSON, explicit stream inspection/selection, bounded
local extraction, and no-overwrite output publication. Checkpoint 2 connects
that workflow to opt-in canonical graph execution and the existing SDK.

- `src/timed_text/` (under `crates/renderflow-core`) owns the portable model,
  pure codecs, typed errors, validation, and bounded provider adapter.
- `schemas/renderflow-timed-transcript-v1.schema.json` owns the canonical wire shape.
  Companion subtitle inspection/export schemas describe stdout evidence.
- `src/commands/subtitles.rs`, `src/cli.rs`, and `src/app.rs` expose
  `subtitles inspect`, `convert`, and `extract`.
- Format/tool registries identify additional subtitle formats and optional
  providers. Registration does not imply qualification.
- Focused tests and original synthetic fixtures are authored with the feature.
  Checkpoint 1 added 31 test functions (9 codec, 11 provider, 5 integration,
  2 publication, 4 registry), plus two expanded existing registration tests.
- [User guide](../../user-guide/subtitles.md) describes the authored interface
  and limits. All command examples are unrun.

Checkpoint 2 interfaces:

- `execution.subtitles: {}` enables built-in SRT/WebVTT → timed-transcript JSON
  → SRT/WebVTT routes. Omission leaves those built-in routes disabled. Both the
  Rust model and checked-in `renderflow/v2` schema describe the optional policy.
- Media sources require `execution.subtitles.stream_index`, the absolute
  FFprobe index. Graph input is one MKV/WebM/MP4/MOV artifact; collections,
  unsupported formats, and stream indices on text inputs are refused.
- `src/timed_text/graph.rs` owns bounded artifact transforms and source lineage.
  Canonical JSON is a first-class intermediate or requested terminal artifact.
  Its output extension is `.timed-transcript.json`; generic JSON is unchanged.
- `src/planning.rs` registers opt-in edges and binds native transforms during
  execution so the SDK cancellation token reaches the process executor. The
  existing SDK request types remain sufficient; no new public request fields.
- Native extraction reads and hash-checks the immutable store object with a
  trusted container format, despite its extensionless filename. It does not
  reopen the original source or stage another full media copy for a suffix.
  Normal source import still consumes storage for an artifact-store copy.
- Native extraction is noncacheable and noncheckpointable while actual tools
  are reprobed rather than pinned to the plan. Actual tool versions remain in
  transcript provenance; planned toolchain evidence is a separate snapshot.
- `validator.core.timed_text` parses SRT/WebVTT/canonical JSON under the strict
  profile with bounded reads. DAG failures retain typed subtitle diagnostics,
  including cancellation state instead of a generic failure.
- Native manifest metadata under `renderflow.subtitle` retains input hashes,
  transcript provenance, selected stream, provider evidence, prior losses, and
  export losses.
  Flow's closed `flow.artifact/v1` projection carries digests/source IDs only;
  details remain in canonical JSON and the native manifest. No Flow wire fields
  were added. Plain-text output remains standalone CLI-only.
- Checkpoint 2 adds 27 focused tests: 8 policy/planner/intake, 7 graph/provider,
  6 validation/DAG diagnostics, and 6 SDK integration tests. One existing
  registry test is expanded. All remain unrun; the two checkpoints contain
  58 newly authored tests in total.

Checkpoint 3 authors the native qualification kit:

- `tests/timed_text_native.rs` (under `crates/renderflow-core`) is an explicit,
  ignored integration entry point. It creates original synthetic native
  fixtures only when deliberately run; no generated media or passing report is
  checked in by this checkpoint.
- `tests/support/subtitle_qualification_report.rs` owns per-case evidence and
  its overall outcome. The companion
  `schemas/renderflow-subtitle-qualification-v1.schema.json` describes the report.
  Every required case begins as `not_run` before any provider probe; missing or
  mismatched prerequisites cannot become successful execution evidence.
- The kit selects exact FFmpeg/FFprobe version-response lines, bounds fixture
  subprocesses through `ProcessExecutor`, and retains the containers, hashes,
  canonical outputs, and SDK manifests in a fresh output directory. It never
  installs providers or dispatches CI.
- Intended positive cases cover Matroska/SubRip and Matroska/WebVTT, including
  explicit nonzero stream selection and graph lineage. Negative cases cover
  no subtitles, missing indices, unsupported ASS/MP4 mov_text, malformed media,
  and pre-cancelled extraction. A final version recheck is required for success.
- The [native qualification guide](NATIVE_QUALIFICATION.md) records the later
  invocation, report interpretation, and still-deferred coverage. This kit
  prepares the completion pass; it does not establish #427 acceptance.
- This checkpoint adds three test functions: one ignored native entry point
  containing ten required cases, and two report-helper tests. An existing
  provider argument test is expanded. All remain unrun; there are 61 newly
  authored test functions across the three checkpoints.

The bounded acceptance boundary is a reviewable implementation candidate with
deferred obligations recorded. It is not #427 acceptance proof or a release.

## Checks and evidence

Checkpoints 1–3 used an implementation-first budget with no executable product
checks. The user subsequently authorized targeted local tests, schema checks,
and native qualification for checkpoint 4. Formatting, lint, full-suite checks,
and GitHub Actions remain outside this checkpoint.

The first focused Cargo command exited 127 because this environment has no
Cargo or Rust compiler. The official toolchain endpoint was unreachable, and
the default package manager failed on environment process permissions. No Rust
test, build, CLI invocation, SDK run, or native qualification harness executed.
All 61 authored Rust test functions remain unrun; local Renderflow readiness is
**unknown**, not a passing result.

Independent FFmpeg/FFprobe smoke checks did execute against the authored native
argument recipes: 12/12 checks passed on the installed 6.1.1-3ubuntu5 builds.
This establishes fixture/argv compatibility for the small synthetic matrix,
not Rust adapter, cancellation, typed diagnostic, or SDK correctness.
See [checkpoint 4 evidence and resume commands](VALIDATION.md) for the exact
scope, schema results, retained evidence, and environment blocker.

Checkpoint 1 source review corrected pre-open special-file refusal,
immediate-input hashing, and registry claims absent from the adapter.
Checkpoint 2 source review covers typed policy/edge registration, immutable
store input, cache identities, bounded parsing, cancellation, and native/Flow
evidence boundaries. It also found and repaired intake classification that
would otherwise select generic JSON over a declared canonical transcript, and
the mismatch between object-only subtitle policy schema and Serde's default
acceptance of positional struct sequences. Publication integrity is distinct
from compiler/runtime correctness.

Checkpoint 3 source review concerns harness gating, bounded fixture creation,
exact cue/stream/hash evidence, report state transitions, and retained outputs.
Native execution remains deferred. The tool-registry note was reconciled with
the opt-in subtitle graph route added in checkpoint 2.
The report also requires the compiled harness SHA-256 and crate version; its
optional source revision is explicitly operator-declared.

Source inspection of
[FFmpeg n7.1](https://github.com/FFmpeg/FFmpeg/blob/n7.1/fftools/ffmpeg_demux.c)
found that guessing an unspecified audio channel layout emits a warning even
when only subtitles are selected. The FFmpeg-only extraction input now uses
`-guess_layout_max 0`; FFprobe arguments are unchanged. This disables irrelevant
audio inference while preserving strict warning refusal. The existing argument
test now checks the option's input placement and absence from FFprobe. Its
effect on the synthetic command recipes was exercised by checkpoint 4's
independent provider smoke. The Rust native kit remains unrun.

No user media was processed. No provider binaries/models were installed.
An isolated Python JSON Schema validator was installed in scratch for the
completion pass; no repository dependency or lockfile was changed.
No dependency/lockfile, workflow, branch protection, or release change is intended.
Use `[skip ci]` in commits; do not dispatch or poll Actions. Existing repository
automation may react to a PR independently of this worker.

## Deferred completion work and finite stop

1. On a host with Rust/Cargo available, compile and run focused `timed_text` /
   `subtitle` tests, inspect the new CLI
   help and error exits, and verify JSON Schema/model agreement. Use locked
   workspace dependencies and the repository's Rust 1.94 baseline.
   Resume commands (the first was attempted but Cargo was unavailable):
   `cargo test --locked --package renderflow --lib timed_text`,
   `cargo test --locked --package renderflow --lib subtitle`, and
   `cargo test --locked --package renderflow --test timed_text`, plus
   `cargo test --locked --package renderflow --test timed_text_graph`.
   Also run `cargo test --locked --package renderflow --test timed_text_native`
   without `--ignored` to exercise the report-helper tests, then run the native
   entry point explicitly as described below. See `VALIDATION.md` for the
   consolidated command list and the blocked attempt from this environment.
2. Execute the authored native qualification kit from
   [NATIVE_QUALIFICATION.md](NATIVE_QUALIFICATION.md), then extend its remaining
   matrix where needed. Use small redistribution-safe Matroska and MP4-family fixtures
   under pinned FFprobe/FFmpeg versions. Prove SRT/WebVTT extraction, stream
   identity/language, no-subtitle handling, unsupported codec refusal, limits,
   cancellation, malformed input, source stability, and exact output evidence.
   A missing native provider remains unavailable, never an executed success.
   Record the 99-hour timestamp profile, literal-ampersand/markup refusal,
   missing MOV `mov_text` support, fixed resource budgets, native warning
   refusal, and blocked-filesystem-read/concurrent-source limits in that review.
3. Prove the newly authored graph/SDK path: explicit opt-in, media stream
   selection, canonical intermediate/terminal lineage, cached pure conversion,
   uncached extraction, parser-backed validation, typed cancellation, policy
   filtering, output naming, and native/Flow evidence agreement. Reconcile
   actual versus planned native tool versions before enabling extraction reuse.
   Decide explicit supported/rejected coverage for ASS/SSA/TTML and whether
   plain-text export needs a graph format. Released Flow integration is separate.
4. Revisit repository-native `task check`, `task test`, `task format`, `task lint`,
   schema/docs/package checks, and the focused review when the sprint switches
   to its completion pass. Hosted CI/release evidence stays separate.

Keep #427 open until its acceptance criteria are actually proven. Do not use
an automatic closing keyword in this draft PR. After checkpoint 4, the user
explicitly deferred tests again and authorized implementation-first progress.
#428 may proceed in a dependent draft while this validation debt stays open;
provider-only smoke is not proof of the shared transcript contract.

Source inspection during that continuation found the two new integration
targets imported the private `renderflow::sdk` module. They now import the
existing public crate-root re-exports. This concrete source correction is
uncompiled and does not change the recorded checkpoint-4 execution results.

## Resume and parallel work

Recheck branch status, main, issue, and PR before editing; read the files above
and inspect the current diff. Repair any named review findings before expanding
the feature. Stop at the next bounded checkpoint and push its handoff.

Optiflow #111 / PR #122 and its ACL qualification belong to the other paused
workstream. Aniflow #36 remains parked. This candidate changes Renderflow only.
No merge, release, upstream submission, or external publication is authorized
by this handoff. Public records contain only repository state and synthetic data.
