// Items in this module form a public aggregation API that is not yet called
// from the main binary entry-point but is exercised through tests and
// available for callers embedding renderflow as a library.
#![allow(dead_code)]

use std::collections::HashMap;
use std::process::Stdio;

use anyhow::{Context, Result};
use tracing::{debug, info};

/// A transform that consumes an ordered collection of inputs and produces a
/// single aggregated output artifact.
///
/// Unlike [`Transform`](super::Transform), which operates on a single input
/// string, an `AggregationTransform` is designed for edition-level workflows
/// such as combining multiple page images into a single CBZ archive or PDF
/// document.
///
/// # Example
///
/// ```rust
/// use renderflow::transforms::aggregation::{AggregationRegistry, AggregationTransform};
/// use anyhow::Result;
///
/// struct JoinLines;
/// impl AggregationTransform for JoinLines {
///     fn name(&self) -> &str { "join-lines" }
///     fn aggregate(&self, inputs: &[&str], output_path: &str) -> Result<()> {
///         std::fs::write(output_path, inputs.join("\n"))?;
///         Ok(())
///     }
/// }
///
/// let mut registry = AggregationRegistry::new();
/// registry.register(Box::new(JoinLines));
/// ```
pub trait AggregationTransform: Send + Sync {
    /// Human-readable name for this transform, used in log messages and
    /// error context.
    fn name(&self) -> &str {
        "AggregationTransform"
    }

    /// Aggregate the ordered `inputs` and write the result to `output_path`.
    ///
    /// `inputs` is an ordered slice of strings.  For file-based aggregation
    /// (e.g. images to CBZ) the strings are file paths; for text-based
    /// aggregation they are document content strings.
    ///
    /// The transform is responsible for writing its output to `output_path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the aggregation fails, the external command
    /// cannot be started, or the output cannot be written.
    fn aggregate(&self, inputs: &[&str], output_path: &str) -> Result<()>;
}

/// A registry of named [`AggregationTransform`] implementations.
///
/// Transforms are stored by name and can be looked up and applied by name.
/// This registry is used to select the correct aggregation strategy for a
/// given collection-based DAG edge (identified by its
/// [`label`](crate::graph::TransformDefinition::label)).
///
/// # Example
///
/// ```rust
/// use renderflow::transforms::aggregation::{AggregationRegistry, CommandAggregationTransform};
///
/// let mut registry = AggregationRegistry::new();
/// registry.register(Box::new(CommandAggregationTransform::cbz("pages-to-cbz")));
/// registry.register(Box::new(CommandAggregationTransform::images_to_pdf("images-to-pdf")));
/// ```
pub struct AggregationRegistry {
    transforms: HashMap<String, Box<dyn AggregationTransform>>,
}

impl AggregationRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { transforms: HashMap::new() }
    }

    /// Register a named aggregation transform.
    ///
    /// If a transform with the same name is already registered, it is
    /// replaced by the new one.
    pub fn register(&mut self, transform: Box<dyn AggregationTransform>) -> &mut Self {
        let name = transform.name().to_string();
        self.transforms.insert(name, transform);
        self
    }

    /// Look up a transform by name.
    ///
    /// Returns `None` when no transform with the given `name` has been
    /// registered.
    pub fn get(&self, name: &str) -> Option<&dyn AggregationTransform> {
        self.transforms.get(name).map(|t| t.as_ref())
    }

    /// Apply the named aggregation transform to the ordered `inputs`,
    /// writing the result to `output_path`.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    /// * no transform with `name` is registered, or
    /// * the transform's [`aggregate`](AggregationTransform::aggregate) call
    ///   fails.
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

/// An aggregation transform backed by an external command.
///
/// The command is invoked with a processed argument list derived from `args`.
/// Two placeholders are supported:
///
/// | Placeholder  | Expansion                                                        |
/// |--------------|------------------------------------------------------------------|
/// | `{inputs}`   | Standalone: one argument per input path.  Embedded in a larger  |
/// |              | string: all paths joined by a single space.                      |
/// | `{output}`   | Replaced with the `output_path` supplied to [`aggregate`].      |
///
/// When neither placeholder appears in `args`, all inputs are joined with
/// newlines and piped to the command's `stdin`; `stdout` is ignored and the
/// command is expected to write its output directly (e.g. via shell
/// redirection captured in a wrapper arg).
///
/// # Example
///
/// ```rust
/// use renderflow::transforms::aggregation::CommandAggregationTransform;
///
/// // Equivalent to: zip -j output.cbz page1.jpg page2.jpg
/// let t = CommandAggregationTransform::new(
///     "images-to-cbz",
///     "zip",
///     vec!["-j".to_string(), "{output}".to_string(), "{inputs}".to_string()],
/// );
/// assert_eq!(t.name(), "images-to-cbz");
/// ```
pub struct CommandAggregationTransform {
    name: String,
    program: String,
    args: Vec<String>,
}

