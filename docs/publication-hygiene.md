# Publication hygiene

Renderflow can apply a named publication-hygiene policy after a derivative is generated and before it is materialized into a release bundle. The stage is non-destructive: canonical sources remain immutable, and sanitation creates a new content-addressed artifact whose evidence points to the unsanitized candidate.

Hygiene is opt-in. A policy can be selected for a complete execution or attached to a derivative profile:

```yaml
schema: renderflow/v2
sources:
  - id: source.comic
    path: comic.md
profiles:
  publication.comic:
    hygiene_policy: public-comic
    targets:
      - role: web
        format: html
      - role: print
        format: pdf
hygiene:
  public-comic:
    audience: commercial
    metadata:
      allow:
        - dc.title
        - dc.creator
      deny:
        - exif
        - xmp
        - iptc
        - geolocation
        - filesystem.*
    secrets:
      enabled: true
      block: true
    protected_references:
      terms:
        - Example Game Studio
        - Example Post-Apocalypse Franchise
      block: true
    rights:
      required: true
      license: All rights reserved
      rights_holder: Example Publisher LLC
      approval_reference: rights-review-2026-09
      reviewed: true
targets:
  profiles:
    - publication.comic
```

Set `execution.hygiene_policy` to override profile policy selection for the whole requested bundle. If several selected profiles name different policies, Renderflow requires an explicit execution-level choice.

## Four separate controls

| Control | Purpose | Decision model |
| --- | --- | --- |
| Metadata sanitization | Removes configured record metadata and supported embedded JPEG/PNG metadata classes while preserving allowlisted publication fields. | Deterministic transform |
| Secret scanning | Detects likely private keys, tokens, authorization values, credential assignments, and configured secret markers. Findings never reproduce the detected value. | Deterministic publication gate |
| Content redaction | Delegates configured PII or content classes to a replaceable provider. | Deterministic or probabilistic transform |
| Rights review | Requires explicit license and approval evidence for public/commercial publication. | Human policy gate, not a legal conclusion |

Protected-reference scanning is an additional deterministic gate for configured brands, franchises, companies, creators, or works. It is useful for catching a named influence that leaked from an internal prompt into generated text or metadata. A match means “remove or review this configured reference”; it does not mean Renderflow has detected copyright infringement or made a legal determination.

## Metadata behavior

`metadata.allow` and `metadata.deny` accept exact field names and namespace wildcards such as `filesystem.*`. With `allowlist_only: true`, all artifact-record metadata outside the allowlist is removed. Embedded metadata sanitation currently supports these field classes where structurally safe:

- JPEG EXIF, XMP, IPTC, and comment segments;
- PNG EXIF and textual chunks.

Other formats remain provider-extensible. Unsupported embedded formats are preserved rather than rewritten unsafely.

Evidence records only changed field classes such as `exif` or `geolocation`; removed values are never copied into the run manifest.

## Blocking and candidate preservation

Secret findings, configured protected references, unavailable redaction providers, and incomplete public/commercial rights evidence can block materialization. The candidate remains in Renderflow's local content-addressed artifact store and appears in the run manifest with an `artifact-store:` locator and `blocked` hygiene evidence. It is not copied into the publication bundle.

Probabilistic redaction always produces `review_required` status for the resulting candidate. Configuring or reviewing a provider is not treated as approval of a particular probabilistically redacted output.

## Provider contract

SDK integrations implement `ContentRedactionProvider` and declare a stable provider ID, version, and determinism class. Providers receive bytes plus configured content classes and return new bytes, changed classes, and safe findings. They must not mutate source files or include sensitive values in diagnostics.

Hygiene evidence uses [`renderflow.hygiene/v1`](../schemas/renderflow-hygiene-v1.schema.json) and is embedded in terminal artifact evidence. A `publication.hygiene` step records the policy digest, provider, input candidate, sanitized output, duration, and fidelity.
