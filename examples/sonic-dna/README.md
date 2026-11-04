# Synthetic Sonic evidence recipe

Everything here is original synthetic Renderflow material under MIT. There is
no real song, performer, voice, measured analyzer result, model response or human
approval. The declared tempo/key exercise confidence/authority shapes; they were
not inferred from the 100 ms silence recipe.

The native-shaped analysis follows the selected fields of the public Aniflow
`aniflow.audio-analysis/v1` contract at
`8008e0176f45b34d2a9e74d8a26d9b3fd374b81f`. Renderflow imports no Aniflow source.
`fixture-evidence.txt` is an exact retained byte subject. `source.wav` is not a
committed binary: prepare it explicitly later with the small no-overwrite Python
recipe, then use a compatible built binary:

```bash
python3 examples/sonic-dna/materialize.py \
  --output "examples/sonic-dna/source.wav"
renderflow dna sonic plan --input "examples/sonic-dna/sonic.json"
renderflow dna sonic build --input "examples/sonic-dna/sonic.json" \
  --output-dir "sonic-candidate"
```

These commands and the materializer are authored/unrun. Normal build does not
materialize a missing source, download a model, start a daemon or call an
analyzer. The recipe disables AI/network/remote permissions. The bundle holds
candidate DNA/descriptions/guidance and external input lineage, not the original
audio or private analyzer payloads. See the
[guide](../../docs/user-guide/sonic-dna.md) for schema/binding limits and explicit
optional runtime configuration. Tests/build/native/model/CI evidence is deferred.
