# Golden artifact conformance

Renderflow's versioned golden corpus binds its artifact claims to a small,
redistribution-safe acceptance harness. The corpus contains synthetic payloads
only. It does not contain publication, comic, customer, or third-party content,
and no fixture requires network or AI access.

## Contracts

The canonical manifest is
`tests/fixtures/golden-artifacts/v1/corpus.json`. Binary payloads are stored as
lowercase hexadecimal inside the manifest so review, reproduction, and source
control remain transparent. The manifest conforms to
`schemas/renderflow-golden-corpus-v1.schema.json`.

The focused Rust harness materializes those payloads in a temporary directory
and exercises production intake, graph planning, execution, validation, cache,
checkpoint, hygiene, profile, and Flow-projection APIs. It can emit a report
conforming to `schemas/renderflow-conformance-report-v1.schema.json`:

```bash
RENDERFLOW_CONFORMANCE_TIER="fast" \
RENDERFLOW_CONFORMANCE_REPORT="/tmp/renderflow-conformance.json" \
cargo test --package renderflow --test golden_conformance --locked
```

The report pins the corpus digest and engine version. Each materialized fixture
records its artifact identity, payload digest, detected format, media type, and
expected outcome. Each scenario records its status, assertions, and selected
providers. Tool-backed runs include observed provider version strings; missing
optional providers are `unavailable`, never silently successful.

## Tiers

| Tier | Purpose | Dependency policy |
| --- | --- | --- |
| `fast` | Pull-request acceptance | Hermetic; Rust toolchain only |
| `tool_backed` | Adapter verification | Optional tools are probed and versioned |
| `maximal` | Scheduled and release evidence | All corpus families and scenario states are reported |

The dedicated conformance workflow runs the fast tier for pull requests and
the maximal tier on its schedule, on published releases, and when manually
dispatched. Both publish the machine-readable report even when a job fails.

## Adding a fixture

1. Use an original synthetic payload small enough to inspect in review.
2. Add its payload, family, expected identity, and outcome to the versioned
   corpus manifest. Never add secrets, protected references, or production
   content merely to exercise a gate; use an unmistakably synthetic marker.
3. Map at least one scenario to the fixture and add an assertion through a
   public production API.
4. Validate both schemas and run the focused harness locally.
5. Increment `corpus_version` when fixture bytes or expected behavior changes.

A capability graduates from `unavailable` or `experimental` only when the
corpus proves its success path, failure classification, validation evidence,
and provider behavior on every supported platform. Nondeterministic formats
must assert structural and semantic invariants rather than byte identity.
