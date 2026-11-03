# Manual timed lyrics

Create and review your SRT in your preferred editor. Keep the lyric wording and
timing there as the source. This example supplies original synthetic French
lines with one repeat and a multiline cue; it is explicitly unreviewed fixture
content, not a real song or an observed audio alignment.

When execution resumes:

```bash
renderflow lyrics inspect --input "manual.srt" --language "fr" --revision "manual-r1"
renderflow graph plan --config "renderflow.yaml"
renderflow build --config "renderflow.yaml"
```

Copy the observed `policy.source_sha256` from inspection into your own spec.
The checked-in example already pins its synthetic file's observed bytes. Changing
the file requires a fresh pin and a fresh review declaration. `review_state`
defaults to `unreviewed`; it is never inferred from a filename or successful run.

The `lyrics` profile requests canonical lyrics JSON, YAML, SRT, WebVTT, plain
text, literal Markdown, millisecond LRC and plain-profile TTML. A source-equivalent
SRT is exported from the unchanged original rather than regenerated. Inspect the
actual run's published targets for success; a plan lists requested/eligible work.
No chords, score, automatic transcription, audio alignment or provider submission
is generated. Missing ChordPro/MusicXML support is an explicit eligibility
diagnostic, not an empty successful output.

These commands and conversions have **not run** for this checkpoint. See the
[guide](../../docs/user-guide/manual-lyrics.md) for supported inputs, losses,
provenance and the retained validation work.
