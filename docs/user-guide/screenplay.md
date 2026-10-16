# Screenplay artifacts

Issue #423 introduces the provider-neutral `renderflow.screenplay/v1` model,
typed screenplay format identifiers, and canonical JSON inspection. This
checkpoint is authored and unqualified: tests, builds, schema validation,
formatting/linting, native runs, and CI remain deferred.

## Inspect a canonical screenplay

```bash
renderflow screenplay inspect --input "script.screenplay.json"
renderflow screenplay inspect --input "script.screenplay.json" --format "text"
```

The command accepts a local regular JSON file up to 16 MiB, validates its
canonical screenplay structure, and writes a report to stdout. JSON is the
default report format. No external executable is required. It writes no
artifact files and performs no conversion. The examples are unrun.

Inspection describes ordered content, element kinds, extension data, declared
source identity, and directional loss policy. The inspected JSON bytes receive
their own digest. Any source digest inside the document is a declaration; the
source file is not reopened or verified by inspection.

This command refuses native Fountain, FDX, Fade In, OSF and Trelby files. The separate
[scripttool conversion command](scripttool.md) authors three explicitly pinned,
unqualified imports into Fountain under #424. Canonical inspection neither
selects nor inventories those provider routes. A generic `.json` path is accepted
only when its contents satisfy the canonical screenplay contract.

## Format and capability status

| Graph identifier | Filename | Authored behavior |
| --- | --- | --- |
| `fountain` | `.fountain` | Recognition and strict body-subset validation; output of three opt-in native imports. |
| `fdx` | `.fdx` | Recognition for Final Draft XML; strict body-subset import to Fountain with explicit scripttool policy. |
| `fadein` | `.fadein` | Recognition for the ZIP-based format; strict single-document body-subset import to Fountain. |
| `osf` | `.osf` | Recognition for Open Screenplay Format XML; strict body-subset import to Fountain. |
| `trelby` | `.trelby` | Recognition for native version 3; strict minimal body export from original Fountain under the separate opt-in policy below. |
| `screenplay_json` | `.screenplay.json` | Canonical model parsing, structural validation, and inspection. |

These identifiers alone do not create conversion graph edges. In particular,
Fountain does not gain an executable converter from an installed Pandoc.
Screenplay formats are not mapped to the existing Pandoc input path.

Filename recognition is a low-confidence hint. XML and ZIP bytes alone cannot
identify a screenplay. General intake may report native format/container
conflicts, including `.fadein` versus ZIP. An explicit screenplay graph policy
permits only the expected XML/ZIP wrapper conflict for its declared native
source, then independently validates the native envelope during conversion.
The shared JSON intake recognizes the canonical
schema marker separately from generic JSON; that marker is not validation.

Default artifact validation uses the bounded screenplay validator for canonical
JSON and the strict Fountain body profile. General FDX/OSF/Fade In artifact
validation remains unavailable; their provider-local readers verify the
restricted conversion inputs. Nonempty UTF-8 text or a ZIP signature is
insufficient screenplay evidence.

## Export an original Fountain body

The #430 checkpoint authors a separate built-in policy for the missing original
Fountain export direction. Save this example beside `original.fountain` and use
the existing `renderflow build --config "screenplay.yaml"` interface:

```yaml
schema: renderflow/v2
sources:
  - id: original
    path: original.fountain
    format: fountain
targets:
  exact:
    - format: screenplay_json
    - format: fdx
    - format: fadein
    - format: osf
    - format: trelby
execution:
  screenplay_body:
    allow_unqualified: true
output:
  bundle_root: screenplay-export
```

This example and its regression coverage are authored, not executed. No external
converter is required. Each direct edge reads the immutable original Fountain;
it does not run Scripttool in reverse or disguise structured Fountain as
unstructured recovery input. Native outputs retain a canonical artifact reference
and hash, source/output hashes, review state and explicit qualification limits.
Publication rechecks the original, canonical payload and deterministic export
even when generic artifact validation is disabled. Cache reuse is disabled.

Canonical bytes are retained in the local `.renderflow/artifacts` store and
listed as an intermediate when not selected. A native-only output bundle does
not copy that companion automatically. Select `screenplay_json` as in the
example when taking the canonical companion with the exported files, and retain
the run evidence and original source; otherwise preserve the local artifact
store needed to resolve its canonical reference.

