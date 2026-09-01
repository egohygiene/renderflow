# Image super-resolution

Renderflow models AI image super-resolution as an explicit local transform capability rather than a hidden image-export option.

The first built-in provider is Upscayl's NCNN backend:

- provider: `tool.upscayl-ncnn`
- capability: `image.super_resolution`
- executable candidates: `upscayl-ncnn`, `upscayl-bin`

## Why model variants are first-class

Super-resolution models make different trade-offs. Renderflow therefore keeps the selected model as stable transform identity and reproducibility evidence instead of treating `-n <MODEL>` as an opaque command flag.

A maximal v2 request can enumerate every enabled model candidate for later comparison. An exact request can name one variant. The unified planner work in #354 consumes this selection API so exact and exhaustive requests use the same execution architecture.

```yaml
schema: renderflow/v2

sources:
  - id: source.page
    path: page.png
    media_type: image/png

targets:
  exact:
    - id: target.upscale
      capability: image.super_resolution
      variant: variant.upscayl-ncnn.digital-art-4x

execution:
  ai: local_only
  network: deny
```

For exhaustive evaluation:

```yaml
targets:
  all_reachable: true
  include:
    capabilities: [image.super_resolution]

execution:
  ai: local_only
```

Use `include.variants` or `exclude.variants` to constrain an exhaustive run without writing duplicate transform definitions.

## Runtime requirements

Upscayl-NCNN runs locally and does not require network access during transformation, but it does require:

1. an Upscayl NCNN executable (`upscayl-ncnn` or packaged `upscayl-bin`),
2. a compatible Vulkan-capable runtime/GPU backend,
3. complete NCNN model pairs (`<model>.param` and `<model>.bin`).

`renderflow tools list` reports executable availability. Model material and GPU readiness are separate evidence because a binary can exist on a host that cannot actually execute the selected model.

When `vulkaninfo --summary` is available Renderflow can proactively verify Vulkan readiness. If `vulkaninfo` is absent, readiness is reported as unverified rather than falsely declaring every non-Linux or MoltenVK setup broken.

## Inspect variants

```bash
renderflow tools inspect tool.upscayl-ncnn
renderflow tools variants tool.upscayl-ncnn
renderflow tools variants tool.upscayl-ncnn --models-dir /path/to/models
renderflow tools variants tool.upscayl-ncnn --models-dir /path/to/models --format json
```

Without `--models-dir`, the command lists the canonical built-in model identities and policy metadata. With a model directory, it also records materialization state and SHA-256 evidence for `.param`/`.bin` pairs and discovers custom models.

## Custom models

Custom model pairs are discovered without being promoted into the canonical built-in catalog. Their variant ID is derived deterministically from the model filename, and their model digest is derived from the actual `.param` and `.bin` bytes.

For example:

```text
my-comic-x2.param
my-comic-x2.bin
```

becomes a runtime variant similar to:

```text
variant.upscayl-ncnn.my-comic-x2
```

Renderflow infers native scale from conventional `x2`/`2x`, `x3`/`3x`, or `x4`/`4x` naming. When the native scale cannot be inferred, the model remains usable as explicit runtime material but carries an actionable `model_scale.unknown` diagnostic.

## Native scale versus requested scale

The canonical default Upscayl models are native x4 models. When a requested scale differs, the distinction is retained in evidence as native model scale versus post-processing/requested scale. Renderflow does not present an emulated x2/x3 output as though a native x2/x3 model produced it.

## Reproducibility evidence

Selected super-resolution variants can contribute the following material to the toolchain/cache fingerprint and artifact provenance:

- provider and capability IDs,
- stable variant/model ID,
- model `.param` and `.bin` SHA-256 material digest,
- Upscayl executable identity and executable SHA-256 when resolvable,
- native scale and requested/post scale,
- output format, compression, tile size, and TTA configuration,
- selected GPU ID and runtime backend line when reported by Upscayl.

GPU/Vulkan implementation identity is evidence because AI super-resolution should not be assumed bit-for-bit portable across hardware backends.

## Publication licensing

Model licensing is independent of the Upscayl application/backend license. The canonical model catalog intentionally records commercial-use policy instead of assuming every bundled model is interchangeable for publication.

See [Upscayl model reference](upscayl-models.md) for the current catalog. A model marked `unknown` requires its own license check before commercial publication. A model marked `prohibited` is currently labeled Non-Commercial by Upscayl and should not be selected for commercial output without separately establishing permission.

## CI and real-GPU smoke testing

Ordinary CI must not assume GitHub-hosted runners expose a supported Vulkan GPU. Renderflow therefore tests catalog discovery, checksums, policy filtering, variant expansion, toolchain evidence, and command construction using fixtures/fake probes.

For a real host, first verify the provider and model material:

```bash
renderflow tools inspect tool.upscayl-ncnn
renderflow tools variants tool.upscayl-ncnn --models-dir /path/to/models
vulkaninfo --summary
```

Then run the ignored real-provider smoke test with explicit paths:

```bash
RENDERFLOW_UPSCAYL_SMOKE_INPUT=/path/to/input.png \
RENDERFLOW_UPSCAYL_MODELS_DIR=/path/to/models \
RENDERFLOW_UPSCAYL_EXECUTABLE=upscayl-bin \
cargo test -p renderflow upscayl_real_provider_smoke -- --ignored --nocapture
```

This same command can move to a GPU-enabled self-hosted CI runner later without creating a second test path.
