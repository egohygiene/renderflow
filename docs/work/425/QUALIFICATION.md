# Qualifying the #425 screenplay PDF adapter

This is a future execution runbook, not a results report. No command below was
run while authoring it. Keep #425 open until its acceptance evidence exists.
The sprint's deferred tests, builds, native runs, and CI remain deferred; this
document does not change that instruction.

## Preserve the candidate and inputs

Use the exact merged candidate from PR #444 or a recorded successor on `main`.
Record its commit and tree, platform/architecture, Rust version, Node executable
path/hash/version, package-tree fingerprint, provider version, selected profile,
and source/config hashes alongside results. Never describe a later checkout as
the same qualified candidate without recording its new identity.

Use a trusted, already installed Node executable and complete afterwriting
package with dependencies. This runbook does not install them. Configure their
observed pins as described in the [user guide](../../user-guide/screenplay-pdf.md).
Use a new evidence directory; preserve failed attempts and select a new directory
for retries. Do not replace a failure with a successful result under the same name.

## Compile and execute focused contracts when resumed

From the repository root, the core package is named `renderflow`; the binary
lives in the separate `renderflow-cli` package. With the pinned dependencies
already available, the focused commands are:

```bash
cargo test --locked --offline --package renderflow --lib screenplay
cargo build --locked --offline --package renderflow-cli --bin renderflow
```

The test filter includes the parent screenplay contracts as well as this
adapter. Record the discovered/executed test names and summary; a filtered run
that discovers zero tests is not evidence. A missing offline dependency is a
blocked run, not permission to silently remove `--offline` or install packages.
No formatter, linter, or GitHub Actions command is part of this runbook.

Validate the generated analysis, provider evidence, PDF inspection, and bundle
receipt against their four committed schemas using the project's selected JSON
Schema validator. Record the validator/version and command actually used. Source
inspection or successful JSON parsing alone does not establish schema validity.

## Run the same source twice

Once native execution resumes, create a fresh working directory outside the
repository with a copy of the committed
`tests/fixtures/screenplay/afterwriting-425/strict.fountain`, its
`expected-analysis.json`, and your pinned `afterwriting.json`. Keep those copies
unchanged for both attempts. Set `renderflow_binary` to the absolute path of the
binary built from the recorded candidate:

```bash
renderflow_binary="/absolute/path/to/renderflow/target/debug/renderflow"

"$renderflow_binary" screenplay analyze \
  --input "strict.fountain" > "source-analysis.json"

"$renderflow_binary" screenplay pdf \
  --input "strict.fountain" \
  --config "afterwriting.json" \
  --output-dir "run-1"

"$renderflow_binary" screenplay pdf \
  --input "strict.fountain" \
  --config "afterwriting.json" \
  --output-dir "run-2"
```

Run each command separately, retain its stdout/stderr and exit status, and stop
to investigate a failure. Neither output directory may exist beforehand. Repeat
the pair in another new working directory for the other paper profile; changing
the profile requires a new recorded config identity.

Check the source report against `expected-analysis.json` as JSON values. For each
successful bundle, independently hash and size-check every member listed by
`bundle.json`. Confirm the PDF hash also matches `provider-evidence.json` and its
nested `inspection.sha256`. Do not trust a receipt merely because it exists.

There are two distinct analysis hashes: the receipt binds the exact pretty JSON
file including its final LF; `analysis_sha256` in provider evidence binds the
compact Rust serialization of the typed analysis. Generic JSON serialization
with different map ordering or whitespace need not reproduce that compact hash.

Compare the following explicitly and retain both complete bundles:

| Evidence | Required interpretation |
| --- | --- |
| Source, Node, package, profile/settings and argv identities | Must match across the pair. Otherwise it is not a pinned two-run comparison. |
| Source analysis | Must match as typed data, including its algorithm and source identity. |
| Raw PDF byte hash | Equal establishes byte equality for this pair only. Different requires a recorded explanation. |
| Page count, boxes, font evidence and text-operation counts | Record equality/differences. Equal counts do not establish equal text or layout. |
| Info metadata, trailer ID hashes and metadata-stream hash | Record differences individually; retain their values/hashes from both inspections. |
| Extracted text and visual layout | Compare independently with the original source and both PDFs; name the tools/viewers and versions used. |

Do not attribute every byte difference to timestamps just because timestamps
changed. If the other differences cannot be explained, report reproducibility
as unqualified. No metadata normalization is performed by the adapter. All
single-run qualification flags intentionally remain false; do not hand-edit
generated evidence to manufacture a passing result.

## Exercise the canonical planner and publication boundary

Copy the v2 spec from the [user guide](../../user-guide/screenplay-pdf.md), supply
the same absolute Node/package paths and pins, point it at `strict.fountain`,
and select a new output root. Save it as `screenplay.yaml` in that working
directory. With the binary variable above:

```bash
"$renderflow_binary" spec validate --config "screenplay.yaml"
"$renderflow_binary" build --config "screenplay.yaml"
```

Record the selected `screenplay.render.fountain.pdf` route, required `tool.node`
and `tool.afterwriting`, artifact lineage, analysis/provider metadata, and terminal
inspection result. A standalone CLI success does not satisfy planner acceptance.
Planning itself probes the configured Node executable; it is a native operation,
even before PDF rendering. SDK consumers must exercise their actual caller too.

Use disposable copies for negative cases. Never mutate the trusted original
runtime/package. Each failed attempt must fail without a successful publication
claim; a partial bundle without its final receipt is not completed output.

| Case | Expected boundary |
| --- | --- |
| Denied Node/afterwriting/transform, restrictive allowlist, or deterministic requirement excluding this route | No configured Node probe or package preflight occurs; the route remains unavailable. |
| Missing Node/package/required Courier metrics; wrong executable/package pins or versions | Prerequisite failure; planner excludes the route. |
| FDX/native input, title page, Unicode, unsupported syntax, or `<?xml` in body text | Explicit source/profile refusal. |
| Missing, empty, malformed, wrong-paper, active-content or unsupported-font PDF | Independent output rejection. |
| Cancellation, timeout, excessive logs, or changed source/config/tool/package | Failed operation; no completed result claimed. |
| Existing destination or publication I/O failure | No overwrite; failure does not become a successful receipt. |
| Detached/tampered source, report, profile or PDF evidence with generic validation disabled | Mandatory screenplay validation still blocks publication. |

Some failure paths require the authored fake-runner/graph contracts rather than
forcing a real executable to misbehave. Record which evidence is synthetic and
which was observed with the real pinned provider.

## Visual review and acceptance record

Open every real PDF in a named viewer and compare all source elements in order.
Review character/dialogue ownership, parentheticals, transitions, wrapping,
clipping, page breaks, page numbers, paper size, and Courier substitution. The
small committed fixture does not qualify multi-page pagination: add an original
longer accepted Fountain fixture, pin its bytes, and repeat both renders and the
review. Print review requires a named print workflow; structural validity alone
does not establish it.

For each #425 acceptance criterion, retain the command, candidate/input pins,
artifact paths/hashes, observed result, and reviewer/date. Mark a criterion
complete only for the scope supported by that evidence. Record any remaining
platform/profile limits. #442, #443, and #444 merged in that order; #441's doctor
integration preserves the configured provider boundary. Merge status is not
qualification evidence. This runbook does not authorize a release.
