# `renderflow publication`

Inspect pinned provider rules and preflight local publication candidates. These
commands never authenticate, upload, allocate an ISBN, order a proof, or publish.

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
