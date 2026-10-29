# #466 — configured command plugins

Date: October 29, 2026. Base: `93c69b37432234201e92ab5af7580c8d3e24b106`.
Branch: `feat/configured-command-plugins`. Status: source-authored, unqualified.

Alan approved making the existing plugin system usable through ordinary CLI/config
after clarifying that the custom extractor should reuse it. This checkpoint is
separate from the optional editorial styling profile #379 and follows closed #357.

## Implementation

- Transform registries admit explicit `plugins` declarations and per-transform
  JSON-object `config`. The host rejects duplicates, unknown IDs, unsupported
  routes/cardinality, mixed backends and misplaced fields before execution.
- `CommandPlugin` implements the existing `PluginTransformV2` contract. Its one
  input/one output file invocation uses `PluginArtifactStore`, `PluginProcessService`
  and `PluginV2ArtifactAdapter`; no dynamic Rust ABI, package discovery or new parser
  framework. The existing descriptor schema/identifier is unchanged.
- Arguments use exact input/output/config placeholders and registry-relative script
  references. Config is a bounded file, not argv interpolation. Programs are trusted
  local code. Deadline, output capture, output-size polling and cancellation are
  delegated to the shared process service; no OS/RSS/aggregate scratch sandbox claim.
- ArtifactTransform has an additive default cancellation-aware entry point. The
  executor supplies its current flag; the configured command adapter forwards it
  into v2 runtime options. Existing adapters retain their previous behavior.
- Plugin references bind by exact selected edge, preserving same-format variants.
  Registry identity and native SDK metadata retain configuration identity, source
  lineage, plugin version, capability and loss. Program/script/dependency bytes are
  not pinned or frozen; command cache and checkpoint reuse are disabled explicitly.
- CLI `plugin --transforms FILE` list/info/validate/doctor use declaration-only
  inspection. Doctor names its metadata-only limit. Existing tool commands/normal
  graph planning may execute bounded version probes; neither was invoked here.
- The Python 3.9+ standard-library example counts known literal names in UTF-8 text,
  retains page/line/character locations and exports counts as CSV. It does not infer
  entities, read PDFs, perform OCR, render articles or create Excel workbooks.
- Seven Rust cases, three Python cases and revised legacy ambiguity cases are
  authored and unrun. Cases cover no-execution inspection, invalid registrations,
  variant identity/cache bypass, normal publication, failed outputs, running-child
  cancellation, input bounds and literal matching/CSV shape semantics.

## Decisions and compatibility

The extension uses the existing transform registry instead of a second package
manager or parser catalog. File-based direct invocation makes non-Rust local code
possible without exposing a Rust ABI or treating arbitrary strings as binary data.
Configuration field meanings remain the plugin's responsibility; host validation
covers only the bounded JSON-object envelope and invocation/route declarations.

YAML backend ambiguity is now refused rather than applying legacy AI/plugin/program
precedence. Unknown registry/transform fields are refused. Legacy host plugins still
work through the text loader; configured command plugins require the artifact graph.
The v2 serialized plugin descriptor contract stays unchanged. No dependencies,
lockfile, release workflow or CI gates change.

## Qualification still owed

Tests, compilation/build/Cargo resolution, lint/format/schema/docs validation,
plugin/native/CLI/provider execution and CI were not run. Do not close #466 or
claim successful example outputs from source review. No release/tag was created.

When authorized, reconcile #458's baseline, compile the workspace, run the focused
Rust/Python cases and invoke the example through both CLI and SDK. Check source
preservation, two native publications, partial/missing-provider outcomes, cancellation
cleanup and platform behavior. Verify schema/docs and actual provider/resource
semantics independently. Existing CSV shape validation is conservative and may reject
correctly quoted commas; this remains a concrete qualification/validator follow-up,
not an excuse to claim broad CSV or XLSX support.

Generic PDF text extraction needs its own supported provider and retained page map.
Current screenplay recovery policies reject mixed custom transforms. An eventual
editorial template can consume the structured report without owning extraction.
