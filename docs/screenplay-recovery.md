# Recover a screenplay from PDF or text

Issue #426 authors a deterministic recovery path from an existing screenplay PDF
or UTF-8 text into `renderflow.screenplay/v1`, with optional Fountain, FDX, Fade In,
and OSF derivatives. This checkpoint is **authored and unqualified**. Tests,
compilation, lint, schema validation, native extraction, application opening,
visual review, and CI are deferred. Commands below are examples, not observed
successful runs.

Recovery guesses structure that the input may no longer describe explicitly.
Every candidate and derivative stays **lossy and review-required**, including
high-confidence classifications. A completed bundle means its files were
published; it does not approve the recovered screenplay.

## Start with unstructured text

The text path uses no external tool. Save a configuration that explicitly
accepts this unqualified implementation:

```json
{
  "allow_unqualified": true
}
```

Then recover a new bundle:

```bash
renderflow screenplay recover \
  --input "scene.txt" \
  --source-format "text" \
  --config "recovery-text.json" \
  --output "scene-recovered" \
  --target "fountain" \
  --target "fdx"
```

The `--target` option is repeatable and optional. Omit it to produce only the
canonical candidate and evidence. Supported native targets are `fountain`,
`fdx`, `fadein`, and `osf`. Each explicitly requested target must fit the
exporter's adopted body profile; refusal prevents a completed bundle. Recover
without that target to inspect the candidate first. No PDF is regenerated.

The source remains read only. Optional `--source-sha256` pins its exact bytes.
The output directory must not exist, and its parent must already exist. The
command refuses source/output overlap and rechecks the source digest before
publication. Recovery uses memory and, for native tools, temporary storage; it
is not an in-place or zero-storage conversion.

Text must be UTF-8. Form-feed characters separate pages; line boundaries and
original page text remain available in evidence. Deterministic heuristics
classify scene headings, character cues, dialogue, parentheticals, action, and
transitions. Ambiguous classifications carry confidence bands and reasons.
These bands are rule outcomes, not measured accuracy probabilities. Review
character/dialogue ownership, page headers, page breaks, reading order, and
lines that could be either action or a character cue.

## Extract an existing PDF

Set `--source-format "pdf"` and supply a pinned, already installed local
`pdftotext`. Replace every placeholder with the identity of the selected tool:

```json
{
  "allow_unqualified": true,
  "pdftotext": {
    "executable": "/absolute/path/to/pdftotext",
    "executable_sha256": "<verified-64-character-lowercase-sha256>",
    "expected_version": "<observed-numeric-version>"
  }
}
```

```bash
renderflow screenplay recover \
  --input "scene.pdf" \
  --source-format "pdf" \
  --config "recovery-pdf.json" \
  --output "scene-pdf-recovered" \
  --target "fountain"
```

The placeholders are intentionally not valid executable configuration. Hashes
identify supplied bytes; they do not establish that the installation is trusted
or qualified. Renderflow does not install tools, search `PATH` for recovery
providers, download models, or silently replace a missing provider.

PDF text extraction runs before OCR selection. Page boundaries are retained,
and each page records its extraction method and text digest. The usable-text
heuristic requires alphabetic text; it is a routing decision, not proof that all
visible text was recovered. A page with a usable but incorrect text layer can
still need human correction. Malformed, encrypted, unsupported, oversized, or
unreadable inputs return typed failures. Supply an explicitly decrypted copy
when necessary; recovery does not guess passwords or rewrite the original.

## Explicit bounded OCR fallback

For pages without usable embedded text, configure both a PDF rasterizer and
Tesseract with one pinned language model:

```json
{
  "allow_unqualified": true,
  "pdftotext": {
    "executable": "/absolute/path/to/pdftotext",
    "executable_sha256": "<verified-pdftotext-sha256>",
    "expected_version": "<observed-poppler-version>"
  },
  "ocr": {
    "pdftoppm": {
      "executable": "/absolute/path/to/pdftoppm",
      "executable_sha256": "<verified-pdftoppm-sha256>",
      "expected_version": "<observed-poppler-version>"
    },
    "tesseract": {
      "executable": "/absolute/path/to/tesseract",
      "executable_sha256": "<verified-tesseract-sha256>",
      "expected_version": "<observed-tesseract-5-version>"
    },
    "language": "eng",
    "traineddata": "/absolute/path/to/eng.traineddata",
    "traineddata_sha256": "<verified-language-model-sha256>"
  },
  "limits": {
    "max_input_bytes": 8388608,
    "max_pages": 100,
    "max_page_text_bytes": 262144,
    "max_total_text_bytes": 2097152,
    "max_raster_side": 3000,
    "timeout_seconds": 300
  }
}
```

