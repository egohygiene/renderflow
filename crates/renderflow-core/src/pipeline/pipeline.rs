use std::collections::HashMap;

use anyhow::Result;

use crate::config::OutputType;
use crate::transforms::{register_transforms, TransformRegistry};

/// Pure in-memory document transform pipeline used by the canonical artifact adapter.
///
/// Format rendering is intentionally not represented as a second pipeline phase anymore;
/// rendering is a graph `ArtifactTransform` executed by `DagExecutor`.
pub struct Pipeline {
    registry: TransformRegistry,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            registry: TransformRegistry::new(),
        }
    }

    pub fn with_registry(registry: TransformRegistry) -> Self {
        Self { registry }
    }

    pub fn with_standard_transforms(
        variables: &HashMap<String, String>,
        output_type: &OutputType,
    ) -> Self {
        Self::with_registry(register_transforms(variables, output_type))
    }

    pub fn run_transforms(&self, input: String) -> Result<String> {
        self.registry.apply_all(input)
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
    use crate::transforms::{Transform, TransformRegistry};

    struct AppendTransform(&'static str);

    impl Transform for AppendTransform {
        fn apply(&self, input: String) -> Result<String> {
            Ok(format!("{}{}", input, self.0))
        }
    }

    #[test]
    fn empty_pipeline_preserves_input() {
        assert_eq!(
            Pipeline::new().run_transforms("hello".to_string()).unwrap(),
            "hello"
        );
    }

    #[test]
    fn registry_transforms_execute_in_order() {
        let mut registry = TransformRegistry::new();
        registry.register(Box::new(AppendTransform("-a")));
        registry.register(Box::new(AppendTransform("-b")));
        let pipeline = Pipeline::with_registry(registry);
        assert_eq!(pipeline.run_transforms("x".to_string()).unwrap(), "x-a-b");
    }
}
