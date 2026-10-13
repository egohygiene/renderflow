# Derivative forest planning

The canonical planner selects a finite set of source-rooted derivative routes. It does not
execute every graph edge or recursively feed every new output back into target discovery.
The contract is `renderflow/artifact-forest-v1`, with selection bounds under
`execution.derivative_planning` using `renderflow/derivative-policy-v1`.

This implementation and its regression fixtures are authored but unqualified. Tests,
compilation, native tool execution, and CI were deferred for the implementation sprint.
Selected paths and provider availability describe planning decisions; they do not establish
successful conversion, measured fidelity, or runtime qualification.

## Target identities

A target is more than an extension. Its stable identity includes canonical format,
publication ID and role, preset, template, variant, options, and explicit capability,
transform, and provider constraints, including effective include/exclude selectors.
Repeating the same effective target is deduplicated.
Different IDs or roles represent distinct publication destinations; different supported
presets or variants represent distinct renditions even when their extensions match.
Required versus optional status does not change an artifact's identity.

Source identity remains separate. The forest digest binds the actual source evidence,
effective policy, target identities, selected routes, and selected toolchain evidence.
Toolchain fingerprints include every selected primary and required provider, plus the union
of variant evidence across all routes. A format-only graph projection cannot replace this
identity material.

Use `{target.id}`, `{target.identity}`, or distinct `{target.role}` values in the output
naming template for multiple outputs of one format. `{target.preset}` and `{target.variant}`
are also available. Two selected targets cannot overwrite the same destination within one
run, including when `collision: replace` is configured; use distinct names or `collision:
dedupe`.

## Eligible routes and ranking

Every candidate route starts at the detected original source. A route may visit each format
at most once, so reversible transforms such as Markdown → HTML → Markdown cannot recurse.
The target is terminal. A selector requiring a particular provider, transform, capability,
or variant must match the terminal edge; finding some other edge into the same format does
not satisfy that selector.

Exhaustive expansion and named-profile planning rank eligible paths in this order:

1. Highest cumulative declared quality: the product of edge quality estimates.
2. Fewest transform steps, preferring direct source conversions when quality is equal.
3. Lowest weighted cost for the configured optimization mode.
4. Stable complete edge identity, making remaining ties independent of registration order.

A higher-quality multistep path can therefore outrank a lower-quality direct path. Shared
prefixes may be reused only when the exact edge and rendering configuration agree. Distinct
renditions retain their own selected route instead of overwriting a format-keyed output.
Exact target-only CLI/SDK requests retain optimization cost as the first criterion,
followed by cumulative quality, hops, and stable identity. Legacy graph pathfinding APIs
also retain their targeted optimization behavior. Both policies keep the same simple-path,
terminal-selector, and search bounds, and branch evidence records the ranking used.

```yaml
execution:
  optimization: balanced
  derivative_planning:
    schema: renderflow/derivative-policy-v1
    max_hops: 8
    max_candidates: 10000
    max_loss: 0.25
```

`max_hops` defaults to 8 and accepts 1–64. `max_candidates` defaults to 10,000 and accepts
1–1,000,000. It limits examined edge candidates, including rejected cycles and terminal
selectors, rather than only successful complete paths. A search that reaches this work
limit cannot claim a winning route. Increase the bound or narrow the target/graph when
inspection reports that limit.

If `execution.budgets.max_depth` is also set, the effective route hop limit is the smaller
of that value and `max_hops`; the forest records this effective bound.

`max_loss` is optional and must be finite in 0–1. Cumulative loss is
`1 - product(edge quality)`; the existing `minimum_fidelity` policy remains a separate
per-edge gate. These values are planning declarations, not perceptual quality measurements.
Runtime output validation and the existing resource budgets still apply independently.

## Source exclusion and diagnostics

`all_reachable` and the bundled `everything` profile exclude the source-format identity.
Explicitly requesting the source format with no rendering changes selects the original
artifact through a zero-edge route. This does not regenerate it or permit a format cycle.
Requests to change source-format rendering require a supported transformation; the source
artifact must not be mislabeled as a transformed result.

Branches retain an explicit state and reason code:

| State | Meaning |
| --- | --- |
| `selected` | An eligible route and available provider set were selected. |
| `excluded` | Source identity or selectors excluded the branch. |
| `unavailable` | The requested route requires an unavailable provider. |
| `unsupported` | No supported route/configuration satisfies the target and planning policy. |
| `budget_pruned` | A resource budget removed the branch. |

Unselected branches are inspection evidence and never fabricated outputs. A dry-run can
explain unsupported or unavailable intent; execution must still honor required targets and
branch-local failure policy. Run manifests identify artifacts actually published separately
from planned targets.

Supported presets and templates remain adapter-specific. Generic target `options` have an
identity representation but are not implemented by canonical execution and produce an
unsupported diagnostic. Native screenplay import and PDF policies keep their existing
original-source, fixed-profile, and opt-in restrictions. Planning does not install or qualify
providers or enable AI/network execution.

## Inspect before execution

Set the source path in `examples/derivative-forest.yaml`, then inspect the same canonical
plan used by execution:

```bash
renderflow graph plan --config examples/derivative-forest.yaml --format json
renderflow graph plan --config examples/derivative-forest.yaml --format text
renderflow build --config examples/derivative-forest.yaml --dry-run
```

The SDK's `Engine::plan` returns the same `ExecutionPlan.artifact_forest` evidence. Each
selected branch records ordered steps with provider/capability/variant evidence, hop count,
cumulative quality/loss, weighted cost, ranking policy, and examined candidate count. JSON
and YAML carry the full contract; text and Markdown include readable branch summaries.
Format graph diagrams are projections and cannot distinguish every rendition on their own.

The forest schema is published at
[`schemas/renderflow-artifact-forest-v1.schema.json`](../../schemas/renderflow-artifact-forest-v1.schema.json)
and embedded in the run manifest contract. These describe evidence shape; they do not
verify source bytes, execute transforms, or establish that outputs were produced.

## Observed publication evidence

Run manifests add an optional `execution` object using
`renderflow/forest-execution-v1`. Every planned target receives an explicit
`not_selected`, `planned`, `published`, or `not_published` outcome. A published
row references the same run's exact artifact inventory index, ID and bundle
locator; its full record retains hashes, producer, lineage, validation and
loss/review evidence. Planning states and the frozen forest identity remain
unchanged. No publication reference is attached to dry-run intent.

`produced_artifacts` now means unique published terminal derivative IDs; it
excludes source inventory, intermediates and unpublished candidates. Explicit
unchanged source exports retain a published row with `source_identity: true`
without being counted as new derivatives. Consumers must retain the full
manifest for supporting evidence and distinct same-content destinations.

The [multi-family fixture](../user-guide/creative-forest.md) authors isolated
source-family runs, and the [Flow handoff](flow-artifact-forest.md) explains how
to combine their evidence without assuming a single mixed-policy native DAG.