OCR runs page by page only where the text pass found no usable text. Mixed PDFs
can therefore retain embedded text on some pages and use OCR on others. With
OCR disabled, a mixed PDF retains pages without usable text as missing or
uncertain content with explicit page diagnostics. An entire PDF without usable
embedded text produces a typed provider-unavailable failure. Evidence retains
the first text pass and identifies pages that used OCR. Raster dimensions,
model identity, tool identities, and invocation hashes
are recorded. No OCR result is labeled original or human-approved.

Limits may be tightened. If a `limits` object is provided, all six fields must
be present. Defaults cap source input at 64 MiB, pages at 1,000, text per page at
1 MiB, total extracted text at 16 MiB, raster side at 3,000 pixels, and the shared
extraction budget at 300 seconds. The canonical recovery profile separately
limits nonblank elements and retained evidence. A native process timeout,
cancellation, or rejected output prevents a completed bundle.

Native execution uses direct arguments, private staged inputs, cleared
environments, bounded captures, and pinned files. These controls are not an
operating-system sandbox or a hard whole-process memory cap. Executable/model
hashes do not attest dynamically loaded libraries, fonts, Poppler data, or an
entire runtime installation. PDF object limits checked after parsing do not
bound every parser allocation. No AI provider is invoked, even if a wider
project policy permits AI.

## Retain the evidence bundle

Every successful command writes these members:

| Member | Purpose |
| --- | --- |
| `screenplay.screenplay.json` | Validated canonical screenplay with recovery provenance and recorded losses. |
| `recovery.json` | Full deterministic candidate, page text, element source spans, confidence, ambiguity, and review-required status. |
| `extracted-pages.json` | Ordered extracted page text and the method used for each page. |
| `provider-evidence.json` | Source identity, page extraction evidence, selected tool/model identities, and observed invocations. |
| `recovery-config.json` | Configuration retained for interpreting the extraction evidence. |
| `screenplay.<format>` and `<format>-evidence.json` | Each explicitly requested native derivative and its recovery lineage. |
| `bundle.json` | Completion receipt binding all member hashes and sizes; written last and also printed to stdout. |

Keep these files together and retain the original source separately. Native
files cannot carry all recovery evidence themselves. Their sidecars preserve
the relationship to the candidate, including text joining needed for the
adopted export profile. Native-application acceptance and complete visual or
semantic fidelity are not established by serialization or structural checks.
The receipt hashes exact saved file bytes. Export evidence additionally hashes
the compact serialized candidate object; its `candidate_sha256` therefore
differs from the byte hash of the pretty-printed `recovery.json` file. Canonical
export identity hashes the exact `screenplay.screenplay.json` bytes, including
the final newline. Extraction configuration and page-object digests likewise
use compact object serialization; their saved member byte identities remain
separately bound by the receipt.

All members are prepared before the output directory is created. Publication
creates the directory exclusively and persists each member without replacing
an existing file. A publication I/O failure or cancellation can leave an
**incomplete directory without `bundle.json`**. Do not treat it as completed.
A stdout failure after publication can leave a completed bundle on disk.
Members are capped at 32 MiB and the entire bundle at 128 MiB; source identity
is rechecked under a separate 30-second publication budget. Filesystem calls
that block are not interrupted by those between-read checks.
The CLI member ceiling is deliberately stricter than the standalone candidate
model's 64 MiB report ceiling; a model that is valid in isolation can still
exceed this bundle limit and be refused without publishing a completion receipt.

## Graph and SDK use

Use the same configuration under `execution.screenplay_recovery` in a v2 spec.
Declare one `pdf` or `text` source and explicitly request `screenplay-json` and
any supported native derivatives. The authored graph path is PDF/text to
canonical recovery, then canonical recovery to Fountain, FDX, Fade In, or OSF.
The original PDF is excluded from exhaustive derivative targets by default;
recovery adds no route that regenerates it.

Recovery is isolated from other native policies, custom transforms, and
post-conversion hygiene so those operations cannot erase recovery evidence.
The capabilities are `screenplay.recover.structure` and
`screenplay.recovery.export`. Planning validates declarations without running
configured native providers. Tool availability remains unobserved until an
explicit execution checks it. Cache reuse and qualification claims remain
disabled for this authored native path.

See the [text example configuration](../examples/screenplay-recovery/text-policy.json),
[text source fixture](../examples/screenplay-recovery/scene.txt), and
[v2 graph example](../examples/screenplay-recovery/text-spec.json).
The dedicated CLI always preserves the canonical candidate and evidence even
when only a native target is requested. Graph recovery retains its review
evidence in the artifact lineage and manifest; select the canonical target
explicitly when you also want a standalone canonical output file.

AI assistance is outside this checkpoint. A future optional adapter would need
separate evidence and explicit operator permission. Neither deterministic
recovery nor future AI assistance may upgrade a candidate's approval status
without a distinct human review step.
