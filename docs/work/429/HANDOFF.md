# Renderflow #429 — creative toolchain reconciliation

## Current checkpoint — 2026-10-13

The maintainer authorized merging pending work and advancing #429. #445
(derivative forest / #422) is merged, and #446 (review-required screenplay
recovery / #426) is merged at
`8e0f5f4a6132364965cc31bbee9c3e72074dfd4b`. The current branch is
`docs/renderflow-429-recovery-reconciliation`; obtain its eventual merge SHA
from its PR. This section supersedes the implementation-status and resume
instructions in the historical records below.

- The creative profile has ten tools, thirteen route rows and five separate
  configuration/asset requirements. Recovery extraction, rasterization and
  native body export capabilities are represented; recovery stays lossy and
  review-required.
- The JSON schema's stale eight-tool limit is corrected to ten. The virtual
  recovery provider joins the five external creative-only tools excluded from
  plain doctor, preserving plain doctor's earlier scope. Two regression tests
  are authored for report/schema contract drift and those exclusions; neither
  was run.
- The registry's FFmpeg note now reflects authored opt-in graph speech
  preparation. The general tool-registry reference was regenerated from the
  canonical YAML using the existing Python documentation generator. This was
  documentation generation, not tool execution or qualification.
- The [profile guide](../../user-guide/creative-toolchain.md),
  [manual setup guide](../../user-guide/creative-toolchain-setup.md), and
  [provider sources](PROVIDER_SOURCES.md) distinguish discovery, configured
  pins, platform acquisition and actual qualification. Node/Afterwriting remain
  unprobed by generic doctor. Pinned recovery execution does not use PATH
  discovery to choose its tools.
- The [qualification record](QUALIFICATION.md) lists exact declared contracts
  and the deferred evidence. No fixture/release toolchain is invented, no
  provider is installed, and no acceptance criterion is checked off.

Only source review, documentation generation and Git identity checks were
performed. Tests, compilation/builds, lint/formatting, schema validation, native
probes/conversions, installation/downloads and CI remain deferred. Commits use
disabled hooks and `[skip ci]`; do not dispatch or poll Actions. Merge is
authorized for this checkpoint, but does not qualify the creative stack.

### Resume from here

1. Advance #430's multi-family artifact forest / Flow contract integration,
   preserving the configured-provider and review-required evidence boundaries.
2. Follow with #406's layered/localized composition lane. Keep #412/#413 fixture
   expansion and #419 release work aligned with the later deep testing session.
3. When execution checks are reauthorized, follow [QUALIFICATION.md](QUALIFICATION.md)
   and the adapter handoffs, then reconcile #429's acceptance criteria with
   observed evidence. Keep #429 open until then; create new audit issues only
   from findings in that session.

## Historical checkpoint records

The sections below describe the earlier eight-tool checkpoint and its then-open
dependencies. They are preserved as history, not current setup instructions.

### Merge reconciliation — 2026-10-12

The maintainer explicitly approved merging the current implementation and
deferring the deep test session. #442, #443, and #444 are merged. This #441
update reconciles the doctor with those changes: one registry entry per tool,
the three scripttool import capabilities, the configured Fountain-to-PDF
capability, and generic Node/afterwriting discovery left unprobed. Doctor route
rows describe these adapters as authored and unqualified. Obtain the final
merge commit from #441; no execution or release qualification is implied.

No tests, build, lint, native probes, or CI run during this integration. Existing
#429 retains recovery integration and qualification work; create new audit
issues only when the later deep testing session produces actual findings.
The original checkpoint record follows.

## Original authority and base

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
- The original checkpoint added scripttool, Node.js, afterwriting, and Poppler
  pdftotext as discovery-only candidates. The merge preserves #424/#425's
  capability-bearing entries for the first three; Poppler remains discovery-only.
  Explicit screenplay policies own execution and pinned preflight. Plain doctor
  preserves its previous scope; the creative profile reports these candidates.
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

Nineteen test functions are authored: eleven core report/fingerprint/probe
tests, three argument-parser tests, three report-presentation tests, and two
fake-probe graph-isolation regressions. None were executed. Independent source
review found no blocking issue; this is not compilation or runtime evidence.
No external provider was run and no real creative report was generated.
The two reconciliation regressions cover pinned PDF providers staying
unobserved in the creative profile and generic runtime-service hints being
unable to qualify those global descriptors.

Existing per-edge planner filtering already prunes unavailable providers while
preserving unrelated routes. This checkpoint adds source-authored regression
coverage, not a new planner algorithm. Speech keeps its separate explicit
provider/model pins and unobserved-until-execution policy.

Issue #429 remains open. #423–#425 now provide the model, imports, and pinned PDF
adapter; #426 recovery remains unimplemented. Generic doctor observations do not
attest to the adapter's configured package/runtime pins. Actual fixture/release
tool versions and supported-platform installation remain unqualified. Source
pins establish what was inspected, not a known-good tool
release. The generated general tool-registry reference has not been refreshed;
the profile guide and canonical YAML record these additions.

## Resume

1. Preserve the integrated doctor/adapter registry and configured preflight
   boundary; do not infer qualification from merge status.
2. Complete #426 recovery, then reconcile its actual tool/data requirements
   before claiming complete creative routing or #430 integration proof.
3. When execution checks are reauthorized, run focused authored coverage,
   compile, validate report/schema parity, reconcile generated documentation,
   and record the exact tested provider/platform/asset versions.
4. Qualify real candidate adapters and model/font/data requirements before
   closing #429 or advertising a released creative stack. Do not infer issue
   completion from `doctor --strict` alone.
