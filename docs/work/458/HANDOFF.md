# #458 — source corrections before qualification

## Draft checkpoint — 2026-10-26

Base: `9dac5428df38691b2922e09ea02d51b02a7f43eb`.
Branch: `codex/458-source-baseline`.
Issue: [#458](https://github.com/egohygiene/renderflow/issues/458).

The maintainer authorized continued source work with frequent draft checkpoints
while keeping executable qualification deferred. This is the bounded correction
of two recorded compiler errors and the optional Snap template's workspace
package selection. It does not restore a verified green baseline.

## Changes

- In `crates/renderflow-core/src/spec.rs`, the `speechAudio.stream_index` and
  `subtitles.stream_index` JSON schema maxima use `u32::MAX`. Their numeric
  meaning remains `4294967295`, with the existing zero minimum and nullability
  unchanged. The explicit unsigned value avoids the unsuffixed literal's
  inferred `i32` overflow reported in #458. The published JSON schema keeps its
  existing numeric values; no schema version or admission range changes.
- `snap/snapcraft.yaml` uses `rust-path: ["crates/renderflow-cli"]` while keeping
  the full workspace as its source. This selects the manifest that declares the
  `renderflow` binary and retains access to the sibling core and plugin crates.
  The Rust plugin's package path uses locked Cargo installation into the part
  root, which already places the executable at `bin/renderflow`; the obsolete
  root-level `renderflow` organize mapping is removed. The existing application
  command, core22 base, Rust 1.94 selection and unpublished/development status
  remain unchanged.

## Primary-source basis

Reviewed on 2026-10-26, without installing or executing Snapcraft:

- [Canonical's Rust plugin documentation](https://forum.snapcraft.io/t/the-rust-plugin/8588)
  explicitly describes `rust-path` as the workspace crate-path list for core22,
  supported by Snapcraft 7 or newer. This recipe supplies one path.
- [Snapcraft 8.14 Rust plugin reference](https://documentation.ubuntu.com/snapcraft/8.14/common/craft-parts/reference/plugins/rust_plugin/)
  describes that value as a package directory containing its manifest.
- [Craft Parts Rust plugin source at `a0cd61920fd07c1886ada28e2aebc99e7a6025e8`](https://github.com/canonical/craft-parts/blob/a0cd61920fd07c1886ada28e2aebc99e7a6025e8/craft_parts/plugins/rust_plugin.py)
  maps each package path to `cargo install` with `--locked`, `--path` and the
  part install root. This is inspected upstream source, not a claim about the
  exact plugin installed by a future packaging run.
- [Cargo's install reference](https://doc.rust-lang.org/cargo/commands/cargo-install.html)
  specifies the installation root's `bin` directory for executable targets.
  The repository's `crates/renderflow-cli/Cargo.toml` declares that executable.

## Evidence and remaining work

Only source/documentation reads and Git diff review were performed. No tests,
compiler/build, lint/formatting, schema validators, Cargo resolution, dependency
downloads, Snapcraft/installer/native/CLI execution or CI ran. No Actions were
dispatched or polled. The mechanical changes add no mirror tests.

Keep #458 open. The later authorized session must investigate the lock delta
with Cargo, retain a reviewed Cargo-produced update and explanation, address
the recorded lint findings without suppression, and run the exact repository
Rust 1.94/MSRV, locked build/test/Clippy, corpus and affected package gates.
The workflow and #419 remain authoritative for those commands and evidence.
In particular, the Snap template still needs an actual package/installed-command
check; this edit supplies no Snap Store or supported-distribution claim.

Preserve `--locked`, deny-warning policies, installer verification and required
workflow gates. #419 owns release qualification; #407 remains closed because it
concerns an older baseline. Creative/composition issue acceptance is unaffected.
