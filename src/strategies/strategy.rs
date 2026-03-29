use std::collections::HashMap;

use anyhow::Result;

use crate::input_format::InputFormat;

/// Structured context passed to every [`OutputStrategy::render`] call.
///
/// Centralises all information a strategy needs to produce output, making
/// the interface future-proof and simplifying parameter passing.
pub struct RenderContext<'a> {
    /// Path to the input file that the strategy should read.
    pub input_path: &'a str,
    /// The document input format; mapped to a pandoc `--from` string via
    /// [`InputFormat::as_pandoc_format`] when invoking pandoc.
    pub input_format: InputFormat,
    /// Destination path where the rendered output should be written.
    pub output_path: &'a str,
    /// Template variables from the renderflow config.
    /// Passed to pandoc as `--variable key=value` arguments, making them
    /// available inside pandoc templates.
    pub variables: &'a HashMap<String, String>,
    /// When `true` the strategy should skip file system writes and external
    /// commands. Strategies are currently never invoked in dry-run mode because
    /// the build command short-circuits before reaching them; this field is
    /// populated for future strategy-level dry-run support.
    #[allow(dead_code)]
    pub dry_run: bool,
}

/// Defines how a specific output format (e.g. HTML, PDF) is rendered.
///
/// Implementors receive a [`RenderContext`] containing all information
/// required to produce the final artefact at the configured output path.
pub trait OutputStrategy: Send + Sync {
    /// Render the document described by `ctx` and write the result to
    /// `ctx.output_path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the rendering process fails or the output cannot be
    /// written to `ctx.output_path`.
    fn render(&self, ctx: &RenderContext) -> Result<()>;
}
