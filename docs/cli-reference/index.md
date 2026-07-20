# CLI Reference

This section documents command syntax, flags, and behavior.

## Command map

- [`build`](build.md)
- [`watch`](watch.md)
- [`inspect`](inspect.md)
- [`audit`](audit.md)
- [`graph`](graph.md)
- [`plugin`](plugin.md)
- [`ai`](ai.md)

## Logging flags

Global flags apply across subcommands:

```bash
renderflow build --verbose
renderflow build --debug
```

`--debug` takes precedence over `--verbose`.
