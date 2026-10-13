# Creative toolchain doctor

The optional `creative-publishing` profile reports local tools used by creative
conversion work and explains which adapters still need implementation or
qualification. It does not enable conversion routes or install dependencies.
The profile is reconciled with the authored #423–#426 screenplay routes.
Examples remain unrun, and provider/route qualification is deferred. For a
practical installation and pinning sequence, start with
[Creative toolchain setup](creative-toolchain-setup.md).

```bash
renderflow doctor --profile "creative-publishing"
renderflow doctor --profile "creative-publishing" --format "json"
renderflow doctor --profile "creative-publishing" --format "json" --strict
```

Text is the default. `--format` requires an explicit profile. Plain
`renderflow doctor` retains its previous provider scope and required-tool
strict behavior. Scripttool, Node, Afterwriting, pdftotext, and pdftoppm are included only
in the explicit creative profile or complete inventories such as
`renderflow tools check-all`; these five remain outside plain doctor's scope.
The in-process `tool.renderflow-screenplay-recovery` is also excluded, making
six excluded IDs while preserving the prior plain-doctor inventory. The creative
profile contains ten tool records. Its in-process recovery provider needs no
executable probe; its availability does not qualify recovery.

## Reading the report

The report separates three questions:

1. **What tools were observed?** Each tool has an independent availability
   status, observed version when available, registry policy, and diagnostic.
   Missing or incompatible tools do not stop discovery of the others.
2. **What does Renderflow implement?** Each route reports `not_implemented` or
   `authored_unqualified`, independently of its providers. Installing a
   candidate cannot make an absent adapter available.
3. **What else must be supplied?** Explicit stream choices, pinned speech
   executables/models, OCR language data, and fonts remain separate requirements.
   Doctor does not inspect those assets or qualify their licenses.

| Tool ID | Discovery | Role and limit |
| --- | --- | --- |
| `tool.scripttool` | `scripttool -version` | Authored strict body-only imports to Fountain require a separately pinned 0.0.10 executable; discovery does not select it. |
| `tool.node` | `not_probed` | Only configured PDF preflight may observe the pinned executable with a cleared environment; doctor never launches Node from PATH. |
| `tool.afterwriting` | `not_probed` | Authored Fountain PDF adapter reads pinned package identity statically; the CLI has no version-only query. |
| `tool.pdftotext` | `pdftotext -v` | Authored bounded PDF text extraction; execution requires a separately pinned absolute path and exact version. |
| `tool.pdftoppm` | `pdftoppm -v` | Authored per-page grayscale rasterization for explicit OCR fallback; discovery does not select the recovery executable. |
| `tool.renderflow-screenplay-recovery` | In-process; no probe | Authored deterministic structure recovery and body exports; always lossy and review-required. |
| `tool.ffmpeg` | Existing registry version probe | Authored subtitle extraction and audio preparation require additional input policies. |
| `tool.ffprobe` | Existing registry version probe | Authored stream inspection and extraction support. |
| `tool.tesseract` | `tesseract --version` | Authored recovery fallback requires pinned Tesseract 5 and one pinned language model; doctor does not inspect that data. |
| `tool.whisper_cpp` | Existing registry version probe | Authored local recognition still requires explicit executable/model pins. |

The registry preserves the authored scripttool and Afterwriting capabilities;
doctor does not add graph edges or select execution providers. The
[scripttool](scripttool.md) and [screenplay PDF](screenplay-pdf.md) routes require
their explicit execution policies, exact tool/package pins, bounded input
profiles, and `allow_unqualified` opt-ins. The
[recovery route](../screenplay-recovery.md) similarly requires
`execution.screenplay_recovery`, including `allow_unqualified: true` even for
plain-text input. Supported-platform and version declarations remain candidate
scope, not a tested package matrix.

Recovery first extracts each PDF page through the pinned pdftotext executable.
Optional OCR uses pinned pdftoppm, pinned Tesseract 5, and one explicitly hashed
`.traineddata` file only on pages without usable embedded text. The routing
heuristic requires alphabetic text; it does not measure extraction accuracy.
Canonical JSON and Fountain/FDX/Fade In/OSF body derivatives retain recovery
lineage, confidence bands, ambiguities, losses, and mandatory review. Neither a
doctor result nor a global AI allowance approves a recovered screenplay.

