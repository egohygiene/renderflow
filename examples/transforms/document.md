# {{title}}

*Written by {{author}} — Version {{version}}*

Welcome to the **Renderflow** transform pipeline demo. This document exercises
all three built-in transforms so you can see their effect on rendered output.

---

## Variable Substitution

Variables are defined in `renderflow.yaml` and referenced with `{{key}}` syntax.
This document was built using version {{version}} of the pipeline demo.

Whitespace inside the braces is ignored, so `{{ author }}` and `{{author}}`
both resolve to the same value: {{author}}.

If a key does not exist in the config, the placeholder is left as-is.
For example, `{{undefined_key}}` stays unchanged: {{undefined_key}}.

### Code Block Protection

Placeholders inside code blocks are **not** substituted, keeping example
code intact.  The variables `{{title}}` and `{{author}}` below are intentionally
preserved verbatim inside the fenced block body:

```rust
// The code block body is passed through unchanged.
// {{title}} and {{author}} remain as literal text here.
fn main() {
    println!("Hello from {{title}}!");
}
```

Inline code spans are protected too.  Writing `{{version}}` in backticks
keeps the placeholder untouched: `{{version}}`.

---

## Emoji Replacement

Emoji in the source document are handled based on the output format:

- **HTML output** — emoji are preserved so browsers can render them natively. 🎉
- **PDF / DOCX output** — emoji are replaced with `[emoji]` to prevent
  rendering failures in LaTeX-based backends. 🚀

The source of this document contains several emoji (🎉, 🚀, ✅) that you
can use to verify this behaviour across output formats.

---

## Syntax Highlighting Normalisation

Language tags on fenced code blocks are lowercased and stripped of surrounding
whitespace before Pandoc processes them.  This ensures consistent syntax
highlighting regardless of how the tag was written.

All of the following opening fences are normalised to lowercase:

```Rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

```Python
def greet(name):
    return f"Hello, {name}!"
```

```JavaScript
const greet = (name) => `Hello, ${name}!`;
```

---

## Pipeline Execution Order

The three transforms run in this fixed order before Pandoc processes the file:

1. **EmojiTransform** — replaces emoji characters (format-aware)
2. **VariableSubstitutionTransform** — substitutes `{{key}}` placeholders
3. **SyntaxHighlightTransform** — normalises fenced code block language tags

Transforms are applied in memory; the source file is never modified.
