# Troubleshooting

This guide covers the most common Renderflow setup and runtime problems.

## `renderflow: command not found`

- confirm the binary is installed,
- re-open your shell after package-manager installs,
- run `renderflow env` to inspect PATH-related details.

If you installed with the portable script, ensure `RENDERFLOW_INSTALL_DIR` is on your `PATH`.

## Pandoc or Tectonic errors

Renderflow shells out to Pandoc for document rendering, and PDF builds may also require a TeX engine depending on your Pandoc setup.

- run `renderflow doctor --strict`
- verify `pandoc --version`
- retry with `renderflow build --verbose`

## FFmpeg-backed output fails

Audio and image rendering depends on FFmpeg.

- install FFmpeg,
- verify `ffmpeg -version`,
- confirm the requested output type appears in [Supported Formats](supported-formats.md),
- if the format is listed as `Encodable = No`, choose an encodable target instead.

## `No output formats configured`

Standard builds require at least one `outputs[]` entry in `renderflow.yaml`.

If you intended to use graph mode, add a `transforms:` file and run one of:

```bash
renderflow build --target pdf
renderflow build --all
```

## Graph target is unreachable

If `--target` or `--all` fails:

- confirm the `transforms` file path is correct,
- ensure the source format and requested target are connected,
- run `renderflow graph explain --config renderflow.yaml`,
- run `renderflow inspect --target <format>` to inspect the planned path.

## AI transform authentication issues

Prefer environment-based secrets:

- use `api_key_env`, not plaintext `api_key`,
- verify the environment variable is exported,
- run `renderflow ai doctor`,
- confirm endpoint and model names in the AI config.

## Plugin commands show an empty registry

The standalone CLI initializes an empty `PluginRegistry`. Plugin commands only show data when an embedding host registers plugins before invoking Renderflow APIs.

See the [Plugin Guide](../plugin-guide/overview.md) for the registration flow.
