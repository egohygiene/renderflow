use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::process::{
    ProcessExecutor, ProcessExpectedOutput, ProcessNetworkPolicy, ProcessRequest,
    DEFAULT_CAPTURE_LIMIT_BYTES, DEFAULT_PROCESS_TIMEOUT,
};
use crate::strategies::{OutputStrategy, RenderContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EbookOutput {
    Epub,
    Kepub,
}

/// Provider-backed e-book output strategy.
///
/// EPUB is generated as EPUB 3 through Pandoc. KEPUB is an explicit second
/// stage from an EPUB input through Kepubify; the planner never hides that
/// provider substitution.
pub struct EbookStrategy {
    output: EbookOutput,
    metadata_file: Option<String>,
    template_dir: String,
}

impl EbookStrategy {
    pub fn epub(metadata_file: Option<String>, template_dir: String) -> Self {
        Self {
            output: EbookOutput::Epub,
            metadata_file,
            template_dir,
        }
    }

    pub fn kepub() -> Self {
        Self {
            output: EbookOutput::Kepub,
            metadata_file: None,
            template_dir: String::new(),
        }
    }

    fn metadata_path(&self) -> Result<Option<String>> {
        let Some(name) = &self.metadata_file else {
            return Ok(None);
        };
        let path = Path::new(&self.template_dir).join(name);
        if !path.is_file() {
            anyhow::bail!(
                "EPUB metadata file not found: '{}'. Supply an OPF metadata fragment through the output template field.",
                path.display()
            );
        }
        Ok(Some(path.to_string_lossy().into_owned()))
    }

    fn epub_args(&self, ctx: &RenderContext) -> Result<Vec<String>> {
        if ctx
            .variables
            .get("epub.layout")
            .is_some_and(|value| value.eq_ignore_ascii_case("fixed"))
        {
            anyhow::bail!(
                "the Pandoc EPUB adapter only advertises reflow generation; fixed-layout EPUB must be supplied by a dedicated provider and can still be inspected and validated"
            );
        }
        let mut args = vec![
            "--from".to_string(),
            ctx.input_format.as_pandoc_format().to_string(),
            "--to".to_string(),
            "epub3".to_string(),
            "--table-of-contents".to_string(),
            ctx.input_path.to_string(),
            "--output".to_string(),
            ctx.output_path.to_string(),
        ];
        if let Some(path) = self.metadata_path()? {
            args.push("--epub-metadata".to_string());
            args.push(path);
        }
        append_metadata(&mut args, ctx.variables);
        Ok(args)
    }
}

impl OutputStrategy for EbookStrategy {
    fn render(&self, ctx: &RenderContext) -> Result<()> {
        if ctx.dry_run {
            info!(input = %ctx.input_path, output = %ctx.output_path, "[dry-run] Would render e-book derivative");
            return Ok(());
        }
        let (program, args) = match self.output {
            EbookOutput::Epub => ("pandoc", self.epub_args(ctx)?),
            EbookOutput::Kepub => (
                "kepubify",
                vec![
                    "--output".to_string(),
                    ctx.output_path.to_string(),
                    ctx.input_path.to_string(),
                ],
            ),
        };
        let request = ProcessRequest::direct(program)
            .args(args)
            .timeout(DEFAULT_PROCESS_TIMEOUT)
            .capture_limit(DEFAULT_CAPTURE_LIMIT_BYTES)
            .network_policy(ProcessNetworkPolicy::Deny)
            .expect_output(
                ProcessExpectedOutput::file(ctx.output_path)
                    .require_non_empty()
                    .require_change(),
            );
        ProcessExecutor::new()
            .execute_checked(request)
            .with_context(|| {
                format!(
                    "{} failed to produce e-book output '{}' from '{}'",
                    program, ctx.output_path, ctx.input_path
                )
            })?;
        Ok(())
    }
}

fn append_metadata(args: &mut Vec<String>, variables: &HashMap<String, String>) {
    const FIELDS: &[(&str, &str)] = &[
        ("title", "title"),
        ("subtitle", "subtitle"),
        ("author", "author"),
        ("creator", "author"),
        ("language", "lang"),
        ("lang", "lang"),
        ("identifier", "identifier"),
        ("date", "date"),
        ("rights", "rights"),
        ("description", "description"),
    ];
    let mut metadata = FIELDS
        .iter()
        .filter_map(|(source, target)| {
            variables
                .get(*source)
                .or_else(|| variables.get(&format!("epub.{source}")))
                .map(|value| ((*target).to_string(), value.clone()))
        })
        .collect::<Vec<_>>();
    metadata.sort();
    metadata.dedup();
    for (key, value) in metadata {
        args.push("--metadata".to_string());
        args.push(format!("{key}={value}"));
    }
}
