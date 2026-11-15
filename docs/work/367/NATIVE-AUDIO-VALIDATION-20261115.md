# Native audio validation checkpoint — November 15, 2026

This source checkpoint addresses the precise gap retained by the prior authored
audio proof: native audio results still had signature-only warnings and all
built-in audio edges reported conservative lossy fidelity. It adds an explicit
pinned verifier policy and fresh publication-time checks through existing owners.
See [the user guide](../../user-guide/native-audio-validation.md).

The policy is optional and closed. When selected, native validation is required
alongside custom validators and even when generic validation is disabled or
unavailable results are otherwise allowed. Unsupported/missing prerequisites
remain unavailable; malformed matching-profile data is invalid. No signature
fallback, codec-only lossless label, metadata-driven cache promotion or global
catalog/planning-score change is introduced.

The implementation reuses ProcessExecutor, held snapshot/byte-read semantics and
the existing Sonic WAV parser. Native facts are target/profile/artifact/producing-
step specific. Exact matching FLAC/ALAC PCM can receive observed Lossless before
existing loss-class gates; declared source-changing profiles and MP3/AAC remain
Lossy. Direct original-source-only admission prevents earlier loss being laundered
through a later lossless codec. Verifier hashes are explicitly separate from
unattested encoder executable hashes.

Ten focused native/provider-policy/feature tests passed, alongside the existing
subtitle secondary-provider policy regression. One explicitly selected ignored native replay
also passed over five retained synthetic payloads, without encoding again:
matching FLAC24/ALAC16 became native-valid/sample-equal; MP3/AAC retained their
clock/priming/padding facts and lossy class. Actual wrong-profile, wrong-codec,
matching-layout PCM mismatch and direct-lineage controls refused. Replay step
bindings are deliberately synthetic/historical, with encoder origin unattested;
this is separate from the final new CLI/producing-step integration proof.

An initial compile caught a missing newly added observational boolean initializer;
it was corrected before the native replay and its failed log is retained. The final-source focused/schema/provider controls also pass. The actual
public-source CLI/producing-step integration candidate remains pending at this
source-preparation checkpoint. No full library runtime suite,
strict Clippy, full matrix, native Mac, release, signing, mastering/loudness or
creative approval is claimed. Parent #367/#419 acceptance remains open; Flow #88
is unchanged and last.

Source/CAS bounds are 32 MiB each, decoder output 64 MiB, duration 600 seconds;
only the initial bounded provider/codec profile is admitted. Original immutable
candidates, pinned audio/video tools and earlier receipts remain preserved.

The final opt-in native replay additionally observes bounded advertised decoder,
demuxer, raw encoder and muxer prerequisites through the existing ProcessExecutor.
Five retained payloads pass using the selected audio-capable verifier pair. The
separately pinned preserved Ani slim pair truthfully returns required Unavailable
for missing FLAC prerequisites, before media probing/decoding. Query/output hashes,
matched exact IDs and comma aliases are retained; advertised prerequisites do not
replace actual structural/clock/PCM checks or attest general native quality.

Existing provider allow/deny and execution requirements are reused for both
verifiers before native reads/probes; absolute explicit pins do not bypass those
gates. The contextual native validator ID is delegated once if requested, while
every other custom validator retains its behavior. No further scope expansion is
part of this checkpoint.
