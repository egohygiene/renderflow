# Native cohort distribution preparation

The existing `scripts/release/release_assets.py` owner now exposes an additive
native v2 path. The older v1 Linux candidate workflow/receipt/installer remain
available with their exact original qualification and signature gates. The new
path prepares bytes and prints a publication handoff; it never publishes itself.

## Native parts

On an explicitly prepared native Linux x86-64 GNU, Apple Silicon or Intel Mac
host, `cohort-build` requires the exact clean source/version, installed Rust 1.94,
prepared Cargo cache and an existing parent for fresh separate work/output roots.
It refuses cross-host preparation, builds twice with frozen offline dependencies,
compares binary and compiled descriptor bytes, checks the executable CPU/container
and version, then reuses existing SBOM and notice preparation. It installs no
toolchain/dependency/model and never implies external-tool or format qualification.

```text
python scripts/release/release_assets.py cohort-build --target TARGET --version REVIEWED_VERSION --commit FULL_SOURCE_SHA --source-dir SOURCE --work-directory FRESH_WORK --artifact-dir FRESH_NATIVE_PART
```

Retain each `native-assets.json`, exact binary, compiled descriptor, native build
receipt, SPDX workspace inventory, notices and checksums. A
`renderflow.native-parts/v1` request supplies `version`, `source_revision` and
`parts` entries with explicit `directory` paths. Assembly checks exact membership
and every asset digest before creating one fresh flat v2 asset directory.

```text
python scripts/release/release_assets.py cohort-assemble --parts-request PARTS.json --source-dir SOURCE --artifact-dir FRESH_COHORT
python scripts/release/release_assets.py cohort-verify --artifact-dir COHORT --manifest MANIFEST.json --manifest-sha256 EXACT_SHA256
python scripts/release/release_assets.py cohort-handoff --artifact-dir COHORT --manifest MANIFEST.json --manifest-sha256 EXACT_SHA256
```

The manifest preserves compiled routes/contracts/provider prerequisites, target
support assets and missing target objectives, including Windows. Its state remains
`prepared_bytes_only`; native builds and supplied hashes do not establish a public
release, publisher authenticity, clean installation or complete native coverage.
The handoff prints a draft-release argv for review and states the outstanding
existing native, supply-chain and release gates. It does not dispatch workflows,
sign, create a tag/release or satisfy the v1 workflow's gates. A successor release
needs independently completed evidence and an explicit maintainer publication.

## Pinned offline candidate installation

The existing `scripts/install.sh` has an explicit v2 opt-in. Supply
`RENDERFLOW_COHORT_MANIFEST`, its independently selected
`RENDERFLOW_COHORT_SHA256`, `RENDERFLOW_COHORT_ASSETS`, a fresh
`RENDERFLOW_INSTALL_DIR` and `RENDERFLOW_ACKNOWLEDGE_UNQUALIFIED_CANDIDATE=1`.
It delegates `cohort-install`; no download or provider process occurs. The old
exact-tag v1 installer retains its behavior when this opt-in is absent.

The selected manifest and all supplied asset/descriptor/support/build bindings
are reobserved; only the current native host's matching binary and descriptor
are copied into the new owned directory. Existing directories are never adopted
or overwritten. `renderflow-install.json` records exact installed hashes and
explicitly unobserved release authentication/native qualification. This is a byte
installation, not evidence that external tools, libc/OS compatibility, models or
every source-declared route can run. Use current native doctor/profile qualification
before claiming a supported workflow. Flow's independent installer can consume
the same immutable asset inventory without importing Renderflow source.

Source authoring/manual review only. Native build/preparation, authored cases,
tests/compiler/validators/CLI/SDK/installer/CI and release publication were not run.
Issue #419 remains open; Flow #88 remains last and unchanged. Known alignment
cancellation/start-marker failure remains unresolved, cause unproven.
