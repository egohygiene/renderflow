use anyhow::Result;

use super::step::PipelineStep;

pub struct Pipeline {
    steps: Vec<Box<dyn PipelineStep>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn add_step(&mut self, step: Box<dyn PipelineStep>) -> &mut Self {
        self.steps.push(step);
        self
    }

    pub fn run(&self, input: String) -> Result<String> {
        let mut current = input;
        for step in &self.steps {
            current = step.execute(current)?;
        }
        Ok(current)
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;

    struct AppendStep(String);

    impl PipelineStep for AppendStep {
        fn execute(&self, input: String) -> Result<String> {
            Ok(format!("{}{}", input, self.0))
        }
    }

    struct FailingStep;

    impl PipelineStep for FailingStep {
        fn execute(&self, _input: String) -> Result<String> {
            bail!("step failed")
        }
    }

    #[test]
    fn test_pipeline_empty_returns_input() {
        let pipeline = Pipeline::new();
        let result = pipeline.run("hello".to_string()).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_pipeline_single_step() {
        let mut pipeline = Pipeline::new();
        pipeline.add_step(Box::new(AppendStep(" world".to_string())));
        let result = pipeline.run("hello".to_string()).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_pipeline_multiple_steps_sequential() {
        let mut pipeline = Pipeline::new();
        pipeline
            .add_step(Box::new(AppendStep(" step1".to_string())))
            .add_step(Box::new(AppendStep(" step2".to_string())))
            .add_step(Box::new(AppendStep(" step3".to_string())));

        let result = pipeline.run("input".to_string()).unwrap();
        assert_eq!(result, "input step1 step2 step3");
    }

    #[test]
    fn test_pipeline_output_of_one_step_is_input_of_next() {
        struct UppercaseStep;
        impl PipelineStep for UppercaseStep {
            fn execute(&self, input: String) -> Result<String> {
                Ok(input.to_uppercase())
            }
        }

        struct AppendExclamation;
        impl PipelineStep for AppendExclamation {
            fn execute(&self, input: String) -> Result<String> {
                Ok(format!("{}!", input))
            }
        }

        let mut pipeline = Pipeline::new();
        pipeline
            .add_step(Box::new(UppercaseStep))
            .add_step(Box::new(AppendExclamation));

        let result = pipeline.run("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO!");
    }

    #[test]
    fn test_pipeline_error_propagates() {
        let mut pipeline = Pipeline::new();
        pipeline
            .add_step(Box::new(AppendStep(" ok".to_string())))
            .add_step(Box::new(FailingStep))
            .add_step(Box::new(AppendStep(" never".to_string())));

        let result = pipeline.run("input".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("step failed"));
    }
}
