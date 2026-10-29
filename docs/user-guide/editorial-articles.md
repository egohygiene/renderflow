# Editorial articles

The bundled `editorial` derivative profile presents one reviewed Markdown article
as HTML and PDF, with optional DOCX and EPUB branches and an exact source copy.
It uses the ordinary `renderflow/v2` planner, `renderflow.profile/v1` selection,
strategy adapter, artifact store, validation and run manifest. The versioned
presentation preset is `editorial-v1`; there is no article-specific build command.

## Start with one article

Use [the synthetic example](../../examples/editorial/README.md). A complete spec
is small:

```yaml
schema: renderflow/v2
sources:
  - id: article
    path: article.md
    format: markdown
targets:
  profiles: [editorial]
output:
  bundle_root: output
```

```bash
renderflow graph plan --config "renderflow.yaml"
renderflow build --config "renderflow.yaml"
```

The equivalent CLI override is `--profile "editorial"`; public SDK requests use
the same configuration and planner. A spec-local profile named `editorial` takes
precedence over the bundle. Named profiles may also `extends: [editorial]` and
exclude roles through the existing selector contract. For an individual output,
select its format, `transform: builtin.markdown.html` (or `.pdf`, `.docx`, `.epub`)
and `preset: editorial-v1`. A bare `--target html` resets the target selection and
does **not** select the editorial presentation.

The profile's web, print and source outputs are required. DOCX and EPUB are
optional. A missing provider remains an unavailable branch; inspect the manifest
and its publication entries before claiming that a requested file exists.

## Markdown and metadata contract

Start the UTF-8 file with YAML front matter. The header is limited to 64 KiB;
the article is limited to 4 MiB. Unknown metadata keys fail explicitly.

```yaml
---
article-schema: renderflow.editorial/v1
title: "A notebook for small observations"
lang: "en-US"
subtitle: "A fictional garden finds a useful rhythm."
author: ["Rowan Vale"]
date: "2026-10-01"
keywords: ["gardens", "shared work"]
toc: true
---
```

| Field | Contract |
| --- | --- |
| `article-schema` | Required; exactly `renderflow.editorial/v1` |
| `title`, `lang` | Required nonempty text; language is an explicit language tag |
| `subtitle` | Optional plain-text dek |
| `author`, `contributors`, `keywords` | Optional lists of 1–32 nonempty text items; contributor roles may be included in their display text |
| `date`, `updated` | Optional quoted ISO calendar dates; update must not precede publication |
| `canonical-url` | Optional explicit HTTP(S) URL; never inferred or fetched |
| `toc` | Optional boolean, default false; adds a table of contents |
| `hero` | Optional mapping with source-relative `path`, nonempty `alt`, and optional `caption` |

No author, date, canonical URL, citation or other factual metadata is invented.
Metadata text is normalized as text rather than interpreted as raw HTML/TeX.
Body headings start at `##`; the template owns the title heading. Pandoc handles
prose, lists, tables, fenced code, quotations, links, footnotes and images.
Generic variable substitution, emoji conversion and syntax-transform passes are
bypassed for this preset so code and literal `{{ ... }}` remain content.

Use `::: {.callout}` or `::: {.aside}` fenced divisions for semantic notes.
HTML receives the corresponding style; PDF uses the shared LaTeX environments.
DOCX/EPUB keep the content with writer/reader-specific presentation. Source raw
HTML/TeX is rejected; arbitrary style/event attributes are removed. The initial
profile uses English presentation labels. The declared body language does not
claim full localized typography or bidirectional-script qualification.

