# Renderflow #429 — creative doctor checkpoint 1

## Authority and base

- Existing issue: https://github.com/egohygiene/renderflow/issues/429 under #421.
- The user authorized merging the existing work and advancing existing
  Renderflow issues. PR #438 merged first at
  `425d0ea3ae499ffe04b27c688d7ea84fa890b8dc`; PR #439 then merged onto `main`
  at `31822e5348dd7793b1beb8a79817809aa0dc00c4`.
- Both merged trees and the local checkpoint-6 base were reconciled. The final
  merged-main tree is `8fd448b5f815f599101b292994142bc1698ee612`, identical to
  local base commit `46790b885202e75336c35d242d8e5f3bf50fa9c2`. Commit ancestry
  differs because earlier publication used GitHub Git objects; tree identity
  is the shared base. New remote publication uses merged `main` as its parent.
- Branch: `feat/renderflow-429-creative-toolchain`. Obtain its eventual head
  from the draft PR; this handoff cannot contain its own commit SHA.
- Tests, compilation/builds, schema validation, formatting/linting, native
  probes/smoke/inference, and CI remain explicitly deferred. Source review,
  authored coverage, Git integrity reads, and draft publication are authorized.
  Use disabled hooks and `[skip ci]`; do not dispatch or poll Actions.
- No new issue scope, installation, model download, user-media processing,
  release, or automatic merge of this new draft is part of the checkpoint.

## Authored behavior

- `doctor --profile creative-publishing --format text|json` assesses eight
  selected tool IDs independently through the existing bounded discovery layer.
  Text is the default; format selection requires an explicit profile.
- Four discovery-only candidates enter the catalog: scripttool, Node.js,
  Afterwriting, and Poppler pdftotext. They have no conversion capabilities,
  media types, or graph edges. Plain doctor filters them before probing;
  explicit complete inventories may report them.
- The source-reviewed Afterwriting CLI has no safe version-only query. Additive
  `ToolDiscovery::Unprobed` / `ToolAvailabilityStatus::NotProbed` represent that
  absence without a process launch or an installation claim. `is_available`
  remains false, so planners cannot treat this candidate as available.
- Strict profile mode writes and flushes the complete report before failing on
  supported discovery failures. Explicitly unprobed tools are counted
  separately and do not fail strict; success cannot qualify a complete stack.
- Eleven route rows distinguish absent adapters from authored, unqualified
  work. Provider status is separate. Built-in timed-text conversion and the
  offline explicit time-map exporter require no external executable.
- Four separate model/font/language-data requirements remain explicit and
  unobserved. The profile does not read media/models/fonts, infer speech pins,
  perform recognition, or change planner policy.
- `renderflow.creative-doctor/v1` includes a deterministic observation
  fingerprint covering selected declarations, discovery results, platform,
  and report policy. It is not an executable/model attestation or cache key.
  The checked-in JSON Schema describes the report, including missing and
  unprobed states.
- [User guide](../../user-guide/creative-toolchain.md) and
  [provider source records](PROVIDER_SOURCES.md) explain the commands,
  installation references, exact source pins, and separate asset licensing.

## Evidence and limits

Seventeen test functions were authored: nine core report/fingerprint/probe
tests, three argument-parser tests, three report-presentation tests, and two
fake-probe graph-isolation regressions. None were executed. Independent source
review found no blocking issue; this is not compilation or runtime evidence.
No external provider was run and no real creative report was generated.

Existing per-edge planner filtering already prunes unavailable providers while
preserving unrelated routes. This checkpoint adds source-authored regression
coverage, not a new planner algorithm. Speech keeps its separate explicit
provider/model pins and unobserved-until-execution policy.

Issue #429 remains open. Screenplay model/adapters (#423–#426) are absent,
Afterwriting discovery/package identity needs a dedicated contract, and actual
fixture/release tool versions and supported-platform installation remain
unqualified. Source pins establish what was inspected, not a known-good tool
release. The generated general tool-registry reference has not been refreshed;
the profile guide and canonical YAML record these additions.

## Resume

1. Review and, when authorized, merge this checkpoint without mistaking that
   operation for qualification.
2. Advance the existing screenplay model/adapter dependency chain before
   claiming complete creative routing or #430 integration proof.
3. When execution checks are reauthorized, run focused authored coverage,
   compile, validate report/schema parity, reconcile generated documentation,
   and record the exact tested provider/platform/asset versions.
4. Qualify real candidate adapters and model/font/data requirements before
   closing #429 or advertising a released creative stack. Do not infer issue
   completion from `doctor --strict` alone.
