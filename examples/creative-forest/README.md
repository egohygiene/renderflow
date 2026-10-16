# Creative forest fixture (#430)

SPDX-License-Identifier: MIT

This is an authored, **unqualified** integration suite. No Renderflow command, conversion,
native probe, test, schema validation or platform qualification was performed
while adding it. The runner uses only the public Renderflow CLI. It does not
import Renderflow implementation code or invoke another Flow repository.

From the repository root, Python 3.9+ can describe the suite without launching
any child process or creating an evidence directory:

```bash
python3 scripts/run_creative_forest_fixture.py --mode describe
```

For the later execution session, use an already built compatible Renderflow
binary and a new output directory whose parent exists:

```bash
python3 scripts/run_creative_forest_fixture.py --mode plan \
  --renderflow /absolute/path/to/renderflow --output /existing/parent/creative-plan

python3 scripts/run_creative_forest_fixture.py --mode execute \
  --renderflow /absolute/path/to/renderflow --output /existing/parent/creative-run
```

`plan` actually runs `renderflow build --config … --dry-run`, so configured
native routes may probe providers. It writes captured plans, not conversion
artifacts. Standalone analysis/text commands have no public dry-run and are
`not_run` in plan mode. `execute` runs every configured job independently. Neither
mode installs tools, downloads models, resumes old output, or overwrites sources.

## Declared output trees

Each graph job has one original source and its own spec, cache and bundle. The
runner writes a concrete `jobs/<id>/config.json` by filling source/bundle paths
and any explicitly supplied native policy. Committed `specs/*.json` are templates,
not directly runnable specs with machine paths. All exact targets are required.

| Job | Source | Exact graph targets / standalone output |
| --- | --- | --- |
| `original-body` | `sources/original.fountain` | `target.screenplay_json`, `target.fdx`, `target.fadein`, `target.osf`, `target.trelby`; built-in body exports with no provider |
| `recovery-text` | `sources/unstructured-scene.txt` | `target.screenplay_json`, `target.fountain`, `target.fdx`, `target.fadein`, `target.osf` |
| `timed-text` | `sources/captions.srt` | `target.timed_transcript_json`, `target.vtt`, `target.text` |
| `fountain-analysis` | `sources/original.fountain` | Analysis JSON in `stdout.log`; no invented graph target |
| `transcript-text` | `sources/captions.srt` | `transcript.txt` plus export report in `stdout.log` |
| `screenplay-pdf` | `sources/original.fountain` | `target.pdf`; optional pinned afterwriting policy |
| `native-import` | `sources/original.fdx` | `target.fountain`; optional pinned Scripttool policy |
| `pdf-recovery` | `sources/text-bearing.pdf` | Same five recovery targets; optional pinned pdftotext |
| `pdf-ocr` | `sources/image-only.pdf` | Same five recovery targets; optional pinned pdftotext + OCR policy |
| `speech` | Explicit external recording | `target.timed_transcript_json`, `target.srt`, `target.vtt`; pinned local speech policy |
| `embedded-subtitles` | Explicit external media | Same three timed-text targets; explicit global subtitle stream index |

Successful graph publications normally live under
`jobs/<id>/bundle/source.fixture/<target.id>.<ext>`. Extensions include
`screenplay.json` and `timed-transcript.json`; the native run's actual bundle
locators are authoritative. Each job retains `stdout.log`, `stderr.log` and, when
Renderflow writes one, `bundle/renderflow-run.json`. The launcher never creates
placeholder output files to satisfy an expected tree.

`fixture.json` records fixed expected effective target identities and formats.
These hashes were authored from the public `renderflow/target-identity-v1`
declaration policy: canonical format strings, explicit exact-target IDs and
roles, no render variants/options/selectors; generated reachable targets have
no declared ID and their role equals their format. They are not observed plan
receipts. Both actual selected membership and actual published membership must
match those declarations. The duplicate case expects one effective target;
the exhaustive cycle case expects canonical transcript, VTT and terminal text, with no source
SRT identity export and no repeating-format route.

