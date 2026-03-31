# Transform Pipeline Example

An end-to-end example that exercises all three built-in Renderflow transforms:

| Transform | What it does |
|-----------|--------------|
| **EmojiTransform** | Replaces emoji with `[emoji]` (skipped for HTML output) |
| **VariableSubstitutionTransform** | Substitutes `{{key}}` placeholders with config values |
| **SyntaxHighlightTransform** | Normalises fenced code block language tags to lowercase |

## Files

| File | Description |
|------|-------------|
| `document.md` | Source document that uses variables, emoji, and mixed-case code fences |
| `renderflow.yaml` | Config that defines `variables` and targets HTML output |

## Prerequisites

- [Renderflow](https://github.com/egohygiene/renderflow) installed and available on your `PATH`
- [Pandoc](https://pandoc.org/installing.html) installed (required by Renderflow)

## Running the Example

From within this directory:

```bash
renderflow build --config renderflow.yaml
```

Or, because `renderflow.yaml` is the default config file name, simply:

```bash
renderflow build
```

## Expected Output

After a successful build the rendered HTML file is written to `dist/`:

```
examples/transforms/
├── dist/
│   └── document.html   ← generated output
├── document.md
├── renderflow.yaml
└── README.md
```

Open `dist/document.html` in your browser to verify the transforms.

## What to Look For

- **Variable substitution** — headings and paragraphs show the resolved values
  (`title`, `author`, `version`) instead of the `{{...}}` placeholders.
- **Code block protection** — the `{{title}}` and `{{author}}` placeholders
  inside the fenced code block and the inline `` `{{version}}` `` span are
  **not** substituted; they remain as literal text.
- **Emoji preservation** — because the output type is `html`, emoji characters
  (🎉, 🚀, ✅) are kept unchanged so the browser can render them natively.
- **Syntax highlighting** — the mixed-case language tags (`Rust`, `Python`,
  `JavaScript`) are normalised to lowercase in the rendered HTML.

## Dry Run

To preview what Renderflow would do without writing any files:

```bash
renderflow build --dry-run
```
