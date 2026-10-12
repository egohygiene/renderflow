# Screenplay PDF and source analysis

The #425 checkpoint authors an optional afterwriting adapter for a restricted
Fountain body profile. It is an **unqualified draft**: tests, builds, schema
validation, native rendering, visual review, reproducibility runs, and CI are
deferred. The examples below have not been run.

Use [screenplay inspection](screenplay.md) for canonical JSON and
[scripttool imports](scripttool.md) for supported native-to-Fountain conversion.
This provider accepts Fountain only. It does not process FDX, OSF, Fade In,
canonical screenplay JSON, or existing PDFs, and does not automatically chain
the native imports into PDF generation.

## Analyze a source without a provider

```bash
renderflow screenplay analyze --input "scene.fountain" > "scene-analysis.json"
```

The report comes from Renderflow's independent Fountain parser. It records
source byte identity, six element-kind counts, ordered scene-heading
occurrences, exact character-cue counts, and action/dialogue word counts. Words
are ASCII whitespace-delimited tokens, with punctuation attached; parentheticals
do not contribute to dialogue word counts. Repeated headings remain separate
occurrences. Differently written cues are not merged into inferred people.
There are no inferred filming locations, reading times, emotional judgments,
or estimated PDF page counts.

Afterwriting's CLI does not expose a stable structured analysis command.
These are independent Renderflow source counts, not claimed afterwriting
statistics. The versioned [analysis schema](../../schemas/renderflow-screenplay-analysis-v1.schema.json)
and original fixtures describe that boundary. Received reports are declarations
until recomputed from the source bytes.

## Select the runtime and installation

Supply an already installed, trusted afterwriting package and Node executable.
Renderflow does not download or install either. Source review found package
version `1.17.3` in distinct upstream revisions with different dependencies;
the version alone cannot identify the installed code. Both the Node executable
and package contents must be pinned.

Obtain the package-content fingerprint without launching Node:

```bash
renderflow screenplay package-fingerprint --package-root "/absolute/path/to/afterwriting"
```

This reads bounded regular files under the package root, including installed
`node_modules`, and returns `tree_sha256`. It excludes root `.git` and
`node_modules/.bin` directories and rejects other symlinks. Its versioned digest
recipe hashes compact JSON records sorted by relative path, with each file's
path, byte length, and SHA-256. Empty directories do not affect it. A matching
fingerprint identifies bytes; it does not establish that they are trustworthy,
complete, or built from the reviewed source.

Create `afterwriting.json`, replacing the paths, digest placeholders, and exact
Node version with your observed values:

```json
{
  "node_executable": "/absolute/path/to/node",
  "node_sha256": "<verified-node-executable-sha256>",
  "expected_node_version": "v24.14.0",
  "package_root": "/absolute/path/to/afterwriting",
  "package_tree_sha256": "<observed-package-tree-sha256>",
  "expected_provider_version": "1.17.3",
  "profile": "screenplay-a4-v1",
  "timeout_seconds": 30,
  "allow_unqualified": true
}
```

The Node version above is an example, not a qualified recommendation. The
authored adapter allows exact declared Node 22 or 24 versions as candidates;
it checks the actual version response and executable hash. JSON config is
bounded at 64 KiB and rejects unknown fields. Missing or mismatched package,
runtime, dependency, or required Courier metric files refuses the operation.
No provider `--version` flag is invented; afterwriting's package identity is
read from its pinned `package.json`.

## Render a new PDF bundle

```bash
renderflow screenplay pdf \
  --input "scene.fountain" \
  --config "afterwriting.json" \
  --output-dir "scene-pdf"
```

Optional `--source-sha256` pins the input bytes. The destination must be new,
and its parent directory must already exist. The original input is read only.
The command stages a private copy of the installed package and source, so it
requires temporary storage; this is not an in-place or zero-storage operation.

The completed bundle contains:

- `screenplay.pdf`: exact provider output, independently inspected.
- `analysis.json`: independently computed source counts.
- `provider-evidence.json`: source/output, runtime, package, profile, argv,
  analysis and PDF inspection evidence, including known limitations.
- `bundle.json`: written last, binds the three member hashes and sizes, with
  `runtime_qualified: false`. The same receipt is written to stdout.

Provider failure or rejected PDF output publishes no completed bundle. A
publication I/O failure or cancellation can leave a partial directory without
the receipt. A stdout failure after publication can leave an already completed
bundle. Existing files are not replaced.

## Versioned formatting profile

`screenplay-a4-v1` fixes A4 paper; `screenplay-usletter-v1` fixes US Letter.
Both select Courier base14, enable page numbers, disable the title page and
scene numbering, and retain the adopted body paragraph kinds. Settings are
generated by Renderflow. Arbitrary settings, custom executable font modules,
snippets, watermarks, or provider arguments are not accepted.

