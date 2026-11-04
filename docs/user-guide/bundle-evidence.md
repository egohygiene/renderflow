# Verify a forest's published files

`RunManifest::published_targets()` joins existing native planning/publication
records without I/O. The additive `RunManifest::verify_published_targets` API
also observes each declared publication file within an explicit local bundle
root. The CLI uses the same implementation:

```bash
renderflow bundle verify \
  --manifest "release/run-manifest.json" \
  --manifest-sha256 "<observed exact receipt SHA-256>" \
  --bundle-root "release"
```

Replace the digest placeholder with the exact receipt selected by the caller.
The receipt filename is illustrative; use the actual saved native run manifest.
The manifest must be `renderflow.run/v1` with explicit artifact-forest execution
evidence. Standalone presentation, speech and other family receipts have their
own schemas and are not interchangeable with this native run contract.

## What is observed

The receipt is read as a bounded regular file and its exact bytes are checked
against the supplied SHA-256 before parsing. The digest identifies the selected
bytes; it does not authenticate the publisher. The existing structural join
selects actual publications, preserving distinct same-content destinations and
retained outputs in partial/failed/cancelled runs. Neither planned targets nor
the full inventory substitute for these publication records.

Each selected record must use a lowercase SHA-256 digest and a `bundle:` locator.
The locator is a relative POSIX path: no absolute paths, dot/parent components,
empty components, backslashes, encoded separators, URI fragments/queries,
controls or drive/scheme prefixes. Components are observed individually;
symlinks and non-regular payloads are refused. Spaces and Unicode names remain
ordinary path text. The explicit caller-selected root is authoritative;
`artifact_manifest.output_dir` and `outputs` never select host files to open.

Payloads are streamed into a hasher under byte/work/cancellation bounds. Actual
byte size and SHA-256 must match the record. Open/named file metadata and the
root are observed again to reject persistent changes during the operation.
Unix device/inode identity participates in these checks. Other platforms retain
the available regular-file/path/size/time checks without inventing a file ID.
The API does not certify a hostile concurrently modified filesystem or keep a
file unchanged after verification. Re-observe at the subsequent point of use.

The JSON result is `renderflow.publication-verification/v1`. It carries the
receipt digest, original run state, verified count, `has_publications` and one
subject per observed destination, retaining original validation/fidelity/source
identity states. Native loss/review metadata remains in the unchanged receipt.
Hashing never upgrades skipped validation, fidelity, human review, provider
qualification or approval to publish. A planned run with no publications
returns zero subjects and remains planned; zero subjects do not prove delivery.
Malformed legacy/mismatched records and missing/tampered payloads fail explicitly.

## SDK

```rust,ignore
use renderflow::evidence::verification::{PinnedRunReceipt, VerificationLimits};
use renderflow::process::ProcessCancellationToken;
use std::path::Path;

# fn example() -> Result<(), Box<dyn std::error::Error>> {
let limits = VerificationLimits::default();
let cancellation = ProcessCancellationToken::new();
let receipt = PinnedRunReceipt::read(
    Path::new("release/run-manifest.json"),
    "<exact lowercase SHA-256>",
    &limits,
    cancellation.clone(),
)?;
let subjects = receipt.manifest().verify_published_targets(
    Path::new("release"), &limits, cancellation,
)?;
for subject in subjects {
    let native = subject.publication();
    let observed = subject.observation();
    // Retain native.artifact.metadata and independently decide review/use policy.
    println!("{} {}", native.branch.target_id, observed.sha256);
}
# Ok(()) }
```

The snippet is illustrative and unrun. Consumers that already own a native model
can use the manifest method directly; it cannot identify the original JSON bytes
without a separate pinned receipt. `PinnedRunReceipt` retains those exact bytes
alongside its parsed model. All returned file observations are read-only facts
at observation time, not signed credentials or filesystem capabilities.

## Limits and evidence still owed

Defaults are a 16 MiB receipt, 8,192 publications, 2 GiB per file, 8 GiB aggregate
read work, 64 path components and a 120-second cooperative timeout. Every
destination counts even if content IDs/digests repeat. The CLI exposes file,
total/count and timeout limits; the SDK additionally configures receipt/path
bounds. Receipt parsing and payload verification have separate timeout phases.
Checks between I/O operations do not interrupt a blocked filesystem syscall.
Ctrl-C cancels the CLI operation. No provider, network, output creation or upload
is part of verification.

This checks published subjects only. It does not automatically resolve
`artifact-store:` canonical companions, saved speech evidence, or other sidecar
dependencies into a portable handoff. Preserve those references and the original
store until the corresponding export/consumer contract is satisfied. It also
does not validate every field in the complete native receipt or prove rendering
quality, source authenticity or release readiness.

The source and seven meaningful filesystem regression cases are authored and
unrun. No product tests, compiler/build/Cargo resolution, lint/format/schema/docs
validators, CLI/native/provider execution or CI ran for this checkpoint. #430
remains open for real CLI/SDK/native integration, portability and release proof;
#458/#419 retain their baseline/immutable release gates.
