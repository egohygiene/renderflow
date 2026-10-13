# Tool Registry

!!! info
    This page is generated from the canonical built-in catalog at
    `crates/renderflow-core/data/tool-registry.yaml` by
    `scripts/generate_tool_registry_doc.py`. Do not edit it by hand.

Registry schema: `renderflow.tool-registry/v1`

The built-in catalog defines stable provider IDs, discovery/version probes,
platform constraints, capability IDs, determinism/locality/fidelity metadata,
runtime requirements, fallback relationships, and distribution/licensing notes.
YAML command transforms and plugins can register additional runtime providers
without editing this built-in catalog.

## Built-in providers

| Provider ID | Name | Discovery | Tier | Determinism | Locality |
| --- | --- | --- | --- | --- | --- |
| `tool.afterwriting` | Explicit afterwriting screenplay PDF provider | unprobed | experimental | configuration_dependent | local |
| `tool.epubcheck` | EPUBCheck | executable: `epubcheck` | optional | deterministic | local |
| `tool.ffmpeg` | FFmpeg | executable: `ffmpeg` | optional | configuration_dependent | local |
| `tool.ffprobe` | FFprobe | executable: `ffprobe` | optional | configuration_dependent | local |
| `tool.ghostscript` | Ghostscript | executable: `gs` | experimental | configuration_dependent | local |
| `tool.handbrake` | HandBrakeCLI | executable: `HandBrakeCLI` | optional | configuration_dependent | local |
| `tool.imagemagick` | ImageMagick | executable: `magick` | experimental | configuration_dependent | local |
| `tool.img2pdf` | img2pdf | executable: `img2pdf` | experimental | deterministic | local |
| `tool.jq` | jq | executable: `jq` | experimental | deterministic | local |
| `tool.kepubify` | Kepubify | executable: `kepubify` | optional | configuration_dependent | local |
| `tool.lulu-rules` | Pinned Lulu publication rule pack | virtual | optional | deterministic | local |
| `tool.node` | Explicit Node runtime for afterwriting | unprobed | experimental | configuration_dependent | local |
| `tool.pandoc` | Pandoc | executable: `pandoc` | required | configuration_dependent | local |
| `tool.pdftoppm` | Poppler pdftoppm recovery page rasterizer | executable: `pdftoppm` | optional | configuration_dependent | local |
| `tool.pdftotext` | Poppler pdftotext recovery extractor | executable: `pdftotext` | optional | configuration_dependent | local |
| `tool.renderflow-epub` | Renderflow fixed-layout EPUB packager | virtual | experimental | deterministic | local |
| `tool.renderflow-screenplay-recovery` | Renderflow screenplay recovery and derivative writer | virtual | experimental | deterministic | local |
| `tool.renderflow-timed-text` | Renderflow plain timed-text codec | virtual | experimental | deterministic | local |
| `tool.scripttool` | scripttool screenplay import | executable: `scripttool` | experimental | configuration_dependent | local |
| `tool.tectonic` | Tectonic | executable: `tectonic` | optional | configuration_dependent | network_optional |
| `tool.tesseract` | Tesseract OCR | executable: `tesseract` | experimental | configuration_dependent | local |
| `tool.upscayl-ncnn` | Upscayl NCNN | executable: `upscayl-ncnn`, `upscayl-bin` | experimental | configuration_dependent | local |
| `tool.whisper_cpp` | whisper.cpp local speech recognition | executable: `whisper-cli` | optional | configuration_dependent | local |
| `tool.wkhtmltopdf` | wkhtmltopdf | executable: `wkhtmltopdf` | experimental | configuration_dependent | local |
| `tool.zip` | Info-ZIP compatible zip | executable: `zip` | experimental | configuration_dependent | local |

## Capability matrix

| Capability ID | Provider ID |
| --- | --- |
| `screenplay.render.fountain.pdf` | `tool.afterwriting` |
| `ebook.validate.epub` | `tool.epubcheck` |
| `audio.convert` | `tool.ffmpeg` |
| `image.convert` | `tool.ffmpeg` |
| `media.convert` | `tool.ffmpeg` |
| `speech.audio.prepare` | `tool.ffmpeg` |
| `subtitle.extract.plain` | `tool.ffmpeg` |
| `video.convert` | `tool.ffmpeg` |
| `speech.audio.inspect` | `tool.ffprobe` |
| `subtitle.streams.inspect` | `tool.ffprobe` |
| `pdf.process` | `tool.ghostscript` |
| `tiff.aggregate.press_pdf` | `tool.ghostscript` |
| `video.transcode.whole_file` | `tool.handbrake` |
| `image.convert` | `tool.imagemagick` |
| `image.aggregate.pdf` | `tool.img2pdf` |
| `publication.generate.pdf.interior` | `tool.img2pdf` |
| `data.json.transform` | `tool.jq` |
| `ebook.convert.kepub` | `tool.kepubify` |
| `publication.lulu.bookstore.preflight` | `tool.lulu-rules` |
| `publication.lulu.epub-distribution.preflight` | `tool.lulu-rules` |
| `publication.lulu.global-distribution.preflight` | `tool.lulu-rules` |
| `publication.lulu.pdf-ebook.preflight` | `tool.lulu-rules` |
| `publication.lulu.print-direct.preflight` | `tool.lulu-rules` |
| `runtime.javascript.node` | `tool.node` |
| `document.convert` | `tool.pandoc` |
| `document.generate` | `tool.pandoc` |
| `screenplay.recovery.rasterize` | `tool.pdftoppm` |
| `screenplay.recovery.extract.text` | `tool.pdftotext` |
| `ebook.generate.epub.fixed-layout` | `tool.renderflow-epub` |
| `screenplay.recover.structure` | `tool.renderflow-screenplay-recovery` |
| `screenplay.recovery.export` | `tool.renderflow-screenplay-recovery` |
| `subtitle.convert.plain` | `tool.renderflow-timed-text` |
| `screenplay.import.fadein.fountain` | `tool.scripttool` |
| `screenplay.import.fdx.fountain` | `tool.scripttool` |
| `screenplay.import.osf.fountain` | `tool.scripttool` |
| `latex.compile` | `tool.tectonic` |
| `pdf.typeset` | `tool.tectonic` |
| `ocr.extract.text` | `tool.tesseract` |
| `image.super_resolution` | `tool.upscayl-ncnn` |
| `speech.transcribe` | `tool.whisper_cpp` |
| `html.render.pdf` | `tool.wkhtmltopdf` |
| `archive.zip.create` | `tool.zip` |
| `comic.cbz.create` | `tool.zip` |

## Runtime inspection

Use the CLI to inspect the live host using the same model:

```text
renderflow tools list
renderflow tools inspect tool.ffmpeg
renderflow capabilities
renderflow doctor
```

Add `--format json` or `--format yaml` to the tools/capabilities commands
when machine-readable evidence is needed.
