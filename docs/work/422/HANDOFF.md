# #422 — derivative forest checkpoint

Status: implementation authored; draft review and runtime qualification outstanding.

This checkpoint extends the canonical planner used by CLI and SDK. It discovers a finite
target set once, excludes the source identity from exhaustive expansion, and selects one
bounded simple source-to-target path per distinct target identity. Reversible edges cannot
turn output discovery into recursive execution. An explicit unchanged source export uses
zero transform steps; same-format regeneration is not invented.

Target identities include normalized format, explicit publication ID/role, preset,
template, variant, options, terminal constraints, and effective include/exclude selectors.
Identical requests deduplicate, with required taking precedence over optional. Distinct
renditions cannot overwrite each other in a format-keyed execution map or output path.
Exact provider/transform/capability/variant selection survives executor registration,
including competing YAML edges. Shared DAGs require compatible selected edges and effective
render settings; otherwise groups run sequentially from the original source. Multiple
groups do not read or write shared cache/checkpoint state.

Exhaustive/profile planning ranks cumulative declared quality, hop count, optimization
cost, and stable edge identity. Ordinary exact-target planning keeps optimization cost as
the first ranking criterion. Both use bounded search, no repeated format nodes, explicit
search-exhaustion diagnostics, a hop policy, and an optional cumulative loss budget.
The quality values are provider declarations, not measured output fidelity.

`renderflow/artifact-forest-v1` records target identities, selected routes, ranking and
loss evidence, and selected/excluded/unavailable/unsupported/budget-pruned branches. Its
identity binds source/spec/policy/options/routes/toolchain evidence. JSON/YAML plans and
run evidence retain the complete model; text/Markdown plans expose branch decisions.
Graph views remain format projections. `ResolvedExecution::dags()` exposes all execution
groups; the legacy `dag()` accessor exposes the first group.

Generic target options without an adapter interpretation are explicitly unsupported.
Supported presets/templates and registered transform variants are the current rendition
mechanisms. Source-format transforms with changed settings require a future distinct
artifact-node capability. Bounded screenplay, speech, subtitles, and collection policies
still reject incompatible multiple execution groups.

## Evidence and deferred work

Source review covered planner/executor identities, selector retention, provider unions,
source passthrough, branch budgets, publication paths, cancellation steps, and schema/model
shapes. Focused fixtures were authored for reversible and dense graphs, competing paths,
aliases, constraints, loss/search bounds, unavailable providers, distinct same-format
outputs, output collisions, and identity changes.

Per the maintainer's sprint instruction, no tests, compilation, lint/format checks, schema
validation, native probes/conversions, installers, or CI were run. Git status/diff and Git
object identity inspection are publication evidence only. None of the authored fixtures
has been reported as passing, and this checkpoint does not close #422's qualification
criteria or the existing release/adapter qualification issues.

During the later deep test session: compile and run the authored fixtures, compare CLI/SDK
plans, exercise real preset outputs and custom parallel providers, inspect the emitted
forest/run manifests, and repeat native screenplay/speech/collection regressions. Record
actual failures before creating follow-up audit issues. The next implementation lane is
#426, then remaining #429 recovery integration, then #430 cross-family/Flow proof.
