# Variable Substitution Transform

`VariableSubstitutionTransform` replaces `{{key}}` placeholders with values from `config.variables`.

## Rules

- surrounding whitespace is ignored, so `{{ title }}` matches `title`
- unknown variables are left unchanged
- unclosed placeholders are left unchanged
- fenced code blocks are preserved verbatim
- inline code spans are preserved verbatim

## Example

Config:

```yaml
variables:
  title: Renderflow Guide
```

Input:

```markdown
# {{title}}
Use `{{title}}` in code.
```

Output:

```markdown
# Renderflow Guide
Use `{{title}}` in code.
```
