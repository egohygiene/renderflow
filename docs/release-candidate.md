# Integration candidate and release evidence

Renderflow's first Flow integration candidate is proposed as `v0.3.0-rc.1`.
This document describes the contract and publication gate; the presence of a
version in source is not a published release. The maintainer merges the reviewed
version PR, waits for required checks on the exact commit, then explicitly
creates a new annotated tag at that commit and dispatches release work with
the tag ref and full `expected_commit` SHA. Tag pushes alone do not publish.
The release workflow refuses to proceed if the tag, checkout, workflow event,
and current `main` no longer point at that commit.
The historical unsigned `v0.2.1` tag at
`96d1e55231c30449b12f4849d7cdda63853abc68` had no GitHub release. It
must not be moved, reused, or presented as verified release evidence.

## Scope and compatibility

The intended first downloaded binary is
`renderflow-x86_64-unknown-linux-gnu` on Ubuntu 24.04 x86_64 (GNU libc 2.39
or newer). Other distribution and libc baselines are unverified. Its
support begins only when a published prerelease provides the binary, matching
SHA-256, clean-install result, and immutable release manifest. Linux
ARM/musl/i686, macOS, and Windows remain unverified for this candidate.
Container, crates.io, Homebrew, Scoop, Chocolatey, Snap, AUR, `.deb`, and `.rpm`
channels have no candidate publication claim. Checked-in build and packaging
recipes do not constitute package-manager availability.

| Surface | Candidate contract | Boundary |
| --- | --- | --- |
| CLI | `renderflow` `0.3.0-rc.1` binary and its exact digest | The release asset's `--version`, `--help`, `doctor`, planning, dry-run, fixture run, and failure exits need downloaded-asset evidence on Ubuntu 24.04 x86_64 GNU |
| Source build | Cargo workspace `0.3.0-rc.1`, minimum Rust 1.94 | Source-checkout tests are distinct from release installation |
| Spec | `renderflow/v2` | Explicit ordered collection and exact targets; not arbitrary format composition |
| Execution | `renderflow.run/v1`, `renderflow.artifact-manifest/v1`, `flow.artifact/v1` | Flow consumes released artifacts and manifests, not Renderflow source |
| Provider | `renderflow.provider/v1`; tool registry `renderflow.tool-registry/v1` | The selected tool/version and capability must appear in each plan/run |
| Plugin SDK | `renderflow.plugin/v2alpha1` | Alpha contract; semver of the crate does not imply stable third-party ABI |
| Ordered print PDF | `publication.generate.pdf.interior` | Homogeneous PNG/JPEG; trusted local `img2pdf` 0.6.3; exact geometry and independent PDF inspection; no printer acceptance |
| Fixed EPUB | `ebook.generate.epub.fixed-layout` | Homogeneous PNG/JPEG; native bounded packager; `renderflow.ebook-evidence/v1` inspection; optional EPUBCheck v5 evidence separate from native result |
| Other tools | Route-specific external providers | Pandoc, Tectonic, FFmpeg, HandBrakeCLI, AI runtimes, etc. are neither bundled nor globally guaranteed by binary installation |

Fixed-layout KEPUB generation, SVG page input, complete accessibility,
retailer approval, physical print proof, publication approval, and arbitrary
real-media execution are outside this candidate. A provider's availability
on one host does not turn an experimental or unselected adapter into a
supported release route. See [ordered collections](user-guide/ordered-collections.md),
[print PDF](user-guide/print-interior-pdf.md), and
[fixed EPUB](user-guide/fixed-layout-epub.md) for limits and refusal behavior.

## Publication setup and independent verification

Before dispatch, enable the repository's immutable-releases setting and provide
`RELEASE_SETTINGS_READ_TOKEN` as a repository secret. This fine-grained token
needs **Administration: read** for the repository, solely to read the release
immutability setting; the default `GITHUB_TOKEN` does not have that permission.
The workflow fails closed when the setting cannot be observed as enabled. Do
not place the token in artifacts, manifests, logs, or command examples.

