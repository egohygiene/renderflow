use anyhow::Result;

/// Defines how a specific output format (e.g. HTML, PDF) is rendered.
///
/// Implementors receive raw input content and a destination path, and are
/// responsible for producing the final artefact at that path.
pub trait OutputStrategy {
    /// Render `input` content and write the result to `output_path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the rendering process fails or the output cannot be
    /// written to `output_path`.
    fn render(&self, input: &str, output_path: &str) -> Result<()>;
}
