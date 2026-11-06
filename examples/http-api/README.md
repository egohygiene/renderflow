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
