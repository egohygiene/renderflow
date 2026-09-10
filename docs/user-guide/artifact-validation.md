# Artifact validation and fidelity gates

Canonical builds validate every terminal artifact before publication. Validation is part of the execution result: an artifact is `valid`, `valid_with_warnings`, `invalid`, `unavailable`, or `skipped` by explicit policy. Intermediate artifacts remain `not_requested` unless they become selected targets.

The built-in registry provides deterministic structural validators for text, JSON, YAML, markup, delimited text, PDF, PNG, JPEG, ZIP-based packages, and RIFF/WAVE. Each result records the validator ID, implementation version, provider, state, and structured diagnostics in `renderflow-run.json`.

```yaml
execution:
  validation:
    required: true
    validators:
      - validator.core.non_empty
      - validator.core.png
    failure_mode: branch_local
    allow_unavailable: false
  reject_loss_classes:
    - lossy
    - unknown
```

With `branch_local`, a failed target is withheld while independent valid targets may be published, producing a partial run. With `fatal`, any blocking validation or fidelity result prevents every target from being published. Setting `required: false` records terminal validation as `skipped`; it does not silently claim validity.

An unavailable requested validator blocks publication unless `allow_unavailable: true`. Warnings remain publishable. `reject_loss_classes` can reject `lossless`, `partial`, `lossy`, `path_dependent`, or `unknown` fidelity declarations independently of the numeric planning-time `minimum_fidelity` threshold.

## Capability conformance matrix

The generated matrix is the source for format support claims:

```bash
renderflow capabilities --matrix
renderflow capabilities --matrix --format json
renderflow capabilities --matrix --format yaml
```

Each row distinguishes `implemented`, `experimental`, `unavailable`, and `planned` support and names declared capabilities, executors, providers, validators, fixtures, platforms, deterministic validation, and loss profile. Its machine-readable contract is `schemas/renderflow-conformance-v1.schema.json`.
