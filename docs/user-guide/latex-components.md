# LaTeX component library

Renderflow's PDF path keeps document meaning in publication profiles and keeps
reusable presentation in a shared LaTeX component library. The current flow is:

```text
publication profile -> Pandoc template -> Renderflow styles -> Tectonic
```

The built-in research template at `templates/research/research.tex` is the
reference composition. Selecting it through the normal PDF target automatically
injects the absolute `renderflow-style-root` for Pandoc. There is no separate
LaTeX executor or manual package installation step.

## Component boundaries

The aggregate package is `templates/latex/renderflow-core.sty`. It centralizes
the design tokens and loads these cohesive components:

| Component | Shared responsibility |
|---|---|
| `renderflow-color.sty` | semantic colors, links, print-safe link behavior |
| `renderflow-typography.sty` | font roles, heading rhythm, quotations |
| `renderflow-layout.sty` | geometry, paragraph rhythm, lists, running furniture |
| `renderflow-figures.sty` | image bounds, placement, captions |
| `renderflow-tables.sty` | booktabs/longtable support and table rhythm |
| `renderflow-code.sty` | Pandoc highlighting containers and listings defaults |
| `renderflow-callouts.sty` | callout and aside presentation primitives |
| `renderflow-metadata.sty` | title/author/date roles and citation-list primitives |

Shared packages own visual treatment. A publication profile or its Pandoc
template still owns article order, front matter, title placement, column model,
editorial section meaning, and which components appear. Do not add research,
magazine, or article semantics to a `.sty` file.

## Compose a profile

A target selects the profile template through the existing artifact graph:

```yaml
targets:
  exact:
    - id: research-pdf
      role: publication/research
      format: pdf
      template: research/research.tex
variables:
  renderflow-color-accent: "4058A6"
  renderflow-margin-inner: "28mm"
```

The template declares any overrides before loading the aggregate package:

```tex
\def\RenderflowStyleRoot{templates/latex}
\def\RenderflowColorAccent{4058A6}
\input{\RenderflowStyleRoot/renderflow-core.sty}
```

Profiles can override the supported Pandoc variables without forking the style
library:

- `renderflow-color-accent`, `renderflow-color-ink`,
  `renderflow-color-muted`, `renderflow-color-rule`, and
  `renderflow-color-surface` accept six-digit hexadecimal colors.
- `renderflow-margin-top`, `renderflow-margin-bottom`,
  `renderflow-margin-inner`, `renderflow-margin-outer`, and
  `renderflow-paragraph-skip` accept LaTeX dimensions.
- `mainfont`, `sansfont`, and `monofont` select semantic font-family roles.
- `renderflow-main-font-file`, `renderflow-sans-font-file`, and
  `renderflow-mono-font-file` select local files under `renderflow-font-root`.

Profiles that need deeper changes may redefine a documented `Renderflow...`
token before loading `renderflow-core.sty`. A change that benefits multiple
profiles belongs in the smallest existing component that owns the concern. Add
a new package only when the concern is cohesive and independently reusable,
then load it from `renderflow-core.sty`.

## Fonts and deterministic fallback

The three font roles default to Latin Modern. Tectonic carries the TeX support
bundle needed for that deterministic fallback, so Renderflow does not assume a
developer workstation font. When `templates/fonts/` exists, the PDF strategy
also injects it as `renderflow-font-root`. A user-provided root remains
authoritative.

Local font variables name files rather than host-installed families. If a
declared file cannot be found, the style layer emits a package warning and uses
the corresponding Latin Modern role. Renderflow does not ship third-party font
binaries; publication owners remain responsible for font licenses and for PDF
font-embedding validation required by their publication contract.

## Pandoc and Tectonic compatibility

The library uses ordinary LaTeX2e packages available to the current Tectonic
toolchain. The research template exposes Pandoc's highlighting macros, table and
figure output, header includes, table of contents, metadata, and supported
Natbib/BibLaTeX hooks. `renderflow-style-root` is an implementation variable;
profile authors normally do not set it.

The synthetic fixture exercises title metadata, headings, prose, a quotation,
callout, table, figure/caption, highlighted code, footnote, and link:

```bash
renderflow build \
  --config "tests/fixtures/latex-components/renderflow.yaml"
```

When Tectonic is unavailable, the Pandoc composition can still be inspected
without producing a PDF:

```bash
pandoc "tests/fixtures/latex-components/showcase.md" \
  --from "markdown" \
  --to "latex" \
  --template "templates/research/research.tex" \
  --variable "renderflow-style-root=$PWD/templates/latex" \
  --output "/tmp/renderflow-component-showcase.tex"
```

Keep custom templates close to Pandoc's current template contract. Pandoc may
add generated commands over time, so profile templates should be checked when
the supported Pandoc tool version changes.
