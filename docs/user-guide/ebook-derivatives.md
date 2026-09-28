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
renderflow ebook inspect --input dist/book.epub --format json
renderflow ebook inspect --input dist/book.epub --format json --epubcheck
renderflow ebook capabilities --format yaml
```

The exact route is discoverable through its graph capability and the virtual
`tool.renderflow-epub` provider. The broad `ebook capabilities` summary still
reports fixed-layout generation as unsupported in #417; #418 owns changing that
claim after independent output validation. A false summary value does not
convert a selected exact route into the Pandoc reflow path.

Native inspection validates the EPUB container and EPUB 3 package envelope and
reports XHTML content, internal title/language/identifier/creator/rights
metadata, navigation, page-list, layout declarations, accessibility metadata,
and KEPUB markers. Evidence is versioned as `renderflow.ebook-evidence/v1` and
includes the immutable source SHA-256 digest.

When requested, the optional EPUBCheck provider runs locally through
Renderflow's bounded process service and embeds its JSON report. Missing
EPUBCheck is recorded as unavailable; it is never represented as a pass.

## Capability boundaries

The bundled Pandoc path generates reflowable EPUB 3. The bundled Kepubify path
generates reflowable KEPUB from EPUB. The native ordered-image route generates
fixed-layout EPUB packages with a pre-paginated declaration and explicit page
order. It does not establish fixed-layout KEPUB support. Native inspection
recognizes `rendition:layout` declarations and reports fixed-layout and mixed
publications, but independent EPUB 3.3 validation and final capability truth
remain the work of [#418](https://github.com/egohygiene/renderflow/issues/418).

Page-list and accessibility metadata are inspected and carried as evidence; a
missing page-list or sparse accessibility metadata produces a warning. EPUB 3.3
conformance is established by EPUBCheck evidence, not by ZIP validity alone.
Image descriptions and accessibility metadata also do not prove complete
accessibility for visually rich pages.

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
