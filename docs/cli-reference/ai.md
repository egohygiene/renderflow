# `renderflow ai`

Inspect model-specific capabilities, resolve reviewed skills, diagnose providers,
and inspect cache state. Inspection commands do not execute a model.

## Subcommands

### `ai matrix`

```bash
renderflow ai matrix [--catalog FILE] [--format text|json|yaml]
```

Displays the versioned compatibility catalog at model granularity, including
typed modalities, operations, structured-output support, availability,
determinism, licenses, runtime evidence, limits, and advisory cost/quality hints.
Bundled entries are `unverified`: a declaration never claims that weights are
installed or that a hosted endpoint is authorized.

### `ai resolve`

```bash
renderflow ai resolve \
  --skill skill.metadata.extract \
  --execution-preference local-preferred \
  --allow-unverified \
  --format json
```

Returns the selected provider/model plus explicit rejection or lower-ranking
reasons for every other catalog candidate. `--allow-remote` is an explicit
request-level permission; it cannot override a skill that forbids network or
remote execution. `local-only` never falls back to a hosted service.

### `ai skills`

```bash
renderflow ai skills list
renderflow ai skills inspect skill.visual-dna.describe --format json
renderflow ai skills validate
renderflow ai skills validate --path custom-skill.json --format json
```

Skills are versioned Renderflow recipes with strict input/output schemas,
reviewed templates, bounded variables, budgets, hygiene, provenance, validators,
and candidate/approval policy. They are not provider-specific prompt strings.

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
renderflow ai matrix --format json
renderflow ai resolve --skill skill.metadata.extract --allow-unverified --format json
renderflow ai skills validate
renderflow ai doctor --ollama-endpoint http://localhost:11434
renderflow ai cache --path .renderflow-ai-cache.json
```
