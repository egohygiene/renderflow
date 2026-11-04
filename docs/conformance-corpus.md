# Golden artifact conformance

The next #413 checkpoint adds the original
[document fixture pack](document-fixture-packs.md) in corpus 1.3.0: one pinned
story file after the 30-source intake slice, a minimal native comic case and a
44+2 planning-only variant. The same harness and v1 schemas are extended; all
new cases remain uncompiled/unrun. Earlier checkpoint counts below describe
the historical #412 slice.

Renderflow's versioned golden corpus binds its artifact claims to a small,
redistribution-safe acceptance harness. Document payloads are original and
synthetic; pack recipes can reuse explicitly pinned licensed fonts with their
full rights records. No private publication or customer content is included,
and no fixture requires network or AI access.

The #412 extension is **authored and unrun**. Corpus 1.2.0 adds a finite
adversarial intake slice, not a universal format-validation claim. The existing
harness and v1 schemas remain the authority; no second fixture runner is added.
Tests, compiler/build/Cargo, lint/format/schema/docs validators, corpus execution,
provider/native/model execution and CI remain deferred during this sprint.

For a small, retained source-to-output run through the public CLI and real
local providers, see the [inspectable artifact gallery](artifact-gallery.md).
The gallery is explicitly invoked on a local machine; it complements this
versioned conformance corpus and does not silently bless changed outputs.

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

Intake `accepted` means the expected identity/conflict/diagnostic contract was
observed. A header-only PDF, MP4 or WAV can be identified without being a usable
document or decodable media file. Every classified fixture records its
assertion scope and residual validation gap. The arbitrary-binary case stays
inspectable without claiming conversion support.

## Adversarial intake slice

All 30 inline sources have declared SHA-256 values and finite construction
notes, a fixed seed of zero and the `renderflow.golden.inline/v1` recipe identity.
UTF-8 and reviewed hex are the deterministic source recipes; materialization
decodes exactly those bytes. Archive notes explain fixed timestamps, stored
entries, permissions and intentional corruption. The finite ratio case expands
to only 4096 bytes and is stored as authoritative hex; reproducing a particular
zlib stream is not a cross-version promise.

| Class | Selected cases and asserted boundary |
| --- | --- |
| Valid | Minimal JSON, literal Markdown and a small stored ZIP with parent-linked retained bytes |
| Boundary | Empty/BOM/CRLF Markdown, 24 JSON levels, original header/unknown signatures |
| Malformed | Truncated JSON/PDF/RIFF, incomplete zero-size PNG, malformed archive and traversal declaration |
| Unsupported | Inert SVG external-resource declaration at intake; stored ZIP symlink refusal |
| Resource stress | Tiny archives deliberately exceeding low byte, count, depth or ratio budgets |

The SVG reference uses reserved `example.invalid` and is never fetched by
intake. Actual renderer external-resource refusal remains an unmapped coverage
obligation. Identifying malformed JSON or a zero-size PNG is intentionally
different from admitting it to the parser/renderer. No executable attack,
uncontrolled archive expansion or private asset is included.

The harness checks exact source bytes before/after intake, named rejected
diagnostic severity, discovered artifact count, parent membership, retained
artifact digests and configured extraction bounds. It refuses escaping or
case-colliding fixture filenames before writing. Source count/per-file/total
byte bounds are enforced before materialization. A cooperative elapsed-time
budget is checked between phases; it is not a hard interrupt or memory sandbox.
Current budgets are at most 128 sources, 64 KiB each, 1 MiB total and 120 seconds.

## Coverage and gaps

The optional `coverage` report joins all operations from the compiled
`FormatCapabilityRegistry`, every route from the exact pinned shared creative
catalog, and named input/failure obligations declared in this corpus. A changed
creative catalog pin requires explicit review. No hand-maintained copy of its
route matrix is introduced.

Each scenario declares its assertion scope. A production intake/native-output
case can yield `observed_case` only after execution; mocks, inventory and tool
discovery remain `contract_or_inventory_only`. Empty mappings are `gap`, and
unavailable/excluded cases remain `not_executed`. Even an observed case does not
qualify a complete capability, provider or platform. The report retains
`runtime_qualified: false`; this checkpoint includes no generated passing report.

Most creative routes and format-specific generation/validation operations
remain explicit gaps in this first slice. The finite failure inventory includes
uncovered version/timeout/signal/output/print/font/layout/active-content/resume
boundaries. Extend one named group at a time using real public APIs; generic
mock executor cases must not be promoted to provider fidelity evidence.

## Tiers

| Tier | Purpose | Dependency policy |
| --- | --- | --- |
| `fast` | Pull-request acceptance | Hermetic; Rust toolchain only |
| `tool_backed` | Adapter verification | Optional tools are probed and versioned |
| `maximal` | Scheduled and release evidence | All corpus families and scenario states are reported |

The fast tier no longer probes optional executables. Tool-backed discovery runs
only when a higher tier is selected. Existing tier/workflow names are retained;
no workflow is added, enabled, dispatched or polled by this source checkpoint.

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

For a malformed case, name the precise production boundary and typed diagnostic.
Keep an intentionally identified-but-unvalidated input's residual explicit.
For a minimized fuzz case, record the finite corruption recipe and redistribution
provenance; never import an uncontrolled sample. Review expected-byte/schema/
scope changes in Git; ordinary runs never rewrite the manifest or baselines.

Current follow-on work is the original document-type pack in #413, using this
same corpus and harness. #412 remains open for the broader provider/format
matrix, enforced CI/sharding/memory budgets, fuzzing and actual execution. The
[source handoff](work/412/HANDOFF.md) records this bounded checkpoint.

A capability graduates from `unavailable` or `experimental` only when the
corpus proves its success path, failure classification, validation evidence,
and provider behavior on every supported platform. Nondeterministic formats
must assert structural and semantic invariants rather than byte identity.
