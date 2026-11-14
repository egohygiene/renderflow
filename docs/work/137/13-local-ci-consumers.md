# Local CI consumer checkpoint — November 13, 2026

Relates to [Flow #137](https://github.com/egohygiene/flow/issues/137).
The maintainer selected focused local builds and smoke checks after the merged
source sprint. This consumer reuses reviewed Realm `75c1b2e1475b972a0ce1c8594e8daba2e1cf8686` and its
`realm.local-ci/v1` contract; it adds no new runner or hosted workflow.

The manifest requires the declared-compiler library check (1.94.0), the two
existing bounded budget commands, and a pinned act source-check fixture.
The other workflow/native/provider/distribution scopes are explicit unsupported
rows. Existing hosted CI/release conditions and publication authority are not
rewritten. Repository-specific setup and exact commands are in
[the local guide](../../local-ci.md).

## Observations

Manifest planning and the existing synthetic budget regressions may be run as
source admission checks. Actual product compile, act completion and scope-specific
smoke evidence belong to the final integrated candidate's local report. A planned
or authored command is not execution. This packet does not grant release or
whole-matrix qualification. Older dated source observations retain their scope;
Flow #88's original final-pass ledger remains unchanged and last.

## Executed source admission

The exact pinned Realm runner accepts this manifest and emits a plan with
`execution: not-performed`. All 12 existing budget regressions pass, and the
existing budget source planner succeeds. Actionlint 1.7.12 accepts both the new
act fixture and the existing workflow wrapper/scope regressions. Task YAML is
parsed with duplicate keys refused; all three aliases are distinct. The selected
compiler is compared with the owning Cargo minimum (Renderflow's workspace
minimum). These are source admission observations, not a Rust or act execution
result. Final integrated execution is recorded separately.

## Executed selected Linux run

All four selected required checks pass at clean local source `7590d6c8476900e655c78cf8c6267d4d333e7850`,
tree `e15a9ef3dc164352337603c75f779589c9a6156c`. The manifest SHA-256 is
`da59ecd5fff7f0cee69bd7c58d921f0a7fadb9bbd88695e045625aff47bbd593`; the retained private report SHA-256 is
`18f7eee2c591233052b19d8b55ca88c5f22c5c7a71d401cf2785daa889576260`.

Rust 1.94's locked/offline library check, the 12-case budget suite, bounded
budget plan and actual act source job pass. The act job runs the same 12 source
cases in the pinned image; it does not build or qualify the product in that
container. Five unsupported rows remain explicit. `complete: true` covers the
four selected required entries, not the full CI or supported release matrix.

Execution uses reviewed Realm 75c1b2e, verified act 0.2.97, Docker 28.4.0 and
the explicit managed proxy/CA adapter. No production credentials, inner Docker
socket or hosted dispatch is selected. The separate #419 local candidate
checkpoint exercises actual captured CLI/package bytes and EPUB/PDF on Debian
13/glibc 2.41; Ubuntu 24.04/glibc 2.39 and native/publication claims remain
unqualified. Adding this dated observation changes no tested command or Rust
product source. Raw logs remain private.