The strict profile supports scene headings, action, character cues, dialogue,
parentheticals and transitions. It rejects title pages, dual dialogue, unsupported
elements or metadata and text the target cannot represent without changing its
semantics. Conversion does not approve the script. PDF, native imports and
PDF/text recovery keep their separate policies; use separate requests for them.

Trelby is the screenplay editor's native `.trelby` artifact, distinct from
Fountain. The independent codec implements only a minimal version 3 body
envelope, based on the [published format](https://github.com/trelby/trelby/blob/86079b162e6514754ce0a2f3ea323eaca81146e7/trelby/fileformat.txt).
General GUI-saved configuration/wrapping/title pages are outside that bounded
reader. Trelby opening, Unicode display/editing, pagination and native layout have
not been observed; exports declare `runtime_qualified: false`. The
[design and qualification checkpoint](../work/430/ORIGINAL_EXPORTS.md) records
the exact scope and upstream source pin.

## Canonical structure

The schema is
[`renderflow-screenplay-v1.schema.json`](../../schemas/renderflow-screenplay-v1.schema.json).
The Rust API is exposed through `renderflow::screenplay`.

- Ordered title-page fields retain arbitrary field names and repeated entries.
- Ordered elements have stable IDs and typed semantic kinds: scene headings,
  action, character cues, dialogue, parentheticals, transitions, lyrics,
  centered text, sections, synopses, and notes.
- Dialogue and parentheticals identify their character cue. Explicit
  dual-dialogue groups link adjacent left-then-right character blocks, each
  containing dialogue. A character block can belong to one such group.
- Source identity, declared losses, and explicit extension bags accompany the
  semantic structure. Unknown/source-specific values belong in extension bags;
  unknown canonical fields are rejected.
- Validation checks schema identity, limits, IDs, references, ordering, and
  supported structure. Parsing rejects duplicate object keys, including inside
  retained extension values.

Stable IDs are supplied by the producer and preserved through canonical
serialization; they do not promise identity across independently imported or
edited source documents. Extension retention keeps the values available for
future adapters without claiming that Renderflow interprets them.

Numeric extension values must be exact JSON integer tokens within the supported
signed 64-bit or unsigned 64-bit range. Store decimals, exponent notation, or
larger numeric source tokens as strings when preserving their exact text is
necessary. Unsupported numeric values are rejected rather than silently rounded.

## Loss and round-trip limits

The model is a conversion boundary. It does not establish that every native
format represents each semantic feature identically. Directional assessments
keep general canonical import/export policy separate from configured native
body converters. No general canonical-input-to-native graph route is implemented.
The original-Fountain policy above uses only its bounded parsed body. The
[recovery-only exporters](../screenplay-recovery.md) accept replayable recovery
candidates and retain their separate lossy, review-required evidence. A missing
writer must not become a lossless conversion claim.

Canonical parse/serialize operations preserve the supported semantic model.
They do not preserve original JSON whitespace or key order, and do not prove
byte-for-byte recovery of Fountain, XML, ZIP containers, layout, revision data,
fonts, or embedded resources. Source-specific values and loss evidence must be
carried forward or explicitly reported by later adapters. See
[format source records](../work/423/FORMAT_SOURCES.md) for reviewed upstream
envelopes and representational gaps.

## Fixtures and next steps

[`tests/fixtures/screenplay`](../../tests/fixtures/screenplay) contains tiny
original synthetic screenplay examples. Their narratives are independently
authored; upstream references supply format information, not script content.
The Fade In fixture is a deterministic authored ZIP envelope. Native
application acceptance and cross-format fidelity have not been tested.

The #424 [scripttool checkpoint](scripttool.md) adds restricted, unqualified
native imports and explicitly rejects reverse native writers. The #425
[PDF and analysis checkpoint](screenplay-pdf.md) authors pinned afterwriting
publication for a strict ASCII Fountain body and independent source reports.
The #426 [recovery checkpoint](../screenplay-recovery.md) authors explicitly
lossy PDF/text recovery and bounded native body exports. Full creative
toolchain qualification (#429) and integration proof (#430) depend on those
adapters and measured execution evidence. Resume from
[the #423 handoff](../work/423/HANDOFF.md).
