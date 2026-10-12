# Renderflow #427 — timed-text checkpoint

## Scope and observed state

- Parent: https://github.com/egohygiene/renderflow/issues/427 (under #421).
- Role: implementation, with focused authored coverage and necessary interface notes.
- Strategy: Aether worker-strategy 0.1.0 at
  `db5f4bd339f9266758c74c3435d47c8005f89a8e`, with specfile,
  repository-continuity, and local-validation-evidence dependencies at that ref.
- Observed 2026-10-12 UTC: #427 open; no open Renderflow PRs at bootstrap.
- Verified base: `8e866f91aa8b20255f25a553031f25fa98aa3856` (`main`).
- Candidate branch: `feat/renderflow-427-timed-text`; obtain its exact head from
  the draft PR. This file cannot contain its own eventual commit SHA.
- Existing SRT/WebVTT format declarations were present, but no subtitle parser
  or extraction adapter existed. No root AGENTS.md or CONTINUITY.md was present.

## Checkpoint outcome and interfaces

Author a coherent standalone subtitle workflow: strict SRT/WebVTT parsing,
timed-transcript JSON, explicit stream inspection/selection, bounded local
extraction, and no-overwrite output publication. Source identity, stream
language/codec, provider evidence, and export losses must remain visible.

- `src/timed_text/` (under `crates/renderflow-core`) owns the portable model,
  pure codecs, typed errors, validation, and bounded provider adapter.
- `schemas/renderflow-timed-transcript-v1.schema.json` owns the canonical wire shape.
  Companion subtitle inspection/export schemas describe stdout evidence.
- `src/commands/subtitles.rs`, `src/cli.rs`, and `src/app.rs` expose
  `subtitles inspect`, `convert`, and `extract`.
- Format/tool registries identify additional subtitle formats and optional
  providers. Registration does not imply graph execution or qualification.
- Focused tests and original synthetic fixtures are authored with the feature.
  There are 31 newly authored test functions (9 codec, 11 provider, 5 integration,
  2 publication, 4 registry), plus two expanded existing registration tests.
- [User guide](../../user-guide/subtitles.md) describes the authored interface
  and limits. All command examples are unrun.

The bounded acceptance boundary is a reviewable implementation candidate with
deferred obligations recorded. It is not #427 acceptance proof or a release.

## Checks and evidence

User-selected implementation-first budget: **no executable product checks**.
Only repository/source inspection and Git publication metadata are used.
No tests, builds, compiler checks, formatting, lint, schema validation, smoke
commands, native media operations, or GitHub Actions have run for this change.
Authored fixtures are not passed fixtures. Local readiness: **unknown**.

Source review corrected pre-open special-file refusal, immediate-input hash
reporting, and registry claims for options/evidence absent from the adapter.
Inspection of GitHub before publication found the same main SHA and no competing
open PR. Publication integrity is distinct from compiler/runtime correctness.

No user media was processed. No provider binaries/models were installed.
No dependency/lockfile, workflow, branch protection, or release change is intended.
Use `[skip ci]` in commits; do not dispatch or poll Actions. Existing repository
automation may react to a PR independently of this worker.

## Deferred completion work and finite stop

1. Compile and run focused `timed_text` / `subtitle` tests, inspect the new CLI
   help and error exits, and verify JSON Schema/model agreement. Use locked
   workspace dependencies and the repository's Rust 1.94 baseline.
   Planned commands (not run): `cargo test --locked --package renderflow --lib timed_text`,
   `cargo test --locked --package renderflow --lib subtitle`, and
   `cargo test --locked --package renderflow --test timed_text`.
2. Execute small redistribution-safe native Matroska and MP4-family fixtures
   under pinned FFprobe/FFmpeg versions. Prove SRT/WebVTT extraction, stream
   identity/language, no-subtitle handling, unsupported codec refusal, limits,
   cancellation, malformed input, source stability, and exact output evidence.
   A missing native provider remains unavailable, never an executed success.
   Record the 99-hour timestamp profile, literal-ampersand/markup refusal,
   missing MOV `mov_text` support, fixed resource budgets, native warning
   refusal, and blocked-filesystem-read/concurrent-source limits in that review.
3. Reconcile canonical planner/SDK integration before advertising subtitle
   forest edges; current `build` and Flow provider contracts do not expose them.
   Decide explicit supported/rejected coverage for ASS/SSA/TTML.
4. Revisit repository-native `task check`, `task test`, `task format`, `task lint`,
   schema/docs/package checks, and the focused review when the sprint switches
   to its completion pass. Hosted CI/release evidence stays separate.

Keep #427 open until its acceptance criteria are actually proven. Do not use
an automatic closing keyword in this draft PR. Do not start #428 on an unproven
shared transcript contract without a focused integration check or an explicit
revised check budget from the user.

## Resume and parallel work

Recheck branch status, main, issue, and PR before editing; read the files above
and inspect the current diff. Repair any named review findings before expanding
the feature. Stop at the next bounded checkpoint and push its handoff.

Optiflow #111 / PR #122 and its ACL qualification belong to the other paused
workstream. Aniflow #36 remains parked. This candidate changes Renderflow only.
No merge, release, upstream submission, or external publication is authorized
by this handoff. Public records contain only repository state and synthetic data.