`original-body` uses the unchanged bundled original Fountain directly, with
`execution.screenplay_body: {"allow_unqualified": true}` in its own spec. Every
target is one independent Fountain edge. The built-in writers accept the strict
six-kind body profile: scene heading, action, character, dialogue, parenthetical
and transition. They reject unsupported structure or unrepresentable text rather
than dropping it. This is separate from lossy recovery and from Scripttool's
native import; Scripttool reverse exports remain unavailable. Canonical JSON
retains the accepted parsed model; native files do not promise title pages,
layout, typography, pagination or application compatibility.

Each original-body publication retains original and canonical references,
digests, policy/profile identity, `runtime_qualified: false`, and mandatory
review. The runner cross-checks those declarations against the actual source,
published canonical target and direct route. A completed job is not a qualified
native-app opening or creative approval.

`timed-text` keeps caption parsing and canonical-to-text export in the same
isolated subtitle job. Text is terminal and retains its canonical parent,
original provenance and partial-fidelity loss declarations in native evidence.
Its bytes omit timing, cue identifiers and structured metadata. The standalone
`transcript-text` job remains a separate public-command example; it does not
provide a dry-run or create an edge back from text to timed data.

## Explicit native configuration

`native-config.empty.json` is deliberately `{}`. To attempt native jobs, author
a separate machine-local JSON object keyed by any of the six optional job IDs.
Each entry accepts only `policy`, optional absolute `source`, and optional
canonical `format`:

```json
{
  "embedded-subtitles": {
    "policy": { "stream_index": 2 },
    "source": "/absolute/path/to/owned-captioned-video.mkv",
    "format": "mkv"
  }
}
```

That example selects the actual **global** stream index 2; replace it with the
index from inspection of your own media. It is not a claim that any supplied
file has that stream. FFmpeg/FFprobe discovery follows Renderflow's existing
subtitle adapter. Other entries need the full pinned adapter policy, not a
placeholder digest or a guessed version:

| Entry | Inserted into execution | Policy contract |
| --- | --- | --- |
| `screenplay-pdf` | `screenplay_pdf` | [Original Fountain PDF](../../docs/user-guide/screenplay-pdf.md) |
| `native-import` | `screenplay` | [Native screenplay conversion](../../docs/user-guide/scripttool.md) |
| `pdf-recovery`, `pdf-ocr` | `screenplay_recovery` | [Recovery](../../docs/screenplay-recovery.md), with `allow_unqualified: true` |
| `speech` | `speech` | [Speech](../../docs/user-guide/speech.md): source digest, provider/model paths and digests, options, explicit audio policy where needed |
| `embedded-subtitles` | `subtitles` | [Timed text](../../docs/user-guide/subtitles.md): explicit selected stream |

Pass that file with `--native-config /absolute/path/to/native.json`. Omitted
jobs are `skipped_unconfigured`: no provider availability observation was made.
A configured job that cannot run is a failure, not a skip. Speech and subtitle
media sources are mandatory; speech defaults to declared `wav` and embedded
subtitles to `mkv`, so supply `format` for another supported input type. All input
sources are bounded to 64 MiB by this fixture runner. Recovery's default bundled
text policy is stricter at 8 MiB. Native policy limits remain authoritative.

The OCR job requires an `ocr` policy and at least one actual `ocr` extraction
page in retained recovery metadata. Overriding it with an entirely text-bearing
PDF does not count as exercising OCR. Original block-letter artwork may prove
unsuitable for a particular OCR installation; that must remain a failure for
investigation, not an invented successful result.

## Negative corpus

Add `--include-negatives` to include these declared scenarios:

| Case | Expected behavior |
| --- | --- |
| `malformed-recovery` | Arbitrary prose is refused with `screenplay.recovery.not_screenplay` during execution. |
| `ambiguous-recovery` | A candidate remains lossy/review-required and retains low-confidence `ambiguous_uppercase_text_retained` evidence for `STATIC`. |
| `duplicate-targets` | Two identical declarations produce one effective canonical transcript target. |
| `cycle-suppression` | Reachability over reversible caption routes selects canonical/VTT once each and excludes the source. |
| `missing-provider` | Planning retains the exact blocked canonical target with no selected output; execution refuses it with `forest.required_targets_blocked`. This tests policy exclusion, not observed executable absence. |
| `pdf-malformed` | With `pdf-recovery` configured, an authored malformed PDF is refused with `screenplay_recovery.malformed_pdf`. |
| `transcription-unavailable` | With `speech` configured, a deliberately absent provider path is refused with `cannot open provider as a local regular file`. |

