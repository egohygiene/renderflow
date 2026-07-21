# Renderflow

Renderflow is a spec-driven rendering engine for turning a single source document into repeatable outputs such as HTML, PDF, DOCX, audio, and images. It combines a YAML configuration file, an in-memory transform pipeline, and a DAG-based planner for graph-driven conversions.

!!! note
    Use the version selector in the site header to switch between the latest published docs and tagged release snapshots.

## Why Renderflow?

- **One YAML spec** describes inputs, variables, templates, outputs, optimization mode, and optional transform graph files.
- **Multi-format output** supports standard document rendering plus FFmpeg-backed audio and image pipelines.
- **Graph-aware planning** can resolve `source -> ... -> target` paths, reuse intermediates, and execute independent waves in parallel.
- **Built-in transforms** normalize code fences, substitute variables, and handle emoji safely for non-HTML targets.
- **Extensibility** comes from YAML-defined command transforms, AI transforms, and a runtime plugin API.
- **Fast rebuilds** use content hashes, dependency tracking, and watch mode.

!!! tip
    Use standard `renderflow build` when you already know your output list, and use `renderflow build --target ...` or `renderflow build --all` when you want graph-based path resolution from a transform YAML file.

## Quick start in 30 seconds

1. Install Renderflow.
2. Create `input.md`.
3. Create `renderflow.yaml`.
4. Run `renderflow build`.
5. Open the generated file from `dist/`.

```yaml
input: input.md
output_dir: dist
outputs:
  - type: html
```

## Core capabilities

### Standard build pipeline

For document builds, Renderflow runs a two-phase pipeline:

1. **Transform phase**: emoji handling, variable substitution, syntax-highlight normalization, then any optional YAML-defined transforms.
2. **Render phase**: output-specific strategy execution for HTML, PDF, DOCX, audio, or image outputs.

### Graph pipeline

When a config includes `transforms: path/to/transforms.yaml`, Renderflow can build a transformation graph where formats are nodes and transforms are weighted edges. The planner can then:

- find reachable targets,
- pick paths according to `speed`, `quality`, `balanced`, or `pareto`,
- merge shared intermediates into a single DAG,
- execute waves in parallel with Rayon.

### AI and plugins

- **AI transforms** support `ollama` and `openai` backends, prompt templates, cache files, artifact output, and API key resolution via environment variables.
- **Plugins** are runtime extensions that implement the `PluginExecutor` trait and register metadata/capabilities in a `PluginRegistry`.

## Start here

- [Installation](getting-started/installation.md)
- [Quick Start](getting-started/quickstart.md)
- [Configuration](user-guide/configuration.md)
- [Supported Formats](user-guide/supported-formats.md)
- [CLI Reference](cli-reference/index.md)
- [Architecture Overview](architecture/overview.md)
