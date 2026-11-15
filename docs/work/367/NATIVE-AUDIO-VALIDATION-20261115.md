# Native audio validation checkpoint — November 15, 2026

The actual public-source CLI now performs fresh, required native audio checks and
assigns fidelity from the observed artifact. Matching FLAC24 and ALAC16 earned
`Lossless` through exact original decoded PCM/sample/clock comparisons. MP3 and
AAC retained `Lossy`; a declared FLAC resample was valid and lossy. This is the
bounded validation gap left by the earlier authored-audio delivery checkpoint,
not a new conversion engine or a change to conservative planning/catalog scores.
See [the user guide](../../user-guide/native-audio-validation.md).

## Source, build and observations

The measured source is real public draft head
`67742e44ebdc4fec855c2af54d04bd2f2d2cf4e5`, tree
`9df023051d126b9af7af0675e8830c7aa7fa80d1`, based on main
`a2e33ed3f049fbd37b8807c2481504cd48a49cc6`. Later changes in this checkpoint
only add the observer, receipts and documentation. Rust 1.94.0 built the release
CLI with the existing private warm cache, two jobs, locked/offline dependencies
and target `x86_64-unknown-linux-gnu` on Debian 13. No independent rebuild or
Ubuntu/native-platform qualification is inferred.

| Captured subject | Bytes | SHA-256 |
| --- | ---: | --- |
| CLI | 20,151,256 | `391814f4bee5c12a989e54e8389db24e44c2abf11adf385d29d6f0e700d05ab2` |
| Compiled release contract | 372,295 | `199bf790b2d78e7706b1e24b3e0fdb189b8286eb231f8be1c8da503d82922f58` |
| Immutable build-time record | 1,794 | `f5155beaa25eeb5aefc8246eb8ec5fbedc24e8f424259d1e1c9658d21905fd60` |

The build-time record truthfully said runtime was pending when captured. It is
unchanged; the later passed observations are separate receipts:

- [Source/build/check bindings](native-audio-validation-bindings-20261115.json).
- [Actual CLI evidence](native-audio-cli-observed-20261115.json): 15 canonical
  build invocations and 25 independent feature/probe/decode commands, 40 commands
  total, with 142 retained input/output/observation files and exact hashes.
- [Same-candidate manual lyrics regression](native-audio-lyrics-observed-20261115.json):
  21 CLI calls, eight formats, 17 exact domain refusals and portable export of nine
  records/eight publications. All 29 accepted files and 42 event streams remain
  pinned. No recognition, alignment or human review was introduced.

Fourteen current native raw run manifests remain rehashable. The cache-positive
baseline is retained as a canonical JSON snapshot with its own inventory hash;
the intentionally failed rerun replaced that one raw manifest. Its earlier raw
hash is an execution-time observation, not a claim that the raw bytes remain.
The retained snapshot preserves the positive source/step/native facts for review.

## Actual native outcomes

| Selected case | Observed result |
| --- | --- |
| PCM24, 44.1 kHz → `flac_max` | Valid; exact decoded PCM, precision/rate/channels/count; lossless |
| PCM16, 44.1 kHz → `alac_cd` | Valid; exact decoded PCM, precision/rate/channels/count; lossless |
| PCM24/16 → AAC LC; PCM24 → MP3 | Valid; lossy; stream/packet/presentation/decode clocks independently reconciled |
| PCM24, 44.1 kHz → `flac_broadcast` at 48 kHz | Valid requested resample; lossy |
| PCM24 → `alac_cd` | Invalid `audio.native.profile.layout`; requested 16-bit output actually remained 24-bit; no accepted publication |
| Same-plan AAC and ALAC | Distinct target/artifact/locator identities, both physical `.m4a` files |
| No native policy | Existing signature warning and conservative lossy edge behavior preserved |
| Explicit signature/native/custom validator IDs | Signature warning retained; native ID delegated once; unrelated unavailable custom check still blocks |
| Wrong verifier SHA | Invalid `audio.native.verifier_digest`, including with generic validation disabled and unavailable allowed |
| Tool policy denies FFprobe | Required unavailable `audio.native.verifier_policy_denied` |
| Preserved slim 8.0.1 tools | Required unavailable `audio.native.feature_missing`; actual complete feature tables lack needed FLAC/raw24 prerequisites |
| PCM32 source | Required unavailable `audio.native.source_precision_unsupported` |
| Existing reject-lossy gate | Partial result: observed lossless FLAC accepted, lossy MP3 refused |
| Identical cached build after private copied FFprobe removal | Reused producing step freshly rechecked; required unavailable `audio.native.verifier_missing`; old payload unchanged, current accepted outputs empty |