A release is ready only when its tag, checkout, current `main`, binary, and
release manifest bind the **same** reviewed commit. Release gates are:

1. Main CI, docs, and one manually dispatched conformance run with both fast
   and maximal jobs are green on the exact reviewed commit. The latest
   scheduled maximal conformance run must be green **and no older than eight
   days** as a separate health gate; its SHA can predate the release commit.
   The workflow logs that run's ID, SHA, completion time, and age without
   treating it as exact-commit proof. Source and release verification run
   again from the exact tag. A failing or stale gate blocks publication rather
   than being promoted through an undocumented exception.
2. The tag is annotated and resolves to the reviewed commit. Release work is
   explicitly dispatched with `--ref "v0.3.0-rc.1"` and the full
   `--field "expected_commit=<reviewed-40-character-sha>"`; it stages
   candidate artifacts without modifying `main` or replacing an existing
   tag/asset.
3. The downloaded Linux binary's SHA-256 matches its checksum and the
   machine-readable `renderflow-release-manifest-v1.json` and its `.sha256`.
   The `renderflow.release-manifest/v1` schema binds version, tag, commit,
   target, verified `ubuntu-24.04-x86_64` host and GNU libc 2.39 minimum,
   artifact name, digest, exact run/artifact/Flow/tool-registry contracts,
   selected providers, and status.
4. `renderflow-sbom.spdx.json` and `THIRD_PARTY_NOTICES.txt` (both with
   checksums) identify dependencies and license notices. GitHub/Sigstore
   `renderflow-attestation.json` and `renderflow-sbom-attestation.json`
   attest to the binary and SBOM. The first candidate has **no separate
   maintainer signature on the binary or tag**. Verify the attestations
   independently; a checksum is not a signature, and attestation is not a
   package-manager publication or platform code signature.
5. A fresh Ubuntu 24.04 Docker image is built from a Dockerfile-only context,
   without a repository checkout or local binary. Provider installation during
   image construction pins `img2pdf` 0.6.3. Before publication, the **draft's
   downloaded assets** and a copied verifier script are mounted read-only;
   the actual smoke runs with network disabled and a temporary writable
   directory. It checks `--version`, help, doctor, spec validation, canonical
   planning, dry-run, deterministic synthetic fixed-layout EPUB and two-page
   print-interior PDF execution, independent output inspection, and expected
   nonzero stale-input refusal. The EPUB packager is native; optional
   EPUBCheck conformance remains separate.
6. The release page lists only assets actually uploaded and verified.
   Package-manager channels remain unavailable until separately installed and
   tested from their published distribution endpoints.

Flow should pin the exact version and binary digest from the release manifest;
never infer support solely from a tag name, README table, or mutable `latest`
URL. Staged and downloaded **draft** checks happen before publication and fail
closed, with candidate evidence retained for investigation. The postpublish
`gh release verify` and `gh release verify-asset` checks confirm the resulting
immutable release but cannot undo or block the publication that already
occurred. If either detects a problem, Flow must refuse that version/digest,
halt further distribution, preserve evidence, and follow the incident response
below; do not overwrite the published asset or retarget its tag.

## Rollback and compromised-release response

For a faulty but uncompromised candidate, stop selecting its digest in Flow,
retain the incident's tag/commit/artifact evidence, and pin the last previously
verified version by digest. Produce a newly numbered replacement after review;
do not retarget an old tag or overwrite a GitHub asset. If no prior verified
release exists, disable the provider route until a replacement is verified.

For suspected compromise, stop downloads and integration immediately, mark the
specific version and digest revoked in downstream locks and advisory text,
preserve release/build logs and attestations, investigate credentials and
build inputs, rotate affected secrets, and publish a security advisory or
incident notice describing affected assets and verification steps. Remove or
mark unsafe distribution links without changing historical tag/asset identity.
A replacement requires fresh review, rebuilt artifacts, clean-host evidence,
and a new immutable tag. A checksum alone proves equality to a published hash;
it does not establish trust when the publishing account or build is compromised.
