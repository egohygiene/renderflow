# Plain root-help fallback — November 14, 2026

Relates to [Flow #137](https://github.com/egohygiene/flow/issues/137).
An interactive root-help fallback could omit the wordmark while Clap still
applied automatic terminal styling to the help body. The CLI now owns the
complete plain-output choice for that root-help branch.

When the existing wordmark selector returns no selection, `main` writes
`help.to_string()` bytes to locked stdout and returns success. Clap 4.6.0's
`Error` Display uses the formatted message's Display implementation;
`StyledStr` documents that implementation as color-unaware and prints only
text segments. No ANSI filtering, second help definition or new Clap dependency
is introduced. The selected wide-terminal path retains its wordmark and native
`help.exit()` behavior. Normal parsing, subcommands, version output and domain
execution continue through the same existing owners.

The source branch starts from the published workspace-compile draft #523 at
`213c09d3990cb9a3e0b79001b07495ef65c25cca`. Its separate locked/offline Rust
1.94.0 release build selects only `renderflow-cli`, binary `renderflow`, target
`x86_64-unknown-linux-gnu`, with two jobs. The previous captured CLI remains
preserved; the new captured executable has a separate digest. Cargo manifests,
lockfile, root-help selector and wordmark bytes are unchanged.

The [portable build observation](evidence/20261114/root-help-build.json) pins
the exact clean compiled source, source/input hashes, toolchain, captured binary
and original build-log hash. It records a local Linux CLI build, rather than
release or native-provider qualification. The separately retained four-CLI PTY
checkpoint owns behavioral execution of wide, accessibility/empty-NO_COLOR,
narrow, unknown-terminal, redirected and ordinary parser paths. This build
receipt does not claim that those cases ran here.

No model, private media, hosted workflow, signing, release, package publication
or human approval is selected. Full tests, strict Clippy, native platform
matrices and the final Flow #88 pass remain separate.
