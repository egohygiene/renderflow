# renderflow
✨ Spec-driven document rendering engine (Markdown → PDF/HTML/LaTeX)

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

### Build from source (requires [Rust](https://rustup.rs))

```bash
cargo install --path .
```

## Usage

```bash
# Render using the default renderflow.yaml config
renderflow build

# Render using a custom config file
renderflow build --config custom.yaml

# Shorthand: pass the config file directly (equivalent to renderflow build --config my-project.yaml)
renderflow my-project.yaml

# Preview what would be built without creating any files
renderflow build --dry-run
```

> **Note:** The argument to `renderflow` is always a YAML configuration file. The Markdown file to render is specified inside the config under the `input` key (e.g. `input: "input.md"`).

### Configuration

Renderflow is driven by a YAML configuration file (default: `renderflow.yaml`):

```yaml
input: "input.md"
output_dir: "dist"
outputs:
  - type: pdf
  - type: html
```
