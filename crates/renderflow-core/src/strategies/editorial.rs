//! Bundled article presentation selected through the ordinary strategy adapter.
//!
//! Pandoc owns Markdown parsing; the bundled Lua filter owns the article content
//! contract. Rendering remains a bounded local provider operation, not an agent.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::config::OutputType;
use crate::font::{resolve_from_variables, FontTarget, FONT_REGISTRY_VARIABLE};
use crate::input_format::InputFormat;
use crate::process::{ProcessEnvironment, ProcessExecutor, ProcessExpectedOutput, ProcessNetworkPolicy, ProcessRequest};
use super::{OutputStrategy, PandocArgs, RenderContext};

pub const EDITORIAL_PRESET: &str = "editorial-v1";
pub const MIN_PANDOC_VERSION: &str = "3.6.1";
pub(crate) const ASSET_ROOT_VARIABLE: &str = "renderflow-editorial-asset-root";

// The complete presentation bundle travels in the binary. Shared LaTeX sources
// remain canonical in templates/latex and are staged without CWD assumptions.
const BUNDLE: &[(&str, &[u8])] = &[
    ("article.html", include_bytes!("../../data/editorial/article.html")),
    ("article.tex", include_bytes!("../../data/editorial/article.tex")),
    ("article.css", include_bytes!("../../data/editorial/article.css")),
    ("article.lua", include_bytes!("../../data/editorial/article.lua")),
    ("fonts/registry.json", include_bytes!("../../data/editorial/fonts/registry.json")),
    ("fonts/LICENSE.txt", include_bytes!("../../data/editorial/fonts/LICENSE.txt")),
    ("fonts/DejaVuSerif.ttf", include_bytes!("../../data/editorial/fonts/DejaVuSerif.ttf")),
    ("fonts/DejaVuSans.ttf", include_bytes!("../../data/editorial/fonts/DejaVuSans.ttf")),
    ("fonts/DejaVuSansMono.ttf", include_bytes!("../../data/editorial/fonts/DejaVuSansMono.ttf")),
    ("latex/renderflow-core.sty", include_bytes!("../../../../templates/latex/renderflow-core.sty")),
    ("latex/renderflow-color.sty", include_bytes!("../../../../templates/latex/renderflow-color.sty")),
    ("latex/renderflow-typography.sty", include_bytes!("../../../../templates/latex/renderflow-typography.sty")),
    ("latex/renderflow-layout.sty", include_bytes!("../../../../templates/latex/renderflow-layout.sty")),
    ("latex/renderflow-figures.sty", include_bytes!("../../../../templates/latex/renderflow-figures.sty")),
    ("latex/renderflow-tables.sty", include_bytes!("../../../../templates/latex/renderflow-tables.sty")),
    ("latex/renderflow-code.sty", include_bytes!("../../../../templates/latex/renderflow-code.sty")),
    ("latex/renderflow-callouts.sty", include_bytes!("../../../../templates/latex/renderflow-callouts.sty")),
    ("latex/renderflow-metadata.sty", include_bytes!("../../../../templates/latex/renderflow-metadata.sty")),
];

