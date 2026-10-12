# Native subtitle qualification — Rust harness not executed

The Rust harness was authored in checkpoint 3 for Renderflow #427 and remains
**unrun draft work**. Checkpoint 4 authorized focused local validation and
successfully exercised the native argument recipes independently, but Cargo
and Rust were unavailable. There is no successful Rust native qualification
record yet. See [the validation record](VALIDATION.md) for executed evidence and
remaining work; lint and GitHub Actions remain deferred.

The opt-in `timed_text_native` integration target prepares the later local
completion pass. Ordinary test runs leave its native entry point ignored.
It never downloads media, installs providers, or dispatches GitHub Actions.

## Intended coverage

The fixture material is original synthetic text plus tiny generated silent
audio. Explicit stream maps place audio before subtitles so selection must use
the absolute container index. Generator arguments and resulting file digests
are retained alongside the evidence.

The intended positive routes are embedded SubRip and WebVTT in Matroska. The
checks compare cue timing/text and source/stream/language identity, then exercise
the canonical graph/SDK route and its output lineage. The negative routes cover
an absent subtitle stream, a missing requested index, unsupported ASS and MP4
`mov_text`, malformed media, and cancellation requested before extraction.

Creating an unsupported-codec fixture is only test setup. It does not enable
conversion of that codec in Renderflow. MP4 is a refusal case in this kit, not
positive extraction coverage. Additional WebM/MOV combinations remain deferred.

## Local invocation

For the now-authorized local completion pass, use the repository's Rust 1.94
baseline and an installed FFmpeg build with
FFprobe. The kit requires WAV/SubRip/WebVTT/ASS input support, Matroska/MP4
muxing and demuxing, SRT/WebVTT output support, stream-copy support, and the
`mov_text` encoder for its negative fixture. No
video encoder, speech model, external media, or provider installation step is
part of this run.

From the repository root, first inspect the installed versions:

```bash
ffmpeg -version
ffprobe -version
```

Choose the builds to qualify, then paste each complete first version line into
the corresponding variable below. Replace the output path with an absolute,
new directory whose parent exists. Keep the checkout unchanged during the run.
The revision variable is an operator-declared label; the report separately
hashes the compiled test executable, which includes the linked Renderflow code.

```bash
export RENDERFLOW_SUBTITLE_QUALIFICATION_DIR="/absolute/path/to/new-subtitle-qualification"
export RENDERFLOW_SUBTITLE_FFMPEG_VERSION="paste the full ffmpeg version line here"
export RENDERFLOW_SUBTITLE_FFPROBE_VERSION="paste the full ffprobe version line here"
export RENDERFLOW_SUBTITLE_QUALIFICATION_REVISION="$(git rev-parse HEAD)"
cargo test --locked --package "renderflow" --test "timed_text_native" -- --ignored --exact "native_subtitle_qualification" --nocapture
```

The placeholders intentionally cannot qualify a build. The test requires exact
version matches, and its output directory must not already exist. Missing or
invalid version pins produce unavailable evidence when the output directory can
be created. A nonzero exit is not a passing qualification.

Open `qualification.json` in the retained directory after the command. The ten
required cases are fixture generation, SubRip extraction, WebVTT extraction,
SDK lineage, no subtitles, unsupported ASS, unsupported `mov_text`, pre-cancelled
extraction, malformed media, and the final provider-version recheck. The two
stream cases also refuse the audio stream's index instead of selecting a
subtitle implicitly.

Retained outputs include `supported.mkv` (audio 0, SubRip 1, WebVTT 2), negative
containers, the original synthetic input copies, extracted canonical JSON,
`sdk-output/` subtitle artifacts, `sdk-run-manifest.json`, and
`sdk-flow-artifacts.json`. Partial runs retain whatever they produced and record
the remaining cases as unexecuted or unavailable.

## Evidence rules

The output directory must be new. A complete report with all required cases in
`not_run` state is written before any provider probe. Each completed case updates
that same report. Existing output is never reused as the result of a new run.

| State | Meaning |
| --- | --- |
| `passed` | The case executed and its assertions succeeded. |
| `failed` | Execution or an assertion failed. |
| `unavailable` | A required prerequisite, such as a tool or selected version, was unavailable. |
| `not_run` | The case did not execute or did not finish recording its result. |

Overall success requires every required case to pass, including the final
provider-version recheck. Missing tools and mismatched versions cannot produce
a passing native test. An interrupted run can leave `not_run` evidence; that
does not establish qualification. A compiler failure or an unwritable/missing
output location can prevent the harness from creating any report at all.

Expected version strings are chosen explicitly by the operator and compared to
the exact first nonempty version-response line before and after the suite.
Extraction provenance must agree with the selected versions. These string pins
identify the observed build response; they are not cryptographic executable
locks or a defense against hostile concurrent replacement. The native run
manifest's planned toolchain snapshot remains distinct from actual extraction
provenance.

The report also requires the compiled harness's SHA-256 and Renderflow crate
version before it can pass. Hashing is bounded to a 1 GiB executable and checks
a 120-second budget between reads. A blocked filesystem read cannot be
interrupted by that elapsed-time check. An optional source revision is explicitly
operator-declared; save the corresponding Git status/diff for a dirty checkout.

Retain the entire output directory for review: the report alone does not replace
the generated containers, canonical JSON, subtitle exports, and SDK manifests.
Do not commit an edited sample report as though it came from a real run. Review
the code revision, platform, selected versions, fixture hashes, assertions, and
per-case results together before updating #427's acceptance criteria.

## Remaining proof

Even a passing Rust run covers only this small fixture matrix on its recorded
platform and provider versions. Mid-process cancellation, timeout/capture limits,
concurrent source changes, other containers/codecs, complete schema agreement,
and the wider compiler/test/release gates remain separate obligations in the
[handoff](HANDOFF.md). Current registration and authored fixtures do not confer
released support.

Fixture command syntax follows FFmpeg's primary documentation for
[explicit stream mapping and metadata](https://www.ffmpeg.org/ffmpeg.html).
Documentation is a design reference, not evidence that this harness has run.
