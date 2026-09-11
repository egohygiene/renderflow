# AI Guide Overview

Renderflow treats AI as an optional, governed artifact provider. Profiles and
transforms request capabilities; they do not need to name a hosted vendor.

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

Provider-wide capability claims are retained for compatibility, but planning
uses the model compatibility catalog. Image support on one model therefore does
not imply image support on every model behind the same endpoint.

See [Model catalog and AI skills](model-catalog-and-skills.md) for the resolver,
schema contracts, hygiene gates, candidate artifacts, and provenance model.
