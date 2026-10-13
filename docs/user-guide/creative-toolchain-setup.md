# Set up a creative toolchain

Choose the route you need, install only its dependencies, then record their
exact identities in that route's configuration. Renderflow does not install
creative tools or download models. This setup guide is source-backed guidance;
none of the commands or platform combinations below has been qualified in this
checkpoint. The sprint's tests, native probes, installations, and CI remain
deferred.

## Choose a first route

| Goal | What to supply | Start here |
| --- | --- | --- |
| Recover screenplay-shaped UTF-8 text | Renderflow and explicit `allow_unqualified: true`; no external provider | [Screenplay recovery](../screenplay-recovery.md) |
| Recover an existing text-bearing PDF | Pinned Poppler `pdftotext` | [Recovery PDF configuration](../screenplay-recovery.md#extract-an-existing-pdf) |
| Add OCR for pages without usable embedded text | Pinned `pdftotext`, `pdftoppm`, Tesseract 5, and one language-data file | [OCR configuration](../screenplay-recovery.md#explicit-bounded-ocr-fallback) |
| Import a supported FDX/OSF/Fade In body into Fountain | Pinned scripttool `0.0.10` | [Restricted imports](scripttool.md) |
| Render the supported Fountain body to PDF | Pinned Node 22 or 24 and an installed Afterwriting `1.17.3` package tree | [Screenplay PDF](screenplay-pdf.md) |
| Convert plain SRT/WebVTT/transcript data | Renderflow's built-in codec | [Subtitles](subtitles.md) |
| Extract embedded subtitles or prepare selected audio | FFprobe and FFmpeg, plus explicit source/stream configuration | [Subtitles](subtitles.md) and [speech](speech.md) |
| Transcribe supported local audio | Pinned whisper.cpp executable and pinned local model; optional pinned preparation tools | [Speech](speech.md) |

The version constraints in this table describe accepted adapter declarations,
not a list of tested releases. Do not install a newer package and change an
expected version merely to bypass a refusal. Preserve the actual installed
identity and use a route whose authored contract accepts it.

The smallest screenplay recovery example uses the committed text fixture and
policy. From the repository root, with an already built Renderflow binary:

```bash
renderflow screenplay recover \
  --input "examples/screenplay-recovery/scene.txt" \
  --source-format "text" \
  --config "examples/screenplay-recovery/text-policy.json" \
  --output "scene-recovered"
```

Use a new output directory. This produces the canonical candidate and evidence;
add `--target "fountain"` when you also want that supported body derivative.
Recovery results always require human review, including this provider-free
route.

## Pick an installation channel

These are manual acquisition options, not commands run by Renderflow. Package
names and upstream pages identify where to start; record the exact package
revision and platform you choose. Package availability is distinct from
compatibility with Renderflow's cleared execution environment.

| Platform | Poppler and OCR | Other creative providers |
| --- | --- | --- |
| Linux | Use your distribution's Poppler utilities package; Debian's [`poppler-utils`](https://packages.debian.org/stable/poppler-utils) includes both extraction tools. Follow [Tesseract's distribution guidance](https://tesseract-ocr.github.io/tessdoc/Installation.html) for the engine and a separate language package, such as `tesseract-ocr-eng`. | Use the [FFmpeg project's package links](https://ffmpeg.org/download.html). Obtain Node through its [official downloads](https://nodejs.org/en/download). Install the exact scripttool/Afterwriting candidates using their route-specific source records below. |
| macOS | [Homebrew's Poppler formula](https://formulae.brew.sh/formula/poppler) includes `pdftotext` and `pdftoppm`; [Tesseract's guide](https://tesseract-ocr.github.io/tessdoc/Installation.html) describes Homebrew and MacPorts. Keep the selected language model explicit. | Use a reviewed FFmpeg package/source build and an official Node distribution matching the adapter's allowed major version. Resolve package-manager symlinks to actual regular executable files before pinning. |
| Windows | The [MSYS2 UCRT64 Poppler package](https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-poppler) supplies both `.exe` files and its runtime dependencies. [Tesseract's upstream guide](https://tesseract-ocr.github.io/tessdoc/Installation.html) links Windows installers and MSYS2 options. Use one compatible runtime installation, not a mixture of DLLs from unrelated packages. | Follow the providers' Windows instructions and the [FFmpeg download links](https://ffmpeg.org/download.html). Keep absolute Windows paths, required DLLs, and the package source in your qualification record. No native Windows combination is yet qualified here. |

For scripttool, the [#424 source evaluation](../work/424/PROVIDER_EVALUATION.md)
records upstream installation references and the restricted `0.0.10` contract.
For Afterwriting, follow the [#425 evaluation](../work/425/PROVIDER_EVALUATION.md)
and retain the installed package together with its dependencies. A global npm
launcher path is not a package tree identity. For whisper.cpp, start with the
[upstream project](https://github.com/ggml-org/whisper.cpp) and the narrower
[#428 evaluation](../work/428/PROVIDER_EVALUATION.md); downloading a model is an
explicit separate operation and never part of doctor.

Do not copy an executable out of its package without preserving the runtime
files it needs. Executable hashes identify those bytes only; they do not pin
linked libraries, fonts, Poppler encoding data, or DLL search behavior. Record
the installation source and dependency environment for the later fixture run.

## Turn an installation into configuration

1. Select the actual local regular files: executable, language/model data, and
   package root as applicable. Use absolute paths. Provider adapters do not use
   a successful PATH discovery as permission to choose a different binary.
2. Obtain each required SHA-256 from those exact files. Use the lowercase
   64-character digest in configuration, without the filename or a `sha256:`
   prefix. An upstream archive/package checksum is not the installed executable
   checksum.
3. Record the exact accepted version string when native observations are
   authorized. Poppler accepts numeric versions such as `25.01.0`; Tesseract
   recovery requires major `5`; neither example identifies a qualified build.
4. Copy the route's documented configuration and replace its placeholders.
   Keep `allow_unqualified: true` where required. Pin the source as well when
   the route offers `--source-sha256` or a source digest field.

Hashing examples for the later setup session:

```bash
# Linux: hash the actual regular executable and language-data file.
sha256sum "/absolute/path/to/pdftotext"
sha256sum "/absolute/path/to/eng.traineddata"

# macOS: the same SHA-256 operation.
shasum --algorithm 256 "/absolute/path/to/pdftotext"
```

```powershell
# Windows: produce the lowercase digest expected in JSON configuration.
(Get-FileHash -LiteralPath "C:\Tools\poppler\pdftotext.exe" -Algorithm SHA256).Hash.ToLowerInvariant()
```

When native probes are authorized, query the chosen file directly rather than
a same-named command elsewhere on PATH: `pdftotext -v`, `pdftoppm -v`,
`tesseract --version`, and `scripttool -version` are the reviewed spellings.
Poppler prints its version to stderr. Preserve each executable's actual version;
do not copy a package revision suffix into an expected numeric provider version.
Afterwriting has no version-only query: use the documented bounded
`renderflow screenplay package-fingerprint --package-root "..."` interface for
its installed tree, and configure the Node executable independently.

OCR configuration selects one language and one exact `.traineddata` path/hash.
Installing a language package does not populate those fields automatically.
The upstream [tessdata repository](https://github.com/tesseract-ocr/tessdata)
describes its model distribution; record the selected file's origin, revision,
digest, and license separately from the executable. Recovery stages that file
under the configured language name and does not use ambient model selection.

## Development containers

The checked-in development container supplies the existing development stack;
it does not install the creative providers listed above. For a dedicated
creative environment, extend that image explicitly with just the selected
distribution packages, preserving package versions or a package snapshot.
Keep exact scripttool/Node/Afterwriting installations and large model assets in
deliberately provisioned locations. Record container image identity and
architecture with the provider pins. Rebuilding a container or mounting a
different model directory can change the selected bytes and requires new pins.

No default Dockerfile change or automatic model download is part of #429.
Acquiring dependencies, building the image, and testing its providers remain
separate from this authored setup strategy.

## Observe discovery, then qualify the route

When execution is authorized, the [creative doctor](creative-toolchain.md)
provides a discovery report:

```bash
renderflow doctor --profile "creative-publishing" --format "json"
```

Node and Afterwriting intentionally stay unobserved there. Recovery execution
checks its configured paths independently; its planning phase performs no
native discovery. A successful doctor report does not validate model data,
recover a screenplay, open a native application, or prove that a configured
package works. Use the route's synthetic fixtures and retain its output/evidence
bundle during the later qualification session. Record failures as observations
instead of promoting an authored route to qualified status.

Use the [#429 qualification record](../work/429/QUALIFICATION.md) to keep declared
version constraints separate from observed provider/platform/fixture results.
