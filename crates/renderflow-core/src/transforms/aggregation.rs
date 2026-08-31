// Items in this module form a public aggregation API that is not yet called
// from the main binary entry-point but is exercised through tests and
// available for callers embedding renderflow as a library.

use std::collections::HashMap;

use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::process::{
    is_explicit_shell_invocation, ProcessExecutor, ProcessExpectedOutput, ProcessInput,
    ProcessOutputMode, ProcessRequest, DEFAULT_CAPTURE_LIMIT_BYTES, DEFAULT_PROCESS_TIMEOUT,
};

/// A transform that consumes an ordered collection of inputs and produces a
/// single aggregated output artifact.
pub trait AggregationTransform: Send + Sync {
    fn name(&self) -> &str {
        "AggregationTransform"
    }

    /// Aggregate ordered input paths and write the result to `output_path`.
    fn aggregate(&self, inputs: &[&str], output_path: &str) -> Result<()>;
}

/// Registry of named collection transforms.
pub struct AggregationRegistry {
    transforms: HashMap<String, Box<dyn AggregationTransform>>,
}

impl AggregationRegistry {
    pub fn new() -> Self {
        Self {
            transforms: HashMap::new(),
        }
    }

    pub fn register(&mut self, transform: Box<dyn AggregationTransform>) -> &mut Self {
        let name = transform.name().to_string();
        self.transforms.insert(name, transform);
        self
    }

    pub fn get(&self, name: &str) -> Option<&dyn AggregationTransform> {
        self.transforms
            .get(name)
            .map(|transform| transform.as_ref())
    }

    pub fn apply(&self, name: &str, inputs: &[&str], output_path: &str) -> Result<()> {
        let transform = self.get(name).ok_or_else(|| {
            anyhow::anyhow!("Aggregation transform '{}' not found in registry", name)
        })?;
        debug!(
            transform = %name,
            inputs = inputs.len(),
            output = %output_path,
            "Starting aggregation transform"
        );
        transform.aggregate(inputs, output_path)?;
        debug!(transform = %name, output = %output_path, "Aggregation transform completed");
        Ok(())
    }
}

impl Default for AggregationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection transform backed by an external executable.
///
/// `{inputs}` expands to one argv entry per input when it is a standalone
/// argument, or a space-joined string when embedded. `{output}` expands to the
/// declared output path. Explicit shell programs remain visibly classified as
/// shell invocations by the canonical process executor.
pub struct CommandAggregationTransform {
    name: String,
    program: String,
    args: Vec<String>,
}

impl CommandAggregationTransform {
    pub fn new(name: impl Into<String>, program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            program: program.into(),
            args,
        }
    }

    pub fn cbz(name: impl Into<String>) -> Self {
        Self::new(
            name,
            "zip",
            vec![
                "-j".to_string(),
                "{output}".to_string(),
                "{inputs}".to_string(),
            ],
        )
    }

    pub fn images_to_pdf(name: impl Into<String>) -> Self {
        Self::new(
            name,
            "img2pdf",
            vec![
                "--output".to_string(),
                "{output}".to_string(),
                "{inputs}".to_string(),
            ],
        )
    }

    pub fn tiff_to_press_pdf(name: impl Into<String>) -> Self {
        Self::new(
            name,
            "gs",
            vec![
                "-dBATCH".to_string(),
                "-dNOPAUSE".to_string(),
                "-sDEVICE=pdfwrite".to_string(),
                "-dPDFSETTINGS=/press".to_string(),
                "-sOutputFile={output}".to_string(),
                "{inputs}".to_string(),
            ],
        )
    }
}

impl AggregationTransform for CommandAggregationTransform {
    fn name(&self) -> &str {
        &self.name
    }

