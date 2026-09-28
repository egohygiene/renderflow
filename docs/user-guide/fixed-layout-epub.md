# Fixed-layout EPUB from ordered pages

`ebook.generate.epub.fixed-layout` consumes one explicitly ordered, immutable
`renderflow/v2` collection of local PNG **or** JPEG pages. Renderflow assembles
the package in process, without an external EPUB generator or network request.
The initial route requires a homogeneous collection and an explicit bounded
`execution.fixed_layout_epub` policy. SVG pages are refused until a safe parser,
fixture, and validation path establish their support.

## Declare an exact build

This example uses synthetic page artwork. Replace each placeholder digest with
the SHA-256 of the exact local input bytes, and keep the page dimensions and
publication metadata consistent with the artwork:

```yaml
schema: renderflow/v2
sources:
  - id: source.page001
    path: pages/001.png
    format: png
    media_type: image/png
    sha256: "<64 lowercase hex characters for page 001>"
    geometry: { width: 90, height: 90, unit: mm }
  - id: source.page002
    path: pages/002.png
    format: png
    media_type: image/png
    sha256: "<64 lowercase hex characters for page 002>"
    geometry: { width: 90, height: 90, unit: mm }
  - id: source.pages
    kind: collection
    members: [source.page001, source.page002]
targets:
  exact:
    - id: target.ebook
      role: ebook
      format: epub
      requirement: required
execution:
  fixed_layout_epub:
    page_progression_direction: ltr
    spread: none
    cover_member_id: source.page001
    max_pages: 44
    max_input_bytes: 268435456
    max_output_bytes: 268435456
publication:
  publication: Synthetic pages
  issue_id: synthetic-pages-01
  title: Synthetic pages
  publication_date: "2026-09-28"
  language: en
  contributors:
    - { name: Example Author, role: author }
  geometry: { width: 90, height: 90, unit: mm }
  artwork:
    - { role: page, path: pages/001.png, alt_text: Synthetic first page. }
    - { role: page, path: pages/002.png, alt_text: Synthetic second page. }
  rights:
    license: CC0-1.0
    rights_holder: Example Author
  accessibility:
    summary: Each page has a concise image description; the illustrated content may require a fuller transcript.
    access_modes: [visual]
    hazards: [none]
output:
  bundle_root: dist
```

The `members` array is the reading order. `cover_member_id` must identify its
first frozen member; it does not authorize an undeclared or separately fetched
cover. `page_progression_direction` changes EPUB progression metadata (`ltr` or
`rtl`), never the pixels or artwork orientation. The first supported spread
policy is `none`. The route requires publication title, issue identity, date,
language, contributor, rights holder and license, accessibility summary,
visual access mode and hazards declaration, and a
matching `publication.artwork` entry with an `alt_text` for every page. A
description of an image is useful accessibility metadata, but it is not a
transcript or proof that the publication is fully accessible.

Every member must declare positive millimeter geometry matching
`publication.geometry`. This first route refuses nonzero bleed, margin, safe
area, or an image aspect ratio inconsistent with the declared page. Its bounded
image preflight admits RGB/grayscale PNG or JPEG in the proven subset and
rejects ambiguous color intent, alpha/palette, unsafe EXIF orientation, and
unsupported image structures. Each source image has its own preflight byte and
decoded-size bounds in addition to the collection limits.

Run the public validation, planning, and execution path:

```bash
renderflow spec validate --config "renderflow.yaml"
renderflow build --config "renderflow.yaml" --dry-run
renderflow build --config "renderflow.yaml"
renderflow ebook inspect --input "dist/source.pages/ebook.epub" --fixed-layout --format json
renderflow ebook inspect --input "dist/source.pages/ebook.epub" --run-manifest "dist/renderflow-run.json" --format json
renderflow ebook inspect --input "dist/source.pages/ebook.epub" --format json --epubcheck
renderflow ebook capabilities --format json
renderflow capabilities --matrix --format json
```

The output path above follows the default naming template; use the path in the
run manifest when the output layout is customized. Inspect
`dist/renderflow-run.json`, the publication metadata under `dist/metadata/`,
and the generated EPUB. A failed or interrupted run does not claim a completed
e-book artifact.

