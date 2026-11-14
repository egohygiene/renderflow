# Manual lyrics publishing candidate — November 14, 2026

This checkpoint uses authored SRT and the existing explicit `lyrics` profile.
There is no recognition, alignment or timestamp inference. Review stays explicitly
`unreviewed`; format conversion does not approve wording, timing or appearance.

## Source repairs

- Lyrics inspect now uses the existing held, bounded timed-text reader. Regular
  files only; the 16 MiB limit is checked before reading, Unix opens are
  nonblocking/nofollow, and held descriptor/path identity is rechecked.
- The required default validation dispatcher now admits native LyricsJson, LRC
  and TTML exports. Canonical JSON uses the existing closed document reader;
  LRC/TTML validation is bounded native-export structure, not universal format,
  player or TTML schema qualification. Export payloads are limited to 32 MiB;
  TTML additionally has a 1,000,016-node parser budget. DTD/entities are refused.
  Existing mandatory original/policy/canonical/lineage and exact rerender checks
  remain required. No `allow_unavailable` bypass is selected.
- Explicit manual lyrics requires fresh payload, manifest and publication-sidecar
  destinations. The existing ArtifactStore atomic publisher has an internal
  no-replace mode using `persist_noclobber`; normal graph/audio replacement
  behavior stays unchanged. All pending paths and existing symlink components
  are checked before output/state creation. This preflight does not claim
  protection against hostile concurrent ancestor-directory replacement.
- Manual state uses a unique retained `.renderflow/manual-lyrics-*` directory.
  Each artifact records its actual store selector relative to the bundle parent,
  including source records retained on a failed execution. Supply this store
  explicitly to bundle export; it is not the shared `.renderflow/artifacts`.
  Manual checkpoint assessment, resume and reset are explicitly unsupported;
  they refuse before touching unrelated shared checkpoint files.

## Validation status

Before repair, an actual old CLI blocked on FIFO input and published only five
of the eight required targets: LyricsJson/LRC/TTML had `validation.no_signature`.
The first repaired CLI completed all eight formats; its proof observer then
failed because its expected VTT string omitted the codec's final blank line.
That observer failure is retained separately. Subsequent diagnostic controls
also showed existing-target replacement; those are failed guards, not accepted
no-clobber evidence.

Five focused native-export validator tests and two registry dispatch/bounds
tests passed. Eight selected publisher/manual-profile/SDK checkpoint tests passed,
including exact owning refusals and preservation of unrelated checkpoint bytes.
Only the known unread speech `frame_count` compiler warning was observed.
The final published-source release CLI, complete eight-format observer and
publication refusal proof are pending at this source-preparation checkpoint.
This is not full test-suite, strict-Clippy, Ubuntu/native-Mac, release or signing
qualification. Final actual runtime observations will be appended to this draft.

## Reproduction scope

Use Rust 1.94.0 with the committed lockfile, two jobs and a private target cache.
Focused selectors are `lyrics::validation::tests`, `manual_lyrics_validator`, and
`manual_`. The candidate invocation is:

```sh
cargo +1.94.0 build --locked --offline --release --target x86_64-unknown-linux-gnu --package renderflow-cli --jobs 2
```

Author manual UTF-8 SRT, select its exact SHA-256 in `execution.lyrics`, choose
`targets.profiles: [lyrics]`, and use a fresh output root outside source/state.
The profile preserves overlap/repeated cues. Text and Markdown lose timing and
cue IDs; LRC loses cue end times and multiline grouping. Source-equivalent SRT
keeps its exact original bytes while canonical JSON carries manual classification,
review, revision and section declarations.
