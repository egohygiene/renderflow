# Creative capability matrix

This page and its [machine-readable JSON](creative-capabilities.json) are
generated from [the shared route catalog](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/data/creative-routes.json).
[Creative doctor](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/creative_doctor.rs)
embeds the same declarations. Edit the catalog and regenerate these files;
do not maintain a separate capability table by hand.

The catalog declares **13 routes**: 13 `authored_unqualified`.
Authored capability declarations shared with creative doctor, not local provider observations or proof that a configured execution will succeed. No conversion, inference, installation, or discovery is performed when generating this matrix.

`authored_unqualified` means code exists for the stated scope; runtime
qualification is deferred. An installed provider, successful doctor
discovery, or a recognized format identifier does not select a provider,
enable a conversion, or establish fidelity. Some rows are standalone
operations rather than graph edges. The guides describe the applicable
surface, full configuration contract, bounds, and evidence to retain.

## Declared routes

| Route | Implementation | Declared providers | Usage |
| --- | --- | --- | --- |
| `ocr.extract.text` | `authored_unqualified` | `tool.pdftotext`, `tool.pdftoppm`, `tool.tesseract` | [Guide](../screenplay-recovery.md) |
| `pdf.extract.text` | `authored_unqualified` | `tool.pdftotext` | [Guide](../screenplay-recovery.md) |
| `screenplay.pdf.afterwriting` | `authored_unqualified` | `tool.node`, `tool.afterwriting` | [Guide](screenplay-pdf.md) |
| `screenplay.recover.structure` | `authored_unqualified` | `tool.renderflow-screenplay-recovery` | [Guide](../screenplay-recovery.md) |
| `screenplay.recovery.export` | `authored_unqualified` | `tool.renderflow-screenplay-recovery` | [Guide](../screenplay-recovery.md) |
| `screenplay.scripttool` | `authored_unqualified` | `tool.scripttool` | [Guide](scripttool.md) |
| `speech.align` | `authored_unqualified` | No external provider | [Guide](speech.md#export-subtitles-using-an-explicit-original-media-time-map) |
| `speech.audio.inspect` | `authored_unqualified` | `tool.ffprobe` | [Guide](speech.md#select-audio-from-a-video-or-another-audio-file) |
| `speech.audio.prepare` | `authored_unqualified` | `tool.ffprobe`, `tool.ffmpeg` | [Guide](speech.md#select-audio-from-a-video-or-another-audio-file) |
| `speech.transcribe` | `authored_unqualified` | `tool.whisper_cpp` | [Guide](speech.md) |
| `subtitle.convert.plain` | `authored_unqualified` | No external provider | [Guide](subtitles.md) |
| `subtitle.extract.plain` | `authored_unqualified` | `tool.ffprobe`, `tool.ffmpeg` | [Guide](subtitles.md#inspect-and-extract-existing-subtitle-streams) |
| `subtitle.streams.inspect` | `authored_unqualified` | `tool.ffprobe` | [Guide](subtitles.md#inspect-and-extract-existing-subtitle-streams) |

Provider IDs refer to the selected declarations in the
[tool registry](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/data/tool-registry.yaml).
This generated document has no live availability results. Doctor combines
these shared route requirements with current registry discovery observations;
runtime configuration separately selects provider bytes and execution policy.
An in-process provider's availability does not qualify its outputs.
See [doctor](creative-toolchain.md) and
[setup and qualification](creative-toolchain-setup.md).

## Scope and configuration requirements

The following identifiers are the doctor's declared configuration and
input-policy requirements. Some identify prerequisites rather than literal
configuration keys. They are not an exhaustive configuration schema; use
the linked guide for an executable request.

### `ocr.extract.text`

Explicitly configured, bounded per-page PDF OCR fallback is authored for pages without usable embedded text. Pinned rasterizer, recognizer, and language data are required; discovery does not qualify OCR, and recovered structure still requires review.

Declared requirements:

- `execution.screenplay_recovery.ocr`
- `ocr.language_data`

Sources: [crates/renderflow-core/src/screenplay/recovery/provider.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/recovery/provider.rs).

### `pdf.extract.text`

Bounded text-bearing PDF extraction for screenplay recovery is authored. Execution uses an explicit pinned provider and preserves page boundaries and extraction evidence; PATH discovery does not select that provider or qualify recovery.

Declared requirements:

- `execution.screenplay_recovery`
- `execution.screenplay_recovery.pdftotext`
- `execution.screenplay_recovery.allow_unqualified`
- `bounded_local_pdf`

Sources: [crates/renderflow-core/src/screenplay/recovery/provider.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/recovery/provider.rs).

### `screenplay.pdf.afterwriting`

Direct original strict ASCII Fountain to PDF is authored with fixed A4/US Letter profiles and independent analysis/PDF inspection. Node and Afterwriting remain unobserved by doctor: explicit pinned clear-environment preflight reads package identity without a fabricated provider version command. Chained native imports, runtime/visual fidelity, and reproducibility remain unqualified.

Declared requirements:

- `execution.screenplay_pdf`
- `execution.screenplay_pdf.node_executable`
- `execution.screenplay_pdf.node_sha256`
- `execution.screenplay_pdf.expected_node_version`
- `execution.screenplay_pdf.package_root`
- `execution.screenplay_pdf.package_tree_sha256`
- `execution.screenplay_pdf.expected_provider_version`
- `execution.screenplay_pdf.profile`
- `execution.screenplay_pdf.allow_unqualified`
- `strict_ascii_fountain_body`
- `screenplay.fonts`

Sources: [crates/renderflow-core/src/screenplay/afterwriting.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/afterwriting.rs); [crates/renderflow-core/src/screenplay/afterwriting_graph.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/afterwriting_graph.rs).

### `screenplay.recover.structure`

Deterministic heuristics recover a validated canonical screenplay from bounded PDF extraction or plain text. Per-element confidence and ambiguities record heuristic evidence rather than calibrated probabilities. Recovery is always lossy and review-required; AI reconstruction is unsupported and no execution setting approves the result.

Declared requirements:

- `execution.screenplay_recovery`
- `execution.screenplay_recovery.allow_unqualified`
- `screenplay.recovery.review`

Sources: [crates/renderflow-core/src/screenplay/recovery/model.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/recovery/model.rs); [crates/renderflow-core/src/screenplay/recovery/graph.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/recovery/graph.rs).

### `screenplay.recovery.export`

Bounded canonical recovery candidates can produce supported body-only Fountain, FDX, Fade In, and OSF derivatives with recovery lineage and mandatory review retained. These authored writers do not claim original-document reconstruction or native runtime qualification.

Declared requirements:

- `validated_recovery_candidate`
- `screenplay.recovery.review`

Sources: [crates/renderflow-core/src/screenplay/recovery/export.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/recovery/export.rs).

### `screenplay.scripttool`

Bounded body-only FDX/OSF/Fade In imports to Fountain are authored. Execution requires an explicitly pinned scripttool 0.0.10, opt-in policy, and independent semantic comparison. Doctor's PATH observation does not select that executable or add graph edges; reverse exports and runtime qualification remain unavailable.

Declared requirements:

- `execution.screenplay`
- `execution.screenplay.executable`
- `execution.screenplay.executable_sha256`
- `execution.screenplay.allow_unqualified`
- `supported_body_only_native_import`

Sources: [crates/renderflow-core/src/screenplay/provider.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/provider.rs); [crates/renderflow-core/src/screenplay/graph.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/screenplay/graph.rs).

### `speech.align`

The standalone offline alignment exporter needs no external executable. It verifies saved evidence and applies an explicit operator map; it does not infer or attest original-video synchronization.

Declared requirements:

- `pinned_saved_result_receipt`
- `pinned_explicit_time_map`

Sources: [crates/renderflow-core/src/speech/alignment.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/speech/alignment.rs).

### `speech.audio.inspect`

The standalone audio inventory is authored and uses explicitly pinned tools; PATH discovery does not select those tools.

Declared requirements:

- `pinned_ffprobe`
- `source_sha256`

Sources: [crates/renderflow-core/src/speech/audio.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/speech/audio.rs).

### `speech.audio.prepare`

Selected-stream preparation is authored with explicit pins and stream selection. Preparation alone does not establish original-video subtitle alignment.

Declared requirements:

- `pinned_ffprobe`
- `pinned_ffmpeg`
- `source_sha256`
- `explicit_audio_stream`

Sources: [crates/renderflow-core/src/speech/audio.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/speech/audio.rs).

### `speech.transcribe`

The local recognition adapter is authored but unqualified. A discovered executable is not a selected pinned provider/model or proof that inference can run.

Declared requirements:

- `pinned_speech_provider`
- `speech.model.configuration`
- `speech.model.license`
- `strict_pcm_wav`

Sources: [crates/renderflow-core/src/speech/provider.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/speech/provider.rs); [crates/renderflow-core/src/speech/graph.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/speech/graph.rs).

### `subtitle.convert.plain`

Built-in bounded SRT/WebVTT and canonical timed-transcript conversion needs no external executable. The opt-in graph/SDK route is authored; runtime qualification remains deferred.

Declared requirements:

- `execution.subtitles`
- `supported_plain_timed_text`

Sources: [crates/renderflow-core/src/timed_text/codec.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/timed_text/codec.rs); [crates/renderflow-core/src/timed_text/graph.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/timed_text/graph.rs).

### `subtitle.extract.plain`

The opt-in plain-subtitle graph route is authored and requires explicit stream selection; runtime qualification remains deferred.

Declared requirements:

- `execution.subtitles`
- `explicit_subtitle_stream`
- `supported_plain_subtitle_codec`

Sources: [crates/renderflow-core/src/timed_text/provider.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/timed_text/provider.rs).

### `subtitle.streams.inspect`

Bounded subtitle inventory is authored; finding FFprobe does not qualify a particular container or codec.

Declared requirements:

- `supported_local_container`

Sources: [crates/renderflow-core/src/timed_text/provider.rs](https://github.com/egohygiene/renderflow/blob/main/crates/renderflow-core/src/timed_text/provider.rs).

## Explicitly unsupported claims and directions

These restrictions do not introduce conversion edges or replace the route
policy. They distinguish rejected paths from authored, unqualified paths.

### `screenplay.general_canonical_export`

General conversion of an original Fountain source to FDX, Fade In, OSF, or canonical screenplay JSON is not an exposed export route. General canonical screenplay JSON to native formats is also unavailable. The internal body parser and canonical codec do not create those routes; recovery-only exporters require a replayable recovery candidate and retain lossy, review-required evidence.

Sources: [docs/user-guide/screenplay.md#loss-and-round-trip-limits](https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/screenplay.md#loss-and-round-trip-limits); [docs/screenplay-recovery.md](https://github.com/egohygiene/renderflow/blob/main/docs/screenplay-recovery.md).

### `screenplay.scripttool.reverse`

Fountain to FDX, OSF, and Fade In through scripttool is not adopted. The independent recovery-candidate exporters are a different, review-required path; their existence does not qualify scripttool's reverse converters.

Sources: [docs/work/424/PROVIDER_EVALUATION.md](https://github.com/egohygiene/renderflow/blob/main/docs/work/424/PROVIDER_EVALUATION.md); [docs/screenplay-recovery.md](https://github.com/egohygiene/renderflow/blob/main/docs/screenplay-recovery.md).

### `screenplay.scripttool.json`

Fountain to scripttool JSON is an unadopted inspection candidate, not canonical Renderflow screenplay JSON and not an execution edge.

Sources: [docs/work/424/PROVIDER_EVALUATION.md](https://github.com/egohygiene/renderflow/blob/main/docs/work/424/PROVIDER_EVALUATION.md).

### `screenplay.pdf.native_chain`

The afterwriting route accepts a direct original strict ASCII Fountain source only. Direct native screenplay inputs and chaining a native import into this PDF graph route are not adopted.

Sources: [docs/user-guide/screenplay-pdf.md](https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/screenplay-pdf.md).

### `screenplay.recovery.lossless`

PDF/text recovery does not reconstruct the original native document, typography, pagination, or editing history. Recovery and its derivatives stay lossy and review-required.

Sources: [docs/screenplay-recovery.md](https://github.com/egohygiene/renderflow/blob/main/docs/screenplay-recovery.md).

### `speech.align.inferred`

Offline speech alignment applies an explicit operator time map. It does not infer or attest synchronization to the original media timeline.

Sources: [docs/user-guide/speech.md#export-subtitles-using-an-explicit-original-media-time-map](https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/speech.md#export-subtitles-using-an-explicit-original-media-time-map).

### `transcript.graph.plain_text`

Plain transcript text is available through standalone speech/subtitle exports, but it is not a creative transcript graph target. Graph transcript derivatives use canonical timed-transcript JSON, SRT, or WebVTT; format recognition does not create a plain-text graph writer.

Sources: [docs/user-guide/subtitles.md#opt-in-to-graph-and-sdk-execution](https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/subtitles.md#opt-in-to-graph-and-sdk-execution); [docs/user-guide/speech.md#use-the-planner-and-rust-sdk](https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/speech.md#use-the-planner-and-rust-sdk).

### `speech.subtitle.combined_selection`

Speech recognition and embedded-subtitle extraction are separate explicitly configured choices and cannot be enabled together in one run. Audio and subtitle stream indexes identify different selected streams; neither policy silently falls back to the other route.

Sources: [docs/user-guide/speech.md#use-the-planner-and-rust-sdk](https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/speech.md#use-the-planner-and-rust-sdk); [docs/user-guide/subtitles.md#opt-in-to-graph-and-sdk-execution](https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/subtitles.md#opt-in-to-graph-and-sdk-execution).

## Regenerate

From the repository root:

```bash
python3 "scripts/generate_creative_capabilities.py"
```

Generation reads the catalog and writes only this Markdown file and its
JSON counterpart. It does not execute Renderflow, probe tools, run native
conversions, validate schemas, install dependencies, or establish that
tests pass. The JSON includes the catalog's SHA-256 so downstream
consumers can identify the declaration snapshot.
