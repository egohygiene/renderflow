# Creative toolchain discovery sources

Original source review: 2026-10-12. Recovery/setup documentation reconciliation:
2026-10-13. No provider, version probe, installer, build, test, schema validator,
or CI job was run. These records explain declared interfaces and acquisition
channels; they do not qualify conversion behavior or redistribution.

Reconciliation with #423–#426 retains one canonical descriptor per ID, including
the authored screenplay imports, PDF rendering, PDF/text extraction, OCR,
deterministic recovery, and body exports. The explicit creative profile reports
ten tool IDs. Plain doctor excludes five optional external providers
(scripttool, Node, Afterwriting, pdftotext, pdftoppm) and the virtual recovery
provider, for six exclusions. Existing FFmpeg, FFprobe, Tesseract, and whisper.cpp
discovery remains in its prior scope. Node and Afterwriting stay `not_probed`
in generic inventories so discovery cannot bypass configured PDF preflight.
Recovery planning likewise leaves its selected toolchain unobserved; its
configured executable/data pins are inspected only during explicit execution.

| Registry ID | Discovery argv | Evidence and limit |
| --- | --- | --- |
| `tool.scripttool` | `scripttool -version` | Upstream handles this before input processing. The authored adapter declares exactly 0.0.10, with runtime qualification still deferred; doctor does not select the execution path. |
| `tool.node` | None; `not_probed` | The official version-only flag is source evidence. Only explicitly configured PDF preflight may invoke the pinned runtime with a cleared environment. Generic discovery does not launch Node. |
| `tool.afterwriting` | None; `not_probed` | Reviewed CLI has no version-only option. Configured PDF preflight reads pinned package/tree identity statically; help output is not package identity. |
| `tool.pdftotext` | `pdftotext -v` | Poppler source prints its version to stderr and exits successfully. The executable name alone does not attest the provider distribution. |
| `tool.pdftoppm` | `pdftoppm -v` | Poppler's packaged manual documents version reporting; recovery separately selects and hashes the actual page rasterizer. |
| `tool.renderflow-screenplay-recovery` | None; virtual | In-process deterministic model and native body writers. No executable installation or native qualification is implied. |
| `tool.ffmpeg` | `ffmpeg -version` | Existing media provider; exact build, codecs, selected streams, and route evidence remain separate. |
| `tool.ffprobe` | `ffprobe -version` | Existing stream inventory provider; discovery alone does not inspect input media. |
| `tool.tesseract` | `tesseract --version` | Existing discovery provider; #426 execution adds exact Tesseract 5 and language-data pins. |
| `tool.whisper_cpp` | Declared candidate with `--version` | Existing speech discovery; model selection and exact executable identity remain explicit. |

## scripttool

