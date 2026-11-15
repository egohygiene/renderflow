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
The final source is public draft commit `9ab167e3e1f96cfb9e6b66c15fe7703acc69dd5b`
(tree `9f028530aabb94cf85f21358fd2dc9e66fb65b94`). Its local Debian-host release
build completed in 5m22s, using Rust 1.94.0, locked offline inputs and two jobs.
Binary SHA-256 is `757a6c7890ff1596347d7f537452307d02a6db12723ae64c33e5c55fa5c07dfa`.
The compiled contract SHA-256 is
`9ff99a410bcf98a82801eeadf3e35fdc40539c36d192ce066365769c98e9aa74`.

The final observer passed 21 actual CLI calls: inspect; full eight-format build;
existing bundle/payload/manifest/all five metadata-sidecar preservation; leaf and
parent aliases; reserved manifest collision; original-under-output refusal; stale
source; symlink/FIFO/directory/oversize admission; explicit retained-store export;
and portable verification of nine records/eight publications. Every negative
requires native exit 1 and the exact owning diagnostic, allowing only the exact
optional resolved-plan INFO line emitted by the existing build logger.

The committed [portable observation](akashic-manual-lyrics-observed-20261114.json)
binds source, compiler, binary, contract, observer, accepted outputs, retained
store, exported bundle and raw log hashes. Private raw originals remain separate.
A first public-source observer run falsely expected no build INFO prefix; that
observer failure is retained and not counted as an accepted guard.
This is not full test-suite, strict-Clippy, Ubuntu/native-Mac, release or signing
qualification. Local output publication does not grant remote publication,
publisher authentication or creative approval.

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

## Run the selected observer

Install Python 3 and PyYAML in your own environment, then select the built CLI
and a fresh private work directory whose parent exists:

```sh
python3 scripts/observations/check_manual_lyrics_candidate.py \
  --binary /path/to/renderflow \
  --work-directory /path/to/existing-parent/new-manual-proof
```

The observer bounds each process output to 2 MiB and 30 seconds, kills its
process group, pins observer/binary bytes before and after, and refuses special
files in accepted output inventories. It emits synthetic authored fixtures and
a hash-bound report. The small example can also be used from `examples/lyrics`:

```sh
renderflow build --config synthetic-manual.json
```

Use a new output destination for each manual export. Inspect the retained store
selector in artifact metadata before `bundle export --artifact-store-root ...`.
