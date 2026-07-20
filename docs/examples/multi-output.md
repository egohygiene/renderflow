# Multi Output Example

One Renderflow config can emit several output formats.

## Example config

```yaml
input: report.md
output_dir: dist
variables:
  title: Annual Report
outputs:
  - type: html
    template: default
  - type: pdf
  - type: docx
```

## What happens

1. the source file is read once,
2. built-in transforms run per target format,
3. final render steps run concurrently,
4. outputs are written to `dist/report.html`, `dist/report.pdf`, and `dist/report.docx`.

## When to use graph mode instead

Use the standard `outputs:` list when you already know the exact files you want.

Use `transforms: transforms.yaml` plus `renderflow build --target ...` or `--all` when output discovery should come from a transformation graph.