impl CommandAggregationTransform {
    /// Create a new `CommandAggregationTransform`.
    ///
    /// * `name`    – human-readable identifier used in logs and registry lookups.
    /// * `program` – executable to invoke (looked up on `PATH`).
    /// * `args`    – argument list; may include `{inputs}` and `{output}` placeholders.
    pub fn new(name: impl Into<String>, program: impl Into<String>, args: Vec<String>) -> Self {
        Self { name: name.into(), program: program.into(), args }
    }

    /// Build a **CBZ** aggregation transform.
    ///
    /// Uses the system `zip` command to package ordered image files into a
    /// Comic Book ZIP (`.cbz`) archive.  The `-j` flag strips directory
    /// components so that all images appear at the archive root.
    ///
    /// Generated command: `zip -j {output} {inputs}`
    pub fn cbz(name: impl Into<String>) -> Self {
        Self::new(
            name,
            "zip",
            vec!["-j".to_string(), "{output}".to_string(), "{inputs}".to_string()],
        )
    }

    /// Build an **images-to-PDF** aggregation transform.
    ///
    /// Uses `img2pdf` to losslessly combine ordered image files into a PDF
    /// document, preserving the original image data without re-encoding.
    ///
    /// Generated command: `img2pdf --output {output} {inputs}`
    pub fn images_to_pdf(name: impl Into<String>) -> Self {
        Self::new(
            name,
            "img2pdf",
            vec!["--output".to_string(), "{output}".to_string(), "{inputs}".to_string()],
        )
    }

    /// Build a **TIFF-to-press-PDF** aggregation transform.
    ///
    /// Uses Ghostscript (`gs`) to combine TIFF source files into a
    /// press-quality PDF document (PDF settings: `/press`), optimised for
    /// high-resolution print output.
    ///
    /// Generated command:
    /// ```text
    /// gs -dBATCH -dNOPAUSE -sDEVICE=pdfwrite -dPDFSETTINGS=/press
    ///    -sOutputFile={output} {inputs}
    /// ```
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

        // Expand placeholders in each argument.
        let mut processed_args: Vec<String> = Vec::new();
        for arg in &self.args {
            if arg == "{inputs}" {
                // Standalone placeholder: one argument per input path.
                processed_args.extend(inputs.iter().map(|s| s.to_string()));
            } else {
                let mut processed = arg.clone();
                if processed.contains("{inputs}") {
                    // Embedded placeholder: join all paths with a single space.
                    processed = processed.replace("{inputs}", &inputs.join(" "));
                }
                if processed.contains("{output}") {
                    processed = processed.replace("{output}", output_path);
                }
                processed_args.push(processed);
            }
        }

        let cmd_output = std::process::Command::new(&self.program)
            .args(&processed_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| {
                format!("Failed to start aggregation command '{}'", self.program)
            })?
            .wait_with_output()
            .with_context(|| {
                format!("Failed to wait for aggregation command '{}'", self.program)
            })?;

        if !cmd_output.status.success() {
            let stderr = String::from_utf8_lossy(&cmd_output.stderr);
            anyhow::bail!(
                "Aggregation command '{}' exited with status {}: {}",
                self.program,
                cmd_output.status,
                stderr.trim()
            );
        }

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

    // ── helpers ───────────────────────────────────────────────────────────────

    /// A simple aggregation transform that joins inputs with newlines.
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

    // ── AggregationRegistry ───────────────────────────────────────────────────

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
        let result = registry.apply("always-fails", &["a"], out.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_apply_succeeds_and_writes_output() {
        let mut registry = AggregationRegistry::new();
        registry.register(Box::new(JoinTransform));
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        registry
            .apply("join", &["first", "second", "third"], out.to_str().unwrap())
            .unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert_eq!(content, "first\nsecond\nthird");
    }

    #[test]
    fn test_registry_register_replaces_existing() {
        struct WriteA;
        impl AggregationTransform for WriteA {
            fn name(&self) -> &str {
                "writer"
            }
            fn aggregate(&self, _: &[&str], p: &str) -> Result<()> {
                std::fs::write(p, "a")?;
                Ok(())
            }
        }
        struct WriteB;
        impl AggregationTransform for WriteB {
            fn name(&self) -> &str {
                "writer"
            }
            fn aggregate(&self, _: &[&str], p: &str) -> Result<()> {
                std::fs::write(p, "b")?;
                Ok(())
            }
        }

        let mut registry = AggregationRegistry::new();
        registry.register(Box::new(WriteA));
        registry.register(Box::new(WriteB));
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        registry.apply("writer", &["ignored"], out.to_str().unwrap()).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "b");
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
        let content = std::fs::read_to_string(&out).unwrap();
        let pos1 = content.find("page1").expect("page1 missing");
        let pos2 = content.find("page2").expect("page2 missing");
        let pos3 = content.find("page3").expect("page3 missing");
        assert!(pos1 < pos2, "page1 must come before page2");
        assert!(pos2 < pos3, "page2 must come before page3");
    }

