# Renderflow #424 — bounded scripttool import checkpoint

## Authority and base

- Existing issue: https://github.com/egohygiene/renderflow/issues/424 under #421.
- The user explicitly approved #424. Implement the existing issue without
  adding new backlog scope or merging unrelated drafts.
- Branch: `feat/renderflow-424-scripttool-adapter`, stacked on the open #423
  draft #442, branch `feat/renderflow-423-screenplay-model`.
- Remote parent at branch start: `388b3813a8dba3c0fec615f4e0ac0fc0f8f1ec73`.
  Local parent: `95cc53292128dcad130d5954681ce75f9c32c383`.
  Both trees: `0c5431db6d4ecee8d086608fa0b7b1ed2a0a8e06`. Git-object publication
  uses the remote parent and verifies the exact authored tree; different commit
  ancestry is not presented as identical history. Obtain this draft's final
  commit from its PR rather than a self-referential SHA in this file.
- Tests, compilation/builds, schema validation, formatting/linting, native
  execution, installs/downloads, and CI are deferred at the user's request.
  Do not dispatch or poll Actions. Disabled hooks and `[skip ci]` are required.
  Source review, authored fixtures/tests, Git reads, commits, and draft
  publication are authorized; none are runtime qualification.

## Authored implementation

- Evaluate optional external scripttool against pinned upstream source and
  dependency identities. Record releases, the ambiguous `0.0.10` version,
  actual argv, installation/build observations, and license discrepancy in
  [PROVIDER_EVALUATION.md](PROVIDER_EVALUATION.md).
- Adopt only FDX → Fountain, OSF → Fountain, and Fade In → Fountain as explicit
  unqualified candidates. Reject all reverse native directions with source
  evidence. Treat Fountain → provider JSON as a separate rejected inspection
  candidate; create no canonical JSON edge or generic converter claim.
- Require absolute executable path, lowercase SHA-256, exact version, bounded
  timeout, and `allow_unqualified: true`. Never auto-install, bundle/link/vendor
  upstream code, or infer executable source identity from its version line.
- Independently parse a strict body-only profile with existing XML/ZIP
  dependencies. Reject unsupported native structures, versions, metadata,
  styling, ambiguous syntax, malformed/oversized archives and XML. Decode one
  parenthetical syntax pair, compare ordered semantic text/kinds/owners exactly,
  and disclose omitted blank spacing/FDX Empty paragraphs and layout losses.
- Execute direct bounded argv in private staging with captured-output limits,
  version checks, source/binary rechecks, cancellation/timeouts, fresh regular
  expected output, independent parsing, and semantic comparison. This does not
  create an OS sandbox or executable build attestation.
- Add `screenplay convert` with receipt-last, no-clobber bundles. Provider and
  semantic failures publish no completed result. I/O failure may retain an
  incomplete directory; stdout failure can follow completed publication.
- Add opt-in `execution.screenplay`, three direct provider capabilities,
  configured version probing/pruning, frozen policy identity and artifact
  evidence. Refuse unsupported source/target routes, mixed native policies,
  collections, custom transforms, and effective profile hygiene. Permit only
  expected XML/ZIP wrapper conflicts for explicitly declared native formats.
- Disable reusable generic cache/checkpoints for screenplay execution and
  retain the absence of a toolchain-attestation claim. Register independent
  strict Fountain artifact validation so default validation can accept an
  adopted output. General FDX/OSF/Fade In artifact validation remains unavailable.
- Preserve canonical `screenplay inspect` as JSON-only. Its adapter policy
  describes canonical import/export, not separately configured native routes.
- Add evidence/bundle schemas, original synthetic fixtures, focused authored
  coverage, [user guide](../../user-guide/scripttool.md), and roadmap status.

## Review and evidence boundary

Parallel source reviews covered the independent parser, process boundary,
planner, publication, registry, and schemas. Review corrected quadratic owner
lookup, stale output from a version probe, inherited hygiene after conversion,
native XML/ZIP wrapper conflicts, and the missing default Fountain validator.

Focused coverage is authored for native grammar/bounds/archives, semantic
differences, exact pins/argv, provider failure and cancellation, missing or
stale output, input/binary mutation, CLI publication, graph direction/pruning,
spec schema/policy, and artifact validation. Fake-runner cases are contracts,
not proof that a native binary succeeds. All tests are unrun. No compiler,
validator, format/lint command, native probe/conversion, or CI has run.

The authored native profile intentionally accepts less than an arbitrary
screenplay application document. The repository's original #423 fixtures are
retained; `tests/fixtures/screenplay/native-424` adds separate strict examples.
Fade In wrappers for focused tests are authored from the original OSF fixture
inside those tests. No upstream screenplay or converter binary is included.

## Remaining completion work

1. Keep #424 open. Compile and execute focused Rust/CLI/schema checks once
   testing is authorized; resolve actual failures before claiming readiness.
2. Qualify each of the three adopted directions against a recorded executable
   digest, version, source/build or release identity, and platform. Exercise
   original positive fixtures plus malformed/loss/failure cases, graph/SDK
   default validation, immutable inputs, and receipt/checkpoint behavior.
3. Reconcile the explicit rejection decisions with issue acceptance. Broader
   native profiles and reverse writers need independent evidence or a revised
   provider, not silent widening or guessed native envelope defaults.
4. Merge/rebase #442 before landing this stacked draft. #441 is an independent
   #429 doctor draft that also introduces discovery-only `tool.scripttool`.
   Reconcile that registry entry during merge: keep a single stable tool ID,
   the three bounded capability declarations, and the doctor profile. Do not
   discard either feature or duplicate the ID.
5. Existing #425 owns screenplay PDF output, #426 lossy PDF recovery, #429
   toolchain qualification, and #430 integration proof. This draft does not
   close or qualify any of those issues and does not publish a release.
