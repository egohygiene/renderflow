# Print-interior PDF from ordered pages

`publication.generate.pdf.interior` consumes one explicitly ordered, immutable
`renderflow/v2` collection of local PNG **or** JPEG pages. The canonical planner
and executor use a locally installed `img2pdf` 0.6.3 provider and independently
inspect its output before admitting it to the artifact store. This route creates
an interior candidate; it does not approve a print job or claim retailer
acceptance.

```yaml
schema: renderflow/v2
sources:
  - id: source.page001
    path: pages/001.png
    format: png
    media_type: image/png
    sha256: "<64 lowercase hex characters for the exact file bytes>"
    geometry: { width: 50, height: 50, unit: mm, bleed: 3 }
  - id: source.page002
    path: pages/002.png
    format: png
    media_type: image/png
    sha256: "<64 lowercase hex characters for the exact file bytes>"
    geometry: { width: 50, height: 50, unit: mm, bleed: 3 }
  - id: source.pages
    kind: collection
    members: [source.page001, source.page002]
targets:
  exact:
    - id: target.interior
      role: interior
      format: pdf
execution:
  print_pdf_interior:
    executable: img2pdf
    provider_version: "0.6.3"
    box_policy: media_bleed_trim_inset
    rotation: none
    scaling: fit
    color_policy: preserve_rgb_gray
    max_pages: 44
    max_input_bytes: 268435456
    max_output_bytes: 268435456
    timeout_seconds: 120
output:
  bundle_root: dist
```

The geometry width and height are the **trim** size. `bleed` expands the
MediaBox equally on all sides; BleedBox equals MediaBox, and TrimBox is inset
by the declared bleed. Zero bleed must be stated explicitly. Every page must
have identical geometry and an image aspect ratio matching the full MediaBox
within 0.02 PDF points. The first route uses a single homogeneous PNG or JPEG
collection, no custom transform registry, no rotation, and a full-page fit
without crop or stretch. PNG supports 8-bit RGB or grayscale without alpha,
interlace, ICC, EXIF, or color intent metadata. JPEG supports decoded RGB or
grayscale without ICC, nonidentity EXIF orientation, or nonsquare pixel aspect.
The color policy proves unprofiled RGB or grayscale image bytes embedded in
the corresponding PDF device space; it does not claim calibrated color
reproduction. Unsupported or ambiguous inputs are refused.
Each image is limited to 128 MiB of source bytes and 512 MiB of decoded data,
in addition to the configured collection and PDF limits.

Use `renderflow spec validate --config renderflow.yaml`, then
`renderflow build --config renderflow.yaml --dry-run`, then
`renderflow build --config renderflow.yaml`. The executable may be an absolute
path when tool installation is isolated; it must be a trusted local
`img2pdf` 0.6.3 binary. Renderflow passes direct argv with `--nodate`, the
internal PDF engine, explicit page/image sizes, box borders, fit and rotation.
The process has a timeout, cancellation, bounded capture, input/page limits,
and a monitored output-byte limit. `network: deny` is recorded as process
intent; it is not an operating-system network sandbox. This adapter makes no
network request itself.

Planning preflights every source and freezes ordered IDs, locators, source
digests, geometry, decoded image properties, expected embedded image-stream
digests, and the exact provider/version configuration. Execution re-imports
each source and refuses changed bytes before running the provider. The PDF
inspector parses the resulting document and requires exactly one drawn image
per page, exact ordered stream digest, matching pixel dimensions and color
space, full-page placement, explicit page boxes, zero rotation, and exact page
count. It records PDF digest, page details, and ordered source lineage in run,
artifact, validation, provider/toolchain, and checkpoint evidence. Failed or
cancelled runs do not materialize an interior artifact.

Publication contracts, when provided, must agree on geometry and color; an
`interior` output-role minimum image DPI is enforced. Font embedding and
arbitrary additional output-role validators are unsupported for this
image-only route. A separate print preflight and physical proof remain necessary
before any publishing or retailer submission.
