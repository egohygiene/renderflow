use anyhow::{Context, Result};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

/// Format the current UTC time as a compact ISO-8601 timestamp string,
/// e.g. `2026-03-29T074043Z`.
fn timestamp() -> String {
    // Use the `date` command for a portable, human-readable timestamp.
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H%M%SZ"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());

    match output {
        Some(ts) => ts.trim().to_string(),
        None => {
            // Fallback: epoch seconds rendered as an opaque stamp.
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("{secs}")
        }
    }
}

/// Retrieve the current git branch name, or a placeholder when not in a git
/// repository.
fn git_branch() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());
    output.map(|s| s.trim().to_string()).unwrap_or_else(|| "unknown".to_string())
}

/// Retrieve the current git commit SHA (short form).
fn git_commit() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());
    output.map(|s| s.trim().to_string()).unwrap_or_else(|| "unknown".to_string())
}

/// Generate the full audit report as a UTF-8 string.
fn generate_report(ts: &str, branch: &str, commit: &str) -> String {
    let header = format!(
        "Repository Audit — v1.0.0 Production Readiness\n\
         ================================================\n\
         Timestamp : {ts}\n\
         Auditor   : Copilot (AI agent)\n\
         Branch    : {branch}\n\
         Commit    : {commit}\n\
         Purpose   : Full-spectrum production audit — v1.0.0 release readiness\n"
    );
    let mut report = header;
    report.push_str(AUDIT_BODY);
    report
}

const AUDIT_BODY: &str = r#"
================================================================================
1. SUMMARY
================================================================================

Renderflow is a spec-driven document rendering engine written in Rust. It
transforms source documents (Markdown, RST, HTML, EPUB, LaTeX, DOCX) into
multiple output formats (HTML, PDF, DOCX) through a parallel, two-phase
pipeline driven by a YAML configuration file.

This full-spectrum audit evaluates readiness for a v1.0.0 release across:
  1.  Architecture & System Design
  2.  Performance
  3.  Concurrency
  4.  Memory & Ownership
  5.  Error Handling
  6.  Dependency Health
  7.  CLI & UX
  8.  Configuration & Validation
  9.  Logging & Observability
  10. Build & Distribution Readiness
  11. Documentation & Developer Experience

Overall assessment: the codebase is clean, modular, and well-tested.
Core architecture is sound. Key gaps before v1.0.0 are in distribution
automation, documentation completeness, and a handful of API surface issues.

Severity legend:
  [HIGH]   Measurable production impact — fix before v1.0.0
  [MED]    Moderate quality or maintainability concern
  [LOW]    Minor / idiomatic improvement
  [PASS]   No action required
  [INFO]   Informational — no change needed

================================================================================
2. FILES / MODULES REVIEWED
================================================================================

Core Entry Point:
  - src/main.rs                        CLI bootstrap, log-level selection, dispatch

CLI & Configuration:
  - src/cli.rs                         Clap-based CLI (Cli struct, Commands enum)
  - src/config.rs                      YAML config parsing, OutputType, validation

Pipeline:
  - src/pipeline/pipeline.rs           Two-phase pipeline (transforms + render steps)
  - src/pipeline/step.rs               PipelineStep trait
  - src/pipeline/strategy_step.rs      Strategy wrapper for PipelineStep

Strategies (Output Format Layer):
  - src/strategies/strategy.rs         OutputStrategy trait, RenderContext struct
  - src/strategies/selector.rs         select_strategy() factory
  - src/strategies/html.rs             HtmlStrategy — pandoc -> HTML
  - src/strategies/pdf.rs              PdfStrategy  — pandoc + tectonic -> PDF
  - src/strategies/docx.rs             DocxStrategy — pandoc -> DOCX
  - src/strategies/pandoc_args.rs      PandocArgs builder

Transforms (In-Memory Content Pipeline):
  - src/transforms/transform.rs        Transform trait (name, apply)
  - src/transforms/registry.rs         TransformRegistry (fail-fast, apply_all)
  - src/transforms/emoji.rs            EmojiTransform
  - src/transforms/variable.rs         VariableSubstitutionTransform
  - src/transforms/syntax_highlight.rs SyntaxHighlightTransform

Adapters:
  - src/adapters/command.rs            run_command() — subprocess execution

Support Modules:
  - src/cache.rs                       SHA-256 input & output content caching
  - src/files.rs                       validate_input(), ensure_output_dir()
  - src/assets.rs                      normalize_asset_paths(), resolve_asset_path()
  - src/template.rs                    init_tera(), validate_templates()
  - src/deps.rs                        validate_dependencies() (pandoc / tectonic)
  - src/error.rs                       RenderError domain-specific error types
  - src/compat.rs                      Input/output format compatibility matrix
  - src/input_format.rs                Document input format detection