Source text must fit the existing strict body-only, single-line Fountain
grammar and additionally contain only printable ASCII and LF. It must begin
with a conventional uppercase scene heading. Title metadata, styling, scene
numbers, notes, dual dialogue, multiline body paragraphs, tabs/CR/BOMs, and
non-ASCII text are rejected. The literal `<?xml` marker is rejected anywhere
because upstream uses it to dispatch to its FDX parser.

Courier's metric assets are part of the pinned PDFKit installation. The PDF
uses standard base14 Courier fonts without embedding programs; a viewer may
substitute its own implementation. This checkpoint does not qualify visual
fidelity, international text, native font rendering, PDF/A, print acceptance,
or general screenplay layout. Provider formatting stays outside the canonical
screenplay semantic model.

## PDF inspection and reproducibility

Independent inspection checks a bounded passive PDF structure, page tree and
counts, selected paper geometry, nonempty text operations, and the permitted
Courier font resources. Encrypted, active-content, unsupported-font, malformed,
or out-of-profile output fails. Inspection does not compare extracted PDF text
with every source element or establish how a viewer renders the page.

Page count is observed from the actual PDF. Metadata values and trailer IDs
are recorded with the PDF byte digest. Upstream PDFKit may generate timestamps
and IDs; this checkpoint neither rewrites those bytes nor asserts that two
renders are identical. Source analysis is defined deterministically for the
same source and algorithm, while PDF byte/layout reproducibility remains
unverified until the deferred two-run qualification.

The Node process uses direct argv, cleared environment, disabled native addons
and global module search paths, bounded output capture, timeout, and
cancellation. Source, Node, installation, and staged bytes are rechecked.
These controls are not an operating-system sandbox or proof of the complete
loaded-code/runtime closure; declared network denial is not OS isolation.

Source size is capped at 16 MiB and PDF output at 64 MiB/1,000 pages. Installed
packages are capped at 512 MiB, 64 MiB per file, 40,000 entries, and depth 32;
Node hashing is capped at 256 MiB. Conversion accepts a 1–300 second timeout;
the separate version probe has five seconds and each captured stream has
64 KiB. File operations check cancellation and a 120-second budget between
reads; a blocked filesystem call is not interrupted by those checks. PDF
inspection limits individual decompressed streams and total page content, but
the parser allocates objects before post-load aggregate checks. This is not a
hard whole-process memory quota.

## Graph and SDK execution

Select one explicitly declared `fountain` source and a `pdf` target in a v2
spec. Put the same config fields under `execution.screenplay_pdf`:

```yaml
schema: "renderflow/v2"
sources:
  - id: "source.screenplay"
    path: "./scene.fountain"
    format: "fountain"
targets:
  exact:
    - id: "target.screenplay-pdf"
      format: "pdf"
execution:
  screenplay_pdf:
    node_executable: "/absolute/path/to/node"
    node_sha256: "<verified-node-executable-sha256>"
    expected_node_version: "v24.14.0"
    package_root: "/absolute/path/to/afterwriting"
    package_tree_sha256: "<observed-package-tree-sha256>"
    expected_provider_version: "1.17.3"
    profile: "screenplay-a4-v1"
    timeout_seconds: 30
    allow_unqualified: true
output:
  bundle_root: "./output"
  naming_template: "{source}.{ext}"
  collision: "error"
```

The opt-in capability is `screenplay.render.fountain.pdf`, with explicit
`tool.node` and `tool.afterwriting` requirements. Planning uses bounded provider
preflight and a cleared-environment Node probe. Missing or incompatible
requirements exclude the route with actionable diagnostics.
Execution policy is applied before that probe: a denied provider/transform or
an incompatible deterministic requirement leaves the configured installation
unprobed. The same rule applies when refreshing the selected execution route.

The policy refuses collections, mixed native policies, custom transforms,
inherited post-conversion hygiene, same-format copies, and transitive routes.
Graph execution carries provider/analysis evidence into its artifact manifest
and requires independent PDF reinspection at publication, even if generic
validation is disabled. Native reusable cache/checkpoints and toolchain
attestation remain unavailable. Combined graph evidence is capped at 4 MiB.

See [provider evaluation](../work/425/PROVIDER_EVALUATION.md) for source,
release, Node, installation and license observations, and
[the handoff](../work/425/HANDOFF.md) for remaining qualification work.
The [qualification runbook](../work/425/QUALIFICATION.md) provides the deferred
commands and evidence checklist for completing that work.