    // ── CommandAggregationTransform ───────────────────────────────────────────

    #[test]
    fn test_command_aggregation_name_stored() {
        let t = CommandAggregationTransform::new("my-agg", "echo", vec![]);
        assert_eq!(t.name(), "my-agg");
    }

    #[test]
    fn test_command_aggregation_empty_inputs_returns_error() {
        let t = CommandAggregationTransform::new("test", "echo", vec![]);
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        let result = t.aggregate(&[], out.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one input"));
    }

    #[test]
    fn test_command_aggregation_invalid_program_returns_error() {
        let t = CommandAggregationTransform::new(
            "bad-program",
            "__nonexistent_program__",
            vec!["{inputs}".to_string()],
        );
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");
        let result = t.aggregate(&["a"], out.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_command_aggregation_inputs_embedded_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.txt");

        let in1 = dir.path().join("a.txt");
        let in2 = dir.path().join("b.txt");
        std::fs::write(&in1, "aaa").unwrap();
        std::fs::write(&in2, "bbb").unwrap();

        // {inputs} embedded in a shell -c argument is space-joined.
        let t = CommandAggregationTransform::new(
            "cat-agg",
            "sh",
            vec!["-c".to_string(), "cat {inputs} > {output}".to_string()],
        );
        t.aggregate(
            &[in1.to_str().unwrap(), in2.to_str().unwrap()],
            out.to_str().unwrap(),
        )
        .unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert_eq!(content, "aaabbb");
    }

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

        let t = CommandAggregationTransform::new(
            "ordered-cat",
            "sh",
            vec!["-c".to_string(), "cat {inputs} > {output}".to_string()],
        );
        t.aggregate(
            &[in1.to_str().unwrap(), in2.to_str().unwrap(), in3.to_str().unwrap()],
            out.to_str().unwrap(),
        )
        .unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        let pos1 = content.find("page1").expect("page1 missing");
        let pos2 = content.find("page2").expect("page2 missing");
        let pos3 = content.find("page3").expect("page3 missing");
        assert!(pos1 < pos2, "page1 must come before page2");
        assert!(pos2 < pos3, "page2 must come before page3");
    }

    // ── factory methods ───────────────────────────────────────────────────────

    #[test]
    fn test_cbz_factory_name_and_program() {
        let t = CommandAggregationTransform::cbz("pages-to-cbz");
        assert_eq!(t.name(), "pages-to-cbz");
        assert_eq!(t.program, "zip");
    }

    #[test]
    fn test_cbz_factory_args_contain_output_and_inputs() {
        let t = CommandAggregationTransform::cbz("cbz");
        assert!(t.args.iter().any(|a| a.contains("{output}")), "args must contain {{output}}");
        assert!(t.args.iter().any(|a| a.contains("{inputs}")), "args must contain {{inputs}}");
    }

    #[test]
    fn test_cbz_factory_uses_dash_j_flag() {
        let t = CommandAggregationTransform::cbz("cbz");
        assert!(t.args.contains(&"-j".to_string()), "CBZ args must include -j to strip paths");
    }

    #[test]
    fn test_images_to_pdf_factory_name_and_program() {
        let t = CommandAggregationTransform::images_to_pdf("images-pdf");
        assert_eq!(t.name(), "images-pdf");
        assert_eq!(t.program, "img2pdf");
    }

    #[test]
    fn test_images_to_pdf_factory_args_contain_output_and_inputs() {
        let t = CommandAggregationTransform::images_to_pdf("images-pdf");
        assert!(t.args.iter().any(|a| a.contains("{output}")), "args must contain {{output}}");
        assert!(t.args.iter().any(|a| a.contains("{inputs}")), "args must contain {{inputs}}");
    }

    #[test]
    fn test_tiff_to_press_pdf_factory_name_and_program() {
        let t = CommandAggregationTransform::tiff_to_press_pdf("press-pdf");
        assert_eq!(t.name(), "press-pdf");
        assert_eq!(t.program, "gs");
    }

    #[test]
    fn test_tiff_to_press_pdf_factory_uses_press_settings() {
        let t = CommandAggregationTransform::tiff_to_press_pdf("press-pdf");
        assert!(
            t.args.iter().any(|a| a.contains("/press")),
            "TIFF-to-press-PDF must use -dPDFSETTINGS=/press"
        );
    }

    #[test]
    fn test_tiff_to_press_pdf_factory_args_contain_output_and_inputs() {
        let t = CommandAggregationTransform::tiff_to_press_pdf("press-pdf");
        assert!(
            t.args.iter().any(|a| a.contains("{output}") || a.contains("OutputFile")),
            "args must contain output placeholder"
        );
        assert!(t.args.iter().any(|a| a.contains("{inputs}")), "args must contain {{inputs}}");
    }
}
