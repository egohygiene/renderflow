# `renderflow publication`

Inspect pinned provider rules and preflight local publication candidates. These
commands never authenticate, upload, allocate an ISBN, order a proof, or publish.

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