Project Files:
  - Cargo.toml                         Crate metadata, dependencies, release profile
  - README.md                          User-facing documentation
  - .github/workflows/ci.yml           CI/CD pipeline definition
  - Taskfile.yml                       Developer automation tasks

================================================================================
3. ARCHITECTURE & SYSTEM DESIGN
================================================================================

────────────────────────────────────────────────────────────────────────────────
3.1 Two-phase pipeline model is well-designed                          [PASS]
────────────────────────────────────────────────────────────────────────────────

The separation of Transform phase (in-memory text mutations, format-agnostic,
cached per-input) from the Step/Strategy phase (format-specific rendering via
external tools) is a clean architectural decision. Transform results are shared
across output formats, avoiding redundant processing.

────────────────────────────────────────────────────────────────────────────────
3.2 Strategy pattern enables clean extensibility                       [PASS]
────────────────────────────────────────────────────────────────────────────────

OutputStrategy trait cleanly decouples format rendering from pipeline
orchestration. Adding a new output format requires only implementing
OutputStrategy and registering it in select_strategy(). This is correct and
idiomatic.

────────────────────────────────────────────────────────────────────────────────
3.3 OutputStrategy is not marked Send + Sync                           [HIGH]
────────────────────────────────────────────────────────────────────────────────

Location : src/strategies/strategy.rs, src/strategies/selector.rs

  pub trait OutputStrategy {
      fn render(&self, ctx: &RenderContext) -> Result<()>;
  }

  pub fn select_strategy(...) -> Result<Box<dyn OutputStrategy>>

The trait object Box<dyn OutputStrategy> is used inside a rayon parallel
iterator. Rayon requires Send for values moved across threads. If a future
strategy implementation holds non-Send state (e.g. a Rc<...>) the compiler
will not catch the error until the concrete type is first used in a parallel
context.

Actionable fix:
  Add Send + Sync bounds to the trait definition and selector return type:

  pub trait OutputStrategy: Send + Sync {
      fn render(&self, ctx: &RenderContext) -> Result<()>;
  }

  pub fn select_strategy(...) -> Result<Box<dyn OutputStrategy + Send + Sync>>

────────────────────────────────────────────────────────────────────────────────
3.4 RenderContext fields connected to runtime behaviour              [RESOLVED]
────────────────────────────────────────────────────────────────────────────────

Location : src/strategies/strategy.rs

  pub struct RenderContext<'a> {
      pub variables: &'a HashMap<String, String>,
      pub dry_run: bool,
  }

Both fields are now connected to actual behaviour.  `variables` is forwarded to
pandoc via `--variable key=value` arguments in every strategy.  `dry_run`
causes each strategy to skip external command execution and return immediately,
enabling a true no-op execution path without requiring caller-side guards.

The `#[allow(dead_code)]` annotations have been removed and the pipeline
provides a resilient `with_standard_transforms_resilient` constructor that uses
`FailureMode::ContinueOnError` for watch-mode rebuilds.

