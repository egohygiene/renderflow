# Creative toolchain doctor

The optional `creative-publishing` profile reports local tools used by creative
conversion work and explains which adapters still need implementation or
qualification. It does not enable conversion routes or install dependencies.
This interface is authored for issue #429; the examples below remain unrun.

```bash
renderflow doctor --profile "creative-publishing"
renderflow doctor --profile "creative-publishing" --format "json"
renderflow doctor --profile "creative-publishing" --format "json" --strict
```

Text is the default. `--format` requires an explicit profile. Plain
`renderflow doctor` retains its previous provider scope and required-tool
strict behavior. The four new candidate descriptors also appear in explicitly
requested complete registry inventories such as `renderflow tools check-all`.

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
| `tool.scripttool` | `scripttool -version` | Screenplay interchange candidate; adapter pending #424. |
| `tool.node` | `node --version` | Runtime candidate; does not establish an Afterwriting installation. |
| `tool.afterwriting` | `not_probed` | Reviewed CLI has no version-only query; PDF adapter pending #425. |
| `tool.pdftotext` | `pdftotext -v` | PDF text candidate; recovery adapter pending #426. |
| `tool.ffmpeg` | Existing registry version probe | Authored subtitle extraction and audio preparation require additional input policies. |
| `tool.ffprobe` | Existing registry version probe | Authored stream inspection and extraction support. |
| `tool.tesseract` | Existing registry version probe | OCR candidate; language-data and confidence integration remain pending. |
| `tool.whisper_cpp` | Existing registry version probe | Authored local recognition still requires explicit executable/model pins. |

The new candidates declare no conversion capabilities or graph edges. No
release compatibility range has been qualified for them. Supported-platform
declarations describe candidate scope, not a tested package matrix.

Afterwriting is intentionally `not_probed`: doctor neither launches it nor
claims it is installed or missing. Its route reports providers as `unobserved`
when this is the only unresolved discovery result; a missing Node installation
instead makes the provider group `unavailable`.

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
the executable/model used by a speech job.

Install optional providers separately using their upstream guidance. The
[source evidence](../work/429/PROVIDER_SOURCES.md) records reviewed commands,
source pins, installation references, and limits. No creative provider was
added to the default devcontainer. Executable licenses, speech model licenses,
OCR language-data licenses, and font embedding/redistribution terms require
separate exact-asset review.

Issue #429 remains open for actual adapter integration, fixture/release version
records, supported-platform qualification, and execution of the authored
coverage. This checkpoint ran no tests, builds, schema validators, formatting,
linting, native probes/inference, or CI.
