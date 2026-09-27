# Ordered collections

A `renderflow/v2` collection names immutable local artifacts in the exact order a
collection-input transform receives them. The public `renderflow spec validate`,
`renderflow build --dry-run`, `renderflow build`, and Rust
`planning::{resolve, execute}` surfaces use the same canonical plan and run
evidence. This is the collection source boundary for later PDF and EPUB
exporters; this checkpoint does not ship those exporters.

```yaml
schema: renderflow/v2
sources:
  - id: source.page001
    path: pages/001.md
    format: markdown
    media_type: text/markdown
    sha256: "<64 lowercase hex characters for the exact file bytes>"
    geometry: { width: 210, height: 297, unit: mm }
  - id: source.page002
    path: pages/002.md
    format: markdown
    media_type: text/markdown
    sha256: "<64 lowercase hex characters for the exact file bytes>"
    geometry: { width: 210, height: 297, unit: mm }
  - id: source.pages
    kind: collection
    members: [source.page001, source.page002]
targets:
  exact:
    - format: html
      role: web
transforms: transforms.yaml
output:
  bundle_root: dist
```

Each member ID must be unique. Its path is relative to the spec directory,
with no absolute path, `..`, `.` component, or symlink in the path. A member
must declare a known common format, a media type supported for that format,
and its exact SHA-256 digest. Geometry is optional at this stage, but if
declared it must have positive dimensions and participates in identity. The
selected collection must name every declared artifact exactly once. Nested or
multiple collection sources are unsupported. No glob or directory discovery
chooses membership for you.

The transform registry must provide a collection-input edge from the common
member format to a distinct output format:

```yaml
transforms:
  - name: synthetic.ordered-pages
    program: python3
    args: ["aggregate.py", "{output}", "{inputs}"]
    input_kind: collection
    from: markdown
    to: html
    cost: 1.0
    quality: 1.0
```

The example command is a synthetic fixture, not a publication capability.
The runnable fixture and parameterized 44-member recipe are exercised by
`ordered_collection` and `ordered_collection_cli` tests. A real transform
must provide its own output validation and provider contract.

Planning imports each source into temporary content-addressed staging and
freezes the ordered member IDs, locators, media types, format, geometry,
digests, sizes, and intake profiles in `plan.source_collection`. The plan
digest names the collection cache namespace and, with the full source-spec
digest, binds checkpoint context and run evidence; the ordered input artifact
list binds transform checkpoints. Execution checks
paths again and imports every member into the durable artifact store before
running a transform. A changed or substituted member produces failed run
evidence and no committed target output.

Run artifact evidence records each member's `renderflow.source_id`,
`renderflow.collection.id`, `renderflow.collection.index`, locator, and
geometry. Aggregated outputs list input artifact IDs in declared order. When
identical member bytes share one content-addressed artifact ID, the distinct
member IDs and indices remain in the frozen plan and source evidence. The
source files are never written by this lifecycle. Resume accepts only a
checkpoint whose plan, source spec, inputs, and provider evidence remain
compatible; otherwise it recomputes or refuses under the existing checkpoint
rules.

The currently supported canonical execution path requires all root collection
members to share a format and the first edge to consume a collection. Mixed
media, same-format aggregation output, arbitrary root fan-out, PDF/EPUB
generation, and publication approval are outside this checkpoint.
