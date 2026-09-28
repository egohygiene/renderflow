# EPUB and KEPUB derivatives

Renderflow models EPUB and KEPUB as provider-neutral artifact formats. Pandoc
generates reflowable EPUB 3 from documents. The exact native
[fixed-layout EPUB](fixed-layout-epub.md) route packages a bounded ordered PNG
or JPEG collection. KEPUB is a separate `epub -> kepub` graph edge backed by
Kepubify for the established reflowable path; its provider and fallback remain
visible in the execution plan and provenance.

## Generate derivatives

Legacy configuration accepts both document outputs:

```yaml
input: manuscript.md
variables:
  title: Example publication
  author: Example Author
  language: en
  identifier: urn:uuid:replace-me
  rights: Copyright holder and license statement
outputs:
  - type: epub
  - type: kepub
```

Document-source spec v2 callers can request `format: epub` or `format: kepub`.
That KEPUB branch reuses the reflowable EPUB artifact and only runs Kepubify
after EPUB generation. Final KEPUB files use the conventional `.kepub.epub`
suffix. Ordered image collections use the separate exact EPUB target and
`execution.fixed_layout_epub` policy described in the fixed-layout guide.

An EPUB output's optional `template` field names an OPF metadata fragment under
the configured template directory and is passed to Pandoc as `--epub-metadata`.
The `title`, `subtitle`, `author`/`creator`, `language`/`lang`, `identifier`,
`date`, `rights`, and `description` variables are also mapped to internal EPUB
metadata. Canonical source metadata remains preferable.

## Inspect and validate

```bash
renderflow ebook inspect --input "dist/book.epub" --format json
renderflow ebook inspect --input "dist/book.epub" --fixed-layout --format json --epubcheck
renderflow ebook inspect --input "dist/book.epub" --run-manifest "dist/renderflow-run.json" --format json
renderflow ebook capabilities --format yaml
```

The fixed-layout generation capability is scoped to the exact bounded ordered
PNG/JPEG collection route, selected through the virtual
`tool.renderflow-epub` provider. The `ebook capabilities` summary and generated
conformance matrix must be read with that route constraint: they do not claim
arbitrary document-to-fixed-layout conversion, SVG input, or fixed-layout
KEPUB. Pandoc's reflowable document path remains separate.

Native inspection validates the EPUB container and EPUB 3 package envelope and
reports XHTML content, internal title/language/identifier/creator/rights
metadata, navigation, page-list, layout declarations, accessibility metadata,
and KEPUB markers. For the exact native fixed-layout route, the fixed-layout
evidence is `validated`, `invalid`, or `unsupported` with typed diagnostics.
Pass `--run-manifest` to check that the inspected bytes match the recorded
output and provenance (`verified`, `stale`, or `corrupt`). Failed binding marks
the inspection invalid. Evidence is versioned as `renderflow.ebook-evidence/v1`
and includes the immutable source SHA-256 digest.

When requested, the optional EPUBCheck v5 provider runs locally through
Renderflow's bounded process service and embeds its result, including provider
identity and the observed executable version when available. A missing
EPUBCheck is recorded as unavailable and the requested CLI command exits
nonzero. It is never represented as a pass. Native inspection and EPUBCheck
conformance evidence are separate decisions.

## Capability boundaries

The bundled Pandoc path generates reflowable EPUB 3. The bundled Kepubify path
generates reflowable KEPUB from EPUB. The native ordered-image route generates
fixed-layout EPUB packages with a pre-paginated declaration and explicit page
order. It does not establish fixed-layout KEPUB support. Native inspection
recognizes `rendition:layout` declarations and reports fixed-layout and mixed
publications; the exact fixed-layout route additionally requires checked
package/member relationships and page evidence.

The exact fixed-layout validator checks the declared page-list and accessibility
relationships rather than inferring them from an EPUB ZIP extension. The
presence of descriptions and metadata does not prove complete accessibility
for visually rich pages. EPUB 3.3 conformance requires successful EPUBCheck
evidence; native structural validity alone makes no such claim.

Retailer acceptance is deliberately outside this generic format capability.
Every inspection reports `requires_provider_profile`; Lulu, Kobo, or another
channel pack must evaluate its own current restrictions before calling an
artifact upload-ready.

## Provider decision

Kepubify is adopted as the focused KEPUB provider: it is standalone,
cross-platform, local, and MIT-licensed. Calibre remains deferred as a broader
fallback because its larger conversion surface and output variation need a
separate adapter and conformance case. EPUBCheck is adopted as the authoritative
external conformance provider alongside Renderflow's deterministic native
structural inspection.

## References

- [EPUB 3.3](https://www.w3.org/TR/epub-33/)
- [EPUBCheck](https://github.com/w3c/epubcheck)
- [Kepubify documentation](https://pgaskin.net/kepubify/docs/)
