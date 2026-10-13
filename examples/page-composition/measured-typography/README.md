# Page composition with a retained real font

This standalone two-panel package replaces the structural fixture's synthetic
font-container stub with an unmodified, locally available DejaVu Sans TTF. It
provides captured font outlines and metrics for the measured-typography provider
to inspect when the example is eventually run. It does not record a successful
render, test result or visual approval.

The original [structural-only fixture](../README.md) is unchanged. This example
has its own composition ID, content files, input pins and font registry, so it
can be copied as a complete directory without parent-path references.

## Package contents

| File | Purpose |
| --- | --- |
| `page.composition.json` | Shared artwork, layout, locale references and exact byte pins |
| `assets/two-panels.svg` | Original MIT-licensed synthetic plate, copied unchanged from the structural fixture |
| `content/en.json` | English source/default content with authored line breaks |
| `content/de.json` | Expanded German copy, including umlauts, with declared layout overrides |
| `content/ar.json` | Unreviewed right-to-left capability case; expected blocked |
| `content/ja.json` | Unreviewed vertical-writing capability case; expected blocked |
| `fonts/DejaVuSans.ttf` | Unmodified real font retained in the repository |
| `fonts/registry.json` | Pinned font metadata and explicit body/caption bindings |
| `fonts/DEJAVU-LICENSE.txt` | Verbatim packaged copyright and license notice |
| `PROVENANCE.json` | Font capture location, package version, hashes and authoring limitations |

The page declares no binding gutter. Both text containers stay within the
authored safe region. German uses a larger vertical region without reducing
the declared font size. These are authored inputs; whether the selected
provider accepts each line must come from an actual plan or render run.

English and German `reviewed: true` flags are synthetic fixture declarations.
The copy is not a reviewed consumer translation or an approval to publish.
Arabic and Japanese remain deliberately unreviewed capability examples; they
must not receive another locale's output. Selecting all declared locales
includes these expected blocked branches.

## Commands for a later authorized run

From the repository root, request English and German explicitly:

```bash
renderflow composition plan \
  --input "examples/page-composition/measured-typography/page.composition.json" \
  --locales "en,de" \
  --targets "svg,html,transcript_json,transcript_text"

renderflow composition render \
  --input "examples/page-composition/measured-typography/page.composition.json" \
  --locales "en,de" \
  --targets "svg,html,transcript_json,transcript_text" \
  --output-dir "composition-measured-output"
```

The output directory must not already exist. Inspect branch diagnostics and
the run receipt before interpreting any output as successful. Check the SVG
and HTML with the intended viewer before visual approval. This package does
not enable unsupported raster, print or layered-interchange targets.

These commands were not executed while authoring the fixture. No font parser,
font inspection tool, renderer, native probe, schema validator, test or build
was run. Hashes were computed directly from copied and authored bytes; they
identify inputs and do not establish font coverage, shaping correctness,
layout fit or publication readiness.

## Font provenance and redistribution

`fonts/DejaVuSans.ttf` was copied byte-for-byte from
`/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf` in the authoring environment.
The installed package entry in `/var/lib/dpkg/status` identifies
`fonts-dejavu-core` version `2.37-8`. The font is 759,720 bytes and has SHA-256:

```text
ae7b7855e115a5966d8b1b3f80f254ccc117ec86f9965e202ee2940453837280
```

The retained notice was copied without edits from
`/usr/share/doc/fonts-dejavu-core/copyright`. Its font section grants use,
copying and redistribution subject to retaining the copyright, trademark and
permission notices, and states that DejaVu changes are in the public domain.
The registry records `Bitstream-Vera` for the font license. The notice also
contains a separate GPL stanza for Debian packaging files; this fixture does
not copy those packaging scripts. The font binary was not subsetted, renamed
internally, converted or otherwise modified.

Keep the retained notice alongside this fixture and alongside redistributed
outputs that embed the font. The permission does not allow selling a font
typeface by itself. See the complete retained notice for its terms.

The project URL in the registry comes from that local package notice; no font
or installer was downloaded for this checkpoint. The registry intentionally
makes no `unicode_ranges` claims. Coverage decisions must inspect the captured
font for the requested text rather than infer support from a family name or
package description.

The artwork, layout and sample copy remain under this repository's MIT
license. Font notices remain separate and must be retained.
