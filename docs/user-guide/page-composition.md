# Layered and localized page composition

The `composition` commands turn an explicitly authored page composition into
per-locale SVG, HTML, and accessible transcript files. Art, shapes, text content,
and typography remain separate inputs. The source language, default language,
and every translation use the same planner and renderer.

This is the **authored, unqualified #406 implementation checkpoint**. Tests,
builds, visual review, native execution, and CI were deferred during the sprint.
A serialized output is not proof of correct glyph shaping, readable print size,
font licensing, or acceptance by a design application.

## Start with an authored composition

The canonical input declares `renderflow.page-composition/v1`. It references
SHA-256-pinned local files instead of embedding translations into artwork:

| Input | Purpose |
| --- | --- |
| Composition manifest | Page geometry, paint order, stable layer IDs, reading order, locale declarations, and provenance |
| Content document for each locale | Text keyed by content ID, source revision, review state, direction, and writing mode |
| Font registry | Pinned local fonts, typography roles, fallback policy, and licensing metadata |
| Shared art assets | Raster or bounded vector artwork referenced by stable asset IDs |
| Original source references | Retained source identity for original, flattened, or reconstructed material |

Composition references resolve from the manifest's directory; font and license
paths resolve from the font registry's directory. References must stay within
their directory and cannot traverse through symlinks. Keep these files together.
The loader reads and verifies the local pins. It does not fetch missing files,
install fonts, translate text, or separate flattened artwork.

Font registries must be JSON in this profile. Font bytes and the registry are
pinned; license files receive an observed hash because the current font
registry contract has no license-file digest field.

Declare each locale explicitly, including the source and default locale. There
is no implicit substitution of English or another language when a translation
is missing. A translation records the source revision it was reviewed against,
so editing source text can expose a stale translation instead of silently
publishing it as current. Unreviewed content, stale source revisions, missing
text, and unknown text IDs block that locale's branches. Unreadable or mismatched
file pins prevent loading the composition.

The model supports raster and vector asset layers, rectangles and ellipses,
and editable text layers. Layers have normalized bounds, optional clipping,
opacity, and normal blending. Text layers select semantic font roles and
explicit font-size, minimum-size, line-height, padding, alignment, and color
settings. An explicit reading order is independent of visual paint order.

## Inspect and plan without output writes

```bash
renderflow composition inspect \
  --input "examples/page-composition/page.composition.json"

renderflow composition plan \
  --input "examples/page-composition/page.composition.json" \
  --locales "en,de" \
  --targets "svg,html,transcript_json"
```

The included [example](../../examples/page-composition/README.md) is a structural
fixture with `en`, `de`, `ar`, and `ja` locales. Its synthetic font is not a
usable typeface: replace it with a real, licensed, pinned local font before
assessing the page visually. No fixture run or visual result is claimed by
the checked-in example. Arabic RTL and Japanese
vertical content are intentional unsupported visual cases; selecting
`all_declared` retains those blocked branches.

Both commands print JSON to standard output and leave the source, output, and
cache directories unchanged. They read the manifest and referenced local
assets and fonts. `inspect` emits the default plan: all declared locales and
the default `svg,html,transcript_json` targets.

`--locales` accepts exactly one locale, a comma-separated subset, or
`all_declared` (the default). Unknown locales do not fall back. Empty and
duplicate selector values are rejected. `--targets` accepts the same
comma-separated syntax; every selected target is required for a complete run.

Planning retains unavailable branches and their reasons. A plan containing an
unsupported target is still a successfully emitted plan; inspect its branch
diagnostics before executing. Invalid input or unreadable/mismatched pinned
files cause an error instead of a fabricated plan.

## Render into a new bundle

```bash
mkdir -p "dist"

renderflow composition render \
  --input "examples/page-composition/page.composition.json" \
  --locales "en,de" \
  --targets "svg,html,transcript_json,transcript_text" \
  --output-dir "dist/page-first-run" \
  --cache-dir "dist/page-composition-cache"
```

The output directory must be new and its parent must already exist. Renderflow
does not replace the original artwork, content documents, fonts, or an existing
bundle. The optional cache is a separate local directory; reuse verifies the
cached payload before publication. Without `--cache-dir`, the bundle uses its
own `.artifact-store`.

