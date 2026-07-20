# AI Configuration Reference

        AI transform definitions live in the transform YAML file, not in the top-level Renderflow config.

        ## AI transform fields

        | Key | Required | Notes |
        |---|---|---|
        | `name` | Yes | human-readable transform name |
        | `ai` | Yes | `ollama` or `openai` |
        | `model` | Usually | model identifier passed to the provider |
        | `prompt` | Usually | prompt template containing optional `{input}` |
        | `endpoint` | No | overrides backend base URL |
        | `api_key` | No | plaintext fallback, discouraged |
        | `api_key_env` | No | environment variable name for key lookup |
        | `artifact_path` | No | also save generated output to a file |
        | `cache_path` | No | AI cache file path |
        | `prompt_version` | No | extra cache-busting dimension |
        | `from` / `to` | Yes | graph/source and target formats |
        | `cost` / `quality` | Yes | planner metadata |

        ## Example: Ollama

        ```yaml
        transforms:
          - name: local-summary
            ai: ollama
            model: mistral
            prompt: "Summarize:

{input}"
            endpoint: http://localhost:11434
            from: markdown
            to: markdown
            cost: 1.5
            quality: 0.8
        ```

        ## Example: OpenAI-compatible

        ```yaml
        transforms:
          - name: polish-copy
            ai: openai
            model: gpt-4o-mini
            prompt: "Rewrite for clarity:

{input}"
            api_key_env: OPENAI_API_KEY
            cache_path: .renderflow-ai-cache.json
            prompt_version: v2
            from: markdown
            to: markdown
            cost: 2.0
            quality: 0.9
        ```

        !!! warning
            Prefer `api_key_env` so credentials stay out of version-controlled YAML.
