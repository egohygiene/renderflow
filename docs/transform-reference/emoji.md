# Emoji Transform

`EmojiTransform` scans characters and replaces emoji code points with the literal string `[emoji]`.

## Behavior

- HTML output: emoji are preserved unchanged
- PDF, DOCX, and other non-HTML outputs: emoji are replaced

## Why it exists

The transform protects non-HTML pipelines from Unicode emoji rendering issues.

## Example

| Input | HTML target | PDF target |
|---|---|---|
| `Hello 😀` | `Hello 😀` | `Hello [emoji]` |

## Implementation notes

The transform currently uses Unicode range matching and a plain placeholder. It is a compatibility layer, not a rich emoji asset renderer.
