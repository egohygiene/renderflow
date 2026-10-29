# #379 editorial profile checkpoint

Date: October 29, 2026. Base: `1a193eb3924ad20ee5add77c2c495bb32e9bb900`.
The maintainer authorized merging #467 and implementing #379. #467 is merged;
#466 remains open for executable acceptance. No release or tag is authorized here.

## Architecture and implemented source

- Inspected the current `SpecV2`/`DerivativeProfile`, ordinary planner, strategy
  adapter, shared process/font/artifact services and existing LaTeX components.
- The bundled `editorial` profile uses `renderflow.profile/v1`; YAML and CLI
  selection share its definition. Spec-local definitions keep precedence.
- `editorial-v1` is a strategy preset for direct Markdown→HTML/PDF/DOCX/EPUB.
  Exact original Markdown is an identity output, not a normalized/package claim.
- The existing Pandoc AST/filter mechanism validates `renderflow.editorial/v1`
  metadata and handles hero figures, local images and semantic notes. No new
  parser framework, plugin registry or content-generation service is introduced.
- Bundled templates, CSS, filter, shared LaTeX components and licensed font files
  are staged from binary bytes. The entire presentation bundle has a SHA-256
  identity recorded on artifacts. User font registries use the existing model.
- The selected run requires Pandoc >=3.6.1; PDF requires cached-only Tectonic.
  Source/template/config identities remain distinct from provider qualification.
- External source images/font registries are not snapshotted at planning, so
  editorial rendering does not reuse cache/checkpoints.

## Review and qualification status

Source review only; regression cases are authored and unrun. The deterministic
fixture authoring recipe was used to create the checked-in synthetic PNG. Local
font files were copied with their observed package version and license notice;
source asset hashes are records, not native-render verification.

No tests, compiler/build/Cargo resolution, lint/format/schema/docs validators,
product CLI/native/provider execution or CI dispatch/polling have run. No
dependency or lockfile changes. #379 acceptance boxes remain open.

Deferred verification must cover actual CLI/SDK discovery and execution, invalid
metadata/images/raw content, missing providers/cache/fonts, source preservation,
HTML/PDF rendering and accessibility, DOCX/EPUB application fidelity, reproducible
reruns under pinned tools, process cancellation/cleanup and supported platforms.
Core validation alone cannot establish visual quality or PDF/UA compliance.

Known limits: English presentation labels; regular font roles are pinned but
bold/italic/math/script behavior needs qualification; no OS process sandbox;
DOCX reference fonts are application-owned; no plan-time image snapshot or image
sidecar attestation; raw HTML/TeX and non-PNG/JPEG images are outside this profile.

## Next issue

#378 can implement the writing/polishing agent against the guide's concrete
Markdown metadata contract. Copilot remains optional and absent from rendering.
Baseline/release qualification remains with #458/#419.
