# AI Transforms

Renderflow supports AI-backed transforms through `AiTransform` and the `renderflow ai` command group.

## Supported backends

| Backend | Default endpoint | Notes |
|---|---|---|
| `ollama` | `http://localhost:11434` | local-first provider, `POST /api/generate` |
| `openai` | `https://api.openai.com` | OpenAI-compatible chat completions, `POST /v1/chat/completions` |

Built-in model lists shown by `renderflow ai models` include:

- Ollama: `mistral`, `llava`, `llama3`, `gemma`, `phi`
- OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`

## YAML example

```yaml
transforms:
  - name: summarize
    ai: openai
    model: gpt-4o-mini
    prompt: |
      Summarize the following document for release notes:

      {input}
    api_key_env: OPENAI_API_KEY
    cache_path: .renderflow-ai-cache.json
    prompt_version: v1
    from: markdown
    to: markdown
    cost: 2.0
    quality: 0.9
```

## Prompt rendering

Renderflow substitutes `{input}` into the prompt template before sending the request.

## Security guidance

- prefer `api_key_env` over `api_key`
- plaintext keys are supported only as a compatibility fallback
- keys are never logged

## Retries and metrics

`AiTransform` can execute through the provider abstraction with retry logic and shared metrics sinks. That internal plumbing is what enables cache keys, advisory diagnostics, and cost tracking.

## CLI commands

- `renderflow ai providers`
- `renderflow ai models`
- `renderflow ai doctor --ollama-endpoint ...`
- `renderflow ai cache --path .renderflow-ai-cache.json`

!!! warning
    AI transforms are optional and only run when configured. In fail-fast mode a backend outage aborts the build; in watch mode the transform is skipped and the original content continues through the pipeline.
