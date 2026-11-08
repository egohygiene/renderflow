# Local briefings through normal build and artifact-forest planning

The November 7 source checkpoint adds one explicit v2 `kind: briefing` model
to the existing common build/inspect/SDK facade. It captures the local model,
bounded pinned source snapshots and optional supplied narration once and passes
that frozen state to the existing publisher. It does not add a voice provider,
AI step, network fetch, second job engine or automatic publication.

Exact primary targets are `briefing_script`, `briefing_text`, `briefing_html`,
`briefing_chapters`, `briefing_feed`, and `briefing_audio`. Each is bound to its
fixed native filename and distinct forest identity. Feed/audio targets require
their explicit native input. Missing inputs, stale source pins, outdated audio/
script binding and absent required review declarations remain unavailable in
the plan and block the required batch before a directory is created. The
explicit `execution.briefing.allow_draft: true` permits a local unreviewed
candidate; it cannot override stale bytes or approve content/publication.

See [the original safe fixture](../../../examples/briefing/README.md) and its
[common build spec](../../../examples/briefing/build.yml). Ordinary `renderflow
build --config ...` and `renderflow inspect --config ...` use the same resolved
backend as the SDK. These instructions are authored and have not been run.
The native publisher writes its complete bounded local package, including the
model/source snapshots and auxiliary representations. Selecting primary forest
branches is not a redaction policy: keep the local package private until its
actual content/rights review. No source, narration or publication review was
performed for this checkpoint.

Successful native publication is followed by exact bounded no-follow member
observation, including the native receipt. Every published member has common
artifact evidence; requested primary branches point to their exact record
index, ID and locator. The native receipt stays intact; `build-plan.json` and
`renderflow-run.json` add the ordinary plan/run projections. Dry-run/cancelled
planning records no observed artifact. Native/write/verification failure returns
an error and may retain partial files; there is no fabricated common success.
Captured source freshness describes the exact local snapshot, not live remote
repository state or later edits outside that snapshot.

The policy is closed: one local model and required exact target formats,
default naming/collision behavior, no selectors/presets/custom transforms,
resume/watch or ignored execution overrides. Local/offline flags retain intent;
no OS sandbox, voice license, measured alignment or fidelity qualification is
inferred. Fixed unit costs/zero quality are unqualified plan placeholders.

Four meaningful regressions are authored, uncompiled/unrun. Tests/compiler/
build/Cargo resolution/dependency/lint/format/schema/docs/corpus/native/provider/
media/model/CLI/SDK/listener/CI execution remain deferred. Schema additions are
authored directly; no generator/validator ran. #349 remains open for its full
content/voice/format/native/accessibility/runtime acceptance; #429/#458/#419 and
the proposed/accepted ADR distinctions remain unchanged.
