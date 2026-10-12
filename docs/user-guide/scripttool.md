# Restricted screenplay imports with scripttool

The #424 adapter is an **authored, unqualified draft**. No tests, build, schema
validation, native conversion, or CI have run for this checkpoint. The examples
below describe the interface; they are unrun. See the
[provider evaluation](../work/424/PROVIDER_EVALUATION.md) for upstream source,
version, installation, and license evidence.

## Directional support

| Direction | Checkpoint behavior |
| --- | --- |
| FDX → Fountain | Explicit opt-in; independently compared body subset. |
| OSF → Fountain | Explicit opt-in; explicit styles and the same body subset. |
| Fade In → Fountain | Explicit opt-in; one bounded `document.xml` ZIP member containing the accepted OSF profile. |
| Fountain → FDX | Rejected: upstream writes an incomplete native root envelope; application acceptance is unqualified. |
| Fountain → OSF | Rejected: upstream writes paragraph types as bookmarks instead of semantic styles. |
| Fountain → Fade In | Rejected: wrapping the same OSF output does not repair those styles. |
| Fountain → scripttool JSON | Separate inspection/derivative candidate, rejected for execution; no independently qualified mapping or loss contract. |

There is no canonical screenplay JSON conversion edge, automatic reverse path,
PDF path, fallback provider, automatic installer, or bundled executable.

## Convert one local screenplay

Supply an already acquired executable at an absolute regular-file path, its
verified lowercase SHA-256, and explicit acceptance of the unqualified profile.
Replace the placeholder digest below with the digest of that exact executable.
The source is read only. The output directory must be new and its parent must
already exist.

```bash
renderflow screenplay convert \
  --input "scene.fdx" \
  --input-format "fdx" \
  --output-format "fountain" \
  --output-dir "scene-import" \
  --scripttool-path "/absolute/path/to/scripttool" \
  --scripttool-sha256 "<verified-lowercase-sha256>" \
  --expected-version "0.0.10" \
  --timeout-seconds 30 \
  --allow-unqualified
```

Choose `osf` or `fadein` explicitly for those sources. Optional `--source-sha256`
also pins the input. Omitting `--allow-unqualified` refuses conversion. The
provider must print its executable basename followed by `0.0.10`; that version
is shared by distinct upstream commits, so a matching response does not attest
to source or build provenance.

Accepted output is a new bundle:

- `screenplay.fountain`: the exact independently parsed provider output.
- `provider-evidence.json`: source/output and executable hashes, exact version
  response, argv and its digest, semantic comparison results, and declared losses.
- `bundle.json`: written last, binds the two member hashes and sizes and keeps
  `runtime_qualified` false. Its receipt is also written to stdout.

Provider or semantic validation failure publishes no bundle. A publication I/O
failure or cancellation can leave a partial directory without `bundle.json`;
that directory is not a completed result. A stdout failure after publication
can leave an already completed bundle. Existing paths are never overwritten.
Staging requires temporary storage; this is not an in-place conversion.

## Accepted content and limits

The independent readers accept only body paragraphs: scene heading, action,
character, dialogue, parenthetical, and transition. Text is a single nonempty
line without controls or surrounding whitespace. The emitted Fountain must
start with a conventional uppercase scene heading. Each character block has
one dialogue line and optionally one preceding parenthetical. Unsupported or
ambiguous Fountain syntax, title pages, notes, styling, scene numbers, dual
dialogue, revision/layout metadata, unknown attributes/elements, and multiline
paragraphs are refused, including otherwise valid native documents outside
this deliberately small profile.

FDX requires `FinalDraft` with exactly `DocumentType="Script"`, `Template="No"`,
and `Version="1"`. OSF requires `document` with exactly
`type="Open Screenplay Format document"` and `version="20"`. Fade In accepts
only one regular, unencrypted root `document.xml` member, stored or deflated,
containing that OSF profile. Other versions and additional archive data are
refused. XML namespaces, DTDs, entity declarations, comments, processing
instructions other than the XML declaration, and CDATA are outside the profile.

Input/output and uncompressed archive content are bounded at 16 MiB, element
count at 100,000, paragraph text at 64 KiB, XML at 500,000 nodes and depth eight.
Executable hashing is bounded at 256 MiB. The version probe has a five-second
timeout; conversion defaults to 30 seconds and allows 1–300 seconds. Both
stdout and stderr are limited to 64 KiB; truncation fails. File reads check
cancellation and a 120-second budget between reads; blocked filesystem reads
are not interruptible by that budget.

Comparison preserves ordered kinds, text, and character ownership. Exactly one
outer parenthetical pair is decoded as syntax; OSF also permits unwrapped
parentheticals. No case folding or arbitrary text normalization is accepted.
Blank-line layout and whitespace-only FDX `Empty` paragraphs are omitted.
Evidence discloses non-preservation of native IDs, layout/pagination, and the
output's distinct source identity. This is semantic-subset evidence, never a
universal lossless round-trip or native-application acceptance claim.

The executable receives a private staged source and fresh output path through
direct argv. It is rechecked along with source/staged bytes; stale, missing,
nonregular, oversized, malformed, or semantically changed output fails. The
process boundary is not an OS sandbox or build attestation. Network denial is
declared executor policy, not an isolation guarantee. Use stable local files
and an independently trusted executable.

## Graph and SDK execution

Set `execution.screenplay` explicitly in a v2 spec, including all five fields.
For example, save this as `scene.renderflow.yaml` after replacing the binary
path and digest:

```yaml
schema: "renderflow/v2"
sources:
  - id: "source.screenplay"
    path: "./scene.fdx"
    format: "fdx"
targets:
  exact:
    - id: "target.fountain"
      format: "fountain"
execution:
  screenplay:
    executable: "/absolute/path/to/scripttool"
    executable_sha256: "<verified-lowercase-sha256>"
    expected_version: "0.0.10"
    timeout_seconds: 30
    allow_unqualified: true
output:
  bundle_root: "./output"
  naming_template: "{source}.{ext}"
  collision: "error"
```

Declare the source format explicitly as `fdx`, `osf`, or `fadein`, and request
only `fountain`. The policy supports one original artifact. Collections,
other native policies, custom transforms, post-conversion hygiene (including
inherited profile hygiene), same-format copies, and transitive routes are
refused. Generic XML/ZIP detection is allowed only as the expected wrapper
of that explicitly declared source; conversion still validates its contents.

Planning performs a configured-executable version probe. Missing or incompatible
providers prune their routes with a diagnostic; no PATH fallback is used for
these edges. Planning does not attest to the executable digest: execution checks
the binary pin before its own version probe and conversion. The graph includes
separate `screenplay.import.fdx.fountain`, `screenplay.import.osf.fountain`, and
`screenplay.import.fadein.fountain` capabilities only under this policy.

The graph binds provider evidence to source/output artifact digests, carries it
in the manifest, and applies the strict Fountain validator. Native routes use
neither reusable generic cache nor checkpoints; no toolchain attestation is
claimed. Canonical `screenplay inspect` remains a separate JSON-only operation.

Wire contracts: [provider evidence](../../schemas/renderflow-scripttool-evidence-v1.schema.json)
and [standalone bundle](../../schemas/renderflow-screenplay-bundle-v1.schema.json).
These schemas describe shape, not proof that validation occurred. Resume the
deferred checks from [the handoff](../work/424/HANDOFF.md).
