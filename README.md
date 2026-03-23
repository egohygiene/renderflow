<div align="center">

# ✨ renderflow

**Spec-driven document rendering engine**

*Transform Markdown into publication-ready PDF and HTML — defined once, rendered anywhere.*

[![CI](https://github.com/egohygiene/renderflow/actions/workflows/ci.yml/badge.svg)](https://github.com/egohygiene/renderflow/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

</div>

---

## What is Renderflow?

Renderflow is a config-driven rendering engine that transforms Markdown documents into polished PDF and HTML output — no complex shell scripts, no Pandoc flags to memorize.

Define your output spec in YAML. Point it at your Markdown. Run one command.

---

## Features

- 📄 **Multi-format output** — Render to PDF and HTML from a single config file
- 🗂️ **YAML-driven spec** — Declarative, repeatable, version-controllable builds
- 🖼️ **Asset management** — Automatically resolves and validates image paths
- 🔄 **Transform pipeline** — Pluggable in-memory content transforms
- 🧩 **Custom templates** — Per-output Jinja2-compatible templates via [Tera](https://keats.github.io/tera/)
- 🔍 **Dry-run mode** — Preview what will be built without writing any files
- 🦀 **Built with Rust** — Fast, safe, and reliable

---

## Quick Start

**1. Create your Markdown document (`input.md`):**

```markdown
# My Document

![Logo](assets/logo.png)

Welcome to my publication.
```

**2. Create a config file (`renderflow.yaml`):**

```yaml
input: "input.md"
output_dir: "dist"
outputs:
  - type: pdf
  - type: html
```

**3. Render:**

```bash
renderflow build
```

Output files appear in `dist/`:

```
dist/
├── input.pdf
└── input.html
```

---

## Installation

### Download from GitHub Releases (recommended)

Pre-built binaries are available for Linux, macOS, and Windows on the [Releases page](https://github.com/egohygiene/renderflow/releases).

**Linux:**
```bash
curl -L https://github.com/egohygiene/renderflow/releases/latest/download/renderflow-linux -o renderflow
chmod +x renderflow
sudo mv renderflow /usr/local/bin/
```

**macOS:**
```bash
curl -L https://github.com/egohygiene/renderflow/releases/latest/download/renderflow-macos -o renderflow
chmod +x renderflow
sudo mv renderflow /usr/local/bin/
```

**Windows:**

Download `renderflow-windows.exe` from the [Releases page](https://github.com/egohygiene/renderflow/releases/latest) and place it somewhere on your `PATH`.

### Build from source

Requires [Rust](https://rustup.rs) and [Pandoc](https://pandoc.org/installing.html).

```bash
cargo install --path .
```

---

## Usage

```bash
# Render using the default renderflow.yaml config
renderflow build

# Render using a custom config file
renderflow build --config my-project.yaml

# Shorthand: pass the config file directly
renderflow my-project.yaml

# Preview what would be built, without writing any files
renderflow build --dry-run

# Enable verbose or debug logging
renderflow build --verbose
renderflow build --debug
```

> **Note:** The argument to `renderflow` is always a YAML config file. The Markdown source is specified inside the config via the `input` key (e.g. `input: "input.md"`).

---

## Configuration

Renderflow is entirely driven by a YAML spec file (default: `renderflow.yaml`):

```yaml
input: "input.md"       # Path to your Markdown source
output_dir: "dist"      # Output directory (default: dist)
outputs:
  - type: pdf           # Render to PDF (requires Pandoc + Tectonic)
  - type: html          # Render to HTML
    template: "default" # Optional: use a custom template
```

### Supported Output Types

| Type   | Description                   | Requirements      |
|--------|-------------------------------|-------------------|
| `html` | Renders to HTML               | Pandoc            |
| `pdf`  | Renders to PDF via LaTeX      | Pandoc + Tectonic |

### Templates

Templates live in a `templates/` directory and use [Tera](https://keats.github.io/tera/) syntax (Jinja2-compatible). Specify a template per output with the `template` key. A `default` HTML template is included out of the box.

---

## Architecture

Renderflow processes documents through a two-phase pipeline:

```
Input Markdown
      │
      ▼
┌─────────────────────┐
│   Transform Phase   │  In-memory text transformations (emoji, etc.)
└─────────────────────┘
      │
      ▼
┌─────────────────────┐
│    Step Phase       │  I/O and external tool execution (Pandoc, Tectonic)
└─────────────────────┘
      │
      ▼
Output Files (PDF / HTML)
```

**Key design patterns:**

- **Pipeline** — Ordered, composable steps with clean error propagation
- **Strategy** — Each output format is an independent, swappable rendering strategy
- **Transform** — Pure in-memory text transforms applied before any I/O

---

## Roadmap

- [ ] DOCX output format
- [ ] Watch mode (re-render on file change)
- [ ] Built-in stylesheet themes
- [ ] SVG / emoji embedding in PDFs
- [ ] Plugin system for custom transforms

---

## License

MIT © [Ego Hygiene](https://github.com/egohygiene)
