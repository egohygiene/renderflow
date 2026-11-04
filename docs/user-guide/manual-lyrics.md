# Manual timed lyrics

Prepare the lyrics and timing yourself in an SRT editor, then select the
`lyrics` profile. Renderflow converts that supplied content into supported
representations. This feature has no audio input, lyric detection, transcription,
automatic alignment, inferred word timing, or future-recognition stub.

The current implementation is source-authored and **unqualified**. Build, codec,
CLI/SDK, format/player and native release qualification remain deferred under
#469 and the existing baseline/release owners.

## Source contract

Accepted inputs are one explicit plain-profile SRT, WebVTT, or
`renderflow.timed-transcript/v1` JSON file. The existing timed-text parsers and
model own cue text, IDs, start/end milliseconds, ordering, overlap policy,
language and upstream loss/provenance. This profile adds source-bound lyric
classification and review/section metadata around that model; it does not add a
second timing engine. Merely discovering `.srt`, `.txt` or `.md` does not classify
a source as song lyrics.

The inherited bounded parser accepts UTF-8 plain-text cues, nondecreasing starts,
explicit end times and millisecond timestamps through 99:59:59.999. It preserves
overlapping cues under its declared overlap policy. Styling, WebVTT settings,
rich markup, entities, literal ampersands and style overrides are outside that
existing strict profile and are refused rather than stripped. Empty cue lines
and malformed timestamps also fail. No words or cue times are repaired silently.

Untimed text and start-only LRC are not inputs to this first manual timed
workflow: they do not provide all cue end times needed by the selected caption
outputs. Supplying a timed SRT or supported equivalent is the intended entry.
LRC and TTML below are export-only adapters. The canonical lyrics wrapper is an
output/inspection representation, not a newly claimed general editable importer.

## Configure and inspect

Use the [complete synthetic example](../../examples/lyrics/README.md). The
selectable [creative forest fixture](creative-forest.md#retain-a-portable-handoff)
also includes a provider-free `manual-lyrics` job, with optional retained-evidence
export and moved-handoff verification. Both examples remain authored/unqualified.
The ordinary `renderflow/v2` configuration selects:

```yaml
targets:
  profiles: [lyrics]
execution:
  lyrics:
    source_sha256: "<exact lowercase SHA-256 from inspection>"
    review_state: unreviewed
    language: en
    revision: manual-r1
```

The digest placeholder must be replaced. `language` and `revision` are optional
explicit declarations. Language cannot overwrite a conflicting declared source
language. `sections` optionally maps existing cue IDs to supplied section labels;
unknown IDs fail, and repeated lyrics retain separate cues. No section, chorus,
translation, author, chord or melody is inferred. Review state applies to the
exact digest and remains an author declaration, not signed approval.

```bash
renderflow lyrics inspect --input "manual.srt" --language "en" --revision "manual-r1"
renderflow graph plan --config "renderflow.yaml"
renderflow build --config "renderflow.yaml"
```

Inspection parses the manually supplied file, computes the binding and emits a
machine-readable eligibility report without invoking tools or creating outputs.
Use `--input-format` for a nonstandard suffix; supported values are `srt`, `vtt`
and `timed_transcript_json`. Add `--reviewed` only to declare your review of the
currently observed bytes. A successful parse never infers review or proves that
timestamps match a recording. No audio asset is bound or synchronization claimed.

`graph plan`, `build`, and SDK requests use the existing planner, cycle-safe
forest identities, store, executor, output policies and run manifest. A
spec-local profile can override/inherit the bundled selector. Exact targets can
select a subset. Other native policies, custom transforms, collections,
composition and post-export hygiene are excluded from this bounded route so
they cannot detach the manual-source evidence. The default profile needs no
external provider, model, account or network.

## Eligible artifacts and losses

The shared `lyrics::eligibility` API and `lyrics inspect` generate their target
view from the same implemented output list used by the graph adapters.

| Format | Representation | Explicit limits/losses |
| --- | --- | --- |
| `lyrics_json` | `renderflow.lyrics/v1`, with the existing transcript and selected policy | Retains modeled metadata; source bytes remain a separate original |
| `yaml` | The same canonical lyric model in YAML | Serialization differs; no inferred metadata |
| `srt` | Shared SubRip plain-text writer, or exact source-equivalent export | Shared writer losses; metadata remains in the companion |
| `vtt` | Shared WebVTT plain-text writer, or exact source-equivalent export | Shared writer losses; metadata remains in the companion |
| `text` | Authored cue text in order, including repeats and multiline cues | Timing, IDs and contextual fields remain in the companion |
| `markdown` | Indented literal text blocks | Same text losses; no Markdown interpretation of lyric punctuation |
| `lrc` | `[minutes:seconds.milliseconds]` per authored line | End times and multiline-cue grouping are not represented; three-decimal LRC/player support remains unqualified |
| `ttml` | Plain TTML body with explicit begin/end times, line breaks and supplied language | New ordinal XML cue IDs; no speakers, styling, word timing or score semantics |
| ChordPro | Unsupported diagnostic | No implemented compatible chord representation/adapter in this profile |
| MusicXML | Unsupported diagnostic | No supplied notes/score alignment or implemented notation adapter; no melody inference |

The structured companion preserves original cue IDs, source/policy binding,
sections, language, revision, declared review, upstream loss evidence and exact
modeled times. Each generated artifact carries the canonical digest, source and
policy identities, loss report and eligibility view in native run metadata.
TTML is a narrow plain profile, not a broadcast/platform conformance claim. LRC
exports line starts only and cannot reconstruct the original cue ends.

Source-equivalent targets retain original bytes; the existing forest suppresses
conversion cycles and duplicate target identities. A new source revision or
different selected review/language/section policy requires fresh planning.
Lyrics transforms disable generic cache reuse while source-bound qualification
is outstanding. Publication independently rereads the exact original, recreates
the expected derivative, and verifies bytes, immediate parent and metadata even
when the optional generic validator registry is disabled. The canonical digest
and transcript/source references remain evidence, not creative approval.

The output bundle uses the normal target roles/layout and machine-readable run
manifest. Read the plain/Markdown outputs for the lyrics and the canonical
JSON/YAML for timing and eligibility details. In machine workflows,
`RunManifest::published_targets()` selects observed publications. An eligibility
status is not `generated`; failed, blocked and partial branches retain the
ordinary executor/manifest states. No empty notation files satisfy a target.

## Limits and remaining evidence

Inputs use the shared 16 MiB/100,000-cue/64 KiB-per-cue limits. Canonical/export
bytes are bounded to 32 MiB; section labels are bound to existing cue IDs with
a 1 MiB aggregate label budget. The wrapper schema references the existing
transcript schema. Recognition of the new LRC/lyrics-JSON format names does not
grant automatic lyric classification or general conversion support.

Original fixtures and regressions are authored for Unicode/repeats, exact
millisecond clocks, structured round trips, TTML parsing, source/policy drift,
unknown cue metadata, missing timing, unsupported formats, cancellation,
detached evidence and actual ordinary SDK/forest publication. No tests, builds,
Cargo resolution, lint/format/schema/docs validators, CLI/provider/media/model
execution or CI ran. Generated bytes, runtime parser/player compatibility,
whole-forest source preservation and CLI/SDK parity remain unobserved.

#469 retains acceptance of this agreed manual workflow. Automatic recognition,
alignment, new audio models and provider submission are outside it; no speculative
follow-up issue or stub was created. #427's broader timed-text qualification,
#440's audio time maps, #430's integration proof and #458/#419's baseline/release
work remain under their existing owners.
