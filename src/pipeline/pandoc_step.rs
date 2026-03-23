use anyhow::{Context, Result};
use tracing::info;

use super::step::PipelineStep;
use crate::adapters::command::run_command;

pub struct PandocStep {
    pub output_path: String,
}

impl PandocStep {
    pub fn new(output_path: impl Into<String>) -> Self {
        Self {
            output_path: output_path.into(),
        }
    }
}

impl PipelineStep for PandocStep {
    fn execute(&self, input: String) -> Result<String> {
        info!(input = %input, output = %self.output_path, "Running pandoc");
        run_command("pandoc", &[&input, "-o", &self.output_path])
            .with_context(|| format!(
                "Failed to run pandoc on '{}' to produce '{}'. \
                 Check that pandoc is installed (`pandoc --version`) and that the input file is valid.",
                input, self.output_path
            ))?;
        info!(output = %self.output_path, "Pandoc step completed successfully");
        Ok(self.output_path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pandoc_step_stores_output_path() {
        let step = PandocStep::new("dist/output.html");
        assert_eq!(step.output_path, "dist/output.html");
    }

    #[test]
    fn test_pandoc_step_execute_errors_on_missing_input() {
        let output_dir = std::env::temp_dir();
        let output_path = output_dir.join("renderflow_test_output.html");
        let step = PandocStep::new(output_path.to_str().unwrap());
        let result = step.execute("/nonexistent/input.md".to_string());
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("Failed to run pandoc"),
            "error should describe what failed: {}",
            msg
        );
    }

    #[test]
    #[ignore = "requires pandoc to be installed"]
    fn test_pandoc_step_converts_markdown_to_html() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut input = NamedTempFile::new().unwrap();
        writeln!(input, "# Hello\n\nThis is a test.").unwrap();

        let output = NamedTempFile::new().unwrap();
        let output_path = output.path().with_extension("html");

        let step = PandocStep::new(output_path.to_str().unwrap());
        let result = step.execute(input.path().to_str().unwrap().to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), output_path.to_str().unwrap());
        assert!(output_path.exists());
    }
}
