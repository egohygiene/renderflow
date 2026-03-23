use anyhow::{Context, Result};
use std::io::Write;
use tempfile::NamedTempFile;
use tracing::info;

use crate::pipeline::step::PipelineStep;
use crate::strategies::OutputStrategy;

/// A pipeline step that delegates rendering to an [`OutputStrategy`].
///
/// Wrapping a strategy as a pipeline step allows the pipeline to execute
/// format-specific rendering without being coupled to any particular output type.
///
/// The step receives document content (as a string), writes it to a temporary
/// file, and passes that file path to the strategy. This ensures transforms
/// applied earlier in the pipeline affect the content that the strategy renders.
pub struct StrategyStep {
    strategy: Box<dyn OutputStrategy>,
    output_path: String,
}

impl StrategyStep {
    pub fn new(strategy: Box<dyn OutputStrategy>, output_path: &str) -> Self {
        Self {
            strategy,
            output_path: output_path.to_string(),
        }
    }
}

impl PipelineStep for StrategyStep {
    fn execute(&self, input: String) -> Result<String> {
        info!(output = %self.output_path, "Executing strategy step");
        let mut temp_file =
            NamedTempFile::new().context("Failed to create temporary file for strategy input")?;
        temp_file
            .write_all(input.as_bytes())
            .context("Failed to write content to temporary file")?;
        let temp_path = temp_file
            .path()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Temporary file path is not valid UTF-8"))?
            .to_string();
        info!(temp = %temp_path, output = %self.output_path, "Strategy rendering from temporary content file");
        self.strategy.render(&temp_path, &self.output_path)?;
        Ok(self.output_path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;

    struct AlwaysOkStrategy;

    impl OutputStrategy for AlwaysOkStrategy {
        fn render(&self, _input: &str, _output_path: &str) -> Result<()> {
            Ok(())
        }
    }

    struct AlwaysFailStrategy;

    impl OutputStrategy for AlwaysFailStrategy {
        fn render(&self, _input: &str, _output_path: &str) -> Result<()> {
            bail!("strategy failed")
        }
    }

    #[test]
    fn test_strategy_step_returns_output_path_on_success() {
        let step = StrategyStep::new(Box::new(AlwaysOkStrategy), "/tmp/output.html");
        let result = step.execute("input.md".to_string()).unwrap();
        assert_eq!(result, "/tmp/output.html");
    }

    #[test]
    fn test_strategy_step_propagates_strategy_error() {
        let step = StrategyStep::new(Box::new(AlwaysFailStrategy), "/tmp/output.html");
        let result = step.execute("input.md".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("strategy failed"));
    }

    #[test]
    fn test_strategy_step_stores_output_path() {
        let step = StrategyStep::new(Box::new(AlwaysOkStrategy), "/custom/path/out.pdf");
        let result = step.execute("input.md".to_string()).unwrap();
        assert_eq!(result, "/custom/path/out.pdf");
    }

    /// Verifies that `StrategyStep` writes the input content to a temporary file
    /// and passes that file's path to the strategy (not the raw content string).
    #[test]
    fn test_strategy_step_passes_content_via_temp_file() {
        use std::sync::{Arc, Mutex};
        use tempfile::NamedTempFile;

        struct CapturingStrategy {
            captured: Arc<Mutex<String>>,
        }

        impl OutputStrategy for CapturingStrategy {
            fn render(&self, input: &str, _output_path: &str) -> Result<()> {
                let content = std::fs::read_to_string(input)
                    .expect("strategy should receive a valid temp file path");
                *self.captured.lock().unwrap() = content;
                Ok(())
            }
        }

        let output_file = NamedTempFile::new().unwrap();
        let output_path = output_file.path().to_str().unwrap().to_string();

        let captured = Arc::new(Mutex::new(String::new()));
        let strategy = CapturingStrategy { captured: captured.clone() };
        let step = StrategyStep::new(Box::new(strategy), &output_path);

        let content = "# Hello World\n\nThis is rendered content.".to_string();
        step.execute(content.clone()).unwrap();

        assert_eq!(*captured.lock().unwrap(), content);
    }

    /// End-to-end test: verifies that transforms applied before `StrategyStep`
    /// affect the content received by the strategy.
    #[test]
    fn test_transforms_affect_strategy_input() {
        use std::sync::{Arc, Mutex};
        use tempfile::NamedTempFile;
        use crate::pipeline::Pipeline;
        use crate::transforms::EmojiTransform;

        struct CapturingStrategy {
            captured: Arc<Mutex<String>>,
        }

        impl OutputStrategy for CapturingStrategy {
            fn render(&self, input: &str, _output_path: &str) -> Result<()> {
                let content = std::fs::read_to_string(input)
                    .expect("strategy should receive a valid temp file path");
                *self.captured.lock().unwrap() = content;
                Ok(())
            }
        }

        let output_file = NamedTempFile::new().unwrap();
        let output_path = output_file.path().to_str().unwrap().to_string();

        let captured = Arc::new(Mutex::new(String::new()));
        let strategy = CapturingStrategy { captured: captured.clone() };

        let mut pipeline = Pipeline::new();
        pipeline.add_transform(Box::new(EmojiTransform::new()));
        pipeline.add_step(Box::new(StrategyStep::new(Box::new(strategy), &output_path)));

        // Input has an emoji; after EmojiTransform it should become "[emoji]"
        pipeline.run("Hello 😀 World".to_string()).unwrap();

        let result = captured.lock().unwrap().clone();
        assert_eq!(result, "Hello [emoji] World");
        assert!(!result.contains('😀'), "emoji should have been replaced by the transform");
    }
}
