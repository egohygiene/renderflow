# `renderflow dna`

Extract, validate, and compare versioned Artifact DNA documents.

## Extract

The command itself is the explicit opt-in for local extraction. Source bytes are
imported into the content-addressed store and remain immutable. The canonical
DNA JSON is also stored as a provenance-linked intermediate artifact.

```bash
renderflow dna extract \
  --input "examples/magazine/assets/cover.svg" \
  --output "cover.dna.json"
```

Useful controls:

```bash
renderflow dna extract \
  --input "cover.svg" \
  --output "cover.dna.json" \
  --store ".renderflow/dna-artifacts" \
  --max-source-bytes 67108864 \
  --max-observations 512 \
  --protected-reference "Protected Example"
```

`--protected-reference` can be repeated. A built-in observation containing a
configured term is omitted before output. SDK policies can supply reviewed
descriptive replacements.

The built-in CLI extractor remains deterministic and local. The permissions
`--allow-ai`, `--allow-network`, and `--allow-remote` never select a provider by
themselves; they only permit a registered compatible extractor. Remote use
requires all three flags and the extractor's own AI/runtime policy.

Omit `--output` to print JSON. Use `--format yaml` only for inspection or a YAML
materialization; the canonical stored artifact remains JSON.

## Validate

```bash
renderflow dna validate --input "cover.dna.json"
renderflow dna validate --input "cover.dna.json" --format json
```

Validation rejects unknown core fields, invalid digests, duplicate observation
IDs, undeclared modalities, unnamespaced dimensions/extensions, unsafe clearance
claims, and approved candidates without an approval reference.

## Compare

```bash
renderflow dna compare \
  --left "cover.dna.json" \
  --right "divider.dna.json" \
  --output "cover-divider-comparison.json"
```

The result is `renderflow.artifact-dna-comparison/v1`. It reports only shared,
eligible descriptive dimensions and never authorizes direct imitation or claims
legal clearance.

Read the [Artifact DNA guide](../user-guide/artifact-dna.md) for extraction
policy, evidence semantics, AI integration, and rights boundaries.
