# #412 — bounded corpus extension

November 3, 2026 (America/New_York). Authored source only; nothing here records a
passing execution. The prior #349/#397/#406 drafts #476–#478 were explicitly
authorized and merged before this branch. This checkpoint targets existing
#412 and reuses #366's manifest, schemas and `golden_conformance` harness.

## Source outcome

- Corpus 1.2.0: original 14 fixtures plus 16 literal adversarial cases. Every
  source now has an exact declared SHA-256, finite construction, classification,
  fixed seed and residual. Header recognition stays separate from validity.
- Five taxonomy classes, including low-budget archive byte/count/depth/ratio
  cases, traversal with a retained safe sibling and an inert symlink declaration.
  No arbitrary malware or large decompression fixture is included.
- Existing Rust harness enforces confined unique filenames, source byte/count
  bounds, cooperative phase deadlines, source immutability, typed rejection,
  discovered counts/parents and retained payload verification.
- Additive optional v1 schema fields keep older manifest shapes representable.
  Existing `Fixture`/`Scenario` definitions are extended; the common contract
  module belongs to the same integration harness, not a second framework.
- Coverage rows come from the engine format registry and exact shared creative
  catalog plus explicit local obligations. Unmapped capabilities remain gaps;
  mock executor, intake, discovery and native-output scope are distinct.
- Fast-tier optional tool probing is removed. Higher-tier behavior is retained;
  no workflow or provider dependency is changed.

## Review and remaining acceptance

Manual source comparison covered current intake diagnostics, extraction budget
accounting (the source counts as one artifact), nested parent lineage and the
schema/report fields. Fixture digests were authored while editing literal
source declarations; no product output, full-format validator or provider result
was observed. Reviewed source ZIP recipes use fixed metadata; stored hex is
canonical. A ratio fixture is only 4096 expanded bytes.

No tests, compiler/build/Cargo, lint/format, schema/docs validator, repository
generator/harness, CLI/native/provider/model/media execution or CI ran. The
new adversarial scenario and changed legacy harness remain uncompiled/unrun.
No passing report or visual baseline is committed. All original #412 acceptance
criteria stay open, including broad format/provider/failure coverage, CI and
memory/sharding budgets, fuzzing and runtime/platform evidence.

Next: #413's small original comic and larger parameterized case must extend
this corpus's pack convention and golden harness. Reuse licensed existing font
assets and the public composition API; keep unsupported aggregation/EPUB/provider
acceptance explicit. No private comic or sibling source/runtime dependency.
