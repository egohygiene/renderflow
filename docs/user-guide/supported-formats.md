# Supported Formats

!!! info
    This page is generated from `src/input_format.rs`, `src/graph/format.rs`,
    `src/audio/format.rs`, and `src/image/format.rs` by
    `scripts/generate_supported_formats_doc.py`. Do not edit it by hand.

Renderflow recognizes format identifiers in four places:

- `input_format` in `renderflow.yaml`,
- `outputs[].type` for standard, audio, and image rendering,
- transform-graph definitions, and
- `renderflow graph ...` / `renderflow inspect ...` target selection.

For audio and image formats, **Encodable = No** means Renderflow recognizes the
identifier but cannot emit that format through the built-in FFmpeg-backed
strategy.

## Input formats

| Config value | Recognized extensions |
| --- | --- |
| `markdown` | `.md`, `.markdown` |
| `docx` | `.docx` |
| `html` | `.html`, `.htm` |
| `epub` | `.epub` |
| `rst` | `.rst` |
| `latex` | `.tex` |

## Transform graph identifiers

These are the canonical node names used in transform YAML files and graph output.

| Identifier |
| --- |
| `markdown` |
| `html` |
| `pdf` |
| `docx` |
| `epub` |
| `rst` |
| `latex` |
| `fountain` |
| `jpeg` |
| `png` |
| `tiff` |
| `cbz` |

## Audio format identifiers

| Identifier | Default extension | Encodable |
| --- | --- | --- |
| `wav` | `.wav` | Yes |
| `aif` | `.aif` | Yes |
| `aiff` | `.aiff` | Yes |
| `bwf` | `.wav` | Yes |
| `pcm` | `.pcm` | Yes |
| `flac` | `.flac` | Yes |
| `m4a_alac` | `.m4a` | Yes |
| `wv` | `.wv` | Yes |
| `ape` | `.ape` | Yes |
| `tta` | `.tta` | Yes |
| `dsf` | `.dsf` | No |
| `dff` | `.dff` | No |
| `shn` | `.shn` | No |
| `mp3` | `.mp3` | Yes |
| `m4a` | `.m4a` | Yes |
| `aac` | `.aac` | Yes |
| `ogg` | `.ogg` | Yes |
| `opus` | `.opus` | Yes |
| `wma` | `.wma` | Yes |
| `amr` | `.amr` | Yes |
| `mp2` | `.mp2` | Yes |
| `ra` | `.ra` | No |
| `oma` | `.oma` | No |
| `ac3` | `.ac3` | Yes |
| `ec3` | `.ec3` | Yes |
| `thd` | `.thd` | No |
| `dts` | `.dts` | Yes |
| `dtshd` | `.dtshd` | No |
| `mid` | `.mid` | Yes |
| `midi` | `.midi` | Yes |
| `mod` | `.mod` | No |

## Image format identifiers

| Identifier | Default extension | Encodable |
| --- | --- | --- |
| `jpeg` | `.jpeg` | Yes |
| `jpg` | `.jpg` | Yes |
| `png` | `.png` | Yes |
| `webp` | `.webp` | Yes |
| `avif` | `.avif` | Yes |
| `gif` | `.gif` | Yes |
| `bmp` | `.bmp` | Yes |
| `tiff` | `.tiff` | Yes |
| `tif` | `.tif` | Yes |
| `exr` | `.exr` | Yes |
| `hdr` | `.hdr` | Yes |
| `dpx` | `.dpx` | Yes |
| `cin` | `.cin` | Yes |
| `fits` | `.fits` | Yes |
| `fit` | `.fit` | Yes |
| `tga` | `.tga` | Yes |
| `sgi` | `.sgi` | Yes |
| `rgb` | `.rgb` | Yes |
| `pcx` | `.pcx` | Yes |
| `pbm` | `.pbm` | Yes |
| `pgm` | `.pgm` | Yes |
| `ppm` | `.ppm` | Yes |
| `pnm` | `.pnm` | Yes |
| `pam` | `.pam` | Yes |
| `xbm` | `.xbm` | Yes |
| `xpm` | `.xpm` | Yes |
| `wbmp` | `.wbmp` | Yes |
| `ras` | `.ras` | Yes |
| `sun` | `.sun` | Yes |
| `jp2` | `.jp2` | Yes |
| `j2k` | `.j2k` | Yes |
| `jxl` | `.jxl` | Yes |
| `jxr` | `.jxr` | No |
| `wdp` | `.wdp` | No |
| `hdp` | `.hdp` | No |
| `bpg` | `.bpg` | No |
| `flif` | `.flif` | No |
| `apng` | `.apng` | Yes |
| `mng` | `.mng` | No |
| `fli` | `.fli` | No |
| `flc` | `.flc` | No |
| `ani` | `.ani` | No |
| `heic` | `.heic` | No |
| `heif` | `.heif` | No |
| `dds` | `.dds` | Yes |
| `ico` | `.ico` | Yes |
| `cur` | `.cur` | No |
| `dcm` | `.dcm` | No |
| `dicom` | `.dicom` | No |
| `pict` | `.pict` | No |
| `pct` | `.pct` | No |
| `iff` | `.iff` | No |
| `lbm` | `.lbm` | No |
| `mac` | `.mac` | No |
| `cals` | `.cals` | No |
| `cal` | `.cal` | No |
| `fax` | `.fax` | No |
| `jbig` | `.jbig` | No |
| `jb2` | `.jb2` | No |
| `pgf` | `.pgf` | No |
| `pic` | `.pic` | No |
| `blp` | `.blp` | No |
| `vtf` | `.vtf` | No |
| `sfw` | `.sfw` | No |
| `raw` | `.raw` | No |
| `dng` | `.dng` | No |
| `cr2` | `.cr2` | No |
| `cr3` | `.cr3` | No |
| `nef` | `.nef` | No |
| `arw` | `.arw` | No |
| `orf` | `.orf` | No |
| `raf` | `.raf` | No |
| `rw2` | `.rw2` | No |
| `svg` | `.svg` | No |
| `eps` | `.eps` | No |
| `pdf` | `.pdf` | No |
| `psd` | `.psd` | No |
| `psb` | `.psb` | No |
| `ai` | `.ai` | No |
| `indd` | `.indd` | No |
| `xcf` | `.xcf` | No |
| `afphoto` | `.afphoto` | No |
| `afdesign` | `.afdesign` | No |
| `cdr` | `.cdr` | No |
| `sketch` | `.sketch` | No |
| `fig` | `.fig` | No |
| `wmf` | `.wmf` | No |
| `emf` | `.emf` | No |
| `skp` | `.skp` | No |
| `dxf` | `.dxf` | No |
| `dwg` | `.dwg` | No |
| `plt` | `.plt` | No |
| `cgm` | `.cgm` | No |
| `cmx` | `.cmx` | No |
| `drw` | `.drw` | No |
| `swf` | `.swf` | No |
| `fla` | `.fla` | No |
