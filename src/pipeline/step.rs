use anyhow::Result;

pub trait PipelineStep {
    /// Human-readable name for this step, used in log messages and performance traces.
    ///
    /// Override this in concrete step types to make timing diagnostics more
    /// actionable (e.g. `"HtmlStep"` instead of the generic `"PipelineStep"`).
    fn name(&self) -> &str {
        "PipelineStep"
    }

    fn execute(&self, input: String) -> Result<String>;
}
