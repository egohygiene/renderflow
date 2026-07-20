# Caching and Incremental Builds

Renderflow uses multiple cache layers.

## Standard build caches

All of these live under `output_dir` during standard builds:

| File | Purpose |
|---|---|
| `.renderflow-cache.json` | transform-phase cache |
| `.renderflow-output-cache.json` | final rendered output cache |
| `.renderflow-deps.json` | file dependency map |

### Transform cache

The transform cache key includes:

- normalized input content
- raw config file content
- sorted variables
- output format suffix in the build loop

This lets Renderflow skip repeated built-in transforms on unchanged inputs.

### Output cache

The output cache key includes:

- transformed content
- output type
- template name
- template file content

This means editing a template invalidates the cache even if the template name stays the same.

### Dependency map

The dependency map records the content hash of:

- the input file
- the config file
- the template file, when one is used

Watch mode uses this map to report which outputs are affected by a changed file.

## Graph build cache

Graph execution stores `.renderflow-dag-cache.json` in `output_dir`. It hashes each DAG node by input content plus `from`/`to` format identifiers.

## AI cache

AI transforms can use a separate cache file, typically `.renderflow-ai-cache.json`. The hash includes the fully rendered prompt and model identifier.

## Watch mode

Watch mode still performs a rebuild cycle after changes, but resilient transform handling plus caches keep rebuilds fast and recoverable.

!!! note
    Corrupt or missing cache files do not abort the build. Load failures are logged at `WARN` level and Renderflow falls back to an empty cache.
