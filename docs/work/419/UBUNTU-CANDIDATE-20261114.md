# Ubuntu 24.04 local package candidate — November 14, 2026

Relates to [Renderflow #419](https://github.com/egohygiene/renderflow/issues/419).
This checkpoint builds and executes the Linux CLI in the declared Ubuntu 24.04
userland. It adds a reproducible offline driver, not a release authorization.

## Selected execution

The source is revision `c8f2b6896ec82c12e75ad6b8bcf7e4b319602db2`, tree
`e3a7b4ab29930dd55154a2b9a71e0f445454a08d`. This includes the bounded standalone
subtitle capture and checked audio profiles. The driver and this guide are
added after that code build; the receipt identifies the exact driver bytes.

The image is pinned to:

```text
ghcr.io/catthehacker/ubuntu:act-24.04@sha256:19619a010b204f44c5340bff7d39b68551109fe6a56a53f75201ea91e05d4270
```

The retained receipt records its local image ID, Ubuntu version, glibc 2.39,
Rust 1.94.0, Cargo, GCC and Python versions, and the shared host kernel.
The actual release build uses `cargo build --frozen --release --package
renderflow-cli --bin renderflow --target x86_64-unknown-linux-gnu --jobs 2`.
Source, the selected Rust toolchain and public Cargo registry are read only.
The target cache is private to this Ubuntu trial. Two build jobs, two
CPUs, 6 GiB memory, 256 processes and a 1 GiB temporary filesystem bound each
container. Its root is read only, networking is disabled, capabilities are
dropped, and no Docker socket or credential directory is mounted.

The initial empty-target build completed in 7m42s. Its
[retained build log](evidence/20261114/ubuntu-fresh-release-build.log) has SHA-256
`7c0799824e81bab78f4a76acbb49319b6f7b2f421386cf19bad37f12bb20810e`.
This is a separate task-runner observation, not an authenticated build
attestation. The final driver execution explicitly reuses that private cache,
records `fresh_target_at_start: false` and
`cache_origin: caller_supplied_existing_cache`, and grants zero independent
rebuilds. The driver does not authenticate earlier caller cache contents.

Independent `readelf --version-info` inspection checks that the resulting ELF
is x86-64 and requires no GLIBC symbol newer than 2.39. The compiled
`release-contract`, frozen offline Cargo metadata, raw-binary checksum,
workspace-lock SPDX inventory and dependency notices are generated inside
Ubuntu. The SPDX inventory includes optional and development dependencies;
it does not claim exact binary composition.

The observed maximum GLIBC symbol is `2.39`. The 19,965,256-byte CLI has
SHA-256 `b0a55e600e7888428b6971b99dc4566eac285cebf7338b99d6ed75415d1a3a56`.
The source build emitted only the existing unused `frame_count` warning in
`speech/provider.rs`.

The existing release owner stages exactly its three verifier modules with
host Python. Their names, bytes and hashes must match the clean source.
Their actual `local-smoke` execution runs inside the same pinned Ubuntu image,
without the source checkout, toolchain or Cargo cache. It executes captured
candidate bytes and generated two-page documents: version/help/doctor,
planning, native fixed-layout EPUB and provenance inspection, exact
img2pdf 0.6.3 PDF conversion, pikepdf inspection, source preservation and
selected failure paths. Public manylinux provider dependencies are mounted
read only; the `img2pdf` wrapper uses Ubuntu's Python, rather than a host venv
interpreter. The observed providers are img2pdf 0.6.3, pikepdf 10.18.1 and
qpdf 12.4.2. No media, account, catalog or private source is supplied.

The first smoke attempt failed at executing the captured binary because the
task's temporary filesystem was non-executable (`EACCES`). The driver now
explicitly permits execution on that bounded private tmpfs while retaining
`nosuid,nodev`. This was a container setup failure; the subsequent real smoke
passed. The [setup failure log](evidence/20261114/ubuntu-smoke-noexec-setup-failure.log)
is retained separately from product results.

## Reproduction

Prepare the exact image, Rust 1.94.0 toolchain, public Cargo registry and public
Python provider packages separately. The driver neither downloads nor installs
them. Provide canonical absolute directories, an existing empty private target
directory and a fresh work directory. Writable target/work roots must be
disjoint from each other and from source, toolchain, registry and providers:

```sh
python3 -B scripts/release/ubuntu_candidate.py \
  --source-dir /absolute/clean/renderflow-checkout \
  --toolchain /absolute/rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu \
  --registry /absolute/public-cargo-home/registry \
  --provider-site /absolute/public-python-site-packages \
  --target-directory /absolute/private/ubuntu-target \
  --work-directory /absolute/private/new-candidate-run
```

The source must be clean before and after execution. Only the `registry`
subdirectory is admitted; never supply an entire Cargo home containing
credentials. The selected img2pdf version is mandatory. The generated report
records the pikepdf and qpdf versions as well. Reuse of a target cache remains
one build environment; it requires explicit `--reuse-target-cache` and is not
independent reproducibility evidence. The raw `proof.json` retains the local
command ledger. `portable-summary.json` includes the explicit result fields,
in-container commands and log hashes without copying local mount paths.

Focused driver admission checks run without Docker or product execution:

```sh
python3 -B -m unittest discover -s scripts/release/tests -p test_ubuntu_candidate.py
```

These cover nonempty-cache refusal, writable mount aliases in both directions,
and rejection of extra files, symbolic links and forged checksums in the closed
seven-file candidate bundle.

## Evidence and limits

The executed receipt is [retained here](evidence/20261114/ubuntu-candidate.json).
It is the allowlisted portable summary of the completed final execution.
The full local logs, prepared assets and private cache remain in the task's
private artifact directory. No binary, release manifest, attestation or
signature is committed.

This demonstrates the selected build and document package smoke in Ubuntu's
24.04 userland on the workspace's host kernel. It does not qualify a complete
native Ubuntu host, a GitHub-hosted runner, every format/provider, other
platforms, real publication quality, or a published download. There is one
isolated build environment. Independent rebuilds, hosted checks, release
authentication/signing, installer delivery, full native matrices and human
release review remain separate gates. All release, full-matrix and publication
authority fields stay false.
