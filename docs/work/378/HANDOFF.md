# #378 — editorial writing agent

Date: November 3, 2026 (UTC). Branch: `codex/378-editorial-agent`.
Base: #379 draft PR #468, commit `256b0e88884db3d6dd768d7acca89d143b8f119d`.

The maintainer authorized the Renderflow implementation batch on separate draft
branches, with testing/CI deferred and a sync after the batch. This branch is
stacked on the existing editorial profile so the content and presentation
contracts can be reviewed together. No merge or release is performed here.

## Delivered source

- One custom agent supports generation, polishing, and partial completion using
  the exact `renderflow.editorial/v1` contract. Its declared tools are read,
  edit, and search. It leaves providers, rendering, and publication separate.
- Generation remains grounded in supplied evidence; polishing preserves voice,
  factual uncertainty, quotations, citations, and code. Missing optional metadata
  is omitted. Questions and change notes stay outside publishable article bytes.
- The guide contains original synthetic generation and polish input/output
  examples, partial-draft guidance, and the handoff to ordinary rendering.
- Read the current GitHub custom-agent configuration reference on November 3;
  description/name/tools front matter and the three aliases follow that source.
  Existing research-agent structure was inspected and left unchanged.

## Editorial profile review (#379)

Source review covered bundled profile selection, ordinary planner/strategy
integration, font identity/cache refusal, provider arguments, Pandoc filter,
metadata and local-image rules, and the existing synthetic article. No additional
source blocker was identified in that review. PR #468 is still draft; review is
not compilation, provider, runtime, visual, or accessibility evidence.

## Deferred acceptance

No tests, compiler/build/Cargo resolution, formatting/lint/schema/docs validators,
product CLI/native/provider execution, model evaluation or CI was run. The
examples do not prove deterministic model responses or platform discovery.
When qualification resumes, check custom-agent discovery, representative
generation/polish/completion results, file-edit scope, factual preservation and
actual compatibility with #379's outputs. Keep #378/#379 open until their
remaining criteria are satisfied. #458/#419 still own baseline and release work.

Next in the approved batch: #350 presentation profile, then #469 manual timed
lyrics. The latter accepts author-supplied timing; no transcription/alignment
stub or speculative issue is added.
