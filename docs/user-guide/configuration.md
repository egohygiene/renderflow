# Configuration

Renderflow's main config is a YAML file that deserializes into `Config` in `src/config.rs`.

## Minimal config

```yaml
input: input.md
output_dir: dist
outputs:
  - type: html
```

## Full document-oriented example

```yaml
input: report.md
input_format: markdown
output_dir: dist
optimization: balanced
transforms: transforms.yaml
variables:
  title: Quarterly Report
  author: Jane Smith
outputs:
  - type: html
    template: default
  - type: pdf
  - type: docx
```

## Key reference

| Key | Required | Default | Notes |
|---|---|---|---|
| `input` | Yes | none | Source file path |
| `input_format` | No | auto-detect, then `markdown` | Supported values: `markdown`, `docx`, `html`, `epub`, `rst`, `latex` |
| `output_dir` | No | `dist` | Destination directory |
| `variables` | No | `{}` | String-to-string map used by `{{key}}` placeholders |
| `optimization` | No | `balanced` | Planner mode for graph-aware commands |
| `transforms` | No | none | Path to a YAML transform graph / transform registry file |
| `outputs` | Yes for standard builds | empty | List of output definitions |

### `outputs[]`

Each output item maps to `OutputConfig`.

| Key | Required | Notes |
|---|---|---|
| `type` | Yes | `html`, `pdf`, `docx`, supported audio formats, or supported image formats |
| `template` | No | Template name looked up in `templates/` |
| `profile` | No | Audio quality profile for audio outputs only |

## Validation rules

Renderflow validates several constraints before running a standard build:

- `input` must not be empty
- `outputs` must contain at least one item
- output types must be known
- document inputs cannot mix in audio/image outputs
- audio inputs only produce audio outputs
- image inputs only produce image outputs
- incompatible document input/output combinations fail early

!!! note
    Graph inspection and graph build commands load config through `load_config_for_graph`, which skips the `outputs` requirement. That allows graph-driven commands to discover targets from the transform graph.

## Input formats

Auto-detection is based on file extension:

| Extension | Format |
|---|---|
| `.md`, `.markdown` | `markdown` |
| `.docx` | `docx` |
| `.html`, `.htm` | `html` |
| `.epub` | `epub` |
| `.rst` | `rst` |
| `.tex` | `latex` |

## Output types

Document outputs are first-class:

- `html`
- `pdf`
- `docx`

Audio and image outputs are broader. Representative examples include:

- Audio: `wav`, `flac`, `mp3`, `aac`, `ogg`, `opus`, `wma`, `ac3`, `ec3`, `dts`, `midi`
- Image: `jpeg`, `png`, `webp`, `avif`, `gif`, `bmp`, `tiff`, `exr`, `hdr`, `jp2`, `jxl`, `dds`, `ico`

For the full generated list of config values, graph identifiers, file extensions, and encode support, see [Supported Formats](supported-formats.md).

## Templates

`template: default` resolves to `templates/default.html` for HTML output. Renderflow validates configured templates before rendering so missing templates fail fast.

## Audio profiles

Audio outputs can specify a named profile, for example:

```yaml
outputs:
  - type: mp3
    profile: 320k
  - type: wav
    profile: broadcast
```

Supported aliases in code include names such as `streaming_128k`, `hq_320k`, `cd_quality`, `broadcast`, `lossless`, and several codec-specific presets.

## Transform YAML file

If `transforms` is set, it should point to a YAML file with a top-level `transforms:` list.

```yaml
transforms:
  - name: md-to-html
    program: pandoc
    args: ["{input}", "-o", "{output}"]
    from: markdown
    to: html
    cost: 0.5
    quality: 0.9
```

The transform definition schema is covered in the [AI guide](../ai-guide/configuration.md), [plugin guide](../plugin-guide/developing.md), and [graph engine docs](../architecture/graph-engine.md).