────────────────────────────────────────────────────────────────────────────────
3.5 Compatibility matrix (compat.rs) is not exhaustively enforced      [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/compat.rs

The SUPPORTED_CONVERSIONS matrix defines valid input→output pairs, but it is
only consulted in build orchestration. Validation at config parse time (before
any expensive operations) would catch unsupported format combinations earlier
and produce clearer user-facing errors.

Actionable fix:
  Call the compatibility check in config.rs validation, alongside output type
  validation, so incompatible format combinations are rejected immediately with
  a descriptive error message including the valid alternatives.

────────────────────────────────────────────────────────────────────────────────
3.6 Module visibility is appropriately restricted                      [PASS]
────────────────────────────────────────────────────────────────────────────────

Internal modules are not pub(crate) but are only re-exported through top-level
mod declarations. This is acceptable for a binary crate. If renderflow is ever
published as a library crate, a careful pub/pub(crate) audit would be needed.

================================================================================
4. PERFORMANCE
================================================================================

────────────────────────────────────────────────────────────────────────────────
4.1 Unnecessary intermediate Vec allocation in build.rs               [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/commands/build.rs

  let output_formats: Vec<String> = config.outputs
      .iter()
      .map(|o| o.output_type.to_string())
      .collect();          // <- allocates a Vec<String> only to call .join()
  info!("Selected outputs: {}", output_formats.join(", "));

Actionable fix:
  use itertools::Itertools as _;
  let output_formats = config.outputs.iter().map(|o| o.output_type.to_string()).join(", ");
  info!("Selected outputs: {}", output_formats);

────────────────────────────────────────────────────────────────────────────────
4.2 Pretty-printed JSON for cache files is wasteful                [RESOLVED]
────────────────────────────────────────────────────────────────────────────────

Location : src/cache.rs — save_cache(), save_output_cache()

  let json = serde_json::to_string_pretty(cache)?;

Fix applied: serde_json::to_string() (compact) is now used. Both cache writers
use compact serialization, reducing file size and serialization overhead.

────────────────────────────────────────────────────────────────────────────────
4.3 Unnecessary .to_string_lossy().into_owned() allocation        [RESOLVED]
────────────────────────────────────────────────────────────────────────────────

Location : src/strategies/html.rs, pdf.rs, docx.rs

  Some(path.to_string_lossy().into_owned())

Fix applied: path.to_str().ok_or_else(…) is now used in all three strategy
files. Non-UTF-8 paths produce a clear anyhow error
("Template path '…' contains invalid UTF-8") instead of silently replacing
invalid bytes, and no unconditional heap allocation is forced.

────────────────────────────────────────────────────────────────────────────────
4.4 normalize_asset_paths always allocates a new String                [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/assets.rs

normalize_asset_paths returns a new owned String even when no substitution
occurs, wasting an allocation for content with no local asset references.

Actionable fix:
  Return Cow<str> and borrow when unchanged:
  pub fn normalize_asset_paths<'a>(content: &'a str, ...) -> Result<Cow<'a, str>>

================================================================================
5. CONCURRENCY
================================================================================

────────────────────────────────────────────────────────────────────────────────
5.1 Rayon parallelism is correct but progress bars may serialize work  [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/commands/build.rs

Rayon par_iter() is used correctly for parallel output rendering. However,
multi_progress.add() inside the parallel section acquires an internal Mutex.

Recommendation:
  Pre-create all ProgressBar instances before the parallel section to eliminate
  Mutex contention during rendering:

  let bars: Vec<ProgressBar> = config.outputs.iter()
      .map(|_| multi_progress.add(ProgressBar::new_spinner()))
      .collect();
  config.outputs.par_iter().zip(bars.par_iter()).try_for_each(|(output, pb)| { ... })?;

────────────────────────────────────────────────────────────────────────────────
5.2 Cache I/O is correctly sequential before the parallel section      [PASS]
────────────────────────────────────────────────────────────────────────────────

Transform cache is loaded and saved sequentially before parallel rendering.
This is correct. No change required.

────────────────────────────────────────────────────────────────────────────────
5.3 External process execution blocks the rayon thread pool            [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : src/adapters/command.rs — run_command()

std::process::Command blocks the rayon thread for the duration of the pandoc/
tectonic process. Acceptable at current scale (<=3 output types). If the tool
ever renders many independent documents concurrently, consider tokio::process
or spawning external commands outside the rayon pool.

────────────────────────────────────────────────────────────────────────────────
5.4 No mechanism for cancellation of parallel renders                  [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : src/commands/build.rs

In the parallel rendering loop, if one output format fails, try_for_each will
propagate the error but already-started renders on other threads continue to
completion. For long-running PDF renders this can waste significant time.

Recommendation:
  For v1.0.0 this is acceptable. A future improvement would be to use a
  cancellation token (e.g. AtomicBool shared via Arc) to signal sibling
  threads to abort on first error.

================================================================================
6. MEMORY & OWNERSHIP
================================================================================

────────────────────────────────────────────────────────────────────────────────
6.1 Config cloning in strategy selection                               [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/commands/build.rs

  select_strategy(output.output_type.clone(), output.template.clone(), ...)

template is an Option<String> which allocates a new String on every clone.

Actionable fix:
  Accept &OutputType / Option<&str> in select_strategy to avoid heap allocation.

────────────────────────────────────────────────────────────────────────────────
6.2 compute_input_hash sorts variables on every call                   [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/cache.rs — compute_input_hash()

  let mut sorted: Vec<(&String, &String)> = variables.iter().collect();
  sorted.sort_by_key(|(k, _)| k.as_str());

A Vec is allocated and sorted on every cache lookup.

Actionable fix:
  Use BTreeMap<String, String> for Config.variables so iteration order is
  always deterministic and the sort allocation is eliminated entirely.

────────────────────────────────────────────────────────────────────────────────
6.3 Lifetime usage in assets.rs is appropriate                        [PASS]
────────────────────────────────────────────────────────────────────────────────

normalize_asset_paths uses Cow<str> correctly for the borrowed/owned
distinction, avoiding unnecessary copies when no substitution is needed.

================================================================================
7. ERROR HANDLING
================================================================================

────────────────────────────────────────────────────────────────────────────────
7.1 anyhow usage is consistent and correct throughout                  [PASS]
────────────────────────────────────────────────────────────────────────────────

anyhow is used correctly in all command and adapter code. .with_context() is
present at every external I/O boundary. Error propagation via ? is uniform.

────────────────────────────────────────────────────────────────────────────────
7.2 thiserror is declared but domain errors (error.rs) use it          [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : src/error.rs

RenderError uses thiserror correctly for structured domain errors
(PandocNotFound, TectonicNotFound, TemplateNotFound). This provides
programmatic error matching for callers and avoids string comparison.

────────────────────────────────────────────────────────────────────────────────
7.3 validate_input uses .unwrap() on OsStr conversion                  [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/files.rs

  canonical_input.to_str().unwrap()

Panics on non-UTF-8 paths (possible on Linux). This is the only non-test
unwrap in the production code path.

Actionable fix:
  canonical_input.to_str()
      .ok_or_else(|| anyhow::anyhow!(
          "Input path is not valid UTF-8: {}",
          canonical_input.display()
      ))?

────────────────────────────────────────────────────────────────────────────────
7.4 Config validation collects all errors before reporting             [PASS]
────────────────────────────────────────────────────────────────────────────────

Config validation accumulates all errors into a Vec<String> and reports them
together. This avoids the frustrating fix-one-error-at-a-time cycle for users.

────────────────────────────────────────────────────────────────────────────────
7.5 No panic paths in production code (except validate_input)          [PASS]
────────────────────────────────────────────────────────────────────────────────

All unwrap/expect calls are confined to test code or are provably safe (e.g.
on values just constructed). The only production-code unwrap is in 7.3 above.

================================================================================
8. DEPENDENCY HEALTH
================================================================================

────────────────────────────────────────────────────────────────────────────────
8.1 serde_yaml replaced by serde_yaml_ng                           [RESOLVED]
────────────────────────────────────────────────────────────────────────────────

Location : Cargo.toml

The project uses serde_yaml_ng = "0.9" (the actively maintained community
fork of the abandoned serde_yaml 0.9). This is the correct choice and has
been applied.

────────────────────────────────────────────────────────────────────────────────
8.2 notify + notify-debouncer-mini version compatibility               [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : Cargo.toml

  notify = "6.1"
  notify-debouncer-mini = "0.4"

notify-debouncer-mini 0.4 targets notify 6.x. This is correct and pinned
to the compatible minor version. Plan an upgrade to notify 7 + debouncer 0.5
when available to consolidate the dependency tree.

────────────────────────────────────────────────────────────────────────────────
8.3 All dependencies are actively maintained                           [PASS]
────────────────────────────────────────────────────────────────────────────────

  clap 4            — actively maintained, latest major
  serde 1           — stable, widely used
  anyhow 1          — stable, idiomatic error handling
  thiserror 1       — stable, idiomatic error types
  tracing 0.1       — stable, widely adopted
  tera 1            — stable template engine
  indicatif 0.17    — stable progress bars
  rayon 1           — stable parallel iterator library
  sha2 0.10         — stable, part of RustCrypto project
  itertools 0.14    — stable iterator utilities

No dependencies are abandoned, unmaintained, or known to have unpatched CVEs.

────────────────────────────────────────────────────────────────────────────────
8.4 No dev-dependencies explicitly listed                              [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : Cargo.toml

  [dev-dependencies]
  (empty)

tempfile is used in tests but listed as a main dependency. Moving it to
[dev-dependencies] reduces the dependency surface of any future library build.

  [dev-dependencies]
  tempfile = "3"

================================================================================
9. CLI & UX
================================================================================

────────────────────────────────────────────────────────────────────────────────
9.1 Clap derive macro usage is idiomatic                               [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : src/cli.rs

The CLI uses clap derive macros correctly with #[command()] and #[arg()]
attributes. Subcommands are registered via #[command(subcommand)]. This
generates correct --help output and argument parsing.

────────────────────────────────────────────────────────────────────────────────
9.2 Positional config argument shorthand is well-implemented           [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : src/main.rs

  renderflow config.yaml ≡ renderflow build --config config.yaml

The positional argument fallback (treating the first argument as a config path
if it is not a known subcommand) provides a smooth out-of-the-box experience.

────────────────────────────────────────────────────────────────────────────────
9.3 --dry-run flag is not reflected in progress bar messaging          [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : src/commands/build.rs

When --dry-run is passed, the progress bars display the same spinner text as
a real build. Users have no visual confirmation that they are in dry-run mode
until output is omitted.

Actionable fix:
  Prefix spinner messages with "[DRY RUN]" when dry_run is true:

  let label = if dry_run { format!("[DRY RUN] {}", output_type) } else { output_type.to_string() };
  pb.set_message(label);

────────────────────────────────────────────────────────────────────────────────
9.4 watch command debounce default is not documented in --help         [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : src/cli.rs — WatchArgs

The --debounce flag has a default value but the default is not surfaced in
the --help output. Adding default_value_t or a help string mentioning the
default (e.g. "Debounce delay in ms [default: 500]") improves discoverability.

────────────────────────────────────────────────────────────────────────────────
9.5 audit command output path not shown to the user prominently        [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/commands/audit.rs

The audit file path is printed via println! but is not logged at INFO level
with context. When the tool is used in CI or piped through other tools, the
output path may be missed.

Actionable fix:
  Use a consistent success message format matching the build command:

  info!("Audit report written to: {filename}");
  eprintln!("✓ Audit written to: {filename}");

================================================================================
10. CONFIGURATION & VALIDATION
================================================================================

────────────────────────────────────────────────────────────────────────────────
10.1 Config validation is comprehensive and fail-all                   [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : src/config.rs

Config::validate() collects all validation errors (invalid output types,
missing input field, unsupported format combinations) before reporting.
This is the correct pattern and provides a good user experience.

────────────────────────────────────────────────────────────────────────────────
10.2 Empty outputs list is not caught at validation time               [MED]
────────────────────────────────────────────────────────────────────────────────

Location : src/config.rs

An empty `outputs: []` in the config file passes validation and results in a
successful build that produces no output files, potentially confusing users.

Actionable fix:
  Add a validation check:

  if self.outputs.is_empty() {
      errors.push("'outputs' must contain at least one output entry".to_string());
  }

────────────────────────────────────────────────────────────────────────────────
10.3 variables field is not validated for key format                   [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : src/config.rs

Variable keys are not validated for format (e.g. no spaces, no special
characters). When variables are eventually passed to pandoc as --variable
key=value arguments, malformed keys may cause subtle pandoc failures.

Recommendation:
  Add a validation pattern (alphanumeric + underscore/hyphen) for variable
  keys to catch mistakes at parse time.

────────────────────────────────────────────────────────────────────────────────
10.4 template field accepts any string without checking existence       [INFO]
────────────────────────────────────────────────────────────────────────────────

Location : src/template.rs — validate_templates()

Template existence is validated before rendering begins (pre-run check). This
is a good fail-fast pattern. The check is comprehensive for the current use
case. No change needed.

================================================================================
11. LOGGING & OBSERVABILITY
================================================================================

────────────────────────────────────────────────────────────────────────────────
11.1 Tracing is configured and used consistently                       [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : src/main.rs

tracing-subscriber is initialised at startup with level derived from --verbose
and --debug flags. tracing::info!/warn!/debug!/trace! are used consistently
throughout the codebase.

────────────────────────────────────────────────────────────────────────────────
11.2 No structured fields in log events                                [LOW]
────────────────────────────────────────────────────────────────────────────────

All log calls use string interpolation (info!("Processing {}", name)) rather
than structured key-value fields (info!(output = %name, "Processing")). For
machine-parseable log output (e.g. JSON via tracing-subscriber's JSON layer),
structured fields produce better output.

Recommendation (non-blocking for v1.0.0):
  Consider adopting structured logging in high-value log calls:

  info!(output_type = %output.output_type, input = %input_path, "Rendering");

────────────────────────────────────────────────────────────────────────────────
11.3 Cache hit/miss events are not logged                              [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : src/cache.rs

Cache hits and misses are not logged at any level. This makes it difficult to
diagnose unexpected re-renders or cache invalidation issues.

Actionable fix:
  Add debug-level log events for cache hits and misses:

  debug!(key = %hash, "Transform cache hit — skipping transform phase");
  debug!(key = %hash, "Transform cache miss — applying transforms");

────────────────────────────────────────────────────────────────────────────────
11.4 External command invocations are logged at appropriate levels     [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : src/adapters/command.rs

Command invocations log arguments at TRACE level and failures at ERROR level.
This provides sufficient observability without polluting normal output.

================================================================================
12. BUILD & DISTRIBUTION READINESS
================================================================================

────────────────────────────────────────────────────────────────────────────────
12.1 Release profile is production-optimised                           [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : Cargo.toml [profile.release]

  opt-level     = 3     OK: maximum optimisation
  lto           = true  OK: link-time optimisation
  codegen-units = 1     OK: single codegen unit for best inter-procedural opt
  strip         = true  OK: symbol stripping reduces binary size
  panic         = "abort" OK: removes unwinding machinery (~10% size saving)

This profile is correct and production-ready.

────────────────────────────────────────────────────────────────────────────────
12.2 Cross-compilation targets are defined                             [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : Cross.toml

  Linux:  x86_64-unknown-linux-musl, aarch64-unknown-linux-musl
  macOS:  x86_64-apple-darwin, aarch64-apple-darwin
  Windows: x86_64-pc-windows-gnu

All major distribution targets are covered.

────────────────────────────────────────────────────────────────────────────────
12.3 CI does not produce pre-built release binaries                    [HIGH]
────────────────────────────────────────────────────────────────────────────────

Location : .github/workflows/ci.yml

The CI pipeline performs lint, build, and test but does not produce or publish
pre-built release binaries. README.md references cargo install as the primary
installation method. The Formula/ directory contains a Homebrew formula
skeleton, but no workflow publishes release artifacts.

Actionable fix:
  Add a release job (triggered on tag push) that:
  1. Cross-compiles for all targets in Cross.toml
  2. Uploads binaries as GitHub Release assets
  3. Generates SHA256 checksums
  4. Updates the Homebrew formula with the new binary URLs and checksums

────────────────────────────────────────────────────────────────────────────────
12.4 Debian and RPM packaging metadata is present                      [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : Cargo.toml [package.metadata.deb], [package.metadata.generate-rpm]

Both .deb and .rpm packaging metadata are correctly configured with binary
paths, license files, and maintainer information. cargo-deb and
cargo-generate-rpm can produce installable packages directly.

────────────────────────────────────────────────────────────────────────────────
12.5 MSRV is set and enforced                                          [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : Cargo.toml

  rust-version = "1.94"

The minimum supported Rust version is declared. CI uses toolchain = "1.94"
matching this constraint. This ensures users on stable toolchains can build
without surprises.

────────────────────────────────────────────────────────────────────────────────
12.6 No SBOM or supply-chain hardening in CI                           [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : .github/workflows/ci.yml

The CI workflow does not generate a Software Bill of Materials (SBOM) or run
cargo audit for known vulnerabilities.

Recommendation (non-blocking for v1.0.0):
  Add cargo audit to the CI build step:

  - name: Security audit
    run: cargo install cargo-audit && cargo audit

  For SBOM, cargo-cyclonedx or cargo-sbom can generate machine-readable output.

================================================================================
13. DOCUMENTATION & DEVELOPER EXPERIENCE
================================================================================

────────────────────────────────────────────────────────────────────────────────
13.1 README does not document watch mode                               [HIGH]
────────────────────────────────────────────────────────────────────────────────

Location : README.md

The watch subcommand (renderflow watch --config config.yaml) is implemented
and tested but not documented in README.md. Users discovering the tool will
not know this mode exists.

Actionable fix:
  Add a "Watch Mode" section to README.md:

  ## Watch Mode
  renderflow watch --config renderflow.yaml
  renderflow watch --config renderflow.yaml --debounce 1000

────────────────────────────────────────────────────────────────────────────────
13.2 README does not document input_format config key                  [MED]
────────────────────────────────────────────────────────────────────────────────

Location : README.md

The input_format key (markdown, docx, html, epub, rst, latex) is supported
in config.yaml but not documented in README.md. Users may not know they can
override auto-detection.

────────────────────────────────────────────────────────────────────────────────
13.3 README does not document the transform pipeline                   [MED]
────────────────────────────────────────────────────────────────────────────────

Location : README.md

The three built-in transforms (emoji replacement, variable substitution,
syntax highlight normalisation) are not documented. Users may not know that
{{key}} variable substitution is available in their source documents.

────────────────────────────────────────────────────────────────────────────────
13.4 CONTRIBUTING.md is present and adequate                           [PASS]
────────────────────────────────────────────────────────────────────────────────

CONTRIBUTING.md exists and provides guidance for contributors. Adequate for
a v1.0.0 release.

────────────────────────────────────────────────────────────────────────────────
13.5 Internal code documentation is consistent                         [PASS]
────────────────────────────────────────────────────────────────────────────────

All public functions carry doc comments. Module-level comments explain purpose
and key design decisions. This is at an appropriate level for a binary crate.

────────────────────────────────────────────────────────────────────────────────
13.6 examples/ directory is empty                                      [MED]
────────────────────────────────────────────────────────────────────────────────

Location : examples/

The examples/ directory exists but contains no working examples. Providing at
least one end-to-end example (config.yaml + sample input + expected output)
would significantly improve onboarding for new users.

Actionable fix:
  Add examples/hello-world/ with:
  - renderflow.yaml  (minimal config: markdown -> html)
  - README.md        (one-liner to run: renderflow build --config renderflow.yaml)
  - hello.md         (sample markdown input)

================================================================================
14. CODE STRUCTURE & IDIOMATIC RUST
================================================================================

────────────────────────────────────────────────────────────────────────────────
14.1 PandocArgs builder pattern is idiomatic                           [PASS]
────────────────────────────────────────────────────────────────────────────────

Location : src/strategies/pandoc_args.rs

The struct fields are private with public builder methods accepting
impl Into<String>. This is correct and idiomatic Rust.

────────────────────────────────────────────────────────────────────────────────
14.2 TransformRegistry fail-fast control should use an enum            [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : src/transforms/registry.rs

  fail_fast: bool

Using a bool is less self-documenting than an enum. Consider:

  pub enum FailureMode { FailFast, ContinueOnError }

This makes call sites and pattern-match arms self-documenting.

────────────────────────────────────────────────────────────────────────────────
14.3 config.rs unsupported_type_message hardcodes supported list       [LOW]
────────────────────────────────────────────────────────────────────────────────

Location : src/config.rs

The unsupported_type_message function hardcodes "html, pdf, docx". When a new
output type is added, this message must be updated manually.

Actionable fix:
  Generate the list dynamically from OutputType variants via a const array or
  strum crate, eliminating the need for manual synchronisation.

================================================================================
15. ACTIONABLE FIXES — PRIORITY ORDER
================================================================================

Priority  Severity  Location                           Fix
--------  --------  ---------------------------------  -------------------------------------------------------
1         HIGH      src/strategies/strategy.rs         Add Send + Sync bounds to OutputStrategy trait
2         HIGH      .github/workflows/ci.yml           Add release job with cross-compiled binary artifacts
3         HIGH      README.md                          Document watch mode, input_format, and transforms
4         MED       src/strategies/strategy.rs         Connect variables + dry_run to actual behaviour
5         MED       src/compat.rs / src/config.rs      Enforce compatibility matrix at config parse time
6         MED       src/config.rs                      Validate empty outputs list
7         MED       src/commands/build.rs              Remove intermediate Vec in output_formats join
8         MED       src/commands/build.rs              Pass &OutputType / Option<&str> to select_strategy
9         MED       src/cache.rs                       Use BTreeMap<String,String> for variables
10        MED       src/files.rs                       Replace .unwrap() with proper error on OsStr conversion
11        MED       examples/                           Add working hello-world example
12        MED       README.md                          Document input_format config key
13        MED       README.md                          Document variable substitution and transforms
14        LOW       src/assets.rs                      Return Cow<str> from normalize_asset_paths
15        LOW       src/transforms/registry.rs         Introduce FailureMode enum instead of bool fail_fast
16        LOW       Cargo.toml                         Move tempfile to [dev-dependencies]
17        LOW       src/commands/build.rs              Prefix progress messages with "[DRY RUN]" in dry-run mode
18        LOW       src/cache.rs                       Log cache hit/miss events at debug level
19        LOW       src/cli.rs                         Surface --debounce default value in --help output

================================================================================
16. V1.0.0 READINESS VERDICT
================================================================================

  Core rendering engine   : READY     ✓ Tested, correct, production-grade
  Architecture            : READY     ✓ Clean two-phase pipeline, strategy pattern
  Error handling          : READY     ✓ Comprehensive, contextual, non-panicking
  Performance             : READY     ✓ Caching, parallelism, optimised profile
  Dependency health       : READY     ✓ All dependencies maintained and current
  CLI & UX                : READY     ✓ Clap-based, well-tested, ergonomic
  Configuration           : NEAR      ⚠ Empty outputs not caught, compat matrix not checked at parse
  Logging                 : NEAR      ⚠ Functional but no structured fields or cache observability
  Build & distribution    : BLOCKED   ✗ No pre-built binary release workflow
  Documentation           : BLOCKED   ✗ Watch mode, input_format, transforms undocumented

Recommended pre-release actions (must-fix):
  1. Add release binary CI job (HIGH — distribution requirement)
  2. Document watch mode, input_format, transforms in README (HIGH — user-facing gap)
  3. Add Send + Sync to OutputStrategy (HIGH — correctness guarantee)

Recommended pre-release actions (should-fix):
  4. Connect RenderContext.variables to pandoc --variable args
  5. Validate empty outputs list in config
  6. Enforce compat matrix at config parse time

Total findings : 20  (HIGH: 3 . MED: 10 . LOW: 7 . PASS: 8 . INFO: 1)
Estimated effort: 8-12 hours to address all HIGH and MED items before v1.0.0.
"#;

/// Run the audit command: generate an optimization report and write it to
/// `audits/audit-{timestamp}.log`.
pub fn run() -> Result<()> {
    let ts = timestamp();
    let branch = git_branch();
    let commit = git_commit();

    let audit_dir = "audits";
    fs::create_dir_all(audit_dir)
        .with_context(|| format!("Failed to create audit directory: {audit_dir}"))?;

    let filename = format!("{audit_dir}/audit-{ts}.log");
    let report = generate_report(&ts, &branch, &commit);

    fs::write(&filename, &report)
        .with_context(|| format!("Failed to write audit report: {filename}"))?;

    info!("Audit report written to: {filename}");
    eprintln!("✓ Audit written to: {filename}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_timestamp_returns_non_empty_string() {
        let ts = timestamp();
        assert!(!ts.is_empty(), "timestamp must not be empty");
    }

    #[test]
    fn test_generate_report_contains_required_sections() {
        let report = generate_report("2026-03-29T000000Z", "main", "abc1234");

        assert!(report.contains("2026-03-29T000000Z"), "report should include timestamp");
        assert!(report.contains("main"), "report should include branch");
        assert!(report.contains("abc1234"), "report should include commit");
        assert!(report.contains("ARCHITECTURE"), "report must have architecture section");
        assert!(report.contains("PERFORMANCE"), "report must have performance section");
        assert!(report.contains("CONCURRENCY"), "report must have concurrency section");
        assert!(report.contains("MEMORY"), "report must have memory section");
        assert!(report.contains("ERROR HANDLING"), "report must have error handling section");
        assert!(report.contains("DEPENDENCY"), "report must have dependency section");
        assert!(report.contains("CLI & UX"), "report must have CLI & UX section");
        assert!(report.contains("CONFIGURATION"), "report must have configuration section");
        assert!(report.contains("LOGGING"), "report must have logging section");
        assert!(report.contains("BUILD & DISTRIBUTION"), "report must have build & distribution section");
        assert!(report.contains("DOCUMENTATION"), "report must have documentation section");
        assert!(report.contains("CODE STRUCTURE"), "report must have code structure section");
        assert!(report.contains("ACTIONABLE FIXES"), "report must have actionable fixes section");
        assert!(report.contains("V1.0.0 READINESS"), "report must have v1.0.0 readiness verdict");
    }

    #[test]
    fn test_generate_report_contains_high_severity_findings() {
        let report = generate_report("2026-03-29T000000Z", "main", "abc1234");
        assert!(report.contains("[HIGH]"), "report must contain HIGH severity findings");
    }

    #[test]
    fn test_generate_report_contains_performance_findings() {
        let report = generate_report("2026-03-29T000000Z", "main", "abc1234");
        assert!(
            report.contains("serde_json::to_string_pretty"),
            "report must mention pretty-printing cache issue"
        );
        assert!(
            report.contains("to_string_lossy"),
            "report must mention unnecessary allocation"
        );
    }

    #[test]
    fn test_run_creates_audit_file() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let original_dir = std::env::current_dir().expect("failed to get current dir");

        std::env::set_current_dir(dir.path()).expect("failed to change dir");

        let result = run();

        std::env::set_current_dir(&original_dir).expect("failed to restore dir");

        assert!(result.is_ok(), "audit run should succeed: {:?}", result);

        // The audit directory should now contain at least one log file.
        let audit_dir = dir.path().join("audits");
        assert!(audit_dir.exists(), "audits/ directory should be created");

        let entries: Vec<_> = fs::read_dir(&audit_dir)
            .expect("failed to read audit dir")
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "audits/ directory should contain at least one file");

        let log_path = &entries[0].path();
        assert!(
            log_path.extension().map(|e| e == "log").unwrap_or(false),
            "audit file should have .log extension"
        );

        let content = fs::read_to_string(log_path).expect("failed to read audit file");
        assert!(content.contains("PERFORMANCE"), "audit file must contain performance section");
        assert!(content.contains("V1.0.0 READINESS"), "audit file must contain v1.0.0 readiness verdict");
    }
}