Thirteen route rows report authored, unqualified behavior. The recovery rows
are `pdf.extract.text`, `ocr.extract.text`, `screenplay.recover.structure`, and
`screenplay.recovery.export`. Their provider status remains separate from
configuration and review requirements. PDF OCR discovery accounts for all
three native tools; plain-text reconstruction and native body writing use the
in-process provider.

Node and Afterwriting are intentionally `not_probed`: doctor neither launches
them nor claims either installation is present or missing. Their PDF route
reports providers as `unobserved` on a declared platform. Unsupported platforms
remain `unavailable`. Only an explicitly configured screenplay PDF operation
checks the pinned Node executable, package tree/manifest, font assets, and
selected formatting profile. Those observations never come from doctor or
generic runtime-service declarations. Recovery planning likewise leaves its
configured toolchain unobserved and never launches a version probe. An explicit
doctor invocation may discover PATH-selected Poppler or Tesseract; that
observation does not attest the different absolute paths selected in recovery
configuration. Only execution checks those executable/data pins.

Built-in plain timed-text conversion and the standalone saved-result alignment
exporter appear without external providers. Missing FFmpeg therefore does not
imply all transcript work is blocked. See [Subtitles](subtitles.md) and
[Local speech](speech.md) for the authored interfaces and their limits.

## Exit status

Without `--strict`, unavailable optional tools are reported without causing a
discovery failure exit. Report-generation or output errors can still fail.
With `--strict`, missing, incompatible, unsupported-platform, configuration,
and failed-probe observations fail after the complete report is written.
Explicitly `not_probed` candidates are counted separately and do not fail strict
mode. A successful exit therefore covers supported discovery only; it does not
qualify the complete creative stack, adapters, models, or fonts.

## JSON fingerprint

The JSON report uses schema `renderflow.creative-doctor/v1`, described in
[`renderflow-creative-doctor-v1.schema.json`](../../schemas/renderflow-creative-doctor-v1.schema.json).
Its `fingerprint` is a SHA-256 observation identifier with scope
`provider-discovery-observations/v1`. It covers the full selected inventory,
including missing/unprobed statuses, exact registry declarations, observed
version strings, platform labels, route policy, and stated requirements.
No timestamp or probe duration is included.

The producer hashes its compact serialized report with `fingerprint` set to an
empty string, then fills that field. Consumers should retain the emitted value
with the report. It is not an executable-byte or model digest, a runtime
environment attestation, or an execution cache key. Equal fingerprints only
mean equal report observations under this policy.

## Setup and qualification

When invoked, doctor requests only the selected registry's bounded local
version probes. It does not transform media, run recognition, inspect model
files, download assets, or install packages. Speech planning retains its
explicit pinned-provider contract; PATH observations do not select or replace
the executable/model used by a speech job. Screenplay imports likewise use
their configured scripttool path and SHA-256. PDF execution uses pinned Node
and package paths; discovery does not inspect those paths, launch either
provider, or attest its compatibility. Fixed PDF profiles use unembedded
base14 Courier and pinned package font metrics, whose fidelity and exact
licensing remain unqualified.

Install optional providers separately using their upstream guidance. The
[source evidence](../work/429/PROVIDER_SOURCES.md) records reviewed commands,
source pins, installation references, and limits. No creative provider was
added to the default devcontainer. Executable licenses, speech model licenses,
OCR language-data licenses, and font embedding/redistribution terms require
separate exact-asset review.

Issue #429 remains open for fixture/release version records, supported-platform
qualification, and execution of authored coverage. Screenplay #423–#426 and
doctor declarations are reconciled at source level. The five separate
requirements cover OCR language data, recovery review, screenplay fonts, speech
model configuration, and speech model licensing. None is satisfied merely by
finding an executable. No tests, builds, schema validators, formatting,
linting, native probes/inference, installations, or CI were run for this
reconciliation.

The [qualification record](../work/429/QUALIFICATION.md) separates accepted
policy versions from actual installation, fixture, and platform observations.