| Bundle member | Purpose |
| --- | --- |
| `composition-plan.json` | The exact plan used by this run |
| `composition-run.json` | Run receipt written after publication, including branch outcomes and artifact evidence |
| `inputs/<sha256>` | Captured input bytes, shared by content identity within the bundle |
| `outputs/<locale>/page.svg` | Published SVG for a successful selected SVG branch |
| `outputs/<locale>/page.html` | Published HTML for a successful selected HTML branch |
| `outputs/<locale>/page.transcript.json` | Published structured transcript |
| `outputs/<locale>/page.transcript.txt` | Published plain transcript |

Using an external cache does not remove the retained inputs and published
outputs from the bundle. Preserve the receipt and plan with them.
The hashed input copies retain evidence; keep the original package for editing
and reloading its relative paths. Cache keys conservatively include the entire
captured package, so changing one locale can invalidate other output keys while
immutable artwork bytes remain shared. Cache reuse does not approve a candidate.

The command prints its run receipt to standard output. A partial, failed, or
cancelled run returns nonzero after emitting its available receipt. Successful
branches remain distinguishable from unavailable or failed branches. A command
error before a run receipt can be created is not evidence of publication.

| Target | Checkpoint behavior |
| --- | --- |
| `svg` | Deterministic serialized page markup with selectable text and embedded image/font bytes |
| `html` | HTML page presentation retaining accessible reading order |
| `transcript_json` | Structured localized text in declared reading order |
| `transcript_text` | Plain localized text in declared reading order |
| `png`, `pdf` | Explicit unsupported branches; no implicit browser or image renderer |
| `ora`, `psd`, `idml` | Explicit unsupported interchange branches; no claim of native editability |

Selecting `svg,pdf`, for example, can publish the supported SVG branch while
retaining the unavailable PDF branch and reporting an incomplete run. Do not
interpret an output filename alone as proof that every requested branch ran.

## Layout and review boundaries

The visual profile supports left-to-right, horizontal text. RTL and vertical
layout remain explicitly unsupported visual branches. Text keeps its authored
line breaks and size; the renderer does not wrap, shrink, translate, or fit it
automatically. Padding and line height determine vertical line boxes, and
known vertical overflow blocks publication.
Text containers outside the declared safe area or excluded by their clip also
block the visual branch. A nonzero gutter reports an unresolved binding-side
warning; the renderer does not guess which side needs the gutter.

Transcript outputs preserve text and declared reading order without applying
those visual layout restrictions. This retains readable content for review
even when its requested visual branch is unsupported.

Layer bounds and clip rectangles use the full page canvas as their reference.
Art plates stretch to their authored bounds; preserve the original aspect
ratio when choosing those bounds if stretching is unwanted.

The planner preserves deterministic layout decisions and reports constraints
that this profile cannot satisfy. Locale-specific overrides must identify a
layer and record their author and reason. They are explicit reviewed inputs,
not silent automatic repositioning or generated text edits.

Font and layout decisions are local. Pinning a font establishes which bytes
were selected; it does not prove glyph coverage, complex-script shaping,
bidirectional behavior, or final ink bounds. SVG/HTML viewers can differ even
when Renderflow serializes the same bytes. Review the rendered page in the
intended viewer and at its intended size before publication.

Trim, bleed, safe area, and gutter values describe page geometry. They do not
establish PDF/X, a printer profile, or commercial print acceptance. Native
PNG/PDF generation and native design-file interchange remain separate work.

For flattened or reconstructed sources, keep the original references and
honest provenance flags. The composition path does not perform AI inpainting,
OCR, art extraction, translation, or recovery of lost editable layers.
Human review remains explicit; generating a derivative does not approve it.

## Consumption by other projects

Use the same SDK composition request for inspection, planning, and execution.
Preserve the canonical inputs and the full native run evidence with the
published files. A consumer should use recorded branch state and actual output
hashes to decide what is available, and carry review flags forward.

This candidate composition contract is separate from `renderflow/v2` build
specs and the general artifact forest. It does not extend `flow.artifact/v1`
or imply that a released Flow adapter already consumes the new contract. See
the [Flow artifact handoff](../architecture/flow-artifact-forest.md) for the
existing distinction between planned inventory and actual publication.

Issue [#406](https://github.com/egohygiene/renderflow/issues/406) remains open
for execution and visual qualification, broader typography, and the remaining
native output and interoperability requirements.
The [format evaluation](../work/406/FORMAT_EVALUATION.md) explains the canonical
manifest choice and the boundaries of each proposed interchange format.
