# Workspace target compilation — November 14, 2026

Relates to [Flow #137](https://github.com/egohygiene/flow/issues/137).
The previous library-only compile selection could not observe API, integration,
example or benchmark source failures. This checkpoint runs the broader default-
feature workspace check with the declared Rust 1.94.0 compiler, locked/offline
dependencies and two jobs:

```sh
cargo +1.94.0 check --workspace --all-targets --locked --offline --jobs 2
```

The baseline is main revision `16e1448c044ebd0a3453991bba9da3b1e32f4a4b`,
after the prior library-test compilation repairs. The 44 previously selected
passing library cases were not re-executed in this checkpoint.
The workspace contains four packages: core `renderflow`, `renderflow-cli`,
`renderflow-plugin-sdk`, and the optional `renderflow-api` local service.

## Observed failures and scoped repairs

The unchanged baseline fails on the API `/v1/formats` route: `json!` parses the
inline array as a JSON array and cannot consume the following iterator expression.
The route now collects the same nine formats into a local value before passing
it to the JSON macro. Their order, media types and implemented flags are unchanged.
A focused in-memory HTTP regression checks the real response envelope, all nine
rows, and that discovery starts no job. It starts no network listener.

After that syntax repair, later workspace targets expose further compile debt:

- Two service test sites refer through the private `renderflow::sdk` module.
  They now use the stable `renderflow.progress/v1` schema literal already declared
  by the public JSON schema, without changing the module's visibility.
- An editorial integration case imports private `config::OutputType`. It now
  checks the same rejected custom-template/editorial combination through the
  public `StrategyArtifactTransform` constructor. This preserves the refusal
  assertion at public admission; it does not exercise the selector's separate
  private guard, since the adapter rejects this combination before calling it.
  No private configuration type is exported.
- The in-process Sonic test provider does not meet `AiProvider`'s existing
  `Debug` bound. Its fixture and response enum now derive `Debug`. No real
  model/provider, runtime fallback, acceptance flag or assertion changes.

All-target compilation passes after those repairs. Five focused changed-boundary
cases also pass: the format inventory, both affected service event cases, the
editorial public adapter and the Sonic mock runtime. Each exact selector ran
one test with no ignored cases. The mock returns authored data in process;
no model, account, private media or remote provider is involved.

## Durable selection and retained evidence

`task check:workspace` and the required `declared-compiler-workspace-targets`
entry in `config/local-ci.json` now select the same exact command. The reviewed
Realm owner's read-only plan accepts that manifest, and the Task YAML/manifest/
plan command vectors agree. The Go Task client itself was not available here;
its source alias was parsed, while the selected Cargo command actually ran.
Other required checks and hosted workflow triggers/admission guards are unchanged.

The [portable observation](evidence/20261114/workspace-targets.json) records
compiler versions, source/input hashes, the workspace target inventory,
failure-before-fix diagnostics, selected commands and raw-log hashes. Absolute
workspace/cache paths and raw runtime command ledgers remain private. Cargo
manifests and the lockfile are unchanged. Development/test debug metadata and
incremental artifacts were disabled through environment variables to bound disk
use; those settings do not alter the checked feature selection or source.

Compilation is distinct from executing every target. This checkpoint does not
run the full test suite, ignored native/provider cases, rustdoc examples or
benchmarks; qualify all feature combinations/platforms; link a release candidate;
or establish strict Clippy/warning cleanliness. Three existing warning sites
remain visible: unused speech `frame_count`, unused test helper `relocate`, and
the comic fixture's narrower `AggregateEvidence` field type. Hosted CI, real
media/model qualification, signing, publication and final Flow #88 acceptance
remain separate. No new ticket or release authority is created.
