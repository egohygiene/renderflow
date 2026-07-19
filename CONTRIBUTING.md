# Contributing to Renderflow

Thank you for your interest in contributing to Renderflow! This guide walks you through everything you need to get started — from setting up a local development environment to submitting a pull request.

---

## Table of Contents

- [Getting Started](#getting-started)
  - [Option A: Dev Container (recommended)](#option-a-dev-container-recommended)
  - [Option B: Local Setup](#option-b-local-setup)
- [Build, Test, and Lint](#build-test-and-lint)
- [Releases](#releases)
- [Architecture Overview](#architecture-overview)
- [Coding Guidelines](#coding-guidelines)
- [Commit Conventions](#commit-conventions)
- [Pull Request Workflow](#pull-request-workflow)

---

## Getting Started

### Option A: Dev Container (recommended)

The repository includes a fully configured [VS Code Dev Container](https://code.visualstudio.com/docs/devcontainers/containers) that provides all required tooling out of the box.

**Prerequisites:**
- [VS Code](https://code.visualstudio.com/) with the [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
- [Docker](https://docs.docker.com/get-docker/)

**Steps:**

1. Clone the repository:
   ```bash
   git clone https://github.com/egohygiene/renderflow.git
   cd renderflow
   ```

2. Open in VS Code and reopen in the dev container when prompted (or run **Dev Containers: Reopen in Container** from the command palette).

3. The container automatically runs `cargo fetch` on creation. All tools are pre-installed:
   - Rust (stable toolchain)
   - Pandoc (document conversion)
   - Tectonic (PDF engine)
   - [Task](https://taskfile.dev) (`task` CLI — Taskfile runner)
   - `cross` (cross-compilation)
   - `cargo-watch` (file watching)

### Option B: Local Setup

**Prerequisites:**

| Tool | Version | Install |
|------|---------|---------|
| Rust | ≥ 1.75  | [rustup.rs](https://rustup.rs) |
| Pandoc | latest | [pandoc.org/installing](https://pandoc.org/installing.html) |
| Tectonic | ≥ 0.15 | [tectonic-typesetting.github.io](https://tectonic-typesetting.github.io/en-US/install.html) |
| Task | ≥ 3     | [taskfile.dev/installation](https://taskfile.dev/installation/) |

**Steps:**

1. Clone the repository:
   ```bash
   git clone https://github.com/egohygiene/renderflow.git
   cd renderflow
   ```

2. Fetch dependencies:
   ```bash
   cargo fetch
   ```

3. Verify the setup:
   ```bash
   task check
   ```

---

## Build, Test, and Lint

All common development tasks are available via [Taskfile](https://taskfile.dev). Run `task --list` to see all available tasks.

### Common Commands

| Command | Description |
|---------|-------------|
| `task build` | Compile in debug mode |
| `task release` | Compile in release mode |
| `task check` | Check for errors without producing a binary |
| `task test` | Run the full test suite |
| `task lint` | Run Clippy linter |
| `task format` | Format source code with rustfmt |
| `task clean` | Remove build artifacts |
| `task run` | Run the project with the default `build` subcommand |

### Cross-Compilation

Cross-compiled binaries require [`cross`](https://github.com/cross-rs/cross) and Docker:

```bash
task cross-build TARGET=x86_64-unknown-linux-gnu
task cross-build TARGET=aarch64-unknown-linux-gnu
task cross-build TARGET=x86_64-pc-windows-gnu
```

### Before Submitting a PR

Run the following to catch issues early:

```bash
task format   # format code
task lint     # run Clippy
task test     # run all tests
```

---

## Releases

Releases are fully automated. A single version bump through the **Bump Version** GitHub Actions workflow triggers the entire release pipeline — no manual steps are required after that.

### Version Bump (single source of truth)

The canonical version lives in `Cargo.toml` under `[package].version`. Running the **Bump Version** workflow:

1. Resolves the new version (explicit input or a patch/minor/major bump).
2. Updates `Cargo.toml`, `Cargo.lock` (workspace package version only), and package manifest versions in one commit:
   - `Formula/renderflow.rb` (Homebrew — URL updated, SHA256 set by release pipeline)
   - `pkg/scoop/renderflow.json` (Scoop)
   - `pkg/chocolatey/renderflow.nuspec` and `chocolateyinstall.ps1` (Chocolatey)
   - `pkg/aur/renderflow/PKGBUILD` (AUR)
3. Creates an annotated git tag (e.g. `v0.3.0`).
4. Pushes the commit and the tag to `main`.

Pushing the tag automatically triggers the **Release** workflow.

### Release Pipeline

The **Release** workflow (`.github/workflows/release.yml`) is triggered by any `v*` tag push. It runs in four sequential stages:

```
[1] generate-changelog
       │
       ├─ [2a] build-binaries (10 targets, in parallel)
       │         └─ update-scoop-manifest (needs: build-binaries + update-homebrew)
       │         └─ package-chocolatey   (needs: build-binaries)
       │
       ├─ [2b] package-deb-x86_64    ┐
       │       package-deb-aarch64   │ (in parallel, all need generate-changelog)
       │       package-rpm-x86_64    │
       │       package-rpm-aarch64   ┘
       │
       ├─ [2c] snap-package
       │
       └─ [3]  update-homebrew-formula
                └─ update-scoop-manifest
                     └─ update-aur-pkgbuild
                          └─ [4] verify-release
```

**Stage 1 — Changelog & GitHub Release**

- Generates `CHANGELOG.md` and per-release notes using `git-cliff`.
- Commits the updated changelog to `main`.
- Creates the GitHub Release with auto-generated release notes.

**Stage 2 — Artifacts (run in parallel)**

- Cross-compiles binaries for all 10 supported targets using `cross` or native Rust.
- Generates SHA256 checksums for every binary.
- Builds `.deb` packages for x86_64 and aarch64.
- Builds `.rpm` packages for x86_64 and aarch64.
- Builds the Snap package (version injected from the tag).
- Packs the Chocolatey `.nupkg` with the real checksum.
- Uploads all artifacts to the GitHub Release.

**Stage 3 — Manifest Updates (sequential)**

- Updates the Homebrew formula (`Formula/renderflow.rb`) with the new tarball URL and SHA256.
- Updates the Scoop manifest (`pkg/scoop/renderflow.json`) with the new version, URL, and checksum.
- Updates the AUR PKGBUILD (`pkg/aur/renderflow/PKGBUILD`) with the new version and SHA256.
- Each update is committed and pushed to `main` with retry + rebase logic to handle concurrent changes.
- Manifest update failures are downgraded to warnings so GitHub Release publishing can still complete.

**Stage 4 — Verification**

- Fetches the GitHub Release asset list and confirms every expected binary/checksum plus `.deb`, `.rpm`, `.snap`, and `.nupkg` assets are present.
- Verifies the source tarball used by Homebrew/AUR is reachable.
- Fails the workflow if any asset is missing.

### Triggering a Release

```bash
# Trigger from GitHub UI: Actions → Bump Version → Run workflow
# Select: bump level (patch / minor / major) or enter an explicit version

# Alternatively, trigger locally (requires push access):
# 1. Bump and tag manually
task release-patch   # or release-minor / release-major

# 2. Push commit and tag
git push && git push --tags
```

> The `bump-version` workflow is the preferred method. It keeps the version bump commit and the release tag in sync and updates all package manifests atomically.

### Supported Distribution Channels

| Channel | Artifact | Updated by |
|---------|----------|------------|
| GitHub Releases | Binaries + checksums + packages | `release.yml` |
| Homebrew | `Formula/renderflow.rb` | `release.yml` → update-homebrew-formula |
| Scoop | `pkg/scoop/renderflow.json` | `release.yml` → update-scoop-manifest |
| Chocolatey | `.nupkg` uploaded to GitHub Release | `release.yml` → package-chocolatey |
| Snap | `.snap` uploaded to GitHub Release | `release.yml` → snap-package |
| Debian/Ubuntu | `.deb` (x86_64 + aarch64) | `release.yml` → package-deb-* |
| RHEL/Fedora | `.rpm` (x86_64 + aarch64) | `release.yml` → package-rpm-* |
| Arch Linux (AUR) | `pkg/aur/renderflow/PKGBUILD` | `release.yml` → update-aur-pkgbuild |

### Release Configuration Files

| File | Purpose |
|------|---------|
| `release.toml` | `cargo-release` settings (tag format, branch restriction) |
| `cliff.toml` | `git-cliff` changelog generation rules |
| `.github/workflows/bump-version.yml` | Version bump workflow (triggers release) |
| `.github/workflows/release.yml` | Full release pipeline |
| `.github/workflows/ci.yml` | Continuous integration (lint, build, test) |

---

## Architecture Overview

Renderflow processes documents through a **two-phase pipeline**:

```
Input Document
      │
      ▼
┌─────────────────────┐
│   Transform Phase   │  In-memory text transformations (emoji, variable substitution, syntax highlighting)
└─────────────────────┘
      │
      ▼
┌─────────────────────┐
│    Step Phase       │  I/O and external tool execution (Pandoc, Tectonic)
└─────────────────────┘
      │
      ▼
Output Files (PDF / HTML / DOCX)
```

### Key Design Patterns

- **Pipeline** — Ordered, composable steps with clean error propagation
- **Strategy** — Each output format (`html`, `pdf`, `docx`) is an independent, swappable rendering strategy in `src/strategies/`
- **Transform** — Pure in-memory text transforms applied before any I/O, located in `src/transforms/`

### Source Layout

```
src/
├── adapters/       # Command adapter patterns
├── commands/       # build and watch subcommands
├── pipeline/       # Core pipeline orchestration
├── strategies/     # Output format rendering (HTML, PDF, DOCX)
├── transforms/     # In-memory text transforms
├── cli.rs          # CLI argument parsing (clap)
├── config.rs       # YAML config deserialization
├── assets.rs       # Asset path resolution
├── cache.rs        # Build caching
├── deps.rs         # External dependency checking (Pandoc, Tectonic)
├── files.rs        # File I/O utilities
├── input_format.rs # Input format detection
├── template.rs     # Tera template rendering
└── main.rs         # Entry point
```

---

## Coding Guidelines

### General

- Write idiomatic Rust (edition 2021). Prefer `anyhow` for error propagation and `thiserror` for defining custom error types.
- Keep functions focused. Prefer composable, single-responsibility units.
- Avoid `unwrap()` and `expect()` in production paths; propagate errors explicitly.
- Use structured logging via `tracing` macros (`tracing::info!`, `tracing::warn!`, `tracing::error!`).

### Formatting and Linting

- Format all code with `rustfmt` before committing (`task format`).
- Fix all Clippy warnings before opening a pull request (`task lint`).

### Tests

- Add tests for new behaviour in the `tests/` directory (integration tests) or as inline `#[cfg(test)]` modules (unit tests).
- Run `task test` to verify the full test suite passes before pushing.

---

## Commit Conventions

This project uses [Conventional Commits](https://www.conventionalcommits.org/) to drive automated changelog generation and predictable version bumps.

Each commit message must follow this structure:

```
<type>(<scope>): <short summary>

[optional body]

[optional footer(s)]
```

### Types

| Type | When to use |
|------|-------------|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation-only changes |
| `refactor` | Code restructuring with no behaviour change |
| `test` | Adding or updating tests |
| `chore` | Build process, tooling, or dependency updates |
| `perf` | Performance improvements |
| `ci` | CI/CD configuration changes |

### Scopes

Scopes are optional but strongly recommended. Use a scope that matches the area of the codebase being changed:

| Scope | Area |
|-------|------|
| `cli` | CLI argument parsing (`src/cli.rs`) |
| `config` | YAML config handling (`src/config.rs`) |
| `pipeline` | Pipeline orchestration (`src/pipeline/`) |
| `strategies` | Output format strategies (`src/strategies/`) |
| `transforms` | In-memory text transforms (`src/transforms/`) |
| `assets` | Asset path resolution (`src/assets.rs`) |
| `cache` | Build caching (`src/cache.rs`) |
| `deps` | External dependency checks (`src/deps.rs`) |
| `template` | Tera template rendering (`src/template.rs`) |
| `release` | Release and versioning tasks |

### Breaking Changes

Breaking changes must be indicated in one of two ways:

1. **Append `!` after the type/scope:**
   ```
   feat(cli)!: remove --legacy flag
   ```

2. **Add a `BREAKING CHANGE:` footer in the commit body:**
   ```
   feat(config): rename output_dir to output_path

   BREAKING CHANGE: The `output_dir` key in renderflow.yaml has been
   renamed to `output_path`. Update your config files accordingly.
   ```

Both approaches may be combined.

### Version Bump Mapping

Commit types determine how the version is automatically incremented following [Semantic Versioning](https://semver.org/):

| Trigger | Version bump | Example |
|---------|-------------|---------|
| Any commit with `BREAKING CHANGE` or `!` | **Major** (`x.0.0`) | `feat(cli)!: redesign config format` |
| `feat` commit | **Minor** (`0.x.0`) | `feat(pipeline): add watch mode` |
| `fix`, `perf`, or any other type | **Patch** (`0.0.x`) | `fix(transforms): handle empty input` |

### Examples

```
feat(cli): add --watch flag for live reloading
fix(pipeline): correct transform caching for unchanged outputs
docs: update README with cross-compilation instructions
refactor(strategies): extract shared HTML rendering logic
test(cli): add integration test for --dry-run flag
chore(release): bump version to 0.2.0
perf(transforms): cache compiled regex patterns
ci: add commit linting workflow
feat(config)!: rename output_dir to output_path
```

### Rules

- Use the imperative mood in the summary ("add", not "added" or "adds")
- Keep the summary line under 72 characters
- Separate the body from the summary with a blank line
- Reference issues where relevant: `fix(config): validate output type (#42)`

---

## Pull Request Workflow

1. **Fork** the repository and create a branch from `main`:
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Make your changes**, following the coding guidelines and commit conventions above.

3. **Verify locally:**
   ```bash
   task format && task lint && task test
   ```

4. **Open a pull request** against the `main` branch. In the PR description:
   - Summarise what changed and why
   - Reference any related issues (e.g. `Closes #42`)

5. **CI runs automatically** on every pull request. All checks must pass before merging:
   - **Build & Test** (`.github/workflows/ci.yml`) — compiles and runs the test suite
   - **Commit Lint** (`.github/workflows/commitlint.yml`) — validates commit messages against the Conventional Commits format

---

## Questions?

Open a [GitHub Discussion](https://github.com/egohygiene/renderflow/discussions) or file an [issue](https://github.com/egohygiene/renderflow/issues) if you have questions or run into problems.
