# Transform Reference

Renderflow ships three built-in document transforms and supports YAML-defined command, AI, and aggregation transforms.

## Built-ins

- [Emoji](emoji.md)
- [Variables](variables.md)
- [Syntax Highlight](syntax-highlight.md)

## Registry order

Built-ins are registered in `register_transforms` in this fixed order:

1. emoji
2. variable substitution
3. syntax-highlight normalization

That ordering is stable even when `variables` is empty.
