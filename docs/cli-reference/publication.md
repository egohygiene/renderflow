# `renderflow publication`

Inspect pinned provider rules and preflight local publication candidates. These
commands never authenticate, upload, allocate an ISBN, order a proof, or publish.

## Coloring-book preflight

```bash
renderflow publication coloring-book-preflight \
  --contract "tests/fixtures/coloring-book/book.yaml" \
  --format json \
  --output "coloring-book-validation.json"
```

The command validates `renderflow.coloring-book/v1` locally and emits
`renderflow.coloring-book-validation/v1`. It hashes reviewed source, artwork,
and font references; checks pagination, geometry evidence, resolution, contrast,
line weight, accessibility, duplicate intent, rights, approvals, and generator
provenance; and exits nonzero when release is blocked.

Remote provider provenance is blocked unless `--allow-remote` is present. The
flag only records an explicit review decision: preflight never invokes a model
or contacts a provider. Local/open-model provenance needs no remote opt-in, and
all generated output remains a candidate until approval binds its exact digest.

## Magazine candidates

```bash
renderflow publication magazine-candidates \
  --config "examples/magazine/renderflow.yaml" \
  --asset-role "cover" \
  --output "cover-candidates.json"
```

This local-only default consumes the artwork role's validated `artifact_dna`
sidecar and emits a schema-bound, review-required asset-brief and metadata
candidate. Add `--ai` and an execution-ready model catalog to request an
additional candidate through the registered AI skill runtime. Local models are
preferred; remote selection also requires `--allow-remote`, and remote exposure
of privacy-reviewed input requires `--privacy-approved-for-remote`.

## Lulu rules

```bash
renderflow publication lulu rules --format yaml
renderflow publication lulu rules --format json --output lulu-rules.json
```

The output includes the exact rule-pack ID, observation date, thresholds, and
official source URLs bundled with this Renderflow build.

## Lulu preflight

```bash
renderflow publication lulu preflight \
  --request lulu-request.yaml \
  --format json \
  --output lulu-conformance.json \
  --epubcheck
```

`--request` accepts `renderflow.lulu-request/v1` YAML. `--epubcheck` asks the
optional local EPUBCheck executable for evidence. If it is missing or does not
identify a compatible v5 release, EPUB distribution remains `unknown`.

The command exits nonzero when any requested channel is `ineligible` or
`unknown`, after writing the report. Omit `--output` for standard output. Output
formats are `text`, `json`, and `yaml`.
