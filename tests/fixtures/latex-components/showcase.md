---
title: Shared Component Showcase
subtitle: Synthetic research profile fixture
author:
  - Renderflow Contributors
date: "2026-09-11"
abstract: |
  A redistribution-safe document that exercises the shared Renderflow LaTeX
  presentation layer without relying on external or licensed assets.
toc: true
---

# Typography and prose

This paragraph demonstrates body copy, a [local-safe link](https://example.com),
and a footnote with inspectable synthetic content.[^fixture]

> A quotation remains semantic Markdown while the shared typography component
> controls its presentation.

\begin{RenderflowCallout}[Reproducibility]
This callout is raw LaTeX by design: profiles opt into a presentation primitive
without moving the surrounding document structure into the style package.
\end{RenderflowCallout}

## Table

| Primitive | Shared concern | Profile concern |
|---|---|---|
| Typography | role styling | editorial hierarchy |
| Figure | caption treatment | asset selection |
| Callout | visual treatment | callout meaning |

## Figure

\begin{figure}
\centering
\fbox{\rule{0pt}{24mm}\rule{0.72\linewidth}{0pt}}
\caption{A synthetic figure placeholder with no external asset dependency.}
\end{figure}

## Code

```rust
fn artifact_is_reproducible(digest: &str) -> bool {
    !digest.is_empty()
}
```

[^fixture]: The fixture uses only original prose and a geometric placeholder.
