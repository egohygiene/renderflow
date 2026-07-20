# Optimization

Renderflow's graph planner scores candidate paths using `OptimizationMode` from `src/optimization.rs`.

## Modes

| Mode | Intent | Edge weight |
|---|---|---|
| `speed` | prefer cheaper/faster paths | `cost` |
| `quality` | prefer higher-quality paths | `1 - quality` |
| `balanced` | combine cost and quality | `0.5 * cost + 0.5 * (1 - quality)` |
| `pareto` | keep non-dominated choices | uses the Pareto frontier, then balanced scoring for single-path helpers |

## What cost and quality mean

Each transform edge carries:

- `cost`: lower is better
- `quality`: `0.0..=1.0`, higher is better

The planner uses those values to rank or filter routes.

## Pareto mode

Pareto mode is useful when you care about trade-offs rather than one "best" answer. A path is removed when another path is both:

- cheaper or equal in cost, and
- better or equal in quality,

with at least one strict improvement.

## Where optimization is used

- `renderflow build --optimization ...`
- `renderflow graph plan/render/explain/export/doctor/stats`
- graph build modes `--target` and `--all`

## Examples

```bash
renderflow build --target pdf --optimization quality
renderflow graph plan --format mermaid --optimization speed
renderflow graph explain --target html --optimization pareto
```

!!! tip
    `balanced` is the config default and the safest starting point when you do not yet have calibrated edge weights.
