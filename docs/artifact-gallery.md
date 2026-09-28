# Inspectable artifact gallery

The gallery is a small synthetic, source-to-output proof. Its first cases use
one Markdown document and two ordered image pages. They run the public
Renderflow CLI with the real Pandoc and `img2pdf` providers, check generated
artifacts and evidence, and leave the complete output tree available for you
to inspect. They never process publication or personal media.

## Run locally

From the repository root, provide a new or empty output directory:

```bash
scripts/run-artifact-gallery.sh --output-dir "$PWD/gallery-output"
```

When `img2pdf` is installed outside `PATH`, specify its executable:

```bash
scripts/run-artifact-gallery.sh \
  --output-dir "$PWD/gallery-output" \
  --img2pdf "/absolute/path/to/img2pdf"
```

The runner requires Pandoc 2.0.0 or newer, exactly `img2pdf` 0.6.3, and a
Rust toolchain compatible with this checkout. Missing or incompatible
providers are reported as **unavailable** and return a nonzero exit code;
they never count as a successful test. The runner does not install tools,
access the network, trigger hosted CI, replace a nonempty output directory,
or approve a changed expected result. Pick a new output directory for each
run to preserve earlier evidence.

The reviewed HTML baseline was generated with Pandoc 3.1.3. Other Pandoc
versions are probed and reported, and an output difference fails for review;
the runner never rewrites the baseline to accommodate provider drift.

The retained tree includes the synthetic inputs, the generated HTML and PDF,
the corresponding `renderflow-run.json` manifests, two synthetic refusal cases,
and `comparison.json` on success. Open the HTML and PDF, then review the report and run
manifests for provider versions, ordered sources, output digests, and validation
state. The PDF case verifies page order, boxes, embedded image streams, and
lineage; this is a synthetic capability proof, not a print approval.

## Test tiers and expected changes

Normal tests can run without the external providers and exercise fixtures,
planning, preflight, refusal, and validation behavior. The explicit runner
executes the ignored real-provider gallery test and keeps its output. Output
bytes may be compared exactly where a provider version and output contract
make them deterministic; otherwise tests compare the relevant structure and
semantics. A missing provider is neither a pass nor an expected-output update.

Reviewed baselines and fixtures live in source control. A difference should
fail with enough evidence to diagnose it, and any update requires a deliberate
code review. We can add more source formats, target formats, configurations,
and failure cases as their capabilities graduate, while keeping this first
local run small and inspectable. The broader conformance tiers are described
in [Golden artifact conformance](conformance-corpus.md).
