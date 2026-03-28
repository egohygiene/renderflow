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

- 📄 **Multi-format output** — Render to PDF, HTML, and DOCX from a single config file
- 🗂️ **YAML-driven spec** — Declarative, repeatable, version-controllable builds
- 🖼️ **Asset management** — Automatically resolves and validates image paths
- 🔄 **Transform pipeline** — Pluggable in-memory content transforms
- 🧩 **Custom templates** — Per-output Jinja2-compatible templates via [Tera](https://keats.github.io/tera/)
- 🔍 **Dry-run mode** — Preview what will be built without writing any files
- 👁️ **Watch mode** — Automatically rebuild on file changes
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

### Multi-output example

Produce PDF, HTML, and DOCX from one config:

```yaml
input: "report.md"
output_dir: "dist"
outputs:
  - type: pdf
  - type: html
    template: "default"
  - type: docx
```

### Variables example

Inject dynamic values that are replaced at build time:

```yaml
input: "report.md"
output_dir: "dist"
variables:
  title: "Q4 Report"
  author: "Jane Smith"
outputs:
  - type: html
```

`report.md`:

```markdown
# {{title}}

*Written by {{author}}*
```

### Template example

Point an output at a custom Tera template stored in `templates/`:

```yaml
input: "report.md"
output_dir: "dist"
outputs:
  - type: html
    template: "newsletter"
```

Renderflow will render using `templates/newsletter.html` (Jinja2-compatible [Tera](https://keats.github.io/tera/) syntax).

---

## Watch Mode

Watch mode monitors your source files and automatically rebuilds whenever a change is detected.

```bash
# Watch using the default renderflow.yaml config
renderflow watch

# Watch using a custom config file
renderflow watch my-project.yaml

# Override the debounce delay (default: 500 ms)
renderflow watch my-project.yaml --debounce 300
```

**How it works:**

1. An initial build runs immediately when watch mode starts.
2. Renderflow watches the config file, the input document, and the `templates/` directory for changes.
3. After a file change is detected, Renderflow waits for the debounce delay (default: 500 ms) before triggering a rebuild — so rapid successive saves don't cause redundant builds.
4. Build errors are logged but do not stop the watcher; the next save will trigger another attempt.

**File watching scope:**

| Watched path       | Mode        | Notes |
|--------------------|-------------|-------|
| Config file        | Non-recursive | e.g. `renderflow.yaml` |
| Input document     | Non-recursive | Path from the `input` key |
| `templates/` dir   | Recursive   | Watched when the directory exists |

Press **Ctrl+C** to stop watch mode.

---

## Configuration

Renderflow is entirely driven by a YAML spec file (default: `renderflow.yaml`):

```yaml
input: "input.md"         # Path to your source document
input_format: markdown    # Optional: override auto-detected format
output_dir: "dist"        # Output directory (default: dist)
variables:                # Optional: key/value pairs for substitution
  title: "My Document"
  author: "Jane Smith"
outputs:
  - type: pdf             # Render to PDF (requires Pandoc + Tectonic)
  - type: html            # Render to HTML
    template: "default"   # Optional: use a custom Tera template
  - type: docx            # Render to Word document
```

### Configuration Reference

| Key            | Required | Default      | Description |
|----------------|----------|--------------|-------------|
| `input`        | ✅ Yes   | —            | Path to the source document (Markdown, HTML, RST, etc.) |
| `input_format` | ❌ No    | auto-detect  | Override the input format; auto-detected from file extension when omitted |
| `output_dir`   | ❌ No    | `dist`       | Directory where output files are written |
| `outputs`      | ✅ Yes   | —            | List of one or more output targets (must contain at least one entry) |
| `outputs[].type` | ✅ Yes | —           | Output format: `html`, `pdf`, or `docx` |
| `outputs[].template` | ❌ No | —        | Name of a Tera template in the `templates/` directory to use for this output |
| `variables`    | ❌ No    | `{}`         | Map of string key/value pairs injected into the document via `{{key}}` placeholders |

### Supported Input Formats

The `input_format` key (or the file extension of `input`) controls how Pandoc reads the source document.

| Value      | File Extensions   | Notes |
|------------|-------------------|-------|
| `markdown` | `.md`, `.markdown`| Default when extension is unknown |
| `html`     | `.html`, `.htm`   | |
| `rst`      | `.rst`            | reStructuredText |
| `docx`     | `.docx`           | |
| `epub`     | `.epub`           | |
| `latex`    | `.tex`            | |

When `input_format` is omitted, Renderflow auto-detects the format from the file extension and falls back to `markdown` when the extension is unrecognised.

### Supported Output Types

| Type   | Description                   | Requirements      |
|--------|-------------------------------|-------------------|
| `html` | Renders to HTML               | Pandoc            |
| `pdf`  | Renders to PDF via LaTeX      | Pandoc + Tectonic |
| `docx` | Renders to Word document      | Pandoc            |

Not every input → output combination is supported. For example, `epub` and `latex` inputs cannot currently be converted to `docx`. Renderflow reports a clear error when an unsupported combination is specified.

### Templates

Templates live in a `templates/` directory and use [Tera](https://keats.github.io/tera/) syntax (Jinja2-compatible). Specify a template per output with the `template` key. A `default` HTML template is included out of the box.

### Variables

Define a `variables` map in your config to inject dynamic values into your document:

```yaml
variables:
  title: "Q4 Report"
  author: "Jane Smith"
  version: "1.0"
```

Reference them in your Markdown using `{{key}}` syntax:

```markdown
# {{title}}

*Written by {{author}}*

Version: {{version}}
```

Placeholders for undefined keys are left unchanged, and a warning is emitted so you can spot typos.

---

## Transforms

Before any output is written, Renderflow applies a series of in-memory text transforms to the source document. Transforms run in order after the file is read and before Pandoc processes it.

### EmojiTransform

Replaces emoji characters with the literal text `[emoji]`.

**Why:** PDF and some LaTeX backends cannot render Unicode emoji directly. This transform ensures the pipeline doesn't crash on emoji-heavy content.

**Example:**

| Input                  | Output                  |
|------------------------|-------------------------|
| `Hello 😀 World`       | `Hello [emoji] World`   |
| `🎉 Party time! 🎉`   | `[emoji] Party time! [emoji]` |

**Limitation:** The replacement is a plain-text placeholder. Full SVG/image-based emoji embedding is planned for a future release.

### VariableSubstitutionTransform

Replaces `{{key}}` placeholders in the source document with values defined in the `variables` map of your config.

**When it runs:** Before Pandoc, so substituted values are part of the rendered content.

**Behaviour:**

- Keys are matched exactly (whitespace around the key name is trimmed, so `{{ title }}` and `{{title}}` are equivalent).
- If a placeholder references a key that is not in `variables`, the placeholder is left unchanged and a warning is emitted.
- Unclosed placeholders (e.g. `{{unclosed`) are also left unchanged.

**Example:**

Config:
```yaml
variables:
  title: "Annual Report"
  year: "2024"
```

Document:
```markdown
# {{title}} — {{year}}
```

Rendered:
```markdown
# Annual Report — 2024
```

### SyntaxHighlightTransform

Normalises the language tags on fenced code blocks (` ``` `) to lowercase with surrounding whitespace stripped.

**When it runs:** Before Pandoc, ensuring consistent language identifiers are passed to the syntax highlighting engine.

**Example:**

| Input fence       | Normalised fence    |
|-------------------|---------------------|
| ` ```Rust `       | ` ```rust `         |
| ` ```  Python  `  | ` ```python `       |
| ` ```JavaScript ` | ` ```javascript `   |

**Limitation (V1):** Only the opening fence language tag is normalised. The code body itself is passed through unchanged.

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

- [ ] Built-in stylesheet themes
- [ ] SVG / emoji embedding in PDFs
- [ ] Plugin system for custom transforms
- [ ] Automated release workflow for pre-built binaries

---

## License

MIT © [Ego Hygiene](https://github.com/egohygiene)
