use anyhow::Result;

use super::step::PipelineStep;
use crate::transforms::{Transform, TransformRegistry};

/// An ordered sequence of transforms and output-format steps.
///
/// The pipeline separates document processing into two distinct phases:
///
/// 1. **Transform phase** – pure, in-memory text mutations (emoji replacement,
///    variable substitution, syntax normalisation, …) that are format-agnostic.
///    Transforms are owned and managed by an internal [`TransformRegistry`];
///    call [`Pipeline::run_transforms`] to execute them.
///
/// 2. **Step phase** – format-specific rendering steps (HTML, PDF, …) that
///    consume the transformed text and write output files.  Call
///    [`Pipeline::run_steps`] after transforms have been applied.
///
/// Use [`Pipeline::with_registry`] to attach a pre-configured
/// [`TransformRegistry`] (e.g. the standard one returned by
/// [`crate::transforms::register_transforms`]) instead of adding transforms
/// one-by-one with [`Pipeline::add_transform`].
pub struct Pipeline {
    registry: TransformRegistry,
    steps: Vec<Box<dyn PipelineStep>>,
}

impl Pipeline {
    /// Create an empty pipeline with an empty transform registry.
    pub fn new() -> Self {
        Self {
            registry: TransformRegistry::new(),
            steps: Vec::new(),
        }
    }

    /// Create a pipeline pre-loaded with an existing [`TransformRegistry`].
    ///
    /// This is the preferred constructor when the standard set of transforms
    /// is needed; pair it with [`crate::transforms::register_transforms`]:
    ///
    /// ```ignore
    /// let pipeline = Pipeline::with_registry(register_transforms(&variables));
    /// let output   = pipeline.run_transforms(input)?;
    /// ```
    pub fn with_registry(registry: TransformRegistry) -> Self {
        Self {
            registry,
            steps: Vec::new(),
        }
    }

    /// Append a transform to the internal registry.
    ///
    /// Transforms run in registration order during [`Pipeline::run_transforms`].
    pub fn add_transform(&mut self, transform: Box<dyn Transform>) -> &mut Self {
        self.registry.register(transform);
        self
    }

    /// Append an output-format step.
    pub fn add_step(&mut self, step: Box<dyn PipelineStep>) -> &mut Self {
        self.steps.push(step);
        self
    }

    /// Execute all registered transforms in order by delegating to the
    /// internal [`TransformRegistry`].
    ///
    /// The output of each transform is fed as input to the next. Returns the
    /// final transformed string, or an error that identifies the failing
    /// transform.
    pub fn run_transforms(&self, input: String) -> Result<String> {
        self.registry.apply_all(input)
    }

    /// Execute all registered steps in order.
    pub fn run_steps(&self, input: String) -> Result<String> {
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
    use crate::transforms::Transform;

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

    struct AppendTransform(String);

    impl Transform for AppendTransform {
        fn apply(&self, input: String) -> Result<String> {
            Ok(format!("{}{}", input, self.0))
        }
    }

    struct FailingTransform;

    impl Transform for FailingTransform {
        fn name(&self) -> &'static str {
            "FailingTransform"
        }
        fn apply(&self, _input: String) -> Result<String> {
            bail!("transform failed")
        }
    }

    #[test]
    fn test_pipeline_empty_returns_input() {
        let pipeline = Pipeline::new();
        let transformed = pipeline.run_transforms("hello".to_string()).unwrap();
        let result = pipeline.run_steps(transformed).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_pipeline_single_step() {
        let mut pipeline = Pipeline::new();
        pipeline.add_step(Box::new(AppendStep(" world".to_string())));
        let transformed = pipeline.run_transforms("hello".to_string()).unwrap();
        let result = pipeline.run_steps(transformed).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_pipeline_multiple_steps_sequential() {
        let mut pipeline = Pipeline::new();
        pipeline
            .add_step(Box::new(AppendStep(" step1".to_string())))
            .add_step(Box::new(AppendStep(" step2".to_string())))
            .add_step(Box::new(AppendStep(" step3".to_string())));

        let transformed = pipeline.run_transforms("input".to_string()).unwrap();
        let result = pipeline.run_steps(transformed).unwrap();
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

        let transformed = pipeline.run_transforms("hello".to_string()).unwrap();
        let result = pipeline.run_steps(transformed).unwrap();
        assert_eq!(result, "HELLO!");
    }

    #[test]
    fn test_pipeline_error_propagates() {
        let mut pipeline = Pipeline::new();
        pipeline
            .add_step(Box::new(AppendStep(" ok".to_string())))
            .add_step(Box::new(FailingStep))
            .add_step(Box::new(AppendStep(" never".to_string())));

        let transformed = pipeline.run_transforms("input".to_string()).unwrap();
        let result = pipeline.run_steps(transformed);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("step failed"));
    }

    #[test]
    fn test_transforms_apply_in_order() {
        let mut pipeline = Pipeline::new();
        pipeline
            .add_transform(Box::new(AppendTransform(" t1".to_string())))
            .add_transform(Box::new(AppendTransform(" t2".to_string())))
            .add_transform(Box::new(AppendTransform(" t3".to_string())));

        let result = pipeline.run_transforms("input".to_string()).unwrap();
        assert_eq!(result, "input t1 t2 t3");
    }

    #[test]
    fn test_transforms_run_before_steps() {
        let mut pipeline = Pipeline::new();
        pipeline
            .add_transform(Box::new(AppendTransform(" transformed".to_string())))
            .add_step(Box::new(AppendStep(" rendered".to_string())));

        let transformed = pipeline.run_transforms("input".to_string()).unwrap();
        let result = pipeline.run_steps(transformed).unwrap();
        assert_eq!(result, "input transformed rendered");
    }

    #[test]
    fn test_transform_error_propagates() {
        let mut pipeline = Pipeline::new();
        pipeline
            .add_transform(Box::new(AppendTransform(" ok".to_string())))
            .add_transform(Box::new(FailingTransform))
            .add_transform(Box::new(AppendTransform(" never".to_string())));

        let result = pipeline.run_transforms("input".to_string());
        assert!(result.is_err());
        // The error context must identify the failing transform.
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Transform failed: FailingTransform"),
            "expected context message, got: {msg}"
        );
    }

    #[test]
    fn test_transform_output_chaining() {
        struct UppercaseTransform;
        impl Transform for UppercaseTransform {
            fn apply(&self, input: String) -> Result<String> {
                Ok(input.to_uppercase())
            }
        }

        let mut pipeline = Pipeline::new();
        pipeline
            .add_transform(Box::new(UppercaseTransform))
            .add_transform(Box::new(AppendTransform("!".to_string())));

        let result = pipeline.run_transforms("hello".to_string()).unwrap();
        assert_eq!(result, "HELLO!");
    }
}
