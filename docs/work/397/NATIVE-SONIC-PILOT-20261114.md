# Native Sonic candidate pilot — November 14, 2026

Relates to [Renderflow #397](https://github.com/egohygiene/renderflow/issues/397).
This checkpoint executes the current CLI on entirely generated inputs. It does
not infer musical properties, authenticate lyrics, grant human approval or
publish a release.

`scripts/run_sonic_evidence_pilot.py` generates 55,125 stereo PCM24 frames at
44,100 Hz (1,250 ms), a manual Unicode SRT companion, and an explicit request
using the existing public typed-analysis example. The supplied tempo/key are
synthetic assertions, with their original heuristic/probabilistic classes and
uncalibrated/unavailable confidence. No musical analyzer or external provider,
model, service or catalog executes. The manual words are unreviewed candidate
context, rather than authenticated human lyrics.

The real `dna sonic plan`, `build` and `status` commands passed. Planning retains
the original 24-bit/44.1 kHz source clock and reports `sample_conversion: none`.
The manual companion contributes two structural cues spanning source frames
5,513–55,125; its words appear only in the generated input. All retained
candidate files are checked for both authored text markers. The DNA retains
the original confidence classes and requires human review. The status reports
`complete`, source `current`, review `candidate`, `runtime_qualified: false`,
and publication approval `not_granted`.

Three negative trials check their specific owning errors: rebuilding into the
same destination returns `sonic.output.exists`; changing the retained Markdown
returns `sonic.readback.file.changed`; setting the receipt's runtime authority
to true returns `sonic.readback.run.contract`, even with a recomputed external
receipt digest. Each refusal requires native exit 1, empty stdout and the exact
anchored `Error: <code>` diagnostic line; a panic or unrelated error containing
the same text cannot count as proof. Four focused observer cases passed with
`python3 -B scripts/test_sonic_evidence_pilot.py`, including code-looking panic,
unrelated error, wrong exit and extra-output refusal. Changing only the explicit
request identity while keeping all relative inputs valid reports source
`stale`. Original inputs, candidate and
selected executable hashes remain unchanged.

## Reproduction and observed source

Build the selected source with pinned Rust 1.94.0, two jobs and the locked
workspace; supply the resulting executable and a fresh private directory:

```sh
CARGO_BUILD_JOBS=2 cargo +1.94.0 build --locked --release \
  --package renderflow-cli --bin renderflow --target x86_64-unknown-linux-gnu
python3 -B scripts/run_sonic_evidence_pilot.py \
  --binary /absolute/path/to/renderflow \
  --work-directory /absolute/canonical/existing-parent/new-pilot
```

The November 14 execution used Rust source revision
`c8f2b6896ec82c12e75ad6b8bcf7e4b319602db2`, tree
`e3a7b4ab29930dd55154a2b9a71e0f445454a08d`, in the Debian 13 Linux workspace.
The release CLI SHA-256 is
`e6a182eb2d1381301312f6f37a725620a988dc761cb972a0a9b19c5d7d6cbe82`.
The exact pilot script and example-template hashes, candidate inventory,
native metadata and all seven command results are retained in
[the executed receipt](evidence/20261114/native-sonic-pilot.json). Generated
inputs and outputs remain in the private caller-selected work directory; the
committed evidence includes no absolute workspace paths.

This proves the selected native CLI candidate path on that host. Provider
reobservation, musical correctness, real-song quality, model execution, human
review, other formats/platforms and publication remain outside this proof.
The separate #419 Ubuntu baseline checkpoint owns its own environment result;
this Debian observation does not qualify Ubuntu or the full release matrix.
