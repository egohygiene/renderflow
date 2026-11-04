# Synthetic local briefing

`canonical.txt` and its two-chapter `briefing.json` are original synthetic
Renderflow material under MIT. Harbor Notes is fictional. No real project,
voice, person, review, provider output or publication approval is represented.

The JSON pins the exact UTF-8 source bytes. Source/script review declarations
are absent, so a later candidate build explicitly needs `--allow-draft`:

```bash
renderflow briefing plan --input "examples/briefing/briefing.json"
renderflow briefing build --input "examples/briefing/briefing.json" \
  --output-dir "briefing-draft" --allow-draft
```

These commands are authored and unrun. They create transcript/HTML, chapters,
source/script copies and a last-written receipt; this example supplies no audio
or feed. Follow the [guide](../../docs/user-guide/repository-briefings.md) for
exact review bindings and separately supplied narration/RSS configuration.
Tests/build/lint/schema/docs/provider/native/CI qualification remains deferred.
