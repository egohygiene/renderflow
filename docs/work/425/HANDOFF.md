# Renderflow #425 — afterwriting PDF and independent analysis checkpoint

## Current integration status — 2026-10-12

With explicit maintainer approval, #444 merged at
`3ca1f97673c2369f487a21e390b697f8fbf37d99` after #442 and #443. Its tree is
`ef5330759ea307d357b88c0da4632cbd38ccfd84`, matching the reviewed local wrap-up.
The #441 integration keeps one entry per provider, generic Node/afterwriting
discovery unprobed, and configured adapter preflight intact. Tests, builds,
linting, native/visual/reproducibility qualification and CI remain deferred.
#425 retains these gaps for the deep test session. The original checkpoint
record follows; its earlier no-merge scope has been superseded by this approval.

## Original authority and base

- Existing issue: https://github.com/egohygiene/renderflow/issues/425 under #421.
- The user explicitly approved #425 after the #424 draft checkpoint. No merge,
  release, new backlog scope, or external runtime installation is implied.
- Branch: `feat/renderflow-425-afterwriting-pdf`, stacked on open draft #443,
  branch `feat/renderflow-424-scripttool-adapter`, which is stacked on #442.
- Remote parent at branch start: `3e076c0333f154b0f1c4f0101bc122ceacb9fd75`.
  Local parent: `01ec9b2d921f413e68d818e68bb70c429a495ff4`.
  Both trees: `aa9cfe30069e4c1af598976f996505d6ea119590`. Publication uses the
  remote parent and verifies exact authored Git blobs/tree; local and remote
  ancestry are not presented as identical. Read the PR for its final commit SHA.
- Tests, builds/compilation, schema checks, formatting/linting, native runtime
  probes/conversion, installs/downloads, and CI remain deferred. Do not dispatch
  or poll Actions. Use disabled hooks and `[skip ci]` for publication. Source
  review, authored fixtures/tests/docs, Git reads, commits and draft PRs are
  authorized. None is runtime qualification.

## Authored implementation

- [Evaluate afterwriting](PROVIDER_EVALUATION.md), current source/release and
  dependency differences, runtime declarations, actual CLI, fonts, MIT and
  third-party licensing, internal analytics, and metadata behavior. Version
  `1.17.3` spans distinct dependency trees. The CLI has no provider version
  probe or stable structured report command.
- Add explicit `AfterwritingConfig` for Node path/hash/exact candidate version,
  installed package root/tree hash/version, versioned profile, timeout, and
  unqualified opt-in. Check static package identity, required project modules,
  declared runtime dependency entry points, and required PDFKit font metrics.
- Add bounded installed-package fingerprints and staged snapshots, including
  dependencies/assets while excluding root `.git` and `node_modules/.bin`.
  Reject other symlinks/special files and escaping names. A fingerprint is byte
  identity, not a trusted package, source-build, or dependency-closure attestation.
- Render original Fountain through direct Node argv with a cleared environment,
  disabled addons/global module search, bounded version probe/conversion,
  cancellation/capture/timeout/output freshness guards, immutable source and
  runtime/package rechecks, and independent PDF inspection. Ancestor module
  resolution and dynamic libraries remain outside a loaded-code attestation;
  network policy is not OS isolation.
- Bind A4/US Letter/Courier formatting to fixed versioned profiles. Keep the
  canonical model independent of provider settings. Accept strict ASCII/LF
  body-only Fountain; reject unsupported syntax, title pages, non-ASCII, custom
  settings/fonts, and `<?xml` anywhere. Direct native FDX remains unavailable.
- Add deterministic core analysis with source identity, element counts, ordered
  exact scene-heading occurrences, exact cue statistics, and precisely defined
  whitespace-token counts. No provider analytics, physical location inference,
  duration estimates, or estimated PDF page counts are claimed.
- Independently inspect passive PDF structure, page tree/geometry, text
  operations and Courier resources. Record metadata/ID evidence and actual
  page count. Do not claim visual/source-text equivalence, embedded fonts,
  pagination or byte reproducibility. Parser object checks are post-load, so
  bounded streams/input do not establish a total process memory quota.
