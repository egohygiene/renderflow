# Installation and distribution status

`v0.3.0-rc.1` is the proposed first integration candidate. Until its immutable
tag, GitHub prerelease, and verified assets exist, there is no supported
download for this candidate. The historical `v0.2.1` tag was unsigned and
never accompanied by a GitHub release. Do not install it as the verified
integration candidate.

## Supported target and channels

| Route | Candidate status | Boundary |
| --- | --- | --- |
| GitHub Release, Ubuntu 24.04 x86_64 GNU binary | Planned for first verified candidate | `renderflow-x86_64-unknown-linux-gnu`, glibc 2.39 or newer, exact tag and SHA-256 required; other distribution baselines unverified |
| Rust source checkout | Local development | Rust 1.94+ and the locked workspace dependencies; not a downloaded binary smoke test |
| macOS, Windows, Linux ARM/musl/other binary targets | Unverified | Existing build configuration alone does not establish supported artifacts |
| crates.io, Homebrew, Scoop, Chocolatey, Snap, AUR, Debian/RPM | Unpublished or unverified for this candidate | Checked-in packaging files and release jobs are not proof of working distribution |

This matrix describes the intended `v0.3.0-rc.1` scope **before publication**.
For the post-publication state, inspect the [actual GitHub release](https://github.com/egohygiene/renderflow/releases)
and its [release evidence](../release-candidate.md). Do not assume that
`/releases/latest` resolves to a prerelease: pin the exact tag and digest.

## Install the candidate after publication

First verify that `v0.3.0-rc.1` appears as an immutable GitHub prerelease with
the Ubuntu 24.04 x86_64 GNU binary, its `.sha256`, and
`renderflow-release-manifest-v1.json` with its `.sha256`. Download the assets
from that exact tag and compare the binary hash to the checksum file and
manifest. A successful comparison proves byte identity with
the published digest, not the safety of an unreviewed upstream binary.

The first-party installer can download and verify the matching per-asset
SHA-256 before replacing a local executable. Fetch the script from the pinned
tag so later `main` edits do not change this installation procedure:

```bash
curl --fail --show-error --silent --location \
  --output "renderflow-install.sh" \
  "https://raw.githubusercontent.com/egohygiene/renderflow/v0.3.0-rc.1/scripts/install.sh"
RENDERFLOW_VERSION="v0.3.0-rc.1" \
RENDERFLOW_INSTALL_DIR="$HOME/.local/bin" \
  sh "renderflow-install.sh"
"$HOME/.local/bin/renderflow" --version
```

This route is verified only on Ubuntu 24.04 x86_64 with GNU libc 2.39 or newer;
other distribution and libc baselines have not been verified. The installer cannot
establish SBOM, provenance, or signing status on its own; review those
separately on the release. An absent asset, checksum, or manifest is a failed
install gate, not evidence of a supported platform. The script requires an
exact `RENDERFLOW_VERSION`; mutable `latest` is refused.

## Build from source for development

Use an exact reviewed checkout, Rust 1.94 or newer, and its committed lockfile:

```bash
cargo install --locked --path "crates/renderflow-cli"
renderflow --version
renderflow --help
renderflow doctor
```

Building from source does not substitute for an independently verified release
asset. Avoid treating the checked-in Homebrew/Scoop/Chocolatey/AUR templates as
installable release metadata while they retain placeholder checksums or refer
to a tag without a published package.

## External providers

The CLI has several exact and optional provider routes. The binary does not
bundle Pandoc, Tectonic, FFmpeg, `img2pdf`, EPUBCheck, HandBrakeCLI, or other
host tools. Install only providers needed for the chosen action and inspect
`renderflow doctor` or `renderflow tools list` on that host.

| Route | Tool boundary |
| --- | --- |
| Standard document rendering | Pandoc; PDF variants may additionally require Tectonic or TeX components |
| Ordered print-interior PDF | Explicit local `img2pdf` 0.6.3, plus bounded PNG/JPEG and page-geometry contract |
| Ordered fixed-layout EPUB | Native packager for the declared PNG/JPEG route; optional EPUBCheck v5 supplies separate external evidence |
| Media conversions | FFmpeg or the selected external adapter, when that route is used |

An installed CLI is not proof that every format, provider, reading system,
printer, or retailer is supported. See the [release compatibility contract](../release-candidate.md)
and each route's user guide before running it on personal files.

## Upgrade, rollback, and compromised releases

Use exact tags and recorded digests for upgrades. Keep the last verified
binary and manifest outside the installation path; a rollback restores those
same bytes after rechecking their digest, rather than moving an old tag. If a
release is suspected compromised, stop distribution, quarantine its digest in
downstream lockfiles, publish an advisory and revoked status, investigate the
tag and attestation, then issue a newly numbered, reviewed replacement. Never
retag or silently replace a published asset. The
[release-candidate guide](../release-candidate.md) has the response checklist.
