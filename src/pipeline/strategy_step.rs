use anyhow::Result;
use tracing::info;

use crate::pipeline::step::PipelineStep;
use crate::strategies::OutputStrategy;

/// A pipeline step that delegates rendering to an [`OutputStrategy`].
///
/// Wrapping a strategy as a pipeline step allows the pipeline to execute
/// format-specific rendering without being coupled to any particular output type.
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
        info!(input = %input, output = %self.output_path, "Executing strategy step");
        self.strategy.render(&input, &self.output_path)?;
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
}
