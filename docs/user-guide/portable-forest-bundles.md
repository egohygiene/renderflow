# Move a forest with its retained evidence

`bundle export` builds an internal evidence handoff from an exact native run
receipt, its published files and an explicitly selected artifact store. It keeps
the original receipt bytes, loss/review metadata and run state unchanged, while
copying retained payloads into a content-addressed directory. `verify-portable`
can then observe that directory without the original bundle or store.

This source is authored and **unqualified**. No packaging command, built-in
conversion, CLI/SDK execution or native/player/browser qualification has run for
this checkpoint. #430 and the existing baseline/release owners retain acceptance.

## Export and verify

First select the actual saved `renderflow.run/v1` receipt and its exact SHA-256:

```bash
renderflow bundle export \
  --manifest "release/run-manifest.json" \
  --manifest-sha256 "<exact selected native receipt SHA-256>" \
  --bundle-root "release" \
  --artifact-store-root "/explicit/run-state/artifacts" \
  --output-dir "handoff"
```

The receipt filename/store path are illustrative; use the actual run locations.
The directory must not exist, and its parent must exist. The operation reads
only the supplied bundle/store roots and known native locators; it never guesses
host paths from `output_dir` or arbitrary metadata strings. It runs no providers
or models and does not upload, submit or publish remotely.

The JSON result supplies `index_sha256` for the completed `bundle.json`. Pin that
digest when moving the directory:

```bash
renderflow bundle verify-portable \
  --bundle-root "moved-handoff" \
  --index-sha256 "<exact exported index SHA-256>"
```

No command example has been executed here. A digest binds selected bytes; it is
not a publisher signature. The command reports original run/validation/fidelity
states and observed native publication mappings, without inferring human review.

## What travels

The directory contains:

- `native/run.json`: the exact original receipt, with its original byte digest;
- `objects/sha256/<prefix>/<digest>`: verified copies of included payloads;
- `bundle.json`: the versioned `renderflow.portable-forest/v1` index, written
  last, binding every native inventory row to a copied object or external source.

Every non-source inventory payload is included, retaining canonical companions,
intermediates, provider results and any recorded unpublished candidates. An
unpublished candidate keeps that state; inclusion never turns it into a
publication. Known v1 body/recovery/subtitle/speech/lyrics references and all
declared parent IDs must resolve to inventory records. A missing companion is
an error, not a guessed successful export.

Required canonical companions are included even if their native lifecycle is
`source`, as can happen when a timed-transcript JSON is itself the input.
Other original sources remain explicitly external by default. Select
`--include-sources` to include those remaining originals as well. The flag does
not remove an explicitly published unchanged source or required canonical
context. Native records and metadata are not rewritten to hide this distinction.

An artifact-store locator must match its exact digest-derived object path.
Source records with virtual/directory locators or missing payloads need their
own packaging contract; this operation does not crawl them. Unknown extension
metadata remains opaque. Required references in an unsupported known schema,
contradictory inventory IDs, missing lineage records or missing files fail
instead of being repaired from untrusted paths or an assumed old cache.

The handoff retains original native metadata, which may include local paths,
transcripts, source claims and production evidence. It is not a sanitized public
release package. Choose what to share and use the normal content/privacy/review
process before external publication. No remote approval flow is added here.

## Native locators and portable mappings

The original `bundle:` locators still describe the original run's destinations.
The portable index supplies the actual copied payload paths. Two publications
with equal bytes retain separate native target/destination records while sharing
one copied object. Read both identities; do not collapse native publications by
digest. The `objects` layout also preserves retained artifacts' canonical store
paths without changing their embedded records.

Use `bundle verify` with an original native bundle. Use `verify-portable` with
the copied handoff. The latter reads the pinned index, exact native receipt and
each included object; it never falls back to the old bundle/store. Consuming
only indexed paths is distinct from declaring every other file in a directory
safe or part of the handoff.

The public SDK uses `evidence::portable::export` and `verify`. A successfully
observed `VerifiedPortableBundle` exposes its pinned receipt, index and
`publications()` views. Each view pairs the original borrowed `PublishedTarget`
with its copied relative payload path and byte observation. These are scoped
observations, not filesystem capabilities or approval to hand content to a new
provider. Recheck bytes when they are subsequently used.

## Preservation and bounds

Source/store files are streamed into new staging copies; originals are never
hard-linked into the output. Identical contents deduplicate only after each
declared origin is observed. Publication reserves a new directory and hard-links
only the owned staging copies into it, then re-observes destination data/receipt
before placing the completed index last. This requires same-filesystem hard-link
support; no platform support is claimed before qualification. An I/O failure can
leave partial files without a completed index, and existing destinations are
never overwritten or automatically deleted.

The shared [file verification limits](bundle-evidence.md) apply. Native/index
JSON uses the receipt byte limit; payloads use the file/total limits. Inventory
defaults to 16,384 rows, capped at 100,000; `--maximum-records` sets the bound.
All three commands share file/total/publication-count/timeout flags. Source
copying, staging verification and destination verification are separate bounded
phases. Totals count logical record observations, including duplicate origins;
object deduplication saves output space, not unrecorded verification work.
Limits are cooperative and do not bound blocked kernel syscalls or hard RSS.

Observed path/metadata checks detect links and persistent changes, not a hostile
concurrent filesystem adversary. Unknown fields in native extension metadata,
external provenance, source authenticity, graph correctness, model/tool closure,
format fidelity, rendering quality and review approval remain outside this
transport proof. A native receipt with incomplete prior-run inventory cannot be
reconstructed from a content hash alone. Preserve a complete run/store for export.

## Remaining evidence

Eight regression cases are authored and unrun, covering moved handoffs without
original roots; required canonical sources; optional originals and source-copy
isolation; duplicate destinations/object deduplication; missing companions/store;
receipt/payload/index/path tampering; cancellation/count/overwrite boundaries;
and an ordinary built-in Fountain-to-FDX run with a retained unpublished canonical.

No tests, compiler/build/Cargo resolution, lint/format/schema/docs validators,
product CLI/provider/native/media/model execution or CI ran. No native run or
minimal Flow wire model was changed. These additive transport interfaces do not
establish a released Flow adapter; #430/#421 and #458/#419 remain open.