The ALAC precision mismatch is a retained encoder/profile limitation under #367.
This checkpoint exposes it truthfully instead of marking a codec-only lossless
result. It does not claim or implement 24-to-16-bit ALAC conversion. Its matching
16-bit route remains a positive proof.

## Focused checks and boundaries

Ten focused native/provider-policy/feature controls passed, plus the existing
subtitle secondary-provider policy regression. The final explicitly selected
ignored native replay also passed five retained synthetic payloads and actual
wrong-profile/codec/PCM/direct-lineage controls, including the separately pinned
slim-provider prerequisite refusal. Those replay step bindings are deliberately
synthetic/historical, with encoder origin unattested; the later CLI observations
above bind real new producing steps. An initial replay compilation caught a
missing observational boolean initializer; its failed log remains identified and
superseded. Earlier successful source/observer iterations are retained separately
from the final accepted receipt.

The implementation reuses ProcessExecutor, held snapshots, the Sonic WAV parser,
existing provider policy and required-validation/artifact/fidelity owners. Both
verifiers satisfy tool policy before native reads. Their bounded, hashed live
feature tables admit only the selected route prerequisites; supported malformed
or mismatching media is invalid, and missing/unsupported prerequisites remain
unavailable. Required native validation runs even with generic `required: false`
or `allow_unavailable: true`. When generic validation is enabled, other custom
validators retain their existing behavior.

Proofs are target/profile/artifact/producing-step specific and freshly recomputed
for cached CAS outputs. Only direct original Source/no-parent lineage is admitted,
so a later lossless encoding cannot launder earlier loss. Fidelity is scoped to
selected-WAV decoded PCM essence and clocks, without ancillary/container metadata,
rights, perceptual, mastering, loudness or creative approval. Verifier hashes do
not attest the encoder executable; encoded bitrate/compression and linked native
library/OS sandbox claims remain explicitly false.

Source and encoded payloads are bounded to 32 MiB each, decoded PCM to 64 MiB and
duration to 600 seconds. This is a local Linux candidate proof over short authored
fixtures, not full runtime/Clippy/provider/platform qualification. No hosted CI,
native Mac, physical volume, signing or external release was executed. Parent
#367/#419 acceptance remains open; Flow #88 is unchanged and last. Original
immutable candidates, selected audio/video tools and earlier receipts remain.

## Reproduce the selected observation

Supply the captured CLI and the exact separately prepared audio/slim tool pairs
from the binding receipt. Each work directory must be new with an existing
canonical parent. No tool preparation or network access is performed by the
observer:

```sh
python3 scripts/observations/check_native_audio_candidate.py \
  --binary /absolute/path/to/captured/renderflow \
  --audio-tool-directory /absolute/path/to/pinned/audio/bin \
  --slim-verifier-directory /absolute/path/to/preserved/slim/bin \
  --work-directory /absolute/path/to/new-native-audio-observation
```

The observer reuses the existing authored-audio helper, pins its own/helper/CLI
bytes before and after execution, checks both selected verifier pairs unchanged,
and records every command's exit and captured stream hashes. Native refusals need
the structured owning validator state/code and actual CLI exit 1; unrelated
errors or panics do not count as a passed domain control.
