# Owner release reports

`scripts/owner_reports.py` assembles unsigned, source-bound report candidates from
explicit existing files. It is an offline adapter owned by this repository; it
does not import Flow or another sibling, invoke a binary, create a native gate,
contact GitHub, sign, publish, or change release qualification.

```text
python3 scripts/owner_reports.py publisher --request /private/publisher-request.json --output-directory /private/fresh-publisher-report
python3 scripts/owner_reports.py qualified --request /private/qualified-request.json --output-directory /private/fresh-qualified-report
```

Every file selection is a Flow-style `FilePin`: `path` is an absolute normalized
`NativePath` (`encoding: utf8`, `value`, or Unix byte encoding), with exact
`sha256` and `byte_count`. Requests and proofs are bounded, duplicate JSON fields
refuse, sources remain unchanged, and outputs require a fresh private directory.

A `flow.publisher-report-request/v1` has exactly `schema_version`, `cohort`
(FilePin), `row_id`, `archive` (FilePin), `published_release` (FilePin of retained
actual GitHub release metadata), `executable_member` (safe archive name, or null
for a library archive), and `contract_members` (identifier/member pairs, or identifier/file/published_asset_id
for a separately published contract). Optional `asset_kind` selects `tar_gz`
(default) or `raw_binary`. A raw native asset has one virtual whole-file member
named after the tool; it never pretends to be an extracted archive.

The selected owner row must be implemented, have the Cargo-authoritative version
and exact artifact pins, and match the actual published tag/asset metadata. The
adapter reads a bounded gzip/tar inventory without extraction: links, special
files, duplicate/escaping names, incorrect CPU identity, changed/truncated sources,
excess expansion and hidden trailing data refuse. The executable and every
declared contract bind exact archive members or separately uploaded, digest-bound
assets in the same retained release metadata. Library rows pin the whole Cargo
source archive and contracts actually present within it; a library source archive
does not imply registry publication or a tested Rust consumer.

The output `flow.publisher-artifact-report/v1` contains portable archive/member
digests and the exact cohort/row subject, target, version/source and release asset
identity. It contains no publisher-machine paths. It can later authenticate a
clean install before any local native witness exists; publisher authentication
and local runtime qualification remain separate.

A `flow.qualified-owner-report-request/v1` has exactly `schema_version`,
`publisher_request` (the complete request above), `clean_install`, `qualification`,
`witness`, `requirements`, and `host_identity`. Proof and host fields use FilePins.
`witness` has `run_directory` (NativePath or null), `plan_sha256`,
`acceptance_sha256`, `accepted_receipt` (FilePin). Requirements are exact
`requirement_id`/`file` pairs for every selected row requirement.

The supplied clean-install/native/gate records must use the existing closed Flow
positive-proof contracts and bind this exact subject, host, target, executable,
contracts, native plan and acceptance. A copy receipt, refusal/unit test, missing
gate, future/older-than-24-hours proof or synthetic evidence cannot substitute.
Implemented Flow routes require the actual retained native acceptance; standalone
and library rows require the owner's real positive native release gate. The
adapter assembles `flow.authenticated-release-report/v1`; it does not generate
positive facts from declarations. Flow independently rechecks actual native status
and current files after authentication.

Both outputs are **unsigned candidates**. After real release/native owners have
produced the necessary facts, review and attest the exact `report.json` bytes in
the independently trusted owner workflow. Retain its Sigstore bundle separately.
No attestation workflow is enabled or dispatched here. The consumer verifies
signer/source policy and actual current release metadata; a supplied metadata
file and report signature-state string alone confer no authority.

This source checkpoint executed no report tool, tests, build, native program,
validator, acquisition, CI, signing or publication. Existing release and native
qualification gates remain open. Flow #88 remains the final sweep; original release/native gates remain intact.

## Existing v2 cohort handoff

Use the existing release delivery command to bind the exact v2 manifest, binary
and descriptor before report assembly:

```text
python3 scripts/release/release_assets.py cohort-report --artifact-dir /private/native-assets --manifest /private/native-assets/renderflow-release-manifest-v2.json --manifest-sha256 EXACT_MANIFEST_SHA --kind publisher --request /private/publisher-request.json --output-directory /private/fresh-report
```

Use the actual manifest filename emitted by cohort assembly. Select
`asset_kind: raw_binary`, executable member `renderflow`, and the exact v2
descriptor as a separately published contract asset. The retained release metadata
must include both uploaded assets with SHA-256 digests. `--kind qualified` takes
the full supplied positive-proof request and applies the same cohort binding.
The old v1 release path and all manual native/signing/publication gates remain
unchanged; no workflow is dispatched or enabled by this source checkpoint.
