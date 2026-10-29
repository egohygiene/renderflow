# Configure a local command plugin

Source-authored checkpoint for [#466](https://github.com/egohygiene/renderflow/issues/466).
Build, tests, CLI/process execution and schema qualification are deferred.
This is not released or runtime-qualified functionality.

An explicitly configured local program can implement an ordinary artifact-native
plugin. It receives one staged input and creates one output. Renderflow retains
planning, source lineage, publication, diagnostics and provider ownership. The
program can be written in Python, Rust or another language; it needs no Rust ABI.

The [names example](../../examples/plugins/names/README.md) contains a complete
registry and a dependency-free Python program. Its two capabilities count a
supplied list of names into JSON and export that JSON as CSV. Neither capability
is a new core parser or editorial template.

## Configure and select

Declare local programs under `plugins` in the same transform registry that your
ordinary `renderflow/v2` configuration references with `transforms: transforms.yaml`.
Each declaration uses the existing `renderflow.plugin/v2alpha1` descriptor plus
the [command invocation contract](../../schemas/renderflow-command-plugin-v1.schema.json).
The `transforms` entries select a plugin by exact ID and supply an optional JSON
object under `config`:

```yaml
transforms:
  - name: count-names
    plugin: example.names
    config:
      names: ["Ada", "Lin"]
      max_matches: 10000
    from: text
    to: json
    cost: 1.0
    quality: 0.95
```

This excerpt requires the corresponding `plugins` declaration; see the complete
example. No download, installation, directory scanning or dynamic-library loading
occurs. Unknown IDs, duplicate IDs/names, unsupported routes, collections, mixed
backends and ignored top-level fields are refused. `program`, `plugin` and `ai`
are now exclusive in YAML validation; older ambiguous configurations must choose
one. The legacy text loader remains available for host-registered legacy plugins,
but refuses configured command plugins and their artifact-specific configuration.

Use canonical format names in descriptors. The first command profile accepts one
input and produces one nonempty file; use multiple graph steps/targets for multiple
output formats. The same plugin can have several transform instances with different
configuration. Select an instance using the ordinary target `transform` selector
and use `{target.id}` in output naming when publishing multiple outputs of one format.
Exact edge registration keeps those instances independent.

## Inspect declarations without running code

```bash
renderflow plugin list --transforms "examples/plugins/names/transforms.yaml"
renderflow plugin info "example.names" --transforms "examples/plugins/names/transforms.yaml" --format "json"
renderflow plugin validate --transforms "examples/plugins/names/transforms.yaml"
```

Configured `list`, `info`, `validate` and `doctor` inspect declarations only;
their JSON reports explicitly say `programs_probed: false` and
`runtime_qualified: false`. They do not validate a plugin's domain-specific
configuration fields or attest that its program exists or works. The host validates
the bounded object envelope; the plugin must validate its own keys and meanings.
The legacy commands without `--transforms` keep their prior host-registry behavior.

`tools list --transforms FILE` performs existing bounded provider discovery/version
probes. Normal graph planning may also probe selected programs with `--version`;
it does not run their conversion arguments. A program should support that probe,
or use an interpreter such as `python3` whose version can be observed. Interpreter
discovery is not script/dependency qualification.

## Invocation contract

| Field or placeholder | Meaning |
| --- | --- |
| `program` | Executable name on PATH, an absolute path, or a registry-relative path containing a directory component. |
| `args` | Direct argv, at most 128 arguments of at most 4096 bytes each. No implicit shell. |
| `{input}` | Standalone argument for an independent read-only staged input. |
| `{output}` | Standalone argument for the new output file the plugin must create. |
| `{config}` | Standalone argument for a read-only JSON object file, at most 64 KiB. Required even for `{}`. |
| `{format}` | Optional standalone argument containing the requested canonical output format. |
| `{registry_dir}` | May appear inside an argument, for example `{registry_dir}/names.py`. |
| `timeout_seconds` | Default 30; range 1–1800. |
| `max_input_bytes` / `max_output_bytes` | Default 8 MiB each; range 1–64 MiB. |

The working directory is private scratch, not the registry directory. Use
`{registry_dir}` for scripts and declared resources. Loader APIs accepting YAML
text resolve relative paths against the caller's current directory; file loader
APIs use the registry file's directory. Configuration values are never interpolated
into a shell or command-line template.

The shared process service enforces the deadline, cooperative cancellation and
64 KiB stdout/stderr capture bounds. Output file size is polled while running and
checked after exit; nonregular, symbolic-linked and (on Unix) hard-linked output
files are refused. JSON output also receives JSON syntax validation before artifact
commit. Ordinary graph validation and publication policy still apply to every format.
Syntax/shape validation does not prove extraction accuracy. Diagnostics report
process failure classes without copying child stdout/stderr into receipts.

Run only reviewed, trusted programs. These bounds are not an OS filesystem/network
sandbox, an RSS limit or an aggregate scratch quota. A process can access other
files or launch children subject to the host environment. `network: deny` is not
network isolation. Renderflow does not install an interpreter or select a cloud
fallback. Do not put credentials in inline configuration; this first profile has
no credential-resolution feature.

## Evidence and reuse

Outputs use the existing SDK to retain source artifact IDs, plugin ID/version,
capability, loss declaration and provenance. The graph binds the complete registry
configuration digest. Command metadata explicitly records
`executable_identity_verified: false`; plugin versions are declarations and the
provider fingerprint is discovery evidence. Files referenced by argv are not frozen
or attested by this profile. Script/dependency changes after planning are therefore
not detected as stale-plan changes.

Command plugins must use `cache_policy: disabled`. Both artifact cache and durable
step checkpoint reuse are disabled for these steps, including downstream command
plugins, so mutable code/dependencies do not result in a falsely reusable command
step. Normal non-plugin descendants still follow their own content-based contracts.
Quality scores rank paths; they do not measure extraction accuracy or erase declared
loss. A derived names report loses most document content; preserve the original.

The Rust SDK can load the same file through
`build_graph_executor_and_tools_from_yaml`, or implement/register
`PluginTransformV2` directly. This adds one adapter to that SDK. It does not define
a second plugin framework or change the serialized v2 descriptor contract.

## Scope of the example

The example starts from UTF-8 text. A future explicitly selected PDF reader/OCR
provider could supply text, with its own page mapping, fidelity and qualification.
Current screenplay-specific PDF recovery policies deliberately reject mixed custom
transforms and are not a generic PDF-text input shortcut. The JSON may feed a later
editorial template or XLSX exporter; neither is implemented here. CSV is a delimited
text artifact, not an Excel workbook. Existing delimited-text validation is conservative
and may reject valid quoted commas; broader CSV qualification remains outstanding.