/// Identity of the actual bundled templates, filter, styles and font bytes.
pub fn bundle_fingerprint() -> String {
    let mut hash = Sha256::new();
    for (name, bytes) in BUNDLE {
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}

fn stage_bundle(root: &Path) -> Result<()> {
    for (name, bytes) in BUNDLE {
        let path = root.join(name);
        fs::create_dir_all(path.parent().context("editorial.bundle.parent")?)?;
        fs::write(path, bytes)?;
    }
    Ok(())
}

fn article_toc(path: &str) -> Result<bool> {
    let mut source = String::new();
    fs::File::open(path)?.take(4 * 1024 * 1024 + 1).read_to_string(&mut source)?;
    anyhow::ensure!(source.len() <= 4 * 1024 * 1024, "editorial.input.limit: article exceeds 4 MiB");
    let mut lines = source.lines();
    anyhow::ensure!(lines.next() == Some("---"), "editorial.metadata: Markdown must begin with YAML front matter");
    let mut header = String::new();
    let mut closed = false;
    for line in lines {
        if matches!(line, "---" | "...") { closed = true; break; }
        header.push_str(line);
        header.push('\n');
        anyhow::ensure!(header.len() <= 64 * 1024, "editorial.metadata.limit: header exceeds 64 KiB");
    }
    anyhow::ensure!(closed, "editorial.metadata: missing closing front matter fence");
    let metadata: serde_yaml_ng::Value = serde_yaml_ng::from_str(&header).context("editorial.metadata.invalid_yaml")?;
    anyhow::ensure!(metadata.is_mapping(), "editorial.metadata: expected a mapping");
    match metadata.get("toc") {
        None => Ok(false),
        Some(value) => value.as_bool().context("editorial.metadata.toc: expected true or false"),
    }
}

/// Presentation implementation of the existing HTML/PDF/DOCX/EPUB strategies.
pub struct EditorialStrategy {
    output: OutputType,
}

impl EditorialStrategy {
    pub fn new(output: &OutputType) -> Result<Self> {
        anyhow::ensure!(matches!(output, OutputType::Html | OutputType::Pdf | OutputType::Docx | OutputType::Epub),
            "editorial.output: supported derivatives are html, pdf, docx and epub");
        Ok(Self { output: output.clone() })
    }

    fn args(&self, ctx: &RenderContext, bundle: &Path) -> Result<Vec<String>> {
        anyhow::ensure!(ctx.input_format == InputFormat::Markdown,
            "editorial.input: the article preset consumes canonical Markdown directly");
        anyhow::ensure!(ctx.variables.keys().all(|key|
            key == FONT_REGISTRY_VARIABLE || key == ASSET_ROOT_VARIABLE),
            "editorial.variables: put content metadata in Markdown; only renderflow-font-registry is configurable");
        let target = match self.output {
            OutputType::Html => FontTarget::Html,
            OutputType::Pdf => FontTarget::Pdf,
            OutputType::Docx => FontTarget::Docx,
            _ => FontTarget::Epub,
        };
        let mut variables = HashMap::new();
        variables.insert(FONT_REGISTRY_VARIABLE.to_string(), ctx.variables.get(FONT_REGISTRY_VARIABLE)
            .cloned().unwrap_or_else(|| bundle.join("fonts/registry.json").to_string_lossy().into_owned()));
        let fonts = resolve_from_variables(&variables, target)?.context("editorial.fonts.missing")?;
        let css = bundle.join("fonts.css");
        fs::write(&css, fonts.css())?;
        let mut builder = PandocArgs::new("markdown", ctx.input_path, ctx.output_path).with_standalone();
        let mut extra = vec!["--lua-filter".to_string(), bundle.join("article.lua").to_string_lossy().into_owned(),
            "--fail-if-warnings".to_string()];
        if article_toc(ctx.input_path)? { extra.push("--table-of-contents".to_string()); }
        if let Some(root) = ctx.variables.get(ASSET_ROOT_VARIABLE) {
            extra.extend(["--resource-path".to_string(), root.clone()]);
        }
        match self.output {
            OutputType::Html => {
                builder = builder.with_template(bundle.join("article.html").to_string_lossy())
                    .with_css(bundle.join("article.css").to_string_lossy()).with_css(css.to_string_lossy())
                    .with_embed_resources();
                extra.extend(["--to".to_string(), "html5".to_string()]);
            }
            OutputType::Pdf => {
                let mut latex: HashMap<String, String> = fonts.latex_variables().into_iter().collect();
                latex.insert("renderflow-style-root".to_string(), bundle.join("latex").to_string_lossy().into_owned());
                builder = builder.with_template(bundle.join("article.tex").to_string_lossy())
                    .with_pdf_engine("tectonic").with_variables(&latex);
                extra.extend(["--pdf-engine-opt=--only-cached".to_string(), "--pdf-engine-opt=--untrusted".to_string()]);
            }
            OutputType::Epub => {
                builder = builder.with_css(bundle.join("article.css").to_string_lossy()).with_css(css.to_string_lossy());
                for path in fonts.embeddable_paths() {
                    builder = builder.with_epub_embed_font(path.to_string_lossy());
                }
                extra.extend(["--to".to_string(), "epub3".to_string()]);
            }
            // DOCX keeps semantic content through Pandoc's reference document;
            // this contract does not pretend to embed the HTML/PDF font bundle.
            OutputType::Docx => extra.extend(["--to".to_string(), "docx".to_string()]),
            _ => unreachable!("constructor restricts editorial output"),
        }
        let mut args = builder.build();
        args.extend(extra);
        Ok(args)
    }
}

impl OutputStrategy for EditorialStrategy {
    fn render(&self, ctx: &RenderContext) -> Result<()> {
        if ctx.dry_run { return Ok(()); }
        let workspace = tempfile::tempdir().context("editorial.bundle.stage")?;
        stage_bundle(workspace.path())?;
        // The child runs beside source assets. Resolve staged paths first so
        // relative output/CAS paths cannot change meaning after that CWD switch.
        let input = fs::canonicalize(ctx.input_path)?;
        let output = Path::new(ctx.output_path);
        let parent = output.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let output = fs::canonicalize(parent)?.join(output.file_name().context("editorial.output.name")?);
        let absolute = RenderContext {
            input_path: input.to_str().context("editorial.input.utf8")?,
            output_path: output.to_str().context("editorial.output.utf8")?,
            input_format: ctx.input_format.clone(), variables: ctx.variables, dry_run: false,
        };
        let args = self.args(&absolute, workspace.path())?;
        let root = match ctx.variables.get(ASSET_ROOT_VARIABLE) {
            Some(root) => fs::canonicalize(root)?,
            None => input.parent().context("editorial.input.parent")?.to_path_buf(),
        };
        ProcessExecutor::new().execute_checked(ProcessRequest::direct("pandoc")
            .args(args).working_directory(root).network_policy(ProcessNetworkPolicy::Deny)
            .environment(ProcessEnvironment::filtered_inherit().set("SOURCE_DATE_EPOCH", "315532800"))
            .expect_output(ProcessExpectedOutput::file(&output).require_non_empty().max_bytes(64 * 1024 * 1024)))
            .context("editorial.render: Pandoc >= 3.6.1 is required; PDF also needs Tectonic with a prepared local TeX cache")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Authored and unrun during the implementation sprint.
    #[test]
    fn reader_options_and_local_asset_flags_follow_the_article_header() {
        let root = tempfile::tempdir().unwrap();
        stage_bundle(root.path()).unwrap();
        let input = root.path().join("input.md");
        fs::write(&input, "---\narticle-schema: renderflow.editorial/v1\ntitle: Example\nlang: en\ntoc: true\n---\n## Body\n").unwrap();
        let input = input.to_str().unwrap();
        let variables = HashMap::new();
        let ctx = RenderContext { input_path: input, output_path: "output.html", input_format: InputFormat::Markdown, variables: &variables, dry_run: false };
        for output in [OutputType::Html, OutputType::Pdf, OutputType::Docx, OutputType::Epub] {
            let args = EditorialStrategy::new(&output).unwrap().args(&ctx, root.path()).unwrap();
            assert!(args.iter().any(|arg| arg == "--table-of-contents"));
            assert!(args.iter().any(|arg| arg == "--lua-filter"));
            if output == OutputType::Pdf { assert!(args.iter().any(|arg| arg == "--pdf-engine-opt=--only-cached")); }
            if output == OutputType::Html { assert!(args.iter().any(|arg| arg == "--embed-resources")); }
        }
        fs::write(input, "---\ntitle: Example\ntoc: false\n---\n## Body\n").unwrap();
        let args = EditorialStrategy::new(&OutputType::Html).unwrap().args(&ctx, root.path()).unwrap();
        assert!(!args.iter().any(|arg| arg == "--table-of-contents"));
    }

    #[test]
    fn malformed_or_missing_headers_fail_before_provider_execution() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("input.md");
        for text in ["# Missing header", "---\ntitle: Unclosed", "---\n- list\n---\n", "---\ntoc: \"false\"\n---\n"] {
            fs::write(&input, text).unwrap();
            assert!(article_toc(input.to_str().unwrap()).is_err());
        }
    }
}
