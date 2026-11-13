# Flow #137 checkpoint 11 — Renderflow, November 13, 2026

This is a packaging **source repair**, stacked after the checkpoint 4 draft.
No native build, installer, Snap, release publication or signing ran.

## Repaired workspace assumptions

- Snap preserves the CLI `rust-path` and now also explicitly builds the locked
  `renderflow-cli` / `renderflow` binary target from its actual member manifest.
  Its override installs the selected output from an explicit target directory,
  avoiding the virtual root manifest fallback. The toolchain is pinned to
  1.94.0. Snap adopts the version derived by Cargo metadata from the inheriting
  CLI package; `version: git` no longer supplies a separate version authority.
- The portable-installer smoke source now supplies the required exact Cargo
  version and a checksum record containing the asset basename. The old source
  omitted `RENDERFLOW_VERSION` and wrote an absolute path in the checksum file,
  so the installer would refuse before a useful smoke observation. Temporary
  assets use the job's own runner directory; installed and Snap version output
  must equal the actual source candidate version.
- Task and AUR packaging entry points select the CLI package/binary explicitly.
  The generic debug build remains an all-workspace build. The stable AUR
  template's source directory uses the exact prerelease source tag instead of
  its normalized distribution package version. Its placeholder checksum stays
  in place: this is still an unpublished, unqualified template.
- The release version guard derives the current package set from the workspace
  manifest, including the optional API member, instead of a fixed three-package
  list. The workspace Cargo version remains the sole product version authority.
  Native cohort preparation uses the same explicit 1.94.0 toolchain selection.

The already authored typed `u32::MAX` schema bounds are preserved unchanged.
No runtime/media/approval contracts, MSRV, dependency requirements, authority
checks or publication gates are relaxed by this checkpoint.

## Bounded source observations

Executed `python3 -B scripts/check-packaging.py check` and `version` exited 0;
the current workspace version is `0.3.0-rc.1`. The adjoining JSON reports only
source metadata and explicitly says that native build, installer and Snap were
not observed. The helper reads TOML and the selected packaging declarations;
it invokes no Cargo process, package builder, network or installer.

Four edited YAML documents and three Python sources were parsed successfully.
`sh -n` accepted the existing installer source and the Snap override body;
`bash -n` accepted both AUR templates. These are source syntax observations,
not execution of any command contained in those scripts. The installer itself
was not changed or executed. Upstream cargo-generate-rpm source was read to
confirm that its workspace `--package` option exists; the tool was not installed
or executed.

Six focused source-declaration regression cases are authored in
`scripts/test_check_packaging.py` and are **unrun**. They cover the virtual-root
mistake, CLI binary identity, the previously omitted API member's lock version,
Snap version authority and the prerelease archive root. This checkpoint makes
no claim that these future tests or full release checks passed.

## Acceptance still required

After separately authorizing compilation and native package execution, run the
locked CLI build, exact-version/checksum installer smoke, Snap build/install
smoke, and relevant Debian/RPM/AUR checks on their actual supported hosts.
Preserve checksums, exact source version, immutable release policy and existing
native evidence requirements. A successful static declaration check cannot
qualify a platform, installer, Snap Store/AUR channel or immutable provider.

No existing CI job is dispatched or rerun by this source delivery. The new
`package:check` alias is read-only; source CI wiring is left for review with the
separate checkpoint 12 cost/tier policy. Flow #88 remains unchanged and last.
