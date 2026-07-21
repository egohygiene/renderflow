# FAQ

## Is Renderflow only for PDF generation?

No. Renderflow handles standard document outputs such as HTML, PDF, and DOCX, plus FFmpeg-backed audio and image workflows.

## When should I use standard build mode instead of graph mode?

Use `renderflow build` when you already know the outputs you want. Use graph mode (`--target` or `--all`) when targets should be discovered through a transform graph.

## Does Renderflow support version-controlled, repeatable builds?

Yes. The core workflow is a checked-in YAML specification plus deterministic transforms, optional caching, and explicit optimization modes.

## Where is the authoritative list of supported formats?

See [Supported Formats](supported-formats.md). That page is generated from the Rust source so it stays aligned with the implementation.

## How do I debug planner behavior?

Use the graph tooling:

```bash
renderflow graph plan
renderflow graph explain
renderflow inspect --all
```

## How do plugins work with the CLI?

The CLI itself starts with an empty plugin registry. Plugins are most useful when Renderflow is embedded as a library and a host application registers executors and metadata first.

## How should I store AI credentials?

Prefer environment variables and `api_key_env`. Avoid committing plaintext API keys into YAML files.

## Where should contributors start?

Start with:

- [Installation](../getting-started/installation.md)
- [Quick Start](../getting-started/quickstart.md)
- [Configuration](configuration.md)
- [Architecture Overview](../architecture/overview.md)