The proposed writing agent in [#378](https://github.com/egohygiene/renderflow/issues/378)
should emit this same header and Markdown structure. Renderflow does not invoke
Copilot or another model at runtime. Manual articles use the identical path.

## Images, styles and fonts

Images are source-relative local PNG/JPEG files, with nonempty alt text. Remote,
absolute, percent-encoded and parent-traversing image paths are rejected. The initial bound is
64 distinct images, at most 16 MiB each. Pandoc's media bag captures image bytes
for the current render. Symlinks and the host process are not an OS sandbox;
use reviewed local sources/assets.

The hero can have distinct alt text and a caption. Ordinary standalone Markdown
images use Pandoc's implicit figure semantics: the image description also serves
as its caption. The fixture demonstrates both forms. No SVG conversion provider
is silently added to the PDF route.

Templates, the original stylesheet, the article filter and licensed regular
DejaVu Serif/Sans/Sans Mono font files are embedded in the binary. The ordinary
font registry verifies their SHA-256 digests and license metadata before use.
HTML and EPUB embed resolved fonts; PDF uses explicit local font files. No Google
Fonts, CDN stylesheet or runtime font download is needed. Bundle provenance is in
`crates/renderflow-core/data/editorial/fonts/registry.json`; the matching license
notice is included beside the fonts and in the staged bundle.

To use your own reviewed fonts, set the existing `renderflow-font-registry`
variable to a config-relative registry file. Use pinned local assets and explicit
fallbacks. Missing/mismatched bundled fonts fail instead of silently choosing
host fonts. Regular faces are pinned; bold/italic synthesis, math fonts, script
coverage and visual output still require provider-specific qualification. DOCX
does not embed this font bundle and uses its reference-document/application fonts.

The preset owns its templates. Combining `preset: editorial-v1` with `template`
or unrelated config variables is rejected. Specialized presentation belongs in
an explicit custom transform/preset with its own declared behavior.

## Providers, reproducibility and evidence

Pandoc **3.6.1+** is required for the filter's figure/caption contract. The planner
raises the selected run's Pandoc minimum when an editorial target is present;
unrelated runs retain the normal provider contract. PDF additionally requires
Tectonic. PDF invocations use `--only-cached` and `--untrusted`: prepare the local
TeX support cache during tool setup, before the canonical offline build. Missing
cached resources fail. Provider discovery does not establish cache completeness.

The process receives a fixed `SOURCE_DATE_EPOCH=315532800` (the ZIP epoch) for tool-generated timestamps;
this does not supply or change the article's publication/update dates. Exact
provider versions, TeX cache contents and platform behavior must still be locked
and qualified before claiming byte-for-byte reproducibility.

The artifact descriptor records the editorial schema and SHA-256 of the entire
embedded presentation bundle. Source lineage, provider inventory, target identity,
branch outcomes and format validation remain normal run evidence. Source-adjacent
images and user font registries are not frozen at plan time, so editorial render
cache/checkpoint reuse is disabled. This avoids reusing stale external assets;
it does not claim complete external-asset attestation. A changed user font
resolution is refused immediately before rendering; this is a drift check, not
an immutable snapshot or protection against concurrent host-file replacement.

| Output | Retained behavior and limits |
| --- | --- |
| HTML | Standalone responsive article with embedded fonts/images, heading/navigation semantics, captions and footnotes; browser layout varies |
| PDF | A4 print presentation using shared LaTeX components; no PDF/UA, print-provider, exact pagination or visual-quality certification |
| DOCX | Editable article semantics; supplemental metadata retained as text; no HTML/PDF layout or font parity |
| EPUB | Reflow semantics and local CSS/fonts; reader pagination and style overrides remain possible |
| Markdown | Exact original source bytes; no normalization, asset copying or promise of a standalone source package |

The normal core format validators inspect outputs. They do not replace native
application opening, accessibility review, visual inspection or repeatability
checks. The process network flag is declarative rather than an OS firewall.

## Current checkpoint

Implementation and regression cases are authored. Tests, compiler/build/Cargo,
lint/format/schema/docs checks, product CLI/provider/native execution and CI are
deferred by the maintainer. No rendered specimen or passing visual/runtime result
is claimed. #379 remains open until those observations satisfy its acceptance;
#458 and #419 retain baseline and release work.

Provider references: [Pandoc manual](https://pandoc.org/MANUAL.html),
[Pandoc Lua API](https://pandoc.org/lua-filters.html), and
[Tectonic cached-only options](https://tectonic-typesetting.github.io/book/latest/ref/v1cli.html).
