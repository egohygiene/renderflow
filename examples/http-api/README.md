# Local composition API source recipe

[`service.json`](service.json) registers the existing original mixed-asset page
package through the optional API adapter. It preserves the package's source
content, art, captured font/license and explicit review requirements. It does
not include credentials or claim a successful service/render.

The operator controls roots, config and explicit file allowlists. Clients use
`sample-page` as an ID; they cannot upload files or substitute a host path,
provider, plugin or config. Startup captures bytes and only changes output
placement inside per-job private copies. Changing original files after capture
does not modify an already-running server's registered package.

The sample uses Linux `/tmp` for private workspace parents and native defaults
for SVG/HTML/transcript JSON in both declared locales. Adjust real nonsymlink
paths and set a fresh `RENDERFLOW_API_TOKEN` before later explicit startup.
See the [API guide](../../docs/http-api.md) for the unrun local/container
commands, job/cancellation/SSE/download contracts and remaining limits.

No generator, test, Cargo build/resolution, formatter, validator, listener,
container, provider/model/native execution or CI ran for this recipe. It has
not rendered or published any consumer content.

## Additional local profiles

`local-profiles.json` registers composition plus manual timed lyrics, standalone
timed text and bounded original-Fountain exports. Existing SDK graph planning
and native evidence stay authoritative. `editorial-opt-in.json` additionally
sets explicit local-process permission for Pandoc/Tectonic. Capture itself does
not probe providers; submitting editorial jobs can. Neither sample was started.

After the later build/runtime sweep, start with `--config` pointing at the chosen
file, then POST a registered `pipeline_id` such as `manual-lyrics`, `timed-text`
or `fountain-body` to the same inspect/plan/execute routes. All roots/files are
operator-owned allowlists; clients cannot supply paths/configs/provider choices.
See [profile contract](../../docs/work/420/LOCAL_PROFILES.md).

`source-uploads.json` is a separate explicit opt-in example for the three
built-in text profiles. It allows one immutable replacement source per captured
pipeline; clients still cannot choose paths, format, config, assets or effects.
Raw upload capture requires a bounded Content-Length and exact SHA-256, creates
no job, and provides a same-pipeline ID for a later explicit submission. Manual
lyrics start unreviewed without inherited language/revision/section annotations.
Composition/editorial uploads remain disabled. The
[upload guide](../../docs/http-api.md#optional-immutable-source-uploads) contains
future commands only; this example has not been started or exercised.
