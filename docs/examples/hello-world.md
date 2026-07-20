# Hello World Example

Renderflow ships a minimal example in `examples/hello-world/`.

## Files

- `hello.md`
- `renderflow.yaml`

The example config is intentionally tiny:

```yaml
input: hello.md
output_dir: dist
outputs:
  - type: html
```

## Run it

```bash
cd examples/hello-world
renderflow build
```

## Result

The generated output appears at `examples/hello-world/dist/hello.html`.

This example is ideal for verifying your installation before moving on to templates, variables, or graph-based planning.
