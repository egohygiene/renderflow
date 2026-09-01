# Configuration

Renderflow supports two explicit configuration contracts:

- **v1 compatibility** — the existing unversioned `input` / `outputs` configuration used by current build commands.
- **spec v2** — the versioned `schema: renderflow/v2` execution-intent contract for arbitrary sources, derivative profiles, maximal artifact forests, explicit execution policy, and deterministic output layout.

Renderflow never silently reinterprets a declared schema version. Unversioned files are treated as v1 compatibility files; unsupported declared schema identifiers are rejected actionably.

!!! important
    Issue #353 defines the v2 intent contract, validation, migration, and generated schema. The canonical planner/executor consumes this model in the follow-up unification work tracked by #354. Existing v1 builds remain backward compatible in the meantime.

## Spec v2

A minimal v2 document looks like this:

```yaml
schema: renderflow/v2
sources:
  - id: source.main
    path: input.md
targets:
  exact:
    - role: web
      format: html
```

V2 can also express multiple immutable sources, ordered collections, named derivative profiles, `all_reachable` expansion, include/exclude selectors, resource budgets, tool/transform allowlists and denylists, deterministic/local/offline requirements, network and AI policy, validation requirements, fidelity thresholds, and deterministic output layout.

Use the CLI to validate, migrate, or export the canonical schema:

```bash
renderflow spec validate --config renderflow.yaml
renderflow spec migrate --config renderflow.yaml --output renderflow.v2.yaml
renderflow spec schema --output schemas/renderflow-v2.schema.json
```

See the generated [Spec v2 Reference](spec-v2-reference.md) for the canonical field matrix and complete example.

## V1 compatibility format

### Minimal config

```yaml
input: input.md
output_dir: dist
outputs:
  - type: html
```

### Full document-oriented example

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

### Key reference

| Key | Required | Default | Notes |
|---|---|---|---|
| `input` | Yes | none | Source file path |
| `input_format` | No | auto-detect, then `markdown` | Supported values: `markdown`, `docx`, `html`, `epub`, `rst`, `latex` |
| `output_dir` | No | `dist` | Destination directory |
| `variables` | No | `{}` | String-to-string map used by `{{key}}` placeholders |
| `optimization` | No | `balanced` | Planner mode for graph-aware commands |
| `transforms` | No | none | Path to a YAML transform graph / transform registry file |
| `outputs` | Yes for standard builds | empty | List of output definitions |

#### `outputs[]`

Each output item maps to the v1 `OutputConfig` compatibility model.

| Key | Required | Notes |
|---|---|---|
| `type` | Yes | `html`, `pdf`, `docx`, supported audio formats, or supported image formats |
| `template` | No | Template name looked up in `templates/` |
| `profile` | No | Audio quality profile for audio outputs only |

## V1 validation rules

Renderflow validates several constraints before running a standard v1 build:

- `input` must not be empty
- `outputs` must contain at least one item
- output types must be known
- document inputs cannot mix in audio/image outputs
- audio inputs only produce audio outputs
- image inputs only produce image outputs
- incompatible document input/output combinations fail early

These family-specific v1 gates are compatibility behavior. They are not constraints on the v2 artifact-forest model.

!!! note
    Graph inspection and graph build commands currently load v1 config through `load_config_for_graph`, which skips the `outputs` requirement. Canonical v1/v2 planner unification is tracked by #354.

## Input formats

V1 auto-detection is based on file extension:

| Extension | Format |
|---|---|
| `.md`, `.markdown` | `markdown` |
| `.docx` | `docx` |
| `.html`, `.htm` | `html` |
| `.epub` | `epub` |
| `.rst` | `rst` |
| `.tex` | `latex` |

V2 source declarations can carry explicit `format` / `media_type` intent or request detection. Universal multi-signal source inspection is expanded by #365.

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

V1 audio outputs can specify a named profile, for example:

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
