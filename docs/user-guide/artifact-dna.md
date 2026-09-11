# Artifact DNA

Artifact DNA is an optional, versioned description of reusable characteristics
observed in an immutable source artifact. It gives publication profiles and
other downstream transforms structured visual, layout, textual, sonic, or
cross-modal guidance without making them repeatedly inspect—or copy—the source
payload.

The v1 contract is `renderflow.artifact-dna/v1`. DNA is stored as an ordinary
intermediate artifact with media type
`application/vnd.renderflow.artifact-dna+json`, a source relationship, content
digest, validation state, hygiene evidence, and extractor provenance.

## Local deterministic baseline

The built-in extractor runs locally, performs no network requests, invokes no
model, and does not retain source payload text or identifying metadata. It
currently emits:

| Source | Built-in observations |
|---|---|
| PNG and JPEG | Canvas width, height, aspect ratio, and orientation from file headers |
| SVG | Canvas geometry, element counts, text-element count, and literal hexadecimal palette |
| Text and structured text | Word, paragraph, line, heading, and average-line-length statistics |
| Audio and containers | Safe format, media type, and size baseline under the appropriate modality |

The built-in values are intentionally modest. Rich semantic visual description
can use the versioned `skill.visual-dna.describe` skill, and provider-neutral
prompt guidance can use `skill.prompt.from-sanitized-dna`. Both are defined by
the [AI skill runtime](../ai-guide/model-catalog-and-skills.md), resolve by model
capability, and produce reviewable candidates. The core DNA extractor trait can
also host local analyzers or explicitly approved remote enrichers without
changing the DNA schema.

Sonic DNA normalization and English audio description remain a specialized
follow-up. Aniflow owns temporal audio analysis; Renderflow owns normalization,
hygiene, semantic description, provenance, and packaging.

## Explicit policy and budgets

SDK extraction is disabled by default. `DnaExtractionPolicy::explicit_local()`
enables the deterministic local path. AI, network, and remote execution are
three independent permissions and all default to false. A remote AI extractor
therefore requires every applicable permission before it is even invoked.

Policies also bound source bytes and observation count. A disabled or
over-budget request returns a structured skipped outcome and never mutates the
source. The engine verifies the source digest before and after extraction.

```rust
use renderflow::{ArtifactDnaEngine, DnaExtractionPolicy};

let engine = ArtifactDnaEngine::with_builtins();
let policy = DnaExtractionPolicy::explicit_local();
let outcome = engine.extract(&source_artifact, &artifact_store, &policy)?;

if let Some(dna_artifact) = outcome.artifact {
    // Select this structured intermediate as a downstream transform input.
    println!("{}", dna_artifact.id());
}
# Ok::<(), anyhow::Error>(())
```

## Evidence and extensions

Every observation records:

- a stable observation ID and namespaced dimension;
- its top-level modality and normalized JSON value;
- optional human-readable description;
- confidence from zero to one;
- evidence origin;
- exact provider/tool identifier and version;
- deterministic, heuristic, or probabilistic classification;
- optional structural, temporal, stream, or member scope.

Provider-specific data belongs under a namespaced `provider_extensions` key.
Consumers that do not understand the extension can ignore it while continuing
to use the stable core observations. Unknown evidence stays omitted or unknown;
an enrichment provider must not manufacture exact technical facts.

Heuristic and probabilistic observations force `human_review_required: true`.
All generated DNA begins as a candidate. An approved document requires a human
approval reference.

## Privacy and protected references

The public-safe default retains neither raw payloads nor identifying metadata.
Before observations or provider extensions are exposed, the DNA hygiene gate
checks for common credential shapes, email-shaped PII, and configured protected
references. Policy may omit, rewrite, or block unsafe values. Findings record a
class and action but never the matched secret, identity, artist, brand,
franchise, creator, or work name.

Built-in text extraction emits structural counts rather than source prose. SVG
extraction ignores element text and metadata. These data-minimizing choices
reduce copyright and privacy exposure before optional semantic analysis begins.

Artifact DNA is descriptive evidence, not proof of authorship, ownership,
originality, licensing, or commercial clearance. Its similarity guidance always
disallows direct imitation and legal-clearance claims.

## Similarity guidance

`renderflow dna compare` compares only observations explicitly marked
`similarity_eligible`. The report includes per-dimension scores, weights,
unmatched dimensions, and a weighted overall score when comparable evidence
exists. It deliberately excludes creator, brand, and protected-work identity
dimensions.

Scores help tune an original sibling artifact toward compatible geometry,
palette, rhythm, or hierarchy. They are not plagiarism detection, rights
clearance, or an instruction to reproduce a source.

## Fixtures and schemas

The canonical schemas are:

- `schemas/renderflow-artifact-dna-v1.schema.json`;
- `schemas/renderflow-artifact-dna-comparison-v1.schema.json`.

Redistribution-safe fixtures cover visual/layout, sonic, textual, and compound
artifacts under `tests/fixtures/artifact-dna/`. They require no paid API,
copyrighted media, model weights, or network access.

See the [`dna` CLI reference](../cli-reference/dna.md) for commands.
