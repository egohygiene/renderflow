<!-- GENERATED FILE: run scripts/generate_spec_v2_reference.py -->
# Renderflow spec v2 reference

This page is generated from the JSON Schema emitted by the Renderflow runtime.
Do not edit it by hand.

**Schema identifier:** `renderflow/v2`

Spec v2 describes source intent, derivative selection, execution policy, and deterministic output layout. Planning resolves this intent into an execution plan; the specification itself does not encode a resolved DAG.

## Top-level fields

| Field | Type | Required | Default |
| --- | --- | --- | --- |
| `execution` | `executionPolicy` | no | — |
| `output` | `outputLayout` | no | — |
| `profiles` | `object` | no | `{}` |
| `schema` | `"renderflow/v2"` | yes | — |
| `sources` | `array` | yes | — |
| `targets` | `targetSelection` | yes | — |
| `transforms` | `string` / `null` | no | — |
| `variables` | `object` | no | `{}` |

## Source

| Field | Type | Required | Default |
| --- | --- | --- | --- |
| `detect` | `boolean` | no | `true` |
| `format` | `string` / `null` | no | — |
| `id` | `stableId` | yes | — |
| `immutable` | `true` | no | `true` |
| `kind` | `artifact` / `collection` | no | `"artifact"` |
| `media_type` | `string` / `null` | no | — |
| `members` | `array` | no | `[]` |
| `path` | `string` / `null` | no | — |
| `role` | `string` / `null` | no | — |
| `uri` | `string` / `null` | no | — |

## Target selection

| Field | Type | Required | Default |
| --- | --- | --- | --- |
| `all_reachable` | `boolean` | no | `false` |
| `exact` | `array` | no | `[]` |
| `exclude` | `selectorSet` | no | — |
| `include` | `selectorSet` | no | — |
| `intermediates` | `cache_only` / `retain` | no | `"cache_only"` |
| `profiles` | `array` | no | `[]` |

## Execution policy

| Field | Type | Required | Default |
| --- | --- | --- | --- |
| `ai` | `deny` / `local_only` / `allow` | no | `"deny"` |
| `budgets` | `budgets` | no | — |
| `max_parallel` | `integer` | no | `1` |
| `minimum_fidelity` | `number` / `null` | no | — |
| `network` | `deny` / `allow` | no | `"deny"` |
| `optimization` | `speed` / `quality` / `balanced` / `pareto` | no | `"balanced"` |
| `publication_policy` | `string` / `null` | no | — |
| `redaction_policy` | `string` / `null` | no | — |
| `requirements` | `requirements` | no | — |
| `retry_policy` | `string` / `null` | no | — |
| `timeout_policy` | `string` / `null` | no | — |
| `tools` | `allowDeny` | no | — |
| `transforms` | `allowDeny` | no | — |
| `validation` | `validation` | no | — |

## Output layout

| Field | Type | Required | Default |
| --- | --- | --- | --- |
| `bundle_root` | `string` | no | `"dist"` |
| `collision` | `error` / `replace` / `dedupe` | no | `"error"` |
| `naming_template` | `string` | no | `"{source.id}/{target.role}.{ext}"` |

## Compatibility

Unversioned configuration files are treated as the explicit v1 compatibility format. Use `renderflow spec migrate` to produce a v2 document. Unsupported declared schema identifiers are rejected rather than reinterpreted.

## Example

```yaml
schema: renderflow/v2

sources:
  - id: source.cover
    role: cover
    path: assets/cover.png
    media_type: image/png
    detect: true

  - id: source.body
    role: manuscript
    path: examples/input.md
    format: markdown
    detect: true

  - id: source.publication
    role: publication
    kind: collection
    members:
      - source.cover
      - source.body

profiles:
  publication.web:
    description: Browser-ready publication derivatives
    targets:
      - id: target.web
        role: web
        format: html
  publication.archive:
    description: Long-lived local archival derivatives
    targets:
      - id: target.pdf
        role: archival
        format: pdf

targets:
  profiles:
    - publication.web
  all_reachable: true
  include:
    families:
      - document
      - image
  exclude:
    capabilities:
      - ai.generate
  intermediates: cache_only

execution:
  optimization: balanced
  max_parallel: 4
  budgets:
    max_output_bytes: 1073741824
    max_storage_bytes: 2147483648
    max_artifacts: 500
    max_depth: 8
  tools:
    deny: []
  transforms:
    deny: []
  requirements:
    deterministic: true
    local_only: true
    offline: true
  network: deny
  ai: deny
  validation:
    required: true
  minimum_fidelity: 0.9

output:
  bundle_root: dist
  naming_template: "{source.id}/{target.role}.{ext}"
  collision: error

variables:
  project: renderflow

transforms: transforms.yaml
```
