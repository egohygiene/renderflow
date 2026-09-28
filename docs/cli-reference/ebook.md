# `renderflow ebook`

Inspect an EPUB or KEPUB artifact and report the built-in e-book capability
contract.

## Inspect

```bash
renderflow ebook inspect --input "dist/book.epub" --format json
renderflow ebook inspect --input "dist/book.epub" --run-manifest "dist/renderflow-run.json" --format json
renderflow ebook inspect --input "dist/book.epub" --fixed-layout --format json
renderflow ebook inspect --input "dist/book.epub" --format json --epubcheck
```

| Flag | Meaning |
| --- | --- |
| `--input FILE` | EPUB/KEPUB file to inspect |
| `--format text\|json\|yaml` | Output format (`text` by default) |
| `--run-manifest FILE` | Bind the inspected output bytes to a Renderflow run manifest and its provenance |
| `--fixed-layout` | Require a validated exact ordered PNG/JPEG route; fail if the route is absent or invalid |
| `--epubcheck` | Also run the optional local EPUBCheck v5 provider |

Exact native fixed-layout inspection is local and bounded. Its result
is `validated`, `invalid`, or `unsupported`; typed diagnostics explain a
malformed, unsafe, incomplete, or unsupported package. A successful native
result does not assert EPUBCheck conformance. The inspected file digest alone
does not prove that it belongs to a previous run; use `--run-manifest` for that
binding. It reports `verified`, `stale`, or `corrupt` provenance, and failed
binding makes the inspection invalid. Reflowable inputs do not claim this
exact fixed-layout validation; fixed-layout KEPUB remains `unsupported`.

When `--epubcheck` is requested, the evidence records provider identity,
observed version, invocation, and result. If the executable is unavailable or
fails, the command emits structured evidence and exits nonzero. An unavailable
check is never reported as a pass.

## Capabilities

```bash
renderflow ebook capabilities --format json
renderflow capabilities --matrix --format json
```

The fixed-layout EPUB generation claim applies only to the exact, bounded
ordered PNG/JPEG collection route. The `ebook capabilities` summary is broad;
it follows `renderflow.ebook-capabilities/v1`, distinct from inspection evidence.
Its `fixed_layout_epub_route` object names the provider, source formats,
required ordered collection and explicit policy, progression/spread choices,
and validation paths. The generated conformance matrix supplies executor,
fixture, validator, and platform evidence. Fixed-layout KEPUB generation
remains unsupported.
See the [fixed-layout EPUB guide](../user-guide/fixed-layout-epub.md) for the
input contract and validation boundaries.
