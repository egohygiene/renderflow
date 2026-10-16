# Multi-family creative forest fixture

Use this fixture to exercise several creative source families through the public
Renderflow CLI and retain their individual plans, runs and evidence together.
The #430 implementation is authored and unqualified. The commands below have
not been run during the sprint; tests, builds, native execution and CI remain
deferred.

The launcher runs isolated jobs because original screenplay bodies, screenplay
PDF, recovery, subtitles and speech have different policies. It does not combine their sources into one
native DAG or silently convert a source into another family's input type.

## Start with the bundled sources

From the repository root, inspect the declared suite without launching Renderflow:

```bash
python3 scripts/run_creative_forest_fixture.py --mode describe
```

When the execution session is authorized, use an already built Renderflow binary
from this checkpoint or a later compatible version. Each output directory must
be new:

```bash
mkdir -p "dist"

python3 scripts/run_creative_forest_fixture.py \
  --mode plan \
  --renderflow "/absolute/path/to/renderflow" \
  --output "dist/creative-forest-plan"

python3 scripts/run_creative_forest_fixture.py \
  --mode execute \
  --renderflow "/absolute/path/to/renderflow" \
  --output "dist/creative-forest-run"
```

`describe` prints suite declarations only. `plan` invokes canonical
`renderflow build --dry-run`, captures plans and can perform the provider probes
that those routes normally require. The launcher writes its own evidence files
even in plan mode; Renderflow's dry-run still does not publish conversion output.
Standalone commands with no planning interface remain deferred in that mode.
`execute` invokes actual conversions and standalone operations, retaining their
results and failures. None of these modes installs a provider or downloads a model.

| Default job | Intended output when later executed |
| --- | --- |
| `original-body` | Built-in direct original Fountain to canonical screenplay JSON, FDX, Fade In, OSF and Trelby body exports; no external provider. |
| `recovery-text` | Recovered canonical screenplay and Fountain/FDX/Fade In/OSF body derivatives, with mandatory loss/review evidence. |
| `timed-text` | Canonical transcript, WebVTT and terminal plain text from original fixture captions; text retains timing/provenance loss evidence. |
| `fountain-analysis` | Standalone analysis of the original bounded Fountain fixture. |
| `transcript-text` | Separate standalone text export from caption data, retaining its existing public-command behavior. |

The sources and policies are in
[`examples/creative-forest`](../../examples/creative-forest/README.md). They are
synthetic fixtures, not observed provider output or proof of conversion fidelity.
The README records their provenance, exact target membership and output layout.

The original-body spec declares `screenplay_body` with explicit
`allow_unqualified: true`. It uses the existing original Fountain bytes directly,
never recovery or Scripttool reverse exports. The authored body profile accepts
scene headings, action, characters, dialogue, parentheticals and transitions;
unsupported constructs and metadata refuse. Every output retains the original
source digest and canonical reference with `runtime_qualified: false` and required
review. FDX/Fade In/OSF/Trelby application opening, editing, layout and pagination
remain unobserved. A successful later CLI run will not imply creative approval.

Graph transcript text remains terminal under the existing subtitle policy. Its
native evidence links the actual retained canonical transcript and discloses
timing, cue-ID and structured-metadata omissions with partial fidelity. Neither
the graph text target nor the standalone example can recreate timing from text.

## Add explicitly configured native jobs

The optional jobs are `screenplay-pdf`, `native-import`, `pdf-recovery`, `pdf-ocr`,
`speech`, and `embedded-subtitles`. Supply only the jobs you want to attempt in a
local JSON file using the fixture README's native configuration shape. Each
entry carries the adapter's real execution policy; media jobs additionally need
an explicit local source and format. Use actual installed file paths, hashes,
versions and stream selections. Keep that machine-specific file outside the
committed synthetic corpus.

```bash
python3 scripts/run_creative_forest_fixture.py \
  --mode execute \
  --renderflow "/absolute/path/to/renderflow" \
  --native-config "/absolute/path/to/creative-native.json" \
  --output "dist/creative-forest-native"
```

An omitted optional job is `skipped_unconfigured`. That means it was not
attempted, not that its provider was probed and found missing. A supplied native
configuration is an explicit execution choice; its failure is retained and
causes an unsuccessful aggregate exit. Independent jobs can still retain their
own outputs. See the [toolchain setup guide](creative-toolchain-setup.md) for
acquisition channels and each adapter's configuration contract.

Speech needs a redistribution-safe spoken fixture and actual recognition
observations before making an accuracy claim. Silence and manually authored
whisper JSON are not positive speech-recognition evidence. Likewise, a valid
synthetic text PDF does not qualify scanned-page OCR or mixed-page routing.
The corpus also includes an authored image-only PDF for the optional OCR job;
its extraction quality remains unobserved.

## Include negative cases

Add `--include-negatives` to the plan or execution invocation to include authored
malformed recovery, ambiguity, duplicate-target, cycle/source-suppression and
missing-provider scenarios, with native negatives gated by their explicit
configuration. Each case records its own expectation; an arbitrary process
failure is not automatically a successful negative test. The suite and its
runner regression sources remain unrun in this checkpoint.

The expected planning behavior includes finite routes, one identity per
effective target, source exclusion from exhaustive output, unsupported/provider
diagnostics and explicit unobserved native policy. Ambiguous recovery can produce
a candidate while still requiring review. This is a different outcome from a
malformed source being refused.

## Read the evidence without overstating it

The suite uses `renderflow.creative-fixture/v1` for declarations and
`renderflow.creative-fixture-result/v1` for its aggregate report. It records job
status, exact arguments, process results and digested references to captured
output and native evidence. Retain the whole new output directory. The report
remains unqualified and does not claim complete coverage of #430's broader
format scenarios.

For a graph job, its native `renderflow-run.json` is authoritative. Pair
`artifact_forest.branches` with `artifact_forest.execution.targets`: the first
describes selected intent, the second distinguishes actual publications.
Published rows reference the exact artifact record containing digest, producer,
lineage and loss/review metadata. Do not count every inventory record as an
output. A complete child run can still carry required review, and a suite with
unconfigured native jobs has not demonstrated those routes.

The [generated capability matrix](creative-capabilities.md) distinguishes the
new authored built-in body and terminal-text routes from qualified behavior.
General original screenplay metadata/layout conversion and Scripttool reverse
exports remain unavailable. Recovery-native exports apply only to recovered
candidates; they never turn a recovered candidate into an approved original.
#430's original diagram is an outcome to reconcile, not evidence that every
arrow has run.

For downstream consumption, use the [Flow handoff](../architecture/flow-artifact-forest.md).
Actual repeated runs, CLI/SDK parity, native/platform qualification and released
binary consumption remain part of the later test and release session.
