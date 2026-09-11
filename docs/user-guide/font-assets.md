# Local font assets

Renderflow font registries make typography reproducible without relying on a
developer workstation, a globally installed family, or a font CDN. A normal
render only reads reviewed local files. It never downloads a font.

## Registry contract

A registry uses the `renderflow.font-registry/v1` schema and keeps three
concerns together:

- immutable identity: stable asset IDs, exact source revisions, and SHA-256
  digests;
- rights: SPDX license identity, a retained license artifact, redistribution
  status, and embedding permission;
- typography intent: semantic roles and deterministic ordered fallbacks.

```yaml
schema: renderflow.font-registry/v1
registry_id: publication.orientation-fonts
version: "1.0.0"
assets:
  - id: font.atkinson.regular
    family: Atkinson Hyperlegible
    style: normal
    weight: 400
    stretch: normal
    path: assets/fonts/AtkinsonHyperlegible-Regular.ttf
    format: ttf
    digest:
      algorithm: sha256
      value: "<sha256-of-the-reviewed-file>"
    provenance:
      source_url: "<exact-versioned-source-or-release-URL>"
      source_version: "<pinned-release-or-commit>"
      project_url: https://fonts.google.com/specimen/Atkinson+Hyperlegible
    license:
      spdx_id: OFL-1.1
      license_file: assets/fonts/OFL.txt
      redistribution: allowed
      embedding: allowed
    unicode_ranges: [U+0000-00FF]
    intended_roles: [body, caption]
roles:
  body:
    primary: font.atkinson.regular
  caption:
    primary: font.atkinson.regular
```

Paths are relative to the registry file. Digests identify exact bytes, not just
a family name or upstream version label. The bundled JSON Schema is
`schemas/renderflow-font-registry-v1.schema.json`.

## Explicit acquisition workflow

For an open Google Fonts family or another external catalog:

1. Review the upstream license and confirm the intended distribution and
   embedding are permitted.
2. Download a specific release outside Renderflow's normal build.
3. Keep only the required weights and styles in a project-owned asset pack.
4. Retain the applicable license file beside the assets.
5. Calculate each file's SHA-256 digest and record its exact upstream source and
   version in the registry.
6. Review and commit the registry and, when redistribution permits, the asset
   pack. A private or separately distributed asset pack can use the same model.

Renderflow intentionally has no `download` command. This prevents a routine
render from accepting changed upstream bytes, unreviewed licenses, or a live
network dependency.

## Semantic roles and fallbacks

Profiles select `body`, `heading`, `display`, `monospace`, `caption`, and `math`
roles. Each role names a primary asset followed by ordered fallback asset IDs.
Resolution rejects a candidate when its bytes are missing, its digest or format
signature is wrong, its style/weight does not match, its format is unsupported
by the target, or its license metadata blocks embedding.

The first usable candidate wins. Selecting a fallback adds
`font.fallback.selected` to the resolution diagnostics and records a nonzero
`fallback_index`; fallback is never silent.

`unicode_ranges` records reviewed coverage claims using CSS-style `U+` ranges.
It is useful for early policy checks, but it is not proof that every glyph is
present. Publication preflight should still inspect final PDF/EPUB artifacts
when exact script coverage matters.

## Validate and inspect

```bash
renderflow font validate --registry "fonts.yaml"
renderflow font resolve --registry "fonts.yaml" --target "pdf" --format "json"
renderflow font css --registry "fonts.yaml" --output "fonts.css"
```

Validation checks registry identity, role references, local files, font
container signatures, SHA-256 values, and retained license artifacts. Resolution
additionally applies renderer-format and embedding policy.

## Use during rendering

Reference the registry through the provider-neutral v2 variable:

```yaml
schema: renderflow/v2
sources:
  - id: manuscript
    path: issue.md
    format: markdown
targets:
  exact:
    - id: web
      role: digital/web
      format: html
    - id: print
      role: print/press
      format: pdf
      template: research/research.tex
variables:
  renderflow-font-registry: assets/fonts/fonts.yaml
execution:
  requirements:
    local_only: true
    offline: true
  network: deny
```

Renderflow resolves the registry relative to the specification before planning.
The complete resolution fingerprint participates in transform cache identity and
is attached to output artifact metadata.

| Target | Adapter behavior |
| --- | --- |
| HTML | Generates local `@font-face` CSS and asks Pandoc to embed resources into standalone output. |
| EPUB | Supplies generated role CSS and explicit local `--epub-embed-font` assets. |
| PDF/LaTeX | Resolves TTF/OTF role files into `mainfont`, `sansfont`, `monofont`, and the shared Renderflow LaTeX variables. |
| DOCX | Validates and records the role resolution; a reference DOCX remains authoritative for actual Office theme/font expectations. |

WOFF/WOFF2 are suitable for HTML and EPUB. TTF/OTF are required for the current
Tectonic/fontspec PDF path. Use separate assets under one family ID when a
publication needs both web and print targets; the deterministic fallback list
can select the first target-compatible file.

## Packaging responsibility

Renderflow ships the registry contract and a tiny synthetic contract fixture,
not an unbounded catalog of third-party fonts. Publication owners decide whether
licensed font bytes belong in the repository, a private asset pack, or another
content-addressed distribution. Normal rendering remains local in every case.
