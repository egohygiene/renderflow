# Synthetic coloring-book fixture

This is a CC0, non-commercial sample made only from basic geometric shapes. It
exists to exercise the coloring-book contract and derivative profile; it is not
publication or comic content.

```bash
renderflow publication coloring-book-preflight \
  --contract "book.yaml" \
  --format json \
  --output "validation.json"

renderflow graph plan \
  --config "renderflow.yaml" \
  --profile "coloring-book"

renderflow build \
  --config "renderflow.yaml" \
  --profile "coloring-book" \
  --dry-run
```

The normal path is entirely local. The fixture's page bytes and approval
digests are intentionally small and inspectable.
