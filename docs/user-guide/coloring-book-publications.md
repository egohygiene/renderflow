# Coloring-book publication profile

The bundled `coloring-book` profile and `renderflow.coloring-book/v1` source
contract provide a deterministic, provider-neutral line-art publication
foundation. Renderflow validates and packages reviewed inputs; it does not
write publication text, generate illustrations, make rights determinations, or
publish a book.

## Authoring contract

Start from `tests/fixtures/coloring-book/book.yaml`, a small CC0 synthetic
geometry fixture. The contract fixes audience and complexity, trim geometry,
margins, bleed and safe area, minimum line weight, contrast and resolution,
pagination, binding, and stable page order.

Every non-blank page records:

- a reviewed source-text path, SHA-256 digest, and approval reference;
- a reviewed illustration or generated candidate with an exact byte digest;
- license, rights holder, source, rights review, and approval evidence;
- measured DPI, contrast and line weight plus trim/bleed/safe-area evidence;
- alt text, caption, and source links; and
- stable continuity references for character or style constraints.

Intentional blank pages carry an explicit reason and no hidden content.
Repeated artwork is rejected unless that page explicitly marks the reuse as
intentional.

## Candidate review and optional generators

Canonical source refers to approved page bytes, not a particular vendor.
Generator provenance is a replaceable evidence block containing candidate,
provider, locality, model, skill version, settings digest, and prompt digest.
This makes local/open models first-class and lets providers change without
changing reviewed source text or the publication graph.

Use the provider-neutral AI skill runtime described in [AI](ai.md) to create an
optional line-art candidate. Keep its approval state as `candidate` while it is
being reviewed. Promotion requires a human approval reference and an
`approved_sha256` matching the exact selected bytes. Privacy, copyright,
protected-reference, and prompt-hygiene reviews are mandatory for generated
candidates.

Remote provenance additionally requires an explicit preflight opt-in:

```bash
renderflow publication coloring-book-preflight \
  --contract "book.yaml" \
  --allow-remote \
  --format json \
  --output "validation.json"
```

This does not contact a provider. Without the flag, remote-generated candidates
block release. Preflight itself never invokes AI, and its report always records
`candidate_output_authoritative: false`.

## Local preflight

Run the normal, offline path before planning derivatives:

```bash
renderflow publication coloring-book-preflight \
  --contract "book.yaml" \
  --format json \
  --output "validation.json"
```

The report includes normalized contract and validator-settings digests, exact
artwork evidence, provider/model identity where applicable, approval and rights
state, deterministic policy evidence, and sorted findings. Identical approved
inputs produce byte-stable JSON output.

## Preview and build

The profile asks the standard graph for print book/proof PDFs and optional
raster, vector, and accessible-web derivatives. Adapter availability remains
visible; it is never replaced by an implicit network service.

```bash
renderflow graph plan --config "renderflow.yaml" --profile "coloring-book"
renderflow build --config "renderflow.yaml" --profile "coloring-book" --dry-run
renderflow build --config "renderflow.yaml" --profile "coloring-book"
```

The bundled profile denies network and AI use and requires validation. Content
repositories remain responsible for mapping their reviewed source and approved
page assets into the normal Renderflow v2 source/transform graph.

## Print proof and release

Inspect the `print/proof` artifact before approving a release. Compare its page
order and blank intent with the coloring-book report, verify trim and bleed
against the selected printer template, and retain the run manifest,
publication metadata, validation report, and checksums. Public or commercial
release must not proceed while any rights field is missing, ambiguous, or
unreviewed.

Renderflow does not upload files, allocate identifiers, order proofs,
auto-publish, or assert therapeutic outcomes. Provider-specific checks remain
separate preflight packs such as the Lulu integration.

## Rollback

Retain the canonical contract, approved inputs, validation report, run manifest,
and checksums for each release candidate. To roll back, restore that source
revision and rebuild with the recorded toolchain. Never relabel a rejected
candidate or failed run as approved; create a new approval bound to the exact
replacement digest.
