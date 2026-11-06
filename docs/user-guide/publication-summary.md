# Source-grounded publication synopsis

This optional standalone CLI/library path creates a review candidate from
selected authored-script, transcript or text-companion bytes. It reuses the
existing AI catalog, skill runtime, adapters, hygiene and artifact store. It
does not inspect comic images, recognize lyrics, rewrite publication metadata
or publish a work. All code and commands below are authored and unrun; compiler,
test, schema, provider, model and quality qualification remain deferred.

## Inspect source coverage with AI disabled

The [original comic text example](../../examples/publication-summary/README.md)
contains a digest-pinned recipe with AI permission off and a disabled profile.
From the repository root, a later invocation could inspect it as follows:

```bash
renderflow ai summary plan \
  --recipe examples/publication-summary/summary.json \
  --profile examples/publication-summary/runtime.disabled.json
```

Plan reads only local declarations and sources; it makes no provider connection,
credential read, installation or model download. It reports selected and omitted
byte counts, source hashes/artifact IDs, evidence IDs and profile status without
printing source text. It uses temporary artifact storage, removed afterward;
those plan locators are not a persisted generation candidate. Each recipe source
is confined to a relative path below the recipe directory. Ranges are sorted,
nonoverlapping UTF-8 byte offsets. Changing a source requires reviewing the new
digest/ranges. The selected-text limit is 32 KiB, not a promise to fit every
model's token context; serialized input and declared profile limits also apply.

## Select a prepared profile and explicit source permission

Preparation is a separate operator action. The bundled model catalog is
unverified. The local example deliberately lacks preparation evidence and is
not runnable as supplied. Record the exact prepared runtime/model/weights and
supported byte/token ceilings in a reviewed catalog; see the
[runtime mapping](../work/456/RUNTIME_PROFILE.md). A profile's `declared_ready`
status checks declarations. It does not attest loaded weights, block server
egress or measure hardware. A literal loopback endpoint alone does not prove an
offline service: disable that service's own remote/cloud behavior separately.

Read each source and explicitly set `approved_for_ai` with an approval reference
in a reviewed recipe copy. The synthetic example's CC0 dedication does not
automatically enable AI use. Remote privacy permission is separate; a remote
profile also needs explicit remote selection, approved sources and compatible
cost policy. Missing local infrastructure never triggers fallback. This sprint
authorizes no paid request. Profile inspection resolves no credential; execution
uses only an explicitly declared credential environment handle.

When preparation and permissions are complete, the later local invocation is:

```bash
renderflow ai summary generate \
  --recipe examples/publication-summary/summary.reviewed.json \
  --profile /path/to/reviewed-local-profile.json \
  --catalog /path/to/reviewed-catalog.json \
  --store /path/to/owned-summary-store \
  --output-dir /path/to/existing-parent/candidate-001
```

The parent must exist; the output directory must be new. Generation attempts
one selected inference, with no retry or alternate model. The native transport
has bounded response/time policy and cooperative cancellation; this is not a
hard-kill process supervisor. Disabled generation writes an outcome without a
candidate. Unavailable/refused/failed/invalid results return a nonzero CLI result
after a successfully written outcome receipt. Input capture, cancellation or
I/O failures may prevent any completed receipt. Unknown usage/cost stays unknown.

Only `awaiting_review` has `candidate.json`. This private review bundle includes
selected source quotes, declared permissions, exact source ranges and the native
processing identities. Keep it and the store under your control. Evidence checks
join IDs, hashes and ranges; they do not establish factual entailment, author
identity, visual understanding or trustworthy model instructions. Source text
resembling instructions remains data, but model prompt-injection resistance is
not guaranteed. Model output has no tools or approval authority.

## Review the exact candidate

Read every synopsis statement against its quoted source slice, check omissions
and uncertainty, and follow the example's
[evaluation criteria](../../examples/publication-summary/EVALUATION.md). Keep
the original candidate unchanged. Record its `candidate_id` and the SHA-256 of
the exact `candidate.json` bytes, including whitespace. A later shell command for
that file is `sha256sum /path/to/candidate-001/candidate.json`.

Create an operator decision JSON. These placeholders are deliberately invalid;
replace them with the actual subject and your own review reference:

```json
{
  "schema_version": "renderflow.summary-review-decision/v1",
  "candidate_id": "REPLACE_WITH_ACTUAL_CANDIDATE_ID",
  "candidate_sha256": "REPLACE_WITH_EXACT_FILE_SHA256",
  "action": "accept",
  "reviewer_reference": "REPLACE_WITH_YOUR_REVIEW_REFERENCE",
  "semantic_review_completed": true,
  "revised_body": null
}
```

Use `accept` only after semantic review. `revise` requires the same review flag
and a full `revised_body` conforming to the synopsis body schema; every revised
statement must still cite original evidence IDs. The body's `review_required`
remains true because it describes the AI candidate contract; acceptance is
recorded separately. `reject` allows no revised body and permits no export.
Decisions bind both the original candidate ID and file hash. Changes to either
require a fresh decision. References are operator declarations, not authenticated
human identities or cryptographic signatures; this is a local review workflow.

```bash
renderflow ai summary review \
  --candidate /path/to/candidate-001/candidate.json \
  --decision /path/to/operator-decision.json \
  --store /path/to/owned-summary-store \
  --output-dir /path/to/existing-parent/review-001
```

Review reimports the exact candidate bytes into the owned store and creates a
separate review artifact. It preserves the candidate's state. The new review
records acceptance/revision/rejection, accepted-content hash, scope and explicit
`publication_approved: false`. Its `review.json` and completion receipt do not
approve media, rights, provider use or publication. Review requires no model.

## Explicit local export

Compute the exact `review.json` file SHA-256, then supply it explicitly:

```bash
renderflow ai summary export \
  --review /path/to/review-001/review.json \
  --review-sha256 REPLACE_WITH_EXACT_REVIEW_FILE_SHA256 \
  --store /path/to/owned-summary-store \
  --output-dir /path/to/existing-parent/accepted-synopsis-001
```

Export observes the exact original candidate through confined, bounded store
reads and rechecks the review/content joins. Rejected, unreviewed, altered or
missing subjects cannot export. The new directory contains `summary.txt`,
`summary.json`, a copy of `review.json` and an `export-receipt.json` written last
after observing each payload's bytes. Plain text retains only the accepted
synopsis; citations, uncertainty, omissions, selected/unselected coverage and
processing identity remain in JSON. Raw source quotes are omitted from the
export. Accepted prose and metadata can still be private. No canonical document
or publication metadata is replaced, and nothing is uploaded.

Existing output directories are refused. An interrupted or failed write can
leave a partial directory; absence of the final receipt means the bundle is not
complete. Select a new destination for another attempt. Moving/copying a review
file alone does not make its store-relative candidate subject portable. The
operator-supplied file hash identifies bytes, not an authenticated reviewer.

## SDK and remaining qualification

`renderflow::publication::summary` owns capture/plan/generation. Its `review`
module owns decisions, bounded subject observation and explicit export. The CLI
is a thin adapter. Optional observer delivery excludes prompts, source text,
paths, endpoints and credentials; delivery failures are separate from candidate
identity. The local review bundle itself intentionally contains private text.

This is a Renderflow-owned proposed mapping to Flow #80. Shared-profile
ratification, released-consumer interoperability, full mock conformance and
measured local model/hardware quality remain open under #456. The included
evaluation template has no results. Deterministic publishing stays independent.
