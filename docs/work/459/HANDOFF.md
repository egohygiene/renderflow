# #459 — contributor and architecture navigation

## Draft checkpoint — 2026-10-26

Base: `9dac5428df38691b2922e09ea02d51b02a7f43eb`.
Branch: `codex/459-contributor-architecture`.
Issue: [#459](https://github.com/egohygiene/renderflow/issues/459).

The maintainer authorized continued implementation with frequent draft PR
checkpoints and the existing testing/polish deferral. This change is bounded to
present contributor and architecture accuracy; it does not run or alter product
code, build/release workflows, dependencies or package recipes.

## Authored changes

- Contributor setup now identifies the virtual three-crate workspace, Rust 1.94
  minimum, explicit CLI package selection and route-specific external tools.
  Dev-container descriptions reflect its source recipe and unpinned inputs.
- Removed the obsolete automatic version-bump/tag-push/multi-platform release
  directions. Contributors follow the existing exact-tag/manual-dispatch
  release guide and #419. `task release` is a local release-mode build; legacy
  cargo-release helpers do not replace the integration-candidate procedure.
- Root `ARCHITECTURE.md` is the current conceptual entry, `SYSTEM.md` owns
  distinct logical responsibilities, and `docs/architecture/overview.md` owns
  current implementation navigation and the code map. README and the existing
  site navigation point to those entry points.
- Five older foundation pages retain their original bodies and dates with
  historical banners and links to current owners. No architecture files move.
- Current source, draft PR #455, deferred qualification and the proposed
  immutable release are named separately. Existing four-digit decisions,
  #405's shared-ADR scheduling gate and #409's later audit are preserved.

## Evidence and remaining review

Source inspection covered the Cargo manifests, CLI entry/app/planning facade,
core module exports, provider and evidence contracts, dev-container recipe,
Taskfile, release workflow/configuration and release guide. An independent
source reader cross-checked the setup and release claims. Changed navigation
targets and source locations were inspected directly. This is source review,
not a generated-site or executable qualification result.

Tests, compiler/build, lint/formatting, schema validators, CLI/native/provider
execution, dependency installs/resolution and CI remain unrun. No Actions were
dispatched or polled. Documentation build/link tooling was not run in this
checkpoint. Do not mark those checks or any product acceptance as passing.

Before closing #459, review the published diff and complete its documentation
structural/link checks under the appropriate session scope. #458 owns the
separate source/lockfile qualification baseline. #419 owns release proof;
#405 and #409 retain their existing scopes. Other old implementation articles
and root conceptual documents remain outside this focused correction.
