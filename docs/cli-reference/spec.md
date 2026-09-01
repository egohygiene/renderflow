# `renderflow spec`

Renderflow spec commands inspect, validate, migrate, and export the canonical execution specification contract.

## Validate a configuration

```bash
renderflow spec validate --config renderflow.yaml
renderflow spec validate --config renderflow.yaml --format json
```

Unversioned files are validated through the explicit v1 compatibility path. Files declaring `schema: renderflow/v2` are parsed and semantically validated as v2. Unsupported declared schema identifiers fail actionably.

## Migrate v1 to v2

```bash
renderflow spec migrate --config renderflow.yaml --output renderflow.v2.yaml
```

Migration is deterministic. It preserves the v1 source path, output targets, output directory, variables, transform registry path, optimization mode, templates, and output profiles while making v2 policy defaults explicit.

## Export the JSON Schema

```bash
renderflow spec schema --output schemas/renderflow-v2.schema.json
renderflow spec schema --format yaml
```

The runtime-emitted schema is the canonical machine-readable contract. Documentation CI regenerates the checked-in schema and reference page from this command to prevent drift.
