# Presentation examples

`synthetic.presentation.json` is an original fictional two-slide deck. Its zero
source identifiers explicitly identify a fictional source, not a verified Git
revision. The fixture contains rehearsal notes so default omission can be
reviewed. `renderflow-product.presentation.json` is a factual source-summary
draft pinned to the recorded README revision and observed bytes; it still needs
the maintainer's content review. Neither example claims publication approval.

When execution resumes:

```bash
renderflow presentation inspect --input "synthetic.presentation.json"
renderflow presentation plan --input "synthetic.presentation.json" --output-dir "shelf-source"
renderflow presentation build --input "synthetic.presentation.json" --output-dir "shelf-source"
```

The default target is a source ZIP, accompanied by a readable HTML transcript,
stylesheet, selected local media and completion manifest. No provider is needed
for this target. The output directory must not already exist.

For web and PDF, copy `provider.template.json` to a private configuration and
replace every placeholder with observed local pins. Select only targets you
intend to produce:

```bash
renderflow presentation fingerprint --installation-root "/absolute/prepared/project"
renderflow presentation plan --input "synthetic.presentation.json" --output-dir "shelf-release" --target source,web,pdf --provider "provider.local.json"
renderflow presentation build --input "synthetic.presentation.json" --output-dir "shelf-release" --target source,web,pdf --provider "provider.local.json" --allow-unqualified
```

No examples, provider calls, archive builds or rendered outputs have been run
for this source checkpoint. See the [guide](../../docs/user-guide/presentations.md)
for preparation, bounds, publication and rollback.