Reviewed commit:
[`ea8ed24020b6f6989e01df44eef64febdfa7bfb2`](https://github.com/rsdoiel/scripttool/commit/ea8ed24020b6f6989e01df44eef64febdfa7bfb2).

- [`cmd/scripttool/main.go`](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/cmd/scripttool/main.go)
  registers the `version` flag, prints the program name and version, and exits
  before invoking screenplay processing. Blob: `6e017f5cdda738e53acfa3f025256aaff779b0c3`.
- [`scripttool.1.md`](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/scripttool.1.md)
  documents `-version`. The single hyphen is the documented spelling.
- [`INSTALL.md`](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/INSTALL.md)
  describes source and release installation for macOS, Linux, and Windows.
  Those paths have not been exercised here.
- [`LICENSE`](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/LICENSE)
  contains AGPL version 3, while the reviewed entry-point file retains a BSD
  2-Clause header. Preserve this distinction in an exact-release packaging
  review; candidate discovery grants no packaging approval. The authored
  screenplay imports and their limits are recorded under [#424](../424/PROVIDER_EVALUATION.md).

## Node.js and Afterwriting

Node reviewed commit:
[`a7a96c2448ab91ca079de5e64233aa79a400b00e`](https://github.com/nodejs/node/commit/a7a96c2448ab91ca079de5e64233aa79a400b00e).
[`doc/api/cli.md`](https://github.com/nodejs/node/blob/a7a96c2448ab91ca079de5e64233aa79a400b00e/doc/api/cli.md)
documents `--version` (blob `fcca81b159874779be9347fe503cd12819b06609`).
[`LICENSE`](https://github.com/nodejs/node/blob/a7a96c2448ab91ca079de5e64233aa79a400b00e/LICENSE)
distinguishes Node's license from its externally maintained components. Obtain
an appropriate supported distribution through [nodejs.org](https://nodejs.org/);
this source pin is not a recommended runtime release or compatibility claim.
The [#425 adapter evaluation](../425/PROVIDER_EVALUATION.md) records the stricter
configured runtime invocation. The existence of `--version` does not permit
doctor to launch a PATH-selected Node under an inherited environment.

Afterwriting reviewed commit:
[`f1912c08d0905bac42e1805830a1e72f98b88c2f`](https://github.com/ifrost/afterwriting-labs/commit/f1912c08d0905bac42e1805830a1e72f98b88c2f).

- [`package.json`](https://github.com/ifrost/afterwriting-labs/blob/f1912c08d0905bac42e1805830a1e72f98b88c2f/package.json)
  declares package `afterwriting`, version `1.17.3`, executable `afterwriting`
  backed by `awc.js`, Node dependency, and MIT license. These are source
  declarations, not observations of any installed package.
- [`awc.js`](https://github.com/ifrost/afterwriting-labs/blob/f1912c08d0905bac42e1805830a1e72f98b88c2f/awc.js)
  loads the application; [`js/client/options.js`](https://github.com/ifrost/afterwriting-labs/blob/f1912c08d0905bac42e1805830a1e72f98b88c2f/js/client/options.js)
  has no version-only flag (blob `725d805a67e625d1867e3d18f26e89ae1f0750e8`).
  [`docs/clients.md`](https://github.com/ifrost/afterwriting-labs/blob/f1912c08d0905bac42e1805830a1e72f98b88c2f/docs/clients.md)
  documents npm/source installation, help, and custom fonts. Doctor launches none
  of these paths. It does not substitute a fabricated `--version` or a help banner.
- [`README.md`](https://github.com/ifrost/afterwriting-labs/blob/f1912c08d0905bac42e1805830a1e72f98b88c2f/README.md)
  contains the MIT license. [`Courier Prime's OFL`](https://github.com/ifrost/afterwriting-labs/blob/f1912c08d0905bac42e1805830a1e72f98b88c2f/js/fonts/source/Courier-Prime/LICENSE/OFL.txt)
  demonstrates that font licensing is separate. Exact dependency, font, and
  platform qualification belongs with the PDF adapter in issue #425.

## Poppler pdftotext

The [official Poppler site](https://poppler.freedesktop.org/) identifies the
upstream GitLab repository and links maintainer `tsdgeos`'s GitHub mirror. GitLab
and cgit source fetches were unavailable during this review, so source evidence
uses that maintainer mirror at
[`5b97e475790a11012ec6095d9a47c23f7f229cb9`](https://github.com/tsdgeos/poppler_mirror/commit/5b97e475790a11012ec6095d9a47c23f7f229cb9).

- [`utils/pdftotext.cc`](https://github.com/tsdgeos/poppler_mirror/blob/5b97e475790a11012ec6095d9a47c23f7f229cb9/utils/pdftotext.cc)
  defines `-v`, prints version and copyright to stderr, and returns success
  without opening a PDF. Blob: `74b327b8c4c556850c60301a61954ec7b88a6af7`.
- [`utils/pdftotext.1`](https://github.com/tsdgeos/poppler_mirror/blob/5b97e475790a11012ec6095d9a47c23f7f229cb9/utils/pdftotext.1)
  documents `-v`; no long-form version flag is declared in the reviewed parser.
- [`README.md`](https://github.com/tsdgeos/poppler_mirror/blob/5b97e475790a11012ec6095d9a47c23f7f229cb9/README.md)
  and [`COPYING`](https://github.com/tsdgeos/poppler_mirror/blob/5b97e475790a11012ec6095d9a47c23f7f229cb9/COPYING)
  record GPL licensing; the utility's source header identifies Poppler changes
  as GPL version 2 or later. No library linking or bundling is introduced.
  Utility installation is separate through an appropriate Poppler distribution;
  operating-system declarations are candidate scope, not a tested package matrix.

## Recovery extraction and OCR references

The authored command contract is in
[`screenplay/recovery/provider.rs`](../../../crates/renderflow-core/src/screenplay/recovery/provider.rs).
It extracts one page at a time, first using pdftotext with explicit page limits,
layout mode, UTF-8, Unix line endings, and no automatic page-break characters.
Renderflow records page boundaries itself. The
[Poppler pdftotext manual packaged by Debian](https://manpages.debian.org/trixie/poppler-utils/pdftotext.1.en.html)
documents those options. This reference is a published package manual, not a
claim that its package version was installed or tested.

For a page lacking usable embedded text, configured OCR uses pdftoppm with the
same page interval, `-singlefile`, bounded `-scale-to`, and `-gray`. It produces
one PGM image. The
[packaged pdftoppm manual](https://manpages.debian.org/trixie/poppler-utils/pdftoppm.1.en.html)
documents page selection, single-file output, scaling, grayscale output, and
`-v`. The Poppler utilities remain separately installed executables; Renderflow
does not bundle or link them. Their declared GPL notes require exact-build and
distribution review, including linked dependencies.

The selected Tesseract 5 process receives that image, a private staged data
directory, one explicit language, LSTM engine mode `--oem 1`, and segmentation
mode `--psm 3`. The upstream
[command-line guide](https://tesseract-ocr.github.io/tessdoc/Command-Line-Usage.html)
describes those engine/segmentation choices and explicit language selection.
Its [data-file guide](https://tesseract-ocr.github.io/tessdoc/Data-Files.html)
distinguishes model distributions. The adapter pins one actual `.traineddata`
file by path and digest; it neither downloads data nor uses ambient language
selection. OCR confidence is not a measured probability of correct screenplay
classification: recovery reports ordinal heuristic bands and always requires
review.

These documentation references were retrieved on 2026-10-13. The new pdftoppm
and Tesseract references do not add a source-commit attestation or establish a
qualified release. Poppler's exact numeric version accepts zero-padded release
components; Tesseract's authored execution contract requires major `5`. Both
versions and executable digests are declarations until the configured execution
observes them. Hashes do not cover shared libraries, Poppler encoding data,
fonts, or process-wide memory/network isolation.

The virtual recovery descriptor refers to Renderflow's own model and export
modules. It carries `screenplay.recover.structure` and
`screenplay.recovery.export`, experimental support, and lossy fidelity. It has
no native version to probe. The recovery record, canonical validation, native
body reparse, and mandatory publication checks are authored evidence contracts;
they never label an OCR result original or upgrade human approval status.

## Installation-channel references

The [setup guide](../../user-guide/creative-toolchain-setup.md) uses these
publisher-maintained references, retrieved on 2026-10-13. Package pages describe
acquisition choices; none supplies a Renderflow-qualified version matrix.

| Publisher reference | Documented use here |
| --- | --- |
| [Debian poppler-utils](https://packages.debian.org/stable/poppler-utils) | Distribution package containing pdftotext and pdftoppm. |
| [Homebrew Poppler](https://formulae.brew.sh/formula/poppler) | macOS package channel and declared binaries/dependencies. |
| [MSYS2 UCRT64 Poppler](https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-poppler) | Windows package with both executable files and runtime dependencies. |
| [Tesseract installation](https://tesseract-ocr.github.io/tessdoc/Installation.html) | Separate engine/language-data installation and Linux/macOS/Windows channels. |
| [Tesseract tessdata](https://github.com/tesseract-ocr/tessdata) | Model repository and its license; record the exact acquired file separately. |
| [FFmpeg downloads](https://ffmpeg.org/download.html) | Upstream source and linked binary/package channels; binary distributions have separate provenance. |
| [Node downloads](https://nodejs.org/en/download) | Official runtime distributions; Renderflow's adapter still restricts acceptable declared major versions. |
| [whisper.cpp](https://github.com/ggml-org/whisper.cpp) | Upstream acquisition/build/model documentation, subject to the narrower #428 adapter contract. |

No package-manager command, installer, model download, or container build was
executed. The default development container is unchanged. A dedicated creative
image remains an explicit local setup choice with its own image, package,
architecture, and asset records.

## Reused providers and separate assets

Existing FFmpeg, FFprobe, and whisper.cpp discovery declarations remain intact.
Tesseract keeps its discovery command while its registry notes now describe the
authored #426 pinned recovery integration.
The speech provider source evaluation is in
[`../428/PROVIDER_EVALUATION.md`](../428/PROVIDER_EVALUATION.md).
The existing whisper.cpp pin's
[`examples/cli/cli.cpp`](https://github.com/ggml-org/whisper.cpp/blob/7e26d3b0d5bced6c8217ef09710ec2f969001241/examples/cli/cli.cpp)
explicitly handles `--version` before model loading (blob
`b015319d76f930fe452e30fa9c04ba6dfa19331e`); no policy change is needed.

Tool discovery does not identify speech model files, OCR language data, fonts,
decoder availability, runtime libraries, or their licenses. Those require
separate exact-asset evidence. Missing assets are not silently downloaded or
treated as satisfied. A tool being available does not make an unimplemented
Renderflow adapter available, nor qualify an authored adapter's output.

[QUALIFICATION.md](QUALIFICATION.md) records what remains unobserved and the
separate fixture/platform evidence needed before closing #429. This source
record supplies reviewed interfaces and acquisition references, not runtime
results.
