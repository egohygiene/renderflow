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
renderflow ebook inspect --input "dist/source.pages/ebook.epub" --format json
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
release. Native structural inspection reports the resulting package shape;
independent EPUB 3.3 conformance and exact capability advertisement belong to
[#418](https://github.com/egohygiene/renderflow/issues/418). Optional local
EPUBCheck evidence may be requested by adding `--epubcheck` to the inspection
command above.
A missing EPUBCheck executable is reported as unavailable, never as a pass.

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
