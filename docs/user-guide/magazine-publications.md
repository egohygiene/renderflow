# Magazine publication profile

The bundled `magazine` profile turns one reviewed editorial source into a capability-driven release tree. It is a `renderflow.profile/v1` document consumed by the normal planner; there is no magazine-specific executor.

## Authoring contract

Add a `publication` block using `renderflow.publication/v1`. It records the publication and issue identity, contributors, date and status, language, artwork roles, trim geometry, color and font policies, rights approval, accessibility metadata, identifiers, and role-specific output constraints.

Use `examples/magazine/renderflow.yaml` as a complete redistribution-safe starting point. Keep editorial content and approved assets in the content repository; Renderflow owns derivative planning, execution evidence, hygiene, validation, and packaging metadata.

Statuses are `draft`, `reviewed`, `approved`, and `released`. An `approved` or `released` contract is rejected unless it contains a license, rights holder, approval reference, an explicit rights review, and a selected hygiene policy whose rights gate is required and reviewed. Renderflow records that decision; it does not make a legal conclusion.

## Preview the artifact forest

From the fixture directory:

```bash
renderflow graph plan --config renderflow.yaml --profile magazine
renderflow build --config renderflow.yaml --profile magazine --dry-run
```

The forest reports optional targets such as KEPUB or cover previews as unavailable when their adapters are absent. It never reports a pruned branch as produced.

## Proof and release roles

The profile declares separate `print/press`, `print/proof`, and `digital/screen` roles. Their constraints live under `publication.output_roles`, where geometry, color policy, minimum image DPI, font embedding, stage, and validator expectations can differ.

When these roles share the same preset, template, variant, and provider-neutral options, the generic DAG renders PDF once and materializes the content-addressed artifact under each role. This is useful when proofing the exact release bytes. Publications requiring differently rendered PDF variants should declare different Transform v2 target nodes rather than claiming shared output.

## Build and inspect

```bash
renderflow build --config renderflow.yaml --profile magazine
renderflow inspect --config renderflow.yaml --all
```

Inspect `release/renderflow-run.json` and the files under `release/metadata/` with ordinary JSON and checksum tooling after the build.

Available adapters determine the exact tree. A completed publication build also writes:

- `metadata/publication.json` — the normalized publication contract;
- `metadata/manifest.json` — publication identity plus artifact evidence;
- `metadata/provenance.json` — source-spec, execution-plan, and toolchain evidence;
- `metadata/preflight.json` — separate constraints, artifact identity, validation state, and unavailable deep checks for each declared role;
- `metadata/checksums.sha256` — stable SHA-256 entries for materialized outputs and metadata.

EPUB and KEPUB remain ordinary optional profile targets and use the e-book capabilities described in [EPUB and KEPUB derivatives](ebook-derivatives.md). No retailer or provider is embedded in the publication model.

## Hygiene and release approval

Hygiene runs before validation and materialization. Blocking secret, protected-reference, rights, or approval findings prevent affected artifacts from entering the release tree. Candidate and proof roles remain explicit in the contract and manifest even when the complete run is partial.

## Resume and rollback

Use `--resume` to reuse checkpoints only when source, plan, configuration, and toolchain fingerprints remain compatible:

```bash
renderflow build --config renderflow.yaml --profile magazine --resume
```

For rollback, retain the canonical source/spec plus `renderflow-run.json`, `metadata/provenance.json`, and `metadata/checksums.sha256` from the desired release. Restore that source revision and rebuild with the recorded toolchain. Never rename a failed or partial run into an approved release; correct the contract or source and create a new evidenced run.
