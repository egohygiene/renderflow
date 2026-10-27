# Contributing to Renderflow

Start with [ARCHITECTURE.md](ARCHITECTURE.md) for the current boundaries and the
[source map](docs/architecture/overview.md#source-map) for where to make a change.
The [live roadmap](https://github.com/egohygiene/renderflow/issues/367) separates
implementation work from runtime qualification and release work.

## Getting started

Renderflow is a three-crate Rust workspace. The root [Cargo.toml](Cargo.toml)
declares edition 2021 and minimum Rust **1.94**; the core package is `renderflow`,
the executable package is `renderflow-cli`, and the plugin boundary is
`renderflow-plugin-sdk`. Run workspace commands from the repository root.

```bash
git clone https://github.com/egohygiene/renderflow.git
cd renderflow
```

### Local setup

Install Rust 1.94 or newer through [rustup](https://rustup.rs). Native build
requirements depend on the host and selected dependencies. Pandoc, Tectonic,
FFmpeg, speech models and other render providers are **route-specific**; a core
build does not install or qualify them. Use the selected route's guide and the
[creative toolchain setup guide](docs/user-guide/creative-toolchain-setup.md)
when that work requires a provider. Node and pnpm are needed for the optional
site workspace, not for a Rust-only source edit.

When dependency and build execution is in scope:

```bash
cargo fetch --locked
cargo check --workspace --locked
cargo build --package renderflow-cli --locked
```

The executable is written to `target/debug/renderflow` (`renderflow.exe` on
Windows). A source installation selects the CLI package explicitly:

```bash
cargo install --locked --path "crates/renderflow-cli"
```

A successful source build is distinct from a verified release installation.
During a maintainer-deferred qualification sprint, keep the commands deferred
and record what was source-reviewed; do not report authored tests as passing.
Known baseline follow-up is tracked in
[#458](https://github.com/egohygiene/renderflow/issues/458).

### Dev container

The [dev-container recipe](.devcontainer/Dockerfile) is a convenience Linux
development environment. Open the repository in VS Code and use **Dev
Containers: Reopen in Container** when container setup is in scope. Building it
downloads tools; its [creation hook](.devcontainer/devcontainer.json) runs
`cargo fetch`.

The recipe includes stable Rust, distro Pandoc, Tectonic 0.15.0, Task 3.43.3,
git-cliff 2.12.0, `cross`, and `cargo-watch`. Several inputs remain unpinned,
and downloaded tool assets target Linux x86_64. It is not a complete optional
provider installation or evidence of cross-platform reproducibility.

## Build, test, and lint

[Taskfile.yml](Taskfile.yml) provides convenience commands. `task --list`
shows the full list.

| Command | Purpose |
| --- | --- |
| `task build` | Compile debug binaries |
| `task release` | Compile release binaries; does not publish a release |
| `task check` | Check the source without producing a binary |
| `task test` | Run the configured Rust tests |
| `task lint` | Run the convenience Clippy command |
| `task format` | Format Rust source |
| `task run` | Run the CLI with its default build command |
| `task clean` | Remove build artifacts |

These shortcuts are not the complete release gate. The checked-in
[CI workflow](.github/workflows/ci.yml) and
[release guide](docs/release-candidate.md) define the exact locked, workspace,
toolchain and warning policies. Keep those requirements intact when fixing a
failure. Cross-compilation recipes and package templates are preparatory
surfaces until their exact artifacts are qualified.

For an ordinary implementation PR, run the relevant tests and required checks
when authorized. During an explicitly deferred sprint, state the deferral in
the PR and handoff, keep acceptance claims bounded, and preserve the later
qualification work. Source review cannot establish runtime, visual or native
provider behavior.

### Dependency maintenance

- Prefer targeted changes with a concrete benefit; explain unexpected lockfile
  movement. Use Cargo to resolve the lockfile when dependency execution resumes.
- The synchronous AI transport uses `ureq` v2 behind `AiHttpClient` in
  [transforms/ai.rs](crates/renderflow-core/src/transforms/ai.rs). Reuse that seam
  for transport changes and review API/error behavior before upgrading it.
- Introduce concurrency or another transport only for a demonstrated need.
  Keep optional provider policy and unavailable-provider diagnostics explicit.

## Releases

The current release contract is the
[integration-candidate guide](docs/release-candidate.md), owned by
[#419](https://github.com/egohygiene/renderflow/issues/419). The proposed version
is `v0.3.0-rc.1`; a version edit, local build or tag is not a published release.
The canonical source version is `[workspace.package].version` in
[Cargo.toml](Cargo.toml).

The maintainer reviews and merges the candidate, qualifies the exact commit,
creates a new annotated tag at that commit, and explicitly dispatches
[release.yml](.github/workflows/release.yml) on that tag with its full
`expected_commit` SHA. **Tag pushes do not publish.** Staged downloads,
clean-install evidence and immutable-release checks must succeed before
publication. The workflow does not bump versions or commit package manifests
back to `main`.

The first candidate targets **Ubuntu 24.04 x86_64 GNU, glibc 2.39 or newer**.
Support begins only with the verified immutable prerelease and its binary
digest, manifest and installation evidence. Other platforms, containers,
crates.io and package-manager channels remain unverified or unpublished.
Their checked-in recipes do not establish availability.

The old automatic Bump Version/tag-push/multi-platform release description is
historical. There is no current `bump-version.yml` workflow.
[release.toml](release.toml) disables its legacy tag, push and publish actions;
the `release-patch`, `release-minor` and `release-major` Task helpers are not
the integration-candidate procedure. Follow the release guide for the complete
gates, signing decision, rollback and compromised-release response.

## Architecture and source navigation

- [ARCHITECTURE.md](ARCHITECTURE.md) is the current architecture entry point.
- [SYSTEM.md](SYSTEM.md) defines logical ownership; the
  [implementation source map](docs/architecture/overview.md#source-map) locates
  code without making the directory tree the conceptual architecture.
- [docs/architecture/overview.md](docs/architecture/overview.md) explains current
  build routing and the disposition of older foundation documents.
- [DECISIONS.md](DECISIONS.md) retains existing decisions and four-digit IDs.
  [#405](https://github.com/egohygiene/renderflow/issues/405) still governs the
  scheduled shared-ADR migration; these navigation corrections do not activate it.
- [#409](https://github.com/egohygiene/renderflow/issues/409) remains the separate
  future post-roadmap audit.

| Location | Contribution boundary |
| --- | --- |
| `crates/renderflow-core/` | Package `renderflow`: domain models, planning, execution, public SDK and shared CLI command implementation |
| `crates/renderflow-cli/` | Thin executable entry point calling the core application layer |
| `crates/renderflow-plugin-sdk/` | Plugin-facing contract types; currently `renderflow.plugin/v2alpha1` |
| `crates/renderflow-core/tests/` | Core integration tests and fixtures |
| `tests/` | Additional integration/CLI tests and shared fixtures wired through crate manifests |
| `schemas/`, `examples/`, `docs/` | Versioned contracts, user examples and documentation |
| `apps/`, `scripts/` | Site workspace and supporting development/release tooling |

## Coding guidelines

- Write idiomatic Rust 2021 with focused responsibilities and explicit errors.
  Prefer `anyhow` for contextual propagation and `thiserror` for typed errors.
- Reuse the shared planning/execution boundary when changing CLI or SDK
  behavior. Preserve native artifact identity, loss/review state and explicit
  partial results when adding a route.
- Avoid `unwrap()` and `expect()` in production paths. Use structured `tracing`
  events without logging private source content, credentials or unnecessary paths.
- Place integration tests in the owning crate or the root tests declared by its
  manifest; keep unit coverage beside the implementation. Formatting, lint and
  test evidence must identify what actually ran.
- Treat examples as user-facing contracts. Update relevant commands, schemas
  and limitations together. Existing example jobs cover named cases; they do
  not establish every example or provider combination as verified.
- Keep generated references tied to their source registry. A new route needs
  explicit capability and qualification status, not an inferred reverse edge.

## Commit conventions

Use [Conventional Commits](https://www.conventionalcommits.org/):

```text
<type>(<scope>): <short summary>
```

Types include `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, and `ci`.
Use a concrete scope such as `cli`, `spec`, `planning`, `composition`,
`screenplay`, `transforms`, `cache`, or `release`. Use the imperative mood, keep
the summary under 72 characters, and explain behavior and evidence in the body.

Mark breaking public changes with `!` or a `BREAKING CHANGE:` footer and provide
migration guidance. Commit classification helps review and changelog generation;
it does not authorize an automatic version bump, tag or publication. Stable API
and deprecation guarantees remain subject to the candidate's compatibility
policy.

## Pull request workflow

1. Create a branch from current `main`:

   ```bash
   git switch --create "feat/your-feature-name"
   ```

2. Make a bounded change and update its user-facing contract and examples.
3. Review the diff and run the relevant checks when authorized. Record failures,
   skips and deferrals separately from successes.
4. Open a draft PR early for work that is still in progress. Describe the
   problem, resulting behavior, related issue, migration impact and evidence.
   Use closing references only when the issue's full acceptance scope is met.
5. Follow the repository's required checks and maintainer review for merge.
   Never bypass protections or present a source-only checkpoint as qualification.

Questions belong in [Discussions](https://github.com/egohygiene/renderflow/discussions)
or a focused [issue](https://github.com/egohygiene/renderflow/issues).
