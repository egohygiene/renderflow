# `renderflow ai`

Inspect built-in AI providers and cache state.

## Subcommands

### `ai providers`

Prints provider name, locality, and declared capabilities.

### `ai models`

Prints each provider's built-in model list.

### `ai doctor`

```bash
renderflow ai doctor [--ollama-endpoint URL]
```

Checks:

- connectivity to the Ollama endpoint,
- whether `OPENAI_API_KEY` is set.

### `ai cache`

```bash
renderflow ai cache [--path FILE]
```

Reads the cache file and prints entry counts by model. The default path is `.renderflow-ai-cache.json`.

## Examples

```bash
renderflow ai providers
renderflow ai models
renderflow ai doctor --ollama-endpoint http://localhost:11434
renderflow ai cache --path .renderflow-ai-cache.json
```