## Package and evidence

The deterministic package starts with an uncompressed `mimetype` member,
followed by `META-INF/container.xml`, `EPUB/book.opf`, `EPUB/nav.xhtml`,
`EPUB/styles.css`, and numbered `EPUB/pages/` and `EPUB/images/` members.
Member order and timestamps are fixed; page XHTML carries an explicit
viewport. OPF records `rendition:layout` as `pre-paginated`, the manifest and
ordered spine, page progression, a cover-image relationship, and publication
metadata. Navigation contains a table of contents and a page list. Each
XHTML page references the matching local image and its declared description.

The package targets EPUB 3.3 while the OPF `package` element
uses `version="3.0"`, as shown in the [EPUB 3.3 specification](https://www.w3.org/TR/epub-33/).
That OPF value does not mean the publication is limited to an older EPUB
release.

## Validate the generated package

The native fixed-layout inspection checks the ZIP/container, OPF and local
manifest references, ordered spine and page members, declared page viewports,
navigation and page-list destinations, cover relationship, and the accessibility
evidence actually present. It checks the resulting EPUB independently of the
planner's claim that a package was produced. Structured diagnostics distinguish
a malformed or unsafe package from a complete one. A local
`renderflow ebook inspect` can inspect a file without replaying the generation
plan. Use `--fixed-layout` when requesting proof of this exact route: it fails
if the route markers are absent or the native validator does not pass. Pass
`--run-manifest` to bind inspection to the recorded output digest
and run provenance. The binding reports `verified`, `stale`, or `corrupt`;
stale/corrupt evidence makes the inspection invalid. Its exact fixed-layout
result is `validated`, `invalid`, or `unsupported`, with
`ebook.fixed_layout.*` diagnostics for failures. Do not treat a stale manifest
or a clean inspection of a different file as evidence for the current output.

`--epubcheck` additionally asks the optional local EPUBCheck v5 executable for
conformance evidence. The inspection records the observed provider identity,
version, invocation, and result separately from native checks. A missing or
incompatible executable is unavailable, never a pass; a requested unavailable
or failed provider produces a nonzero CLI exit after reporting structured
evidence. Native structural validity is not an EPUBCheck pass, accessibility
certification, or retailer approval. These are separate checks with different
authority.

| Evidence | What it establishes |
| --- | --- |
| Native `validated` | This package satisfies the bounded fixed-layout structural checks. |
| Native `invalid` | A required package or page relationship is missing, malformed, or unsafe. |
| Native `unsupported` | The package is outside the proven fixed-layout route. |
| Provenance `verified` | The inspected path, bytes, page order, and image digests match the supplied completed run's recorded output and source lineage. |
| Provenance `stale` or `corrupt` | The requested binding failed; inspect the manifest and output rather than claiming a completed validated artifact. |
| EPUBCheck pass | The observed EPUBCheck v5 invocation accepted these bytes. |
| EPUBCheck unavailable | No external conformance pass was obtained. |

The native result and the optional provider result should both be retained
when a consumer needs conformance evidence. Inspecting a ZIP without an
EPUBCheck pass cannot establish EPUB 3.3 conformance.

The capability summary advertises generation for this exact PNG/JPEG route
only. The generated conformance matrix describes its fixture, validator,
provider, and platform evidence. A positive fixed-layout generation value is
not a promise that arbitrary EPUBs are valid, that SVG is safe to package, or
that a KEPUB fixed-layout route exists.

Planning freezes each member's ID, path, digest, media type, geometry, and
position. Execution checks the sources again before import. The output records
ordered source lineage, transform configuration, toolchain, and artifact
digests. Source, order, geometry, metadata, or toolchain changes invalidate
compatible checkpoints. Source files are never modified. The policy's explicit
page and byte limits bound the package; unsupported or ambiguous inputs fail
with `fixed_epub.*` diagnostics. No renderer, retailer, upload, or publication
approval is invoked.

Pandoc's existing reflowable EPUB route remains distinct. This route does not
generate fixed-layout KEPUB; EPUB-to-KEPUB conversion must not be taken as
proof of fixed-layout KEPUB compatibility. The selected reading system and
distribution channel still require their own review.
