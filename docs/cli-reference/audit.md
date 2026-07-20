# `renderflow audit`

Generate an optimization audit report.

## Syntax

```bash
renderflow audit
```

## Flags

This command currently has no command-specific flags.

## Output

`audit` writes a timestamped report to `audits/audit-<timestamp>.log`.

The generated report includes sections such as:

- Architecture
- Performance
- Concurrency
- Memory
- Error Handling
- Dependency
- CLI & UX
- Configuration
- Logging
- Build & Distribution
- Documentation
- Code Structure
- Actionable Fixes
- V1.0.0 Readiness

## Example

```bash
renderflow audit
```