The last two reuse those entries' policies but force their own negative inputs,
ignoring source/format overrides. The speech negative replaces only the provider
path and source digest for authored one-second silence, removes media-audio
preparation and historical recovery, and disables saved-result replay. It does
not purport to recognize silence. An unrelated diagnostic,
timeout, crash or unexpected success fails a refusal case. Content-refusal cases
can still plan successfully: planning does not pretend to execute the source.

## Evidence, bounds and limitations

The request is `renderflow.creative-fixture/v1`; `result.json` is
`renderflow.creative-fixture-result/v1`. `started.json` and individual job result
files preserve partial progress. Results reference exact subprocess argv,
exit status, bounded stdout/stderr, native run/standalone reports, source digests,
and published files with SHA-256 and byte length. A failed run can retain real
partial publications. A requested filename alone never becomes output evidence.
Retain the whole directory, including native metadata, sidecars and caches.
Both envelopes have an authored
[JSON Schema](../../schemas/renderflow-creative-fixture-v1.schema.json); the
launcher does not run an external schema validator.

The runner cross-checks `artifact_forest.execution` against its planning branches,
the selected target's native terminal record and publication marker, native output
membership, and actual payload digests. Full native manifests retain lineage,
loss/review declarations and toolchain evidence that the smaller Flow projection
does not carry. This is bounded envelope checking, not a replacement for native
semantic validation or a security attestation of the supplied executable.

Each child defaults to 900 seconds (`--timeout-seconds` accepts 1–7200). Captures
are limited to 8 MiB per stream. JSON evidence is limited to 64 MiB and other
referenced files to 512 MiB. POSIX timeout cleanup kills the child's process group;
Windows cleanup covers only the direct child. Still-open captures are retained
without digest claims and fail the job. This launcher is not an OS sandbox and
does not constrain a deliberately detached process or total disk/memory use.

Exit zero means selected/configured jobs met their mode-specific expectations.
It does not mean skipped native jobs, standalone planning, OCR/ASR accuracy,
platform support, every requested #430 conversion direction, or released Flow
consumption was qualified. Results always retain `qualification: unqualified`
and `coverage_complete: false`. Failed children produce `partial`/`failed` with
nonzero exit; interruption produces `cancelled` when the runner can write it.

Original-Fountain export is restricted to the built-in body profile above and
remains unqualified. Native import is native-to-Fountain; recovery exports apply
to recovered PDF/plain-text candidates. Original Fountain is never mislabeled
as plain text. Fountain-to-PDF, standalone Fountain analysis, caption conversion,
speech and embedded subtitle extraction remain separate jobs. Text cannot supply
timing, speech is never inferred from missing captions, and automatic cross-job
chaining is not implied by this example.

## Source provenance

All bundled stories, dialogue, subtitles and block-letter artwork are original
synthetic Renderflow fixture material under the repository MIT license.
`original.fountain`, `original.fdx` and `captions.srt` copy the existing
`tests/fixtures/screenplay/afterwriting-425/strict.fountain`,
`tests/fixtures/screenplay/native-424/strict.fdx`, and
`tests/fixtures/timed-text/plain-unicode.srt` respectively. Other text is newly
authored for #430. The two PDFs were assembled directly from authored PDF object
bytes: a built-in Courier text page and an image-only grayscale block-letter
page. No third-party PDF, image, font file, recording or provider output is
redistributed. The silent WAV contains authored zero-valued PCM samples and is
only a refusal fixture. OCR/mixed-page accuracy and positive recorded-speech /
embedded-media corpus qualification remain deferred.

The source-only regression cases are in
[`scripts/test_creative_forest_fixture.py`](../../scripts/test_creative_forest_fixture.py).
They have not been run. See the [suite guide](../../docs/user-guide/creative-forest.md)
and [Flow handoff](../../docs/architecture/flow-artifact-forest.md) for downstream
boundaries and the later testing checkpoint.
