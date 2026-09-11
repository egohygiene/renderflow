# Model catalog and AI skills

Renderflow separates stable artifact intent from replaceable model adapters:

```text
profile or artifact transform
  -> versioned AI skill and JSON contracts
  -> deterministic policy/capability resolver
  -> model-specific catalog entry
  -> configured local or explicitly approved remote adapter
  -> validated candidate artifact and redacted execution evidence
  -> human review and publication hygiene
```

The contracts are:

- `renderflow.ai-model-catalog/v1` — providers, runtimes, individual models,
  modalities, operations, limits, availability, licenses, determinism, and
  advisory evidence;
- `renderflow.ai-skill/v1` — reviewed instructions, bounded variables, schemas,
  budgets, hygiene, validation, provenance, and approval policy;
- `renderflow.ai-resolution/v1` — deterministic selection plus an explanation
  for every rejected, unavailable, or lower-ranked candidate;
- `renderflow.ai-execution/v1` — redacted identities, digests, usage,
  validators, hygiene outcomes, candidate state, and approval evidence.

The canonical JSON Schemas live in `schemas/`. Built-in catalog and skill assets
live under `crates/renderflow-core/data/ai/` and are parsed through the same SDK
types exposed to callers.

## Local-first resolution

`local-preferred` is the default. A local model must still match every required
operation and input/output modality. `local-only` rejects every remote model and
never silently falls back. Remote execution requires all of the following:

1. `--allow-remote` or the equivalent SDK request permission;
2. a skill whose network and remote-execution budgets permit it;
3. an available configured provider adapter;
4. explicit privacy approval when detected PII could leave the machine;
5. approval for every input artifact exposed to the model.

The bundled catalog begins with Ollama and an OpenAI-compatible adapter, but the
contract can represent llama.cpp servers, vLLM, local OpenAI-compatible servers,
image graphs, and speech runtimes without changing profile contracts. Renderflow
does not bundle model weights.

`unverified` means the catalog entry is structurally usable for planning but is
not execution-ready. Live discovery should replace it with bounded evidence for
the exact runtime, model revision, digests, source, and licenses. Unknown data
must remain unknown; never invent a digest, revision, commercial-use grant, or
provider-terms version.

## Adding a provider or model

To add an adapter, implement `AiProvider`, give the adapter a stable name, and
add a provider entry with locality, network requirements, runtime identity, and
a bounded availability probe. Add each model separately. Do not copy a
provider-wide capability set onto models that do not actually support it.

Each model entry must include:

- stable model and family IDs;
- typed input/output modalities and operations;
- structured JSON and JSON Schema behavior;
- limits, controls, multi-input, streaming, batch, and tool-use behavior;
- an honest determinism class;
- availability plus runtime/model/weight identity evidence;
- model, weight, code, and provider licensing evidence where known;
- commercial constraints and human-review requirements;
- timestamped cost, quality, and latency hints when supplied;
- maturity, conformance, and required provenance fields.

Validate custom catalogs with `renderflow ai matrix --catalog FILE --format
json`. Catalog parsing rejects unknown fields and duplicate IDs.

## Adding a skill

A skill is an inspectable execution recipe, not an autonomous agent and not an
opaque provider prompt. Start from one of the bundled synthetic skills and:

1. choose a stable `skill.*` ID and semantic version;
2. declare artifact families, operations, and typed modalities;
3. define strict input and output JSON Schemas;
4. declare every `{{variable}}`, sensitivity, and byte bound;
5. use provider-neutral generation settings; namespace optional adapter
   extensions instead of leaking them into profile contracts;
6. set network, locality, byte, token, time, retry, and cost budgets;
7. configure secret, PII, protected-reference, and post-output hygiene;
8. keep generated output in `candidate` state and name its validators/review;
9. declare cache/evidence identity and redact raw inputs/prompts by default;
10. add a redistribution-safe synthetic fixture.

The v1 runtime supports a deliberately conservative JSON Schema subset and
rejects unsupported schema keywords rather than pretending they were enforced.
Use `renderflow ai skills validate --path FILE` before registering an external
skill.

## Execution, validation, and evidence

The SDK `AiSkillRuntime` validates input, resolves the exact model, applies
pre-prompt hygiene, executes through `AiProvider`, parses and sanitizes the
structured response, validates the output schema, and stores it in the artifact
store as an intermediate candidate. It never marks generated content as
publication-approved.

Cache and resume identity commits to the catalog revision, provider, runtime,
model and weight identities, skill version/content, input, schemas, settings,
and hygiene policy. Reusing that fingerprint means the configuration is
compatible; it does not claim byte-level reproducibility for a probabilistic
model. Execution evidence stores digests instead of raw private prompts or
source payloads, and includes usage/cost fields only when the adapter can report
them.

Protected-reference rewrites replace configured imitation labels with reviewed,
descriptive characteristics before model exposure and again after generation.
Evidence records the finding class and action without publishing the blocked
term. Automated hygiene, model metadata, and provider terms are not legal
clearance.

## Initial proving skills and downstream use

The built-in fixtures require no paid API or bundled weights:

- `skill.metadata.extract`;
- `skill.visual-dna.describe`;
- `skill.prompt.from-sanitized-dna`;
- `skill.accessibility.describe-candidate`.

The visual skills reference the original geometric
`data/ai/fixtures/synthetic-layout.svg` asset by SHA-256 digest. Validation does
not download or execute a model.

Visual DNA and accessibility skills may resolve to a compatible local
multimodal model when one is installed and discovered. Otherwise, resolution
returns structured unavailability evidence and deterministic/non-AI publication
paths remain usable.

Artifact DNA (#387) can consume the visual-description and sanitized-prompt
contracts. The coloring-book profile (#348) can use the same layer for optional
line-art planning while retaining a deterministic non-AI path. Neither consumer
should copy provider prompts or treat an AI candidate as authoritative.

## Rollback and reproduction

Keep the source artifact, skill version, catalog revision, schemas, settings,
hygiene policy, model/runtime identity, and candidate evidence together. Roll
back by selecting an earlier reviewed candidate or disabling the optional AI
stage. Re-running the same fingerprint reproduces the configuration and audit
trail, but probabilistic byte output may differ and must be validated again.
