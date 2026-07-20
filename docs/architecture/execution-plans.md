# Execution Plans

`ExecutionPlan` is the canonical, presentation-independent view of a planned graph execution.

## Plan contents

- source format
- target formats
- optimization mode
- plan nodes
- plan edges
- execution waves
- metadata
- diagnostics

## Node and edge classification

Nodes are labeled as:

- `source`
- `intermediate`
- `output`

Edges are labeled as:

- `lossless`
- `lossy`
- `aggregation`

## Metadata fields

`ExecutionMetadata` includes:

- total nodes / edges
- execution depth
- execution waves
- estimated cost
- estimated quality
- reused intermediates
- output count

## Rendering formats

`renderflow graph plan` and `renderflow graph export` support:

- text
- JSON
- YAML
- Mermaid
- DOT
- Markdown

These renderers live in `src/graph/renderers/`.

## Diagnostics

Planner diagnostics are used by:

- `graph explain`
- `graph doctor`

They surface lossy edges, reuse observations, and plan-level warnings/errors in a human-readable form.
