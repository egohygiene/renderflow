# Local repository briefings

Use an authored briefing script over exact source text to produce a transcript,
accessible HTML, chapters and retained evidence. Optionally attach an existing
recording or generate an explicitly configured RSS candidate. Sources stay
canonical; Renderflow does not invent narration, fetch a repository or publish
a feed.

The #349 source checkpoint is authored and **unqualified**. No commands below
have been executed during this sprint. See the
[format evaluation](../work/349/EVALUATION.md) for adopt/adapt/defer/reject choices.

## Start with the synthetic example

From the repository root, using a later compatible built binary:

```bash
renderflow briefing plan --input "examples/briefing/briefing.json"

renderflow briefing build --input "examples/briefing/briefing.json" \
  --output-dir "briefing-draft" --allow-draft
```

`inspect` is an alias for `plan`. Planning reads bounded local snapshots and
reports expected files and the exact `script_sha256`; it creates no output.
Building writes only to a new directory whose parent exists. The synthetic
example has no human review claims, recording, model or feed; `--allow-draft`
explicitly packages it as a candidate. No publication approval is inferred.

The bundle contains exact `briefing.json`, copied `sources/<id>.txt`,
`script.json`, `transcript.txt`, `transcript.html`, `chapters.json`, optional
`narration.wav` / `feed.rss`, then `briefing-run.json` last. The receipt records
the plan and observed path/size/SHA-256 for every payload. A failed write may
leave partial files without a completion receipt; prior outputs are never
overwritten. This is an internal evidence package containing the chosen source
text and metadata, not an automatically sanitized public upload.

## Source and script contract

The [JSON contract](../../schemas/renderflow-briefing-v1.schema.json) uses
`renderflow.briefing/v1`:

- Each source declares an ID, confined relative path, exact lowercase SHA-256,
  revision label, rights and optional canonical HTTP(S) URL/review reference.
- Every authored chapter contains literal text, a stable ID/title and inclusive
  one-based source line ranges. References must name existing sources and lines.
  Citations establish selected context, not proof that the script is accurate.
- `language`, `as_of` and rights are explicit. No current-state claim is inferred
  from a filename, release label, clock or successful file read.
- Optional source/script/narration review references bind their exact bytes.
  The script digest is the exact emitted `script.json`, including selected
  source identities, metadata and chapters. Its own review claim is excluded.

To supply review declarations, first bind and review each source, then run plan
and review its emitted script identity before setting `script_review` to
`{"sha256":"<script_sha256>","reference":"<your review record>"}`. A later
source/revision/chapter change requires a fresh script review. These fields
record declarations, not authenticated approval. Without all required review
references a build requires `--allow-draft`. Every bundle still records
`review_state: candidate` and `publication_approval: not_granted`.

Changed local source bytes yield `freshness: stale`; building refuses them even
with `--allow-draft`. Source revisions are not remotely fetched. HTML and text
always show the authored as-of label and local-snapshot/publication boundary.
Keep old releases visibly tied to those recorded revisions when sharing them.

## Supplied narration and RSS

The first audio attachment profile accepts an existing bounded RIFF/WAVE file:
mono, 16,000 Hz, signed little-endian PCM16, at most ten minutes. It reuses the
speech package's existing structural inspector; no tool or model is invoked.
Compressed/extensible WAVE, multiple streams and malformed chunks are refused.
Prepare a compatible recording separately with an explicitly selected route.

The narration object records its exact audio/script digests, human-recording or
supplied-synthetic origin, voice/consent/license references and optional review.
Supplied synthetic narration additionally requires provider/model references.
Those are declarations; neither ownership, license terms, model weights nor
the words actually spoken are independently verified. No voice is synthesized,
cloned or chosen by this operation.

Optional chapter `start_ms` values must be supplied for all chapters or none,
increase strictly and lie inside the inspected audio duration. The recording's
script digest must match the current plan. Plan reports
`narration_script_current: false` when it needs updating, and build refuses it.
Audio/text binding and chapter positions remain operator assertions rather than
automatic alignment. Without supplied times, chapters stay explicitly untimed.

An optional feed object supplies channel title/description/site URL, the final
episode URL and, only with attached audio, an explicit narration URL. It creates
RSS 2.0 bytes with an input-digest-bound GUID and a WAV enclosure when applicable.
It does not infer a publication date, upload files, contact those URLs or register
a podcast. No feed is generated by default. Review and host the chosen artifacts
through your product's normal publication process; player compatibility and
actual audio synchronization remain unqualified.

## SDK and bounds

Use `publication::briefing::plan(&input, token)` and
`build(&BriefingRequest { input, output_dir, allow_draft }, token)` from the
public Renderflow crate. CLI uses those same services. This standalone profile
does not register a general v2 graph target or imply a released Flow adapter.

The request is at most 2 MiB, with 16 UTF-8 sources of at most 1 MiB each,
64 chapters of at most 32 KiB each and at most 32 source references per chapter.
Capture/output phases are bounded to 64 MiB and 256 files, with cooperative
cancellation and 120-second phase budgets. Symlinks/traversal and observed
persistent identity changes are refused. These are bounded local observations,
not a hostile-filesystem sandbox, hard RSS limit or blocked-syscall deadline.

Nine source-authored cases remain unrun. Tests, compiler/build/Cargo resolution,
lint/format/schema/docs validators, product/native/voice/model execution and CI
remain deferred. No runtime/accessibility/listening/feed/platform qualification,
release, or issue closeout is established by the authored code or examples.
