use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::info;

use crate::input_format::InputFormat;
use crate::pipeline::step::PipelineStep;
use crate::strategies::{OutputStrategy, RenderContext};

/// A temporary file that is automatically deleted when dropped.
///
/// This is a minimal stdlib-only alternative to `tempfile::NamedTempFile`
/// for use in production code, keeping `tempfile` a dev-only dependency.
struct TempFile {
    path: std::path::PathBuf,
}

impl TempFile {
    /// Create a new temporary file containing `content` and return its handle.
    fn with_content(content: &[u8]) -> Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "renderflow-{}-{}.tmp",
            std::process::id(),
            count
        ));
        let mut file = std::fs::File::create_new(&path)
            .context("Failed to create temporary file for strategy input")?;
        file.write_all(content)
            .context("Failed to write content to temporary file")?;
        Ok(Self { path })
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            tracing::warn!(path = %self.path.display(), error = %e, "Failed to remove temporary file");
        }
    }
}

/// A pipeline step that delegates rendering to an [`OutputStrategy`].
///
/// Wrapping a strategy as a pipeline step allows the pipeline to execute
/// format-specific rendering without being coupled to any particular output type.
///
/// The step receives document content (as a string), writes it to a temporary
/// file, and passes that file path to the strategy via a [`RenderContext`].
/// This ensures transforms applied earlier in the pipeline affect the content
/// that the strategy renders.
pub struct StrategyStep {
    strategy: Box<dyn OutputStrategy>,
    output_path: String,
    input_format: InputFormat,
    variables: HashMap<String, String>,
    dry_run: bool,
}

impl StrategyStep {
    pub fn new(
        strategy: Box<dyn OutputStrategy>,
        output_path: &str,
        input_format: InputFormat,
        variables: HashMap<String, String>,
        dry_run: bool,
    ) -> Self {
        Self {
            strategy,
            output_path: output_path.to_string(),
            input_format,
            variables,
            dry_run,
        }
    }
}

impl PipelineStep for StrategyStep {
    fn name(&self) -> &str {
        "StrategyStep"
    }

    fn execute(&self, input: String) -> Result<String> {
        info!(output = %self.output_path, "Executing strategy step");
        let temp_file = TempFile::with_content(input.as_bytes())?;
        let temp_path = temp_file
            .path()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Temporary file path is not valid UTF-8"))?
            .to_string();
        info!(temp = %temp_path, output = %self.output_path, "Strategy rendering from temporary content file");
        let ctx = RenderContext {
            input_path: &temp_path,
            input_format: self.input_format.clone(),
            output_path: &self.output_path,
            variables: &self.variables,
            dry_run: self.dry_run,
        };
        self.strategy.render(&ctx)?;
        Ok(self.output_path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;

    struct AlwaysOkStrategy;

    impl OutputStrategy for AlwaysOkStrategy {
        fn render(&self, _ctx: &RenderContext) -> Result<()> {
            Ok(())
        }
    }

    struct AlwaysFailStrategy;

    impl OutputStrategy for AlwaysFailStrategy {
        fn render(&self, _ctx: &RenderContext) -> Result<()> {
            bail!("strategy failed")
        }
    }

    fn make_step(strategy: Box<dyn OutputStrategy>, output: &str) -> StrategyStep {
        StrategyStep::new(strategy, output, InputFormat::Markdown, HashMap::new(), false)
    }

    #[test]
    fn test_strategy_step_returns_output_path_on_success() {
        let step = make_step(Box::new(AlwaysOkStrategy), "/tmp/output.html");
        let result = step.execute("input.md".to_string()).unwrap();
        assert_eq!(result, "/tmp/output.html");
    }

    #[test]
    fn test_strategy_step_propagates_strategy_error() {
        let step = make_step(Box::new(AlwaysFailStrategy), "/tmp/output.html");
        let result = step.execute("input.md".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("strategy failed"));
    }

    #[test]
    fn test_strategy_step_stores_output_path() {
        let step = make_step(Box::new(AlwaysOkStrategy), "/custom/path/out.pdf");
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
            fn render(&self, ctx: &RenderContext) -> Result<()> {
                let content = std::fs::read_to_string(ctx.input_path)
                    .expect("strategy should receive a valid temp file path");
                *self.captured.lock().unwrap() = content;
                Ok(())
            }
        }

        let output_file = NamedTempFile::new().unwrap();
        let output_path = output_file.path().to_str().unwrap().to_string();

        let captured = Arc::new(Mutex::new(String::new()));
        let strategy = CapturingStrategy { captured: captured.clone() };
        let step = make_step(Box::new(strategy), &output_path);

        let content = "# Hello World\n\nThis is rendered content.".to_string();
        step.execute(content.clone()).unwrap();

        assert_eq!(*captured.lock().unwrap(), content);
    }

    /// Verifies that the [`RenderContext`] built by `StrategyStep` carries the
    /// correct `input_format`, `variables`, and `dry_run` values.
    #[test]
    fn test_strategy_step_context_fields_are_propagated() {
        use std::sync::{Arc, Mutex};
        use tempfile::NamedTempFile;

        #[derive(Default)]
        struct ContextCapture {
            input_format: Option<InputFormat>,
            dry_run: Option<bool>,
            variables: Option<HashMap<String, String>>,
        }

        struct CapturingStrategy {
            captured: Arc<Mutex<ContextCapture>>,
        }

        impl OutputStrategy for CapturingStrategy {
            fn render(&self, ctx: &RenderContext) -> Result<()> {
                let mut guard = self.captured.lock().unwrap();
                guard.input_format = Some(ctx.input_format.clone());
                guard.dry_run = Some(ctx.dry_run);
                guard.variables = Some(ctx.variables.clone());
                Ok(())
            }
        }

        let output_file = NamedTempFile::new().unwrap();
        let output_path = output_file.path().to_str().unwrap().to_string();

        let mut vars = HashMap::new();
        vars.insert("key".to_string(), "value".to_string());

        let captured = Arc::new(Mutex::new(ContextCapture::default()));
        let strategy = CapturingStrategy { captured: captured.clone() };
        let step = StrategyStep::new(
            Box::new(strategy),
            &output_path,
            InputFormat::Html,
            vars.clone(),
            true,
        );

        step.execute("content".to_string()).unwrap();

        let guard = captured.lock().unwrap();
        assert_eq!(guard.input_format, Some(InputFormat::Html));
        assert_eq!(guard.dry_run, Some(true));
        assert_eq!(guard.variables.as_ref().unwrap().get("key").map(String::as_str), Some("value"));
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
            fn render(&self, ctx: &RenderContext) -> Result<()> {
                let content = std::fs::read_to_string(ctx.input_path)
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
        pipeline.add_step(Box::new(make_step(Box::new(strategy), &output_path)));

        // Input has an emoji; after EmojiTransform it should become "[emoji]"
        let transformed = pipeline.run_transforms("Hello 😀 World".to_string()).unwrap();
        pipeline.run_steps(transformed).unwrap();

        let result = captured.lock().unwrap().clone();
        assert_eq!(result, "Hello [emoji] World");
        assert!(!result.contains('😀'), "emoji should have been replaced by the transform");
    }

    #[test]
    fn test_strategy_step_name() {
        let step = make_step(Box::new(AlwaysOkStrategy), "/tmp/out.html");
        assert_eq!(step.name(), "StrategyStep");
    }
}
