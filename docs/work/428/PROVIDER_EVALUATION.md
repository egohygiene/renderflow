# Local speech provider source evaluation — #428

Status: source inspection only, 2026-10-12 UTC. No provider binary, model,
compiler, or test was executed. These observations inform the first standalone
WAV-to-transcript checkpoint; they do not qualify a release or a platform.

## Provider and source pin

The candidate is an operator-supplied `whisper-cli` executable from
[`ggml-org/whisper.cpp` at `7e26d3b0d5bced6c8217ef09710ec2f969001241`](https://github.com/ggml-org/whisper.cpp/tree/7e26d3b0d5bced6c8217ef09710ec2f969001241).
The model is a separate, already-local GGML file. Renderflow should not install
the provider, download weights, or silently select a replacement model.

Primary sources read at that revision:

- [CLI argument parser, inference setup, and JSON writer](https://github.com/ggml-org/whisper.cpp/blob/7e26d3b0d5bced6c8217ef09710ec2f969001241/examples/cli/cli.cpp).
- [Audio decoder and timestamp conversion](https://github.com/ggml-org/whisper.cpp/blob/7e26d3b0d5bced6c8217ef09710ec2f969001241/examples/common-whisper.cpp).
- [Backend selection and model initialization](https://github.com/ggml-org/whisper.cpp/blob/7e26d3b0d5bced6c8217ef09710ec2f969001241/src/whisper.cpp).
- [Build version and optional backend switches](https://github.com/ggml-org/whisper.cpp/blob/7e26d3b0d5bced6c8217ef09710ec2f969001241/CMakeLists.txt).
- [Upstream overview](https://github.com/ggml-org/whisper.cpp/blob/7e26d3b0d5bced6c8217ef09710ec2f969001241/README.md),
  [model notes](https://github.com/ggml-org/whisper.cpp/blob/7e26d3b0d5bced6c8217ef09710ec2f969001241/models/README.md),
  and [code license](https://github.com/ggml-org/whisper.cpp/blob/7e26d3b0d5bced6c8217ef09710ec2f969001241/LICENSE).

## Invocation contract

The inspected parser accepts these long-form options:

| Option | Source behavior and adapter implication |
| --- | --- |
| `--version` | Prints `whisper.cpp version: <version>` to standard output and exits before model loading. Record this observation separately from the required executable digest. |
| `--model`, `--file` | Accept local model and input paths. Supply one input and an explicit model. |
| `--output-json-full`, `--output-file` | Enable JSON with token details; append `.json` to the output base path. Use an isolated output location because upstream opens output files for replacement. |
| `--language` | Accept a language or `auto`; `--detect-language` instead requests detection-only behavior and is unsuitable for transcription. |
| `--threads`, `--processors` | Configure worker threads and parallel processing. Fix both explicitly for the bounded initial profile. |
| `--temperature`, `--temperature-inc`, `--no-fallback` | Control sampling temperature and retries. `--no-fallback` sets the effective temperature increment to zero. |
| `--beam-size`, `--best-of` | Set decoding candidates. Beam size greater than one selects beam search; one selects greedy decoding when no grammar is configured. |
| `--no-gpu`, `--no-flash-attn` | Disable the corresponding context parameters. This is a GPU-disabled request, not proof that every compiled acceleration path is disabled. |
| `--no-prints` | Suppresses the library logger but not every direct standard-error message. Do not equate nonempty stderr with failure. |

The inspected build defaults to the version string `1.9.5-dev`. A version string
does not attest a source revision, compiler, backend configuration, shared
libraries, or model. Preserve content hashes and the requested settings as
separate provenance. Fixed decoding parameters improve repeatability; they do
not establish bit-for-bit determinism across machines or builds.

Unknown options, missing option values, and unknown languages can exit with
status zero in the inspected parser. Successful exit therefore does not replace
checking for a bounded, well-formed output file and validating its contents.

## JSON mapping

The full JSON writer emits `systeminfo`, `model`, `params`, `result`, and
`transcription`. The first checkpoint can use a strict subset while retaining a
digest of the complete provider output:

| Field | Meaning |
| --- | --- |
| `params.model` | The model path passed to the CLI. |
| `params.language` | The effective requested language after possible upstream adjustment. |
| `params.translate` | Whether translation was requested; require false for this transcription-only profile. |
| `result.language` | Resolved language from the inference context. |
| `transcription[].text` | Complete segment text; preserve it as text, with no inferred speaker identity. |
| `transcription[].offsets.from/to` | Integer milliseconds, derived from upstream centisecond values multiplied by ten. |
| `transcription[].tokens[].text/id/p/t_dtw` | Provider token details; `p` is a token probability, not a measured transcription accuracy. |
| `transcription[].tokens[].offsets` | Optional token timing. The writer omits it when token timing is unavailable. |

`--output-json-full` enables token timestamp computation. Tokens are not words:
upstream can merge adjacent token byte fragments to form a valid UTF-8 codepoint,
while retaining the first token's probability and identifier. Do not synthesize
word confidence or a segment-wide accuracy score from these values. Segment
times and text are the initial canonical cue representation; detailed token
confidence mapping can be a later explicit contract.

An English-only model can cause upstream to rewrite language to English and
disable translation. Validate reported settings against the chosen profile;
the absence of a nonzero exit does not prove the requested language was honored.

## Input, offline, and acceleration boundaries

The README recommends mono 16 kHz PCM16 WAV, while the inspected implementation
uses miniaudio to decode and resample a broader input set. A mono 16 kHz PCM16
RIFF/WAV restriction is therefore a deliberate Renderflow checkpoint boundary,
not a claim that current upstream only understands WAV. Restrict and validate
input before spawning; arbitrary media extraction and resampling belong to a
later checkpoint.

Local files and a local CLI avoid a hosted transcription API in this design.
They do not create an operating-system network sandbox for an arbitrary
operator-supplied executable. Record the requested offline policy accurately.

`--no-gpu` alone cannot establish CPU-only execution. The inspected source can
initialize accelerator-class GGML backends independently of that flag; compiled
Core ML and Vitis AI paths can load model sidecars; `ANEFORGE_ENCODER` can request
an additional encoder through the environment. The CLI can also initialize
OpenVINO when compiled with it. CPU-only qualification consequently needs an
identified build configuration and controlled environment. The first profile
must describe its settings as GPU-disabled until that evidence exists.

## Distribution and remaining evidence

The inspected whisper.cpp code is MIT-licensed. The model notes describe
converted OpenAI weights and English-only versus multilingual variants; model
bytes, origin, and license must be recorded for the actual supplied model. No
model or upstream sample audio is vendored by this checkpoint.

Upstream lists macOS, Linux, and Windows support. Renderflow has not qualified
this adapter on any of them. Follow-up evidence must include compilation,
synthetic parser/refusal tests, a licensed speech fixture with a known reference,
model and executable hashes, language behavior, retained output, timeout and
cancellation behavior, and platform-specific execution results. Singing/lyrics,
diarization, translation, and arbitrary video input remain outside this first
checkpoint's claims.
