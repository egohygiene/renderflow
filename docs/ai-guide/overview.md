# AI Guide Overview

Renderflow treats AI as just another transform backend.

## Supported scenarios

- summarization before rendering
- translation as a preprocessing step
- style or tone rewriting
- structured post-processing when routed through an OpenAI-compatible backend

## Provider model

Providers implement `AiProvider` and advertise:

- `name`
- whether they are local
- capability set
- model list
- execution method

`OllamaProvider` is local-first; `OpenAiProvider` targets OpenAI-compatible APIs.
