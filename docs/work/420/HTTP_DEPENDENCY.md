# HTTP syntax and dependency boundary

Source evaluation only for #420. No dependency was installed, resolved, built,
executed, benchmarked or qualified. No runtime provider or hosted application
was invoked; published source/metadata was read through GitHub.

## Selected

Use **httparse 1.10.1**, exact version, MIT OR Apache-2.0, for HTTP/1 syntax.
Its published normal dependency set is empty. Existing Clap and Ctrl-C packages
are reused for the optional executable; Ctrl-C's termination feature also
handles the container stop signal. Core planning/rendering dependencies are
unchanged. No asynchronous runtime or second HTTP framework is introduced.

The transport owns a deliberately limited one-request/Content-Length profile
over bounded loopback sockets, connection admission and a fixed worker pool.
It does not implement another HTTP syntax parser, proxy, TLS stack, upload
service or generic web framework. Authentication remains an adapter port.

Published metadata/source inspected:

- [Crates.io index record](https://github.com/rust-lang/crates.io-index/blob/master/ht/tp/httparse),
  exact `1.10.1` entry: checksum
  `6dbf3de79e51f3d586ab4cb9d5c3e2c14aa28ed23d180cf89b4df0454a69cc87`,
  not yanked; no normal/build dependencies. Dev dependencies are not added to
  the consumer lock node.
- [Tagged Cargo source](https://github.com/seanmonstar/httparse/blob/v1.10.1/Cargo.toml)
  confirms version, 2018 edition and license.

The existing workspace lock was edited from this metadata without Cargo
resolution. Exact-source/MSRV/build/dependency/license qualification remains
deferred under #458/#419. The comic corpus's lock digest was explicitly refreshed
to 1.3.2; no output baseline or passing report was rewritten.

## Other direction reviewed

Tiny-http 0.12.0 offers a small synchronous server with four normal dependencies.
Its [tagged client reader](https://github.com/tiny-http/tiny-http/blob/0.12.0/src/client.rs)
accumulates header lines before application admission, and its
[server](https://github.com/tiny-http/tiny-http/blob/0.12.0/src/lib.rs)
dispatches accepted connections through its own task pool. That source did not
provide the selected adapter's pre-parse header/connection bounds. No tiny-http
dependency or code is included. This is a scoped source decision, not a general
security finding or upstream runtime evaluation.
