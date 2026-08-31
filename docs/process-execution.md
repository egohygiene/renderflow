# External Process Execution

Renderflow wraps external tools, so subprocess behavior is part of the engine's
security, reproducibility, and reliability boundary. Production tool execution
should use `renderflow::process::ProcessExecutor` rather than invoking
`std::process::Command` independently inside adapters.

## Execution contract

`ProcessRequest` separates executable identity from arguments and defaults to
**direct argv execution**. No shell parsing occurs unless the caller explicitly
constructs a shell request.

```rust
use renderflow::process::{ProcessExecutor, ProcessRequest};

let result = ProcessExecutor::new().execute_checked(
    ProcessRequest::direct("pandoc")
        .args(["input.md", "--output", "output.html"]),
)?;
```

Explicit shell execution is a wider trust boundary and must remain visible:

```rust
let request = ProcessRequest::shell("sh")
    .args(["-c", "printf '%s' hello"]);
```

A direct request that names a known shell and supplies a command-evaluation flag
such as `-c`, `/C`, or `-Command` is rejected. Existing legacy transform YAML
that intentionally names a shell is classified as shell execution by its
compatibility adapter instead of being treated as ordinary direct argv.

## Bounded lifetime

Ordinary requests default to a 30-minute wall-clock timeout. Callers may choose
a shorter timeout or explicitly disable it when a capability has a reviewed
reason to run without a deadline.

The executor polls the child synchronously; Renderflow does not require an async
runtime merely to gain cancellation. `ProcessCancellationToken` is clonable and
may be triggered from another thread or, later, bridged to higher-level engine
cancellation.

### Process-tree termination

Cancellation and timeout attempt to terminate spawned child work rather than
only abandoning the caller:

- Linux/macOS/other Unix targets start the child in its own process group, send
  `SIGTERM` to the group, wait a bounded grace period, then escalate to
  `SIGKILL`.
- Windows attempts `taskkill /T /F` for tree termination and falls back to the
  direct child kill API if that facility is unavailable.
- Other unsupported platforms fall back to terminating the direct child.

The chosen capability is recorded in `ProcessPlatform` evidence. Callers can
explicitly request child-only termination for a capability that cannot safely
be grouped.

## Bounded stdout and stderr

Captured stdout and stderr are drained concurrently so a noisy child cannot
block on a full pipe. Only a bounded prefix is retained in memory; the executor
continues draining the remainder and records:

- retained bytes;
- total bytes observed; and
- whether truncation occurred.

The default capture limit is 256 KiB **per stream**. Binary callers can access
the retained raw bytes explicitly. Raw bytes are private from `Debug` output;
user-facing diagnostics should use the redacted text projection.

Transforms that produce large binary artifacts should write declared files into
the artifact workflow rather than using stdout as an unbounded payload channel.

## Environment policy

The default child environment is a filtered inheritance of the parent process.
Variables whose names look credential-bearing are removed unless the caller
explicitly allows or sets them. The filter includes names containing patterns
such as:

- `TOKEN`;
- `SECRET`;
- `PASSWORD` / `PASSWD`;
- `API_KEY` / `APIKEY`;
- `CREDENTIAL`;
- `PRIVATE_KEY`; and
- authorization-style names.

`ProcessEnvironment::clear()` provides a clean environment, while explicit
allow/deny/override methods support reviewed adapter requirements.

A sensitive environment value that is intentionally passed to a child is also
registered with the diagnostic redactor.

## Secret redaction

Process diagnostics never intentionally log raw sensitive arguments or
sensitive environment values. The executor redacts:

- arguments explicitly marked sensitive;
- values following credential-looking flags;
- credential-looking `NAME=value` arguments;
- registered secret values;
- bearer-token values; and
- credentials embedded in URL authorities.

This is defense in depth, not a license to put secrets into command arguments.
Prefer environment/OIDC/provider-native secret mechanisms whenever possible.

## Expected outputs

A request can declare expected files or directories. Checked execution only
succeeds when both the process exit state and output validation succeed.
Expected outputs can require:

- a path to exist;
- the expected file/directory kind;
- a non-empty file; or
- a change relative to the pre-execution snapshot.

This prevents a subprocess that exits `0` without producing its promised output
from being treated as a successful transform. Artifact import/materialization
remains owned by the artifact kernel; process execution only validates the
filesystem contract it was given.

## Tool version probes

`ProcessExecutor::probe_version()` executes `<tool> --version` with a short
bounded timeout and small capture budget. It produces structured
`ToolProbeEvidence` containing:

- available/missing/failed/timed-out state;
- first version line when available;
- duration; and
- platform evidence.

This is the execution primitive for dependency checks and doctor/plugin
inspection. The broader capability registry and reproducible toolchain
fingerprint are intentionally owned by Renderflow #359.

## Network and sandbox policy hooks

`ProcessRequest` carries network intent and an optional sandbox-profile ID, and
`ProcessExecutor` accepts `ProcessPolicyHook` implementations. These fields are
**not** claims that the core executor automatically provides network isolation or
OS sandboxing. A deployment/profile that requires those guarantees must attach a
policy hook backed by an actual enforcement mechanism.

## Temporary state

Temporary input/output paths used by compatibility transforms remain
caller-owned RAII state. The executor does not recursively delete arbitrary
filesystem paths and never removes a verified final artifact as cleanup.

## Migration boundary

The following production paths use the canonical executor after #356:

- built-in Pandoc/FFmpeg rendering through the command adapter;
- legacy YAML `CommandTransform` execution;
- command-backed collection/aggregation transforms;
- dependency availability probes;
- PDF/Tectonic probing;
- `renderflow doctor` probes; and
- plugin required-tool probes.

The historical repository-audit command still shells out only to gather its own
`date`/`git` report metadata. It is not a wrapped transform/tool adapter and is
outside the reusable execution-provider boundary defined here.

Future adapters, including HandBrake work in #345, must use this process port.
#355 will project process outcomes into richer execution evidence, #358 will
bridge orchestration cancellation/resume semantics, and #359 will add stable tool
capability IDs and complete version/toolchain fingerprints.
