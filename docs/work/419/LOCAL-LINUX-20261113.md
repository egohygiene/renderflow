# Local Linux candidate — November 13, 2026

The maintainer authorized a focused local CLI/package candidate checkpoint
under [Renderflow #419](https://github.com/egohygiene/renderflow/issues/419),
after the source merge sweep. No publication, signing, immutable tag or broad
native-matrix execution is authorized by this checkpoint.

## Source and repairs

The reviewed main input is `575abb9c1802e141c219b12488f3f15ea992f4f4`, source
tree `91b5f9ec4eedaeb63eab73fdb30878fbf8feb0ff`. An isolated worktree with
that exact tree was used; the containing PR identifies the proposed source.

The first Rust 1.94 locked release build failed with `E0433`: the Ollama
connectivity helper referenced `Duration` without importing it. This checkpoint
adds that existing type import and removes the unused `RunManifest` import in
portable evidence. It changes no AI request, native/provider authority or
release gate.

Review also reproduced a clean-room verifier failure: copying only
`release_assets.py` cannot import `native_cohort` and `cohort_assets`. The
workflow now stages and mounts the closed three-file owner bundle. Parser
source-directory defaults remain equivalent inside the checkout and tolerate
the shallower staged layout. Manual triggers, permissions, prerequisite job
admission, signing, publication and release-asset verification remain intact.

The new `local-smoke` command reuses the existing isolated release fixtures and
records captured binary/output hashes and the actual host. Its separate local
receipt carries explicit false source/release authentication, supported-host,
native-matrix and publication fields. The production release manifest is not
fabricated from this local smoke.

## Executed observations

- `cargo +1.94.0 build --locked --release --package renderflow-cli --bin
  renderflow --target x86_64-unknown-linux-gnu` succeeds after the import fix.
  Cargo jobs are limited to two and output is in a task-private target root.
- The executable is 19,979,800 bytes, SHA-256
  `e93a71583a3c3530871c292e56fc21fac5e836fec6dafd1dc14eb849f542dc3c`.
  Actual version is `renderflow 0.3.0-rc.1`.
- Actual host: Debian 13 x86_64 / glibc 2.41. The binary's maximum referenced
  GLIBC symbol version is 2.39. This is a local host observation, not proof on
  the declared Ubuntu 24.04/glibc 2.39 release host.
- Real smoke passes from both the source tool and the staged verifier in an
  empty working directory: isolated copied CLI; version/help/doctor; spec
  validation, planning/dry-run; two-page native EPUB with fixed-layout and
  provenance inspection; two-page PDF with exact `img2pdf 0.6.3`, Renderflow
  and `pikepdf` inspections; missing-config and stale-source failure exits.
  Original synthetic PDF input bytes remain unchanged before the intentional
  stale-input refusal trial.
- Both runs produce the same inspected EPUB SHA-256
  `b4da7dcc5300932fb9ed638a41001bf7cb38146846dd7b0e614f5cf84556e3e5`
  and PDF SHA-256
  `0ed5830ed858876a4bf2a7dc690181a1540a4995e797817bfbb5df952b522416`.
  This checks these two synthetic outputs on this host, not general rendering
  or cross-host reproducibility.
- The focused Python release suite passes 15 cases, including six new local
  candidate/staging cases. The staged-CLI regression first reproduces the old
  one-file import failure, then exercises the complete copied bundle without
  checkout imports. No test binary or fixture metadata is release evidence.
- The pinned Rustfmt 1.94 check and Actionlint wrapper pass. Packaging source
  declarations remain coherent. No lockfile change is required by the build.
- Locked dependency fetch and offline Cargo metadata succeed. Local preparation
  stages the copied ELF binary, compiled release-contract descriptor, SPDX
  dependency inventory, notices and three independently verified SHA-256
  sidecars. The package has seven payload files plus a local-only inventory;
  no release manifest, tag, signature, attestation or upload is manufactured.

## Retained gates

One existing release-build warning remains: the crate-private speech
`PcmWavInspection.frame_count` field is unread in production. This checkpoint
does not run or claim strict Clippy; warning-clean release qualification stays
open rather than adding an allowance or unrelated behavior.

No full Rust suite, actual Ubuntu clean-room container, native macOS/Windows,
Snap/package-manager install, real media/model execution, release attestation,
hosted workflow dispatch or publication ran. The exact production release
checks, two isolated native build requirement, immutable tag/asset checks and
downloaded-asset installation criteria remain under #419. Flow's #52 adapter
still requires the actual immutable release. Flow #88 remains unchanged and
last, with its original ledger and prior known failure limits.
