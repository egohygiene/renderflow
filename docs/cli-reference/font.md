# `renderflow font`

Inspect versioned, local-first font registries. These commands never acquire
fonts or use the network.

## Validate a registry

```bash
renderflow font validate \
  --registry "assets/fonts/fonts.yaml" \
  --format "text"
```

Validation checks schema identity, local asset presence, file signatures,
pinned digests, role references, and license artifacts.

## Resolve semantic roles

```bash
renderflow font resolve \
  --registry "assets/fonts/fonts.yaml" \
  --target "epub" \
  --format "json" \
  --output "dist/font-resolution.json"
```

Targets are `html`, `latex`, `pdf`, `epub`, and `docx`. The report records the
selected asset and `fallback_index` for every role plus rejected-candidate and
fallback diagnostics.

## Generate local CSS

```bash
renderflow font css \
  --registry "assets/fonts/fonts.yaml" \
  --output "dist/fonts.css"
```

The CSS contains `@font-face` declarations for pinned local assets and semantic
role variables. HTML and EPUB adapters generate equivalent CSS automatically
when `renderflow-font-registry` is configured.
