# Quick Start

This walkthrough uses the standard document build pipeline.

## 1. Create `input.md`

```markdown
# Hello from Renderflow

This page was generated from a YAML spec.
```

## 2. Create `renderflow.yaml`

```yaml
input: input.md
output_dir: dist
outputs:
  - type: html
```

## 3. Build

```bash
renderflow build
```

Because `renderflow.yaml` is the default config filename, no extra flags are required.

## 4. View the output

Renderflow writes the result to `dist/input.html`.

- Open it in a browser, or
- inspect the generated file from the terminal.

## Next steps

- Add `pdf` or `docx` to `outputs`
- add `variables` and use `{{key}}` placeholders
- add a `template` for HTML output
- add `transforms: transforms.yaml` to unlock graph-based planning

!!! tip
    You can preview the plan without writing files by running `renderflow build --dry-run`.
