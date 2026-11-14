# Flow #137 checkpoint 4 — renderflow, November 13, 2026

Accounts for all 24 diagnostics in the retained Rust/Clippy 1.94 run:

- Complete the existing composition SDK re-exports at the crate root and remove
  the unused canonical-root field while retaining root/file identity checks.
- Initialize PNG limits directly; use supported integer multiple/ceiling methods,
  borrowed one-item slices, derived equivalent defaults, and simpler equivalent
  option/reference expressions without widening bounds or accepting new output.
- Give native bundle payload tuples a transparent internal alias. Group five
  private long-parameter functions into typed context records. Their existing
  guard/evidence/effect sequence is retained; call arguments remain named and
  ordered. No new execution engine or sibling dependency is introduced.
- Box the large `SpeechCommands::Transcribe` delivery payload. Native flags and
  serialized speech contracts stay the same. Rust source callers constructing
  this public CLI enum variant now need `Box::new(arguments)`; the dispatcher
  unwraps the box before calling the existing SDK/command implementation.
- The two historical overflowing integer literals were already replaced with
  typed `u32::MAX` in the preceding main source and are not repaired twice.

These edits address the observed source findings. Clippy has not run against
the newer complete source tree, so additional findings or type errors can remain.

## Reproducible formatting and lint policy

`task format` applies Cargo/Rustfmt 1.94.0; `task format:check` uses that exact
formatter. The root manifest's MSRV is unchanged. Flow's MSRV matrix row owns
the canonical formatting gate; its moving-stable row retains strict Clippy and
product checks. Aniflow/Optiflow install the pinned formatter without changing
their selected compiler lane. Renderflow uses the existing 1.94 MSRV at the
explicit 1.94.0 patch in its owning workflows. No product lane is made advisory
or given `continue-on-error`, and no new lint allowance is introduced.

The maintainer selected the generated-source and formatting repair checkpoints.
Canonical source formatting was applied and the corresponding explicit-version
`cargo fmt --all -- --check` commands exited 0. The adjacent JSON records exact
formatter identities and scope; Optiflow includes its separate fuzz workspace.
The formatter parses source but does not perform Rust name/type/borrow checking,
run build scripts, execute tests or qualify providers. Source-formatting edits
are extensive because many prior source checkpoints had deferred formatting;
review the logical repairs separately from line wrapping, indentation and field
shorthand. No unrelated repository or Identity workspace was formatted.

Clippy, product compilation/builds/tests, schema/corpus/native checks and hosted
CI remain deferred. The existing `-D warnings` policy stays enforced for the
later authorized lint pass. Correct additional findings at their owner; do not
hide failures, raise the MSRV to silence a lint, regenerate fixtures blindly or
report a skipped/unavailable lane as qualified. #88 remains unchanged and last.

This branch is a source candidate stacked after checkpoint 3. Formatting success
and historical-diagnostic source fixes do not establish a green Actions suite.
