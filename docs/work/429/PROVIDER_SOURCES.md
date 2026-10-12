# Creative toolchain discovery sources

Source review: 2026-10-12. No provider, version probe, installer, build, test,
schema validator, or CI job was run for this checkpoint. These records explain
the doctor descriptors; they do not qualify conversion behavior or redistribution.
The original doctor checkpoint added four candidate declarations. Merge
reconciliation with #423–#425 retains one canonical descriptor per ID and the
authored scripttool/Afterwriting adapter capabilities; pdftotext remains
discovery-only. Plain doctor excludes these four optional providers; the
explicit creative profile and complete inventories may report them. Node and
Afterwriting are `not_probed` in those inventories so discovery cannot bypass
the configured PDF adapter's pinned, clear-environment preflight.

| Registry ID | Discovery argv | Evidence and limit |
| --- | --- | --- |
| `tool.scripttool` | `scripttool -version` | Upstream handles this before input processing. The authored adapter declares exactly 0.0.10, with runtime qualification still deferred; doctor does not select the execution path. |
| `tool.node` | None; `not_probed` | The official version-only flag is source evidence. Only explicitly configured PDF preflight may invoke the pinned runtime with a cleared environment. Generic discovery does not launch Node. |
| `tool.afterwriting` | None; `not_probed` | Reviewed CLI has no version-only option. Configured PDF preflight reads pinned package/tree identity statically; help output is not package identity. |
| `tool.pdftotext` | `pdftotext -v` | Poppler source prints its version to stderr and exits successfully. The executable name alone does not attest the provider distribution. |

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

## Reused providers and separate assets

Existing FFmpeg, FFprobe, Tesseract, and whisper.cpp descriptors remain intact.
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
