# Broader release inventory v2

`renderflow release-contract` returns `renderflow.release-contract/v2` as JSON.
The public library entry is `release_contract::describe()`. It uses the canonical
planner's built-in edge registration and embedded tool registry, plus the creative
route declarations. It does not run discovery, probe tools, load models, invoke
providers or contact services. The ordinary graph, conditional creative routes
and additional native surfaces remain separately described. A source declaration
is not a working conversion on the current host.

`renderflow.release-manifest/v2` is an additive multi-target asset inventory. It
retains actual compiled route directions, provider and model/tool requirements,
package/library versions and schema digests through each target's descriptor.
Apple Silicon macOS, Linux x86-64, Intel macOS and Windows objectives remain
visible, including targets with no prepared asset. The narrow Linux/PDF/EPUB v1
candidate receipt, script and evidence are unchanged.

After native builds resume, capture the exact binary's descriptor into a separate
asset. A native build receipt must bind `target`, `source_revision`,
`binary_sha256` and `release_contract_sha256`. Prepare a
`renderflow.release-assets-request/v2` document with `version`, `source_revision`
and `targets` rows containing `target`, `binary`, `descriptor` and
`native_build_evidence` asset names. Then use:

```text
python scripts/release/cohort_assets.py prepare --asset-directory ASSETS --input REQUEST.json --source-directory SOURCE --output FRESH-MANIFEST.json
python scripts/release/cohort_assets.py verify --asset-directory ASSETS --input FRESH-MANIFEST.json
```

These commands inspect already supplied bytes only. They never build, execute a
binary, download, sign or publish. They check exact identities, source-schema
bindings during preparation, matching descriptor projection, retained targets and
missing coverage. Receipts and hashes are unauthenticated declarations until the
separate existing supply-chain/release gates verify them. Prepared assets remain
`prepared_bytes_only`; this inventory cannot assert publication or native
qualification. The existing release workflow does not automatically publish v2.
Actual target packaging, clean installation and native evidence remain under #419.

Flow can use the native CLI build/dry-run/graph-resume commands and the new thin
readback:

```text
renderflow publication-status --manifest RUN.json --manifest-sha256 EXACT_SHA256 --output-root OUTPUT
```

Readback delegates to the existing pinned receipt and confined publication file
verifier. It preserves run, validation and fidelity states and never converts
partial output, successful exit or file existence into complete domain acceptance.
Graph checkpoint resume remains separate from unsupported frozen-native resume.
Checksums do not grant creative approval, publication or publisher authenticity.

Manual source review and authored cases only in this checkpoint. Tests, builds,
validators, CLI/SDK/native/CI execution and publication remain deferred. Flow #88
remains last and unchanged; known alignment cancellation/start-marker failure
remains unresolved, cause unproven.