    fn aggregate(&self, inputs: &[&str], output_path: &str) -> Result<()> {
        if inputs.is_empty() {
            anyhow::bail!(
                "Aggregation transform '{}': at least one input is required",
                self.name
            );
        }

        info!(
            transform = %self.name,
            program = %self.program,
            inputs = inputs.len(),
            output = %output_path,
            "Running aggregation transform"
        );

        let has_inputs_placeholder = self.args.iter().any(|arg| arg.contains("{inputs}"));
        let mut processed_args: Vec<String> = Vec::new();
        for arg in &self.args {
            if arg == "{inputs}" {
                processed_args.extend(inputs.iter().map(|value| (*value).to_string()));
            } else {
                let mut processed = arg.clone();
                if processed.contains("{inputs}") {
                    processed = processed.replace("{inputs}", &inputs.join(" "));
                }
                if processed.contains("{output}") {
                    processed = processed.replace("{output}", output_path);
                }
                processed_args.push(processed);
            }
        }

        let request = if is_explicit_shell_invocation(&self.program, &processed_args) {
            ProcessRequest::shell(&self.program)
        } else {
            ProcessRequest::direct(&self.program)
        }
        .args(processed_args)
        .stdin(if has_inputs_placeholder {
            ProcessInput::Null
        } else {
            ProcessInput::Bytes(inputs.join("\n").into_bytes())
        })
        .stdout(ProcessOutputMode::capture(DEFAULT_CAPTURE_LIMIT_BYTES))
        .stderr(ProcessOutputMode::capture(DEFAULT_CAPTURE_LIMIT_BYTES))
        .timeout(DEFAULT_PROCESS_TIMEOUT)
        .expect_output(ProcessExpectedOutput::file(output_path).require_change());

        ProcessExecutor::new()
            .execute_checked(request)
            .with_context(|| format!("Aggregation command '{}' failed", self.program))?;

        info!(
            transform = %self.name,
            output = %output_path,
            "Aggregation transform completed successfully"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct JoinTransform;
    impl AggregationTransform for JoinTransform {
        fn name(&self) -> &str {
            "join"
        }

        fn aggregate(&self, inputs: &[&str], output_path: &str) -> Result<()> {
            std::fs::write(output_path, inputs.join("\n"))
                .context("JoinTransform: failed to write output")?;
            Ok(())
        }
    }

    struct AlwaysFails;
    impl AggregationTransform for AlwaysFails {
        fn name(&self) -> &str {
            "always-fails"
        }

        fn aggregate(&self, _inputs: &[&str], _output_path: &str) -> Result<()> {
            anyhow::bail!("intentional failure")
        }
    }

    #[test]
    fn test_registry_empty_get_returns_none() {
        let registry = AggregationRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = AggregationRegistry::new();
        registry.register(Box::new(JoinTransform));
        assert!(registry.get("join").is_some());
    }

    #[test]
    fn test_registry_apply_missing_returns_error() {
        let registry = AggregationRegistry::new();
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        let result = registry.apply("nonexistent", &["a"], out.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_registry_apply_failing_transform_propagates_error() {
        let mut registry = AggregationRegistry::new();
        registry.register(Box::new(AlwaysFails));
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        assert!(registry
            .apply("always-fails", &["a"], out.to_str().unwrap())
            .is_err());
    }

    #[test]
    fn test_registry_ordering_preserved_in_output() {
        let mut registry = AggregationRegistry::new();
        registry.register(Box::new(JoinTransform));
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        registry
            .apply("join", &["page1", "page2", "page3"], out.to_str().unwrap())
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "page1\npage2\npage3"
        );
    }

    #[test]
    fn test_command_aggregation_name_stored() {
        let transform = CommandAggregationTransform::new("my-agg", "echo", vec![]);
        assert_eq!(transform.name(), "my-agg");
    }

    #[test]
    fn test_command_aggregation_empty_inputs_returns_error() {
        let transform = CommandAggregationTransform::new("test", "echo", vec![]);
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        let result = transform.aggregate(&[], out.to_str().unwrap());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one input"));
    }

    #[test]
    fn test_command_aggregation_invalid_program_returns_error() {
        let transform = CommandAggregationTransform::new(
            "bad-program",
            "__nonexistent_program__",
            vec!["{inputs}".to_string()],
        );
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        assert!(transform.aggregate(&["a"], out.to_str().unwrap()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_command_aggregation_inputs_embedded_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        let in1 = dir.path().join("a.txt");
        let in2 = dir.path().join("b.txt");
        std::fs::write(&in1, "aaa").unwrap();
        std::fs::write(&in2, "bbb").unwrap();

        let transform = CommandAggregationTransform::new(
            "cat-agg",
            "sh",
            vec!["-c".to_string(), "cat {inputs} > {output}".to_string()],
        );
        transform
            .aggregate(
                &[in1.to_str().unwrap(), in2.to_str().unwrap()],
                out.to_str().unwrap(),
            )
            .unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "aaabbb");
    }

    #[cfg(unix)]
    #[test]
    fn test_command_aggregation_ordering_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        let in1 = dir.path().join("p1.txt");
        let in2 = dir.path().join("p2.txt");
        let in3 = dir.path().join("p3.txt");
        std::fs::write(&in1, "page1").unwrap();
        std::fs::write(&in2, "page2").unwrap();
        std::fs::write(&in3, "page3").unwrap();

        let transform = CommandAggregationTransform::new(
            "ordered-cat",
            "sh",
            vec!["-c".to_string(), "cat {inputs} > {output}".to_string()],
        );
        transform
            .aggregate(
                &[
                    in1.to_str().unwrap(),
                    in2.to_str().unwrap(),
                    in3.to_str().unwrap(),
                ],
                out.to_str().unwrap(),
            )
            .unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "page1page2page3");
    }

    #[test]
    fn test_factory_shapes() {
        let cbz = CommandAggregationTransform::cbz("pages-to-cbz");
        assert_eq!(cbz.program, "zip");
        assert!(cbz.args.iter().any(|arg| arg.contains("{output}")));
        assert!(cbz.args.iter().any(|arg| arg.contains("{inputs}")));
        assert!(cbz.args.contains(&"-j".to_string()));

        let pdf = CommandAggregationTransform::images_to_pdf("images-pdf");
        assert_eq!(pdf.program, "img2pdf");
        assert!(pdf.args.iter().any(|arg| arg.contains("{output}")));
        assert!(pdf.args.iter().any(|arg| arg.contains("{inputs}")));

        let press = CommandAggregationTransform::tiff_to_press_pdf("press-pdf");
        assert_eq!(press.program, "gs");
        assert!(press.args.iter().any(|arg| arg.contains("/press")));
        assert!(press.args.iter().any(|arg| arg.contains("{inputs}")));
    }
}
