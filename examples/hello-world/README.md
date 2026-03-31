# Hello World Example

A minimal end-to-end example showing how to use Renderflow to render a Markdown
file into HTML.

## Files

| File | Description |
|------|-------------|
| `hello.md` | Sample Markdown input |
| `renderflow.yaml` | Renderflow configuration (markdown → HTML) |

## Prerequisites

- [Renderflow](https://github.com/egohygiene/renderflow) installed and available on your `PATH`
- [Pandoc](https://pandoc.org/installing.html) installed (required by Renderflow)

## Running the Example

From within this directory:

```bash
renderflow build --config renderflow.yaml
```

Or, because `renderflow.yaml` is the default config file name, you can simply run:

```bash
renderflow build
```

## Expected Output

After a successful build, the generated HTML file will be written to the `dist/`
directory inside this example folder:

```
examples/hello-world/
├── dist/
│   └── hello.html   ← generated output
├── hello.md
├── renderflow.yaml
└── README.md
```

Open `dist/hello.html` in your browser to view the rendered document.

## Dry Run

To preview what Renderflow would do without writing any files:

```bash
renderflow build --dry-run
```
