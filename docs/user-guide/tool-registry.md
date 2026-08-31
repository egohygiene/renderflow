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
| `tool.ffmpeg` | FFmpeg | executable: `ffmpeg` | optional | configuration_dependent | local |
| `tool.ghostscript` | Ghostscript | executable: `gs` | experimental | configuration_dependent | local |
| `tool.img2pdf` | img2pdf | executable: `img2pdf` | experimental | deterministic | local |
| `tool.pandoc` | Pandoc | executable: `pandoc` | required | configuration_dependent | local |
| `tool.tectonic` | Tectonic | executable: `tectonic` | optional | configuration_dependent | network_optional |
| `tool.wkhtmltopdf` | wkhtmltopdf | executable: `wkhtmltopdf` | experimental | configuration_dependent | local |
| `tool.zip` | Info-ZIP compatible zip | executable: `zip` | experimental | configuration_dependent | local |

## Capability matrix

| Capability ID | Provider ID |
| --- | --- |
| `audio.convert` | `tool.ffmpeg` |
| `image.convert` | `tool.ffmpeg` |
| `media.convert` | `tool.ffmpeg` |
| `video.convert` | `tool.ffmpeg` |
| `pdf.process` | `tool.ghostscript` |
| `tiff.aggregate.press_pdf` | `tool.ghostscript` |
| `image.aggregate.pdf` | `tool.img2pdf` |
| `document.convert` | `tool.pandoc` |
| `document.generate` | `tool.pandoc` |
| `latex.compile` | `tool.tectonic` |
| `pdf.typeset` | `tool.tectonic` |
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
