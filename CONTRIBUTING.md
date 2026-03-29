# Contributing to Renderflow

Thank you for your interest in contributing to Renderflow! This guide walks you through everything you need to get started — from setting up a local development environment to submitting a pull request.

---

## Table of Contents

- [Getting Started](#getting-started)
  - [Option A: Dev Container (recommended)](#option-a-dev-container-recommended)
  - [Option B: Local Setup](#option-b-local-setup)
- [Build, Test, and Lint](#build-test-and-lint)
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

This project uses [Conventional Commits](https://www.conventionalcommits.org/). Each commit message must follow this structure:

```
<type>(<scope>): <short summary>
```

**Types:**

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

**Examples:**

```
feat(pipeline): add caching support for unchanged outputs
fix(transforms): handle unclosed template placeholders gracefully
docs: add CONTRIBUTING.md
test(cli): add integration test for --dry-run flag
chore: update Cargo dependencies
```

**Rules:**
- Use the imperative mood in the summary ("add", not "added" or "adds")
- Keep the summary under 72 characters
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

5. **CI runs automatically** on every pull request (see `.github/workflows/ci.yml`). All checks must pass before merging.

---

## Questions?

Open a [GitHub Discussion](https://github.com/egohygiene/renderflow/discussions) or file an [issue](https://github.com/egohygiene/renderflow/issues) if you have questions or run into problems.
