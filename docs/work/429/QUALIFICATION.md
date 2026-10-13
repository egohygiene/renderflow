# Creative toolchain qualification record

Status on 2026-10-13: **source contracts reviewed; runtime qualification deferred**.
The reviewed adapter base is merged commit
`8e0f5f4a6132364965cc31bbee9c3e72074dfd4b` (#446). This #429 checkpoint
reconciles its doctor contract and documentation. It does not select or certify
an installed toolchain.

| Evidence category | Current record |
| --- | --- |
| Observed native tools in this checkpoint | None |
| Qualified fixture toolchains | None recorded for this creative lane |
| Qualified release toolchains | None recorded for this creative lane |
| Qualified provider/platform combinations | None established by this checkpoint |
| Tests, compilation, lint, schema validation | Deferred by the maintainer |
| Native probes, conversions, OCR, speech, visual/native-application review | Deferred by the maintainer |
| CI | Not dispatched or polled |

These statements concern the current creative lane; they do not erase unrelated
historical test evidence. Source revisions in [PROVIDER_SOURCES.md](PROVIDER_SOURCES.md)
identify material inspected, not installed executable versions.

## Declared contracts to qualify

The table distinguishes generic discovery from the exact policy used by an
adapter. A compatible PATH observation neither chooses a configured executable
nor attests to its package, language data, model, fonts, or runtime dependencies.

| Provider | Generic discovery contract | Configured execution identity | Source of truth |
| --- | --- | --- | --- |
| scripttool | `>=0.0.10`, `<0.0.11` | Exact `0.0.10`, executable path and SHA-256; bounded FDX/OSF/Fade In imports only | [Import guide](../../user-guide/scripttool.md) |
| Node.js | Intentionally unprobed | Operator-selected exact `v22.x.y` or `v24.x.y`, executable path and SHA-256 | [PDF guide](../../user-guide/screenplay-pdf.md) |
| Afterwriting | Intentionally unprobed | Package `1.17.3`, package root and versioned tree digest including dependencies; Node pinned separately | [PDF guide](../../user-guide/screenplay-pdf.md) |
| Poppler pdftotext | Version observation; no qualified global release | Exact operator-selected three-component numeric version, executable path and SHA-256 | [Recovery guide](../../screenplay-recovery.md) |
| Poppler pdftoppm | Version observation; no qualified global release | Exact operator-selected three-component numeric version, executable path and SHA-256; used only with explicit OCR policy | [Recovery guide](../../screenplay-recovery.md) |
| Tesseract | Minimum `5.0.0` | Recovery requires exact operator-selected `5.x.y`, executable path and SHA-256, one language and pinned `.traineddata` path/SHA-256 | [Recovery guide](../../screenplay-recovery.md) |
| FFmpeg | Minimum `4.0.0` | Speech preparation pins executable bytes and records the actual version; discovery minimum is not a qualified release | [Speech guide](../../user-guide/speech.md) |
| FFprobe | Version observation without a global version range | Speech inspection/preparation pins executable bytes and records the actual version | [Speech guide](../../user-guide/speech.md) |
| whisper.cpp | Version observation without a qualified release | Executable path/SHA-256 and local model path/SHA-256; observed version recorded at execution, no expected-version configuration field | [Speech guide](../../user-guide/speech.md) |
| Renderflow screenplay recovery | Virtual in-process provider | Renderflow revision, `screenplay-body-lines/v1` heuristic and `renderflow.recovery-native-body/v1` export profile; every result needs human review | [Recovery handoff](../426/HANDOFF.md) |

The accepted Node majors, exact package declarations, and example numeric
versions describe authored policy, not measured compatibility. Distinct
Afterwriting source trees can declare the same package version. Executable
digests do not cover shared libraries or a complete installation environment.

## Evidence required when the test session resumes

1. Record the Renderflow revision/build, operating system, architecture,
   installation channel and package revision. For containers, record the image
   identity and provisioned dependencies. Use the
   [setup guide](../../user-guide/creative-toolchain-setup.md) to choose only the
   providers required for the fixture routes.
2. Record observed tool versions, exact executable/package digests and runtime
   dependencies. Record OCR data, speech models and fonts separately with their
   source, revision, digest, license and redistribution terms. No executable
   license automatically covers those assets.
3. Run the authored focused coverage and compilation, then validate real
   `renderflow.creative-doctor/v1` reports against the checked-in schema. Include
   missing, incompatible and intentionally unprobed tools; confirm that one
   unavailable provider does not suppress unrelated routes and plain doctor
   retains its existing scope.
4. Run bounded synthetic fixtures through each claimed configured route and
   retain the complete provenance/output bundle, command/configuration, status,
   refusals and observed losses. Include text-bearing, scanned and mixed PDFs,
   explicit OCR language data, native body opening/reparse, and human recovery
   review. Follow #424/#425/#426/#427/#428 for route-specific evidence; do not
   substitute a doctor report for those checks.
5. Record measured repeatability and platform limitations for the exact fixture
   toolchain. Keep failed or unattempted combinations explicit. Reuse a recorded
   toolchain for release qualification only after the #419 release gates pass.

The doctor observation fingerprint remains
`provider-discovery-observations/v1`. It is not an executable/model attestation,
an execution cache key, or proof of a reproducible fixture run. Keep #429's
acceptance criteria unchecked until the corresponding evidence exists.

## Next implementation boundary

#430 can now integrate the authored screenplay, recovery, transcript and page
families against the versioned forest/Flow contract. It must preserve configured
provider policies, review-required recovery lineage, unsupported-target
diagnostics, and the distinction between authored integration and demonstrated
end-to-end output. This record does not authorize native execution during the
current sprint or imply that a verified release already exists.