- Add `screenplay analyze`, `screenplay package-fingerprint`, and
  `screenplay pdf`. PDF bundles publish PDF/analysis/provider evidence and a
  receipt last, refusing existing destinations. Partial publication has no
  completion receipt; stdout failure can occur after completed publication.
- Add one explicit `execution.screenplay_pdf` graph edge,
  `screenplay.render.fountain.pdf`, with `tool.node` and `tool.afterwriting`.
  Preflight uses the adapter's cleared-environment probe rather than generic
  inherited-environment probing. Missing prerequisites prune the route.
- Require an original Fountain source and one PDF target. Refuse collections,
  mixed native policies, custom transforms, target formatting overrides,
  effective profile hygiene, native normalization chains, and unrelated routes.
  Carry evidence and analysis in the artifact manifest; mandatory final PDF
  inspection and source/report/policy binding cannot be bypassed by disabling
  generic validation. Disable shared native cache/checkpoint reuse.
- Author analysis, PDF inspection, provider evidence, and standalone bundle
  schemas; retain original synthetic fixtures and focused contract coverage.
  [User guide](../../user-guide/screenplay-pdf.md) documents configuration and
  exact limitations. Nothing is bundled, installed, or released.

## Evidence boundary

All implementation and tests are authored and unrun. Source reviews covered
runtime/package pinning, native input guards, PDF inspection, CLI publication,
graph policy and terminal evidence binding, schemas, and documentation. Authored
fake-runner PDFs exercise contracts only when later run; they do not demonstrate
that afterwriting itself succeeds. Fixture hashes identify authored bytes.

Sixty-six focused tests are authored and unrun: fourteen provider/installation,
eighteen PDF inspection, seventeen graph/planning/spec, ten analysis, and seven
CLI/publication/config cases. Source review also corrected an evidence count
type mismatch, exact package exclusions, and terminal source/report/policy
binding. No compile or passing-test claim follows from that review.

The wrap-up source review found and corrected a planning policy bypass: a
configured afterwriting installation could be probed after policy had excluded
its PDF edge. Planning and execution refresh now require the exact permitted
edge before invoking package/runtime preflight. Two added callback-based
regression cases cover provider/transform denials, restrictive tool allowlists,
deterministic requirements, and missing/mismatched selected edges without
launching a process. These tests are also unrun. The provider evaluation's
`requirejs-text` lockfile version was corrected to `2.0.15`.

The initial profiles intentionally accept less than general Fountain. Normal
title pages, Unicode, styling, multiline body elements, or application-specific
native documents need a later independently supported profile. A profile refusal
is not a claim that the source is invalid in its native application.

## Remaining completion work

The [qualification runbook](QUALIFICATION.md) supplies the deferred commands,
two-run evidence comparison, failure cases, and visual acceptance record. It is
authored guidance, not evidence that these checks ran.

1. Keep #425 open. Once checks are authorized, compile and run focused provider,
   package, parser/analysis, PDF, graph, CLI and schema coverage; resolve actual
   failures before claiming usable runtime support.
2. Qualify exact Node/package/dependency/font bytes and platform. Exercise
   standalone and graph/SDK rendering, missing prerequisites, malformed or
   changed files, cancellation/timeouts, truncated logs, invalid PDF, and
   no-success-on-failure publication using original fixtures.
3. Render twice with pinned source/config/runtime/package. Preserve both PDFs,
   compare raw hashes and documented structural/text/layout evidence, and
   account for timestamps/IDs. Current code reports metadata variability;
   no normalization or observed reproducibility is claimed.
4. Perform visual/readability/print review separately, including nonembedded
   Courier substitution. Do not equate structural validity with complete source
   fidelity, typography, PDF/A, or publishing approval.
5. Preserve the reconciled single registry entries, doctor surface, configured
   availability path, and exact capability scope when extending #429.
6. Existing #426 owns explicitly lossy PDF recovery; #429 owns toolchain
   qualification and #430 integration proof. This checkpoint does not close
   those issues, authorize their execution, or publish a release.
