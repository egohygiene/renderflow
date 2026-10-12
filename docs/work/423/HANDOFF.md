# Renderflow #423 — canonical screenplay checkpoint 1

## Authority and base

- Existing issue: https://github.com/egohygiene/renderflow/issues/423 under #421.
- The user approved advancing this issue after the #429 doctor checkpoint.
  #441 remains a separate draft; this change is based directly on merged main.
- Remote base: `31822e5348dd7793b1beb8a79817809aa0dc00c4`, tree
  `8fd448b5f815f599101b292994142bc1698ee612`, matching local base
  `46790b885202e75336c35d242d8e5f3bf50fa9c2`. Commit ancestry differs because
  previous publication used GitHub Git objects; publication verifies exact
  trees and uses remote main as its parent.
- Branch: `feat/renderflow-423-screenplay-model`. Obtain its published head
  from the draft PR; this file cannot contain its own final commit SHA.
- Tests, compilation/builds, schema validation, formatting/linting, native
  execution, and CI are explicitly deferred. Do not install tools, dispatch or
  poll Actions, or claim source review as runtime evidence. Use disabled hooks
  and `[skip ci]`. Source reads, authored tests, fixture/document authoring,
  Git integrity reads, commits, and draft publication are authorized.

## Authored checkpoint

- Add canonical `renderflow.screenplay/v1`, bounded strict JSON parsing,
  serialization, validation, inspection evidence, and directional loss policy.
  Preserve ordered title fields/elements, stable IDs, character references,
  explicit dual dialogue, declared source identity/losses, and extension data.
- Register FDX, Fade In, OSF, and canonical screenplay JSON as typed formats.
  Native formats remain recognition-only with no conversion graph edges or
  Pandoc mapping. Remove Fountain's incorrect Convert/Pandoc capability claim.
- Preserve `.screenplay.json` as a multipart suffix, separate its schema-marker
  intake signal from generic JSON, and keep native container conflicts honest.
- Add `screenplay inspect --input ... --format json|text`, JSON by default.
  Inspection is local and read-only, validates canonical JSON, and reports
  its own input digest separately from unverified declared source identity.
- Add the experimental canonical artifact validator. Native screenplay
  structural validation stays unavailable rather than passing on UTF-8 alone.
- Author schemas, original synthetic fixtures, source references, regression
  coverage, and [user documentation](../../user-guide/screenplay.md).

## Evidence boundary

All implementation and tests are authored but unrun. Source review and exact
Git publication checks do not establish compilation, schema conformance,
native application acceptance, cross-format fidelity, or release readiness.
The fixture ZIP and generated format-reference page are authored artifacts;
creating them does not execute Renderflow or qualify a format.

Forty-three test functions are authored for the later pass: eighteen core
model/codec/evidence tests, twelve CLI/command tests (one Unix-only), ten format,
intake, planning, and migration regressions, and three artifact-validator tests.
Source review corrected numeric precision handling in opaque extensions,
bounded serialization, and Fade In's canonical wire spelling. Extension numbers
now require exact i64/u64 integer tokens; other source numeric tokens must be
strings or are refused. No runtime pass is claimed.

Native fixtures are original minimal examples based on the primary sources in
[FORMAT_SOURCES.md](FORMAT_SOURCES.md). No upstream script text, user screenplay,
model, font, or native provider is included or required. The Fade In fixture
contains the documented OSF `document.xml` envelope, but native-app acceptance
remains unobserved. The canonical fixture is a hand-authored model example,
not a claimed result of converting the native fixtures.

## Remaining work

- Keep #423 open until authored coverage and acceptance boundaries are executed
  and reconciled. Native fixture import/export proof belongs with actual
  parsers/providers; this checkpoint does not create those adapters.
- Advance existing #424 for supported screenplay interchange, using this
  model and carrying directional loss/unknown extension evidence per result.
  Then #425 supplies screenplay PDF output and #426 supplies lossy recovery.
- When checks are authorized, compile and run focused model, CLI, intake,
  artifact-validation, and format regressions; validate both JSON Schemas and
  verify native fixtures in the exact qualified provider/application versions.
- Reconcile #441's independent #429 changes during merge, preserving both the
  creative doctor profile and screenplay model. Do not close #429 or claim
  #430 integration proof merely because the model or doctor has merged.
