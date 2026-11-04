# Editorial article example

This fictional article exercises metadata, a hero image with separate alt text
and caption, headings, a quotation, a table, code, callouts, links and a footnote.
The image is original geometric artwork; its source recipe is `make_fixture.py`.
The article, recipe and illustration use the repository's MIT license. No
private content, third-party article or AI/network service is needed.

After building/installing a qualified Renderflow binary and preparing the local
providers, from this directory:

```bash
renderflow graph plan --config "renderflow.yaml"
renderflow build --config "renderflow.yaml"
```

The same selection is available as `--profile "editorial"`. Inspect the run
manifest before assuming that every requested target was published.

| Requested output | Path under `output/article/` |
| --- | --- |
| Responsive article | `web/article.html` |
| Print article | `print/article.pdf` |
| Exact original Markdown | `source/article.markdown` |
| Optional editable document | `editorial/article.docx` |
| Optional reflow ebook | `ebook/article.epub` |

Pandoc **3.6.1 or newer** is required. PDF also requires Tectonic and its already
prepared local TeX cache. The renderer supplies `--only-cached`; a cold cache
fails instead of downloading support files. The shared tool registry reports
missing/incompatible providers; planning does not prove the cache is complete.

To select just web output, replace `targets` with:

```yaml
targets:
  exact:
    - id: article.web
      role: web/article
      format: html
      transform: builtin.markdown.html
      preset: editorial-v1
```

See [the editorial guide](../../docs/user-guide/editorial-articles.md) for the
content contract, fonts, format differences and deferred qualification.

**Checkpoint status:** these examples and regression cases are authored but
unrun. No rendered output or successful provider/visual check is claimed.
