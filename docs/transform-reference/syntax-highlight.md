# Syntax Highlight Transform

`SyntaxHighlightTransform` normalizes fenced-code language tags.

## What changes

- strips surrounding whitespace from the language tag
- lowercases the language tag
- leaves the code body untouched

## Example

````text
```Rust      -> ```rust
```  Python  -> ```python
```JavaScript -> ```javascript
````

## Why it matters

Consistent language tags improve downstream syntax highlighting behavior in Pandoc-generated output.
