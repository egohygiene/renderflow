# Local names extractor example

This original, repository-owned fixture demonstrates explicitly configured plugins
using the existing artifact forest. Code and tests are authored and **unrun** under
[#466](https://github.com/egohygiene/renderflow/issues/466); no working-build or
release claim is made. It uses Python 3.9+ standard library only.

Once the implementation is built and execution qualification resumes, run from
the repository root:

```bash
renderflow plugin list --transforms "examples/plugins/names/transforms.yaml"
renderflow build --config "examples/plugins/names/renderflow.yaml"
```

The intended outputs under `examples/plugins/names/output/` are:

- `source.notes/names.json`: exact configured names, counts, occurrence locations
  and the original text's SHA-256. Both Ada and Lin should have three matches.
- `source.notes/counts.csv`: name, count, semicolon-separated pages and source digest.
- The normal native run/artifact manifests and retained source evidence.

Keep the JSON for full locations. CSV drops individual offsets and line/column
details. Formula-like name prefixes are protected with an apostrophe in CSV; the
JSON keeps exact strings. This is not `.xlsx` output. The existing generic CSV
validator may refuse valid quoted commas; the minimal fixture avoids that case.

Edit the `names` array in `transforms.yaml`. Matches are literal, case-sensitive,
bounded by Unicode alphanumeric/underscore word boundaries, with no normalization,
alias merging or inferred names. Each name is counted independently. Locations
use zero-based Unicode code-point offsets and one-based page/line/column numbers.
Only explicit form-feeds create page boundaries; page numbers must not be mistaken
for original PDF page evidence when text was flattened without a retained map.
An exceeded match limit refuses the complete report instead of silently truncating.

This example never loads private media, invokes OCR/AI, accesses the network or
changes its source. `names.py` is ordinary trusted local code. To implement a
different plugin, copy the command contract and replace its parsing/export logic;
see the [author guide](../../../docs/plugin-guide/command-plugins.md).

Deferred focused checks:

```bash
cargo test --package renderflow --locked --test command_plugins
python3 -m unittest discover --start-directory "examples/plugins/names" --pattern "test_names.py"
```

The Rust runtime cases require `python3`; a missing interpreter is a failure, not
a passing native test. No command above was executed for this source checkpoint.
