# Publication summary — source checkpoints

This example is authored, uncompiled and unrun. It does not contain model output,
weights, a prepared runtime, passing evidence or publication approval.

`runtime.disabled.json` is the default non-AI path. `runtime.local.json` names
one existing Ollama catalog entry and literal loopback endpoint. Its egress flag
is an example operator assertion: only retain it after disabling the local
service's own cloud/remote features. The bundled model catalog is unverified;
the example therefore does not produce an executable binding. Record the actual
selected runtime/model versions, observed native weight digest, preparation
reference and supported byte/token limits in a reviewed separate catalog before
claiming declared readiness. Preparation is not part of normal plan/run.

`runtime.remote.json` is an explicitly selected HTTPS deployment example, with
an environment handle and unknown cost refused by default. No key is bundled or
read during inspection. It is not permission for a paid request or a fallback.

Later inspection commands (not executed in this sprint):

```bash
renderflow ai profile --profile examples/publication-summary/runtime.disabled.json
renderflow ai profile --profile examples/publication-summary/runtime.local.json --catalog /path/to/reviewed-catalog.json
```

Inspection reads declarations only and reports disabled/unavailable/incompatible/
refused/declared_ready. It does not probe a server, read credentials, install or
download anything. Reported cost can remain unknown; it is not zero. See the
[runtime mapping](../../docs/work/456/RUNTIME_PROFILE.md) and
[profile schema](../../schemas/renderflow-ai-runtime-profile-v1.schema.json).

The second source checkpoint adds a public
`renderflow::publication::summary` capture/plan/generate API and the bundled
`skill.publication.summary@1.0.0`. It consumes explicitly approved selected UTF-8
text, not images/audio. Every synopsis statement references exact captured slice
IDs. Reference checks remain separate from factual/semantic correctness and
human publication review. No inference has occurred. See the
[candidate contract](../../docs/work/456/CANDIDATES.md); CLI/review/export follow
in the third draft.
