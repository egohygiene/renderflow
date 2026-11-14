use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zip::write::SimpleFileOptions;

use super::slidev::SlidevProvider;
use super::{
    asset_path, parse, read_file, sha256, Presentation, Theme, MAX_ASSET_BYTES, MAX_INPUT_BYTES,
    MAX_TOTAL_ASSET_BYTES, PROFILE_VERSION,
};
use crate::process::ProcessCancellationToken;

const MAX_BUNDLE_BYTES: usize = 256 * 1024 * 1024;
const MAX_BUNDLE_FILES: usize = 4096;
const STYLE: &str = include_str!("../../data/presentation/style.css");
const LAYOUT: &str = "<template><main class=\"rf-slide\"><slot /></main></template>\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationTarget {
    Source,
    Web,
    Pdf,
}

#[derive(Debug, Clone)]
pub struct PresentationRequest {
    pub input: PathBuf,
    pub output_dir: PathBuf,
    pub targets: Vec<PresentationTarget>,
    pub provider: Option<SlidevProvider>,
    pub include_speaker_notes: bool,
    pub allow_unqualified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationPlan {
    pub schema_version: String,
    pub profile: String,
    pub input_sha256: String,
    pub model_sha256: String,
    pub presentation_id: String,
    pub slides: usize,
    pub targets: Vec<PresentationTarget>,
    pub speaker_notes_included: bool,
    pub assets: Vec<Value>,
    pub provider_required: bool,
    pub provider_configured: bool,
    pub runtime_qualified: bool,
    pub limits: Vec<String>,
}

pub(crate) struct Captured {
    pub(crate) input_size: u64,
    model: Presentation,
    files: BTreeMap<String, Vec<u8>>,
    pub(crate) plan: PresentationPlan,
}

pub fn plan(request: &PresentationRequest) -> Result<PresentationPlan> {
    Ok(capture(request)?.plan)
}

pub(crate) fn capture(request: &PresentationRequest) -> Result<Captured> {
    ensure!(
        !request.targets.is_empty(),
        "presentation.targets: select at least one target"
    );
    let unique = request.targets.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        unique.len() == request.targets.len(),
        "presentation.targets: repeated target"
    );
    let bytes = read_file(&request.input, MAX_INPUT_BYTES)?;
    let mut model = parse(&bytes)?;
    let source_root = request
        .input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut files = BTreeMap::new();
    let mut total = 0;
    for slide in &mut model.slides {
        if !request.include_speaker_notes {
            slide.speaker_notes = None;
        }
        for image in &mut slide.images {
            let image_bytes = read_file(&asset_path(source_root, &image.path)?, MAX_ASSET_BYTES)?;
            ensure!(
                sha256(&image_bytes) == image.sha256,
                "presentation.asset.changed: image digest differs"
            );
            let extension = if image.path.to_ascii_lowercase().ends_with(".png") {
                "png"
            } else {
                "jpg"
            };
            ensure!(
                (extension == "png" && image_bytes.starts_with(b"\x89PNG\r\n\x1a\n"))
                    || (extension == "jpg" && image_bytes.starts_with(b"\xff\xd8\xff")),
                "presentation.image.signature"
            );
            let dimensions =
                imagesize::blob_size(&image_bytes).context("presentation.image.dimensions")?;
            ensure!(
                dimensions.width > 0
                    && dimensions.height > 0
                    && dimensions.width.saturating_mul(dimensions.height) <= 100_000_000,
                "presentation.image.geometry"
            );
            let name = format!("media/{}.{}", image.sha256, extension);
            if !files.contains_key(&name) {
                total += image_bytes.len();
                ensure!(
                    total <= MAX_TOTAL_ASSET_BYTES,
                    "presentation.assets.limit: exceeds 64 MiB"
                );
                files.insert(name.clone(), image_bytes);
            }
            image.path = name;
        }
    }
    let model_bytes = serde_json::to_vec_pretty(&model)?;
    let plan = PresentationPlan {
        schema_version: "renderflow.presentation-plan/v1".into(),
        profile: PROFILE_VERSION.into(), input_sha256: sha256(&bytes), model_sha256: sha256(&model_bytes),
        presentation_id: model.id.clone(), slides: model.slides.len(), targets: request.targets.clone(),
        speaker_notes_included: request.include_speaker_notes,
        assets: files.iter().map(|(path, bytes)| json!({"path":path,"sha256":sha256(bytes),"size_bytes":bytes.len()})).collect(),
        provider_required: request.targets.iter().any(|target| *target != PresentationTarget::Source),
        provider_configured: request.provider.is_some(), runtime_qualified: false,
        limits: vec!["Source/review/rights metadata are author declarations; source revision freshness is not independently fetched.".into(),
            "No runtime, visual, accessibility, PDF/UA, browser or platform qualification is claimed.".into(),
            "System font metrics and native PDF metadata may vary; deterministic assembly is distinct from identical native bytes.".into(),
            "Standalone and v2 builds share this frozen profile; a forest projection is not publication approval.".into()],
    };
    files.insert("presentation.json".into(), model_bytes);
    files.insert("slides.md".into(), slidev_markdown(&model)?.into_bytes());
    let mut css = STYLE.to_string();
    if model.theme == Theme::Night {
        css.push_str("\n:root { --rf-paper: #101c28; --rf-ink: #f4f5f7; --rf-link: #8bdaf0; color-scheme: dark; }\n");
    }
    files.insert("style.css".into(), css.into_bytes());
    files.insert("layouts/default.vue".into(), LAYOUT.as_bytes().to_vec());
    // Keep Vite's mutable cache outside the read-only installation link.
    files.insert(
        "vite.config.ts".into(),
        b"export default { cacheDir: './.vite-cache' }\n".to_vec(),
    );
    files.insert("transcript.html".into(), transcript(&model).into_bytes());
    files.insert("README.txt".into(), b"Renderflow presentation source. Requires a separately prepared local @slidev/cli 53.0.0 installation. No install command runs during publication. See Renderflow's presentation guide. Speaker notes are present only when explicitly selected. Review and publication approval remain the author's responsibility.\n".to_vec());
    Ok(Captured {
        input_size: bytes.len() as u64,
        model,
        files,
        plan,
    })
}

/// Direct SDK entry point used by the CLI. Write only to a new output directory;
/// the completion receipt is last. A failed run can leave unaccepted partial files.
pub fn build(
    request: &PresentationRequest,
    cancellation: ProcessCancellationToken,
) -> Result<Value> {
    check_cancelled(&cancellation)?;
    let captured = capture(request)?;
    build_captured(request, &captured, cancellation)
}

/// Consume the captured model/assets; never reload mutable source paths.
pub(crate) fn build_captured(
    request: &PresentationRequest,
    captured: &Captured,
    cancellation: ProcessCancellationToken,
) -> Result<Value> {
    check_cancelled(&cancellation)?;
    let native = captured.plan.provider_required;
    if native {
        ensure!(
            request.allow_unqualified,
            "presentation.unqualified: explicit opt-in required for Slidev"
        );
        ensure!(
            request.provider.is_some(),
            "presentation.provider: selected targets require a pinned local Slidev configuration"
        );
    }
    ensure!(
        fs::symlink_metadata(&request.output_dir)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
        "presentation.output: destination must not exist"
    );
    let parent = request
        .output_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ensure!(parent.is_dir(), "presentation.output: parent must exist");
    let work = tempfile::Builder::new()
        .prefix("renderflow-presentation-")
        .tempdir_in(parent)?;
    let project = work.path().join("project");
    fs::create_dir(&project)?;
    for (name, bytes) in &captured.files {
        write_new(&project.join(name), bytes)?;
    }
    let mut outputs = BTreeMap::new();
    let mut provider_evidence = Value::Null;
    if native {
        provider_evidence = super::slidev::render(
            request.provider.as_ref().context("presentation.provider")?,
            &project,
            &request.targets,
            captured.model.slides.len(),
            &cancellation,
        )?;
        if request.targets.contains(&PresentationTarget::Web) {
            let web = read_tree(&project.join("web"), &cancellation)?;
            let index = web
                .get("index.html")
                .context("presentation.web: missing index.html")?;
            ensure!(
                std::str::from_utf8(index)?
                    .to_ascii_lowercase()
                    .contains("<html"),
                "presentation.web: expected an HTML document"
            );
            let mut package = web;
            // A static readable companion is separate from Slidev's interactive UI.
            package.insert(
                "transcript.html".into(),
                captured.files["transcript.html"].clone(),
            );
            package.insert("style.css".into(), captured.files["style.css"].clone());
            for (name, bytes) in &captured.files {
                if name.starts_with("media/") {
                    package.insert(name.clone(), bytes.clone());
                }
            }
            outputs.insert("presentation-web.zip".to_string(), zip_files(&package)?);
        }
        if request.targets.contains(&PresentationTarget::Pdf) {
            let bytes = read_file(&project.join("slides.pdf"), MAX_BUNDLE_BYTES)?;
            inspect_pdf(&bytes, captured.model.slides.len())?;
            outputs.insert("presentation.pdf".to_string(), bytes);
        }
    }
    if request.targets.contains(&PresentationTarget::Source) {
        outputs.insert(
            "presentation-source.zip".to_string(),
            zip_files(&captured.files)?,
        );
    }
    outputs.insert(
        "transcript.html".to_string(),
        captured.files["transcript.html"].clone(),
    );
    outputs.insert("style.css".to_string(), captured.files["style.css"].clone());
    for (name, bytes) in &captured.files {
        if name.starts_with("media/") {
            outputs.insert(name.clone(), bytes.clone());
        }
    }
    ensure!(
        outputs.values().map(Vec::len).sum::<usize>() <= MAX_BUNDLE_BYTES,
        "presentation.outputs.limit"
    );
    let manifest = json!({
        "schema_version":"renderflow.presentation-bundle/v1", "status":"completed",
        "profile":PROFILE_VERSION,"generator_version":env!("CARGO_PKG_VERSION"),
        "plan":captured.plan,"source":captured.model.source,"as_of":captured.model.as_of,
        "review_state":captured.model.review_state,"review_is_declared":true,"rights":captured.model.rights,
        "theme_sha256":sha256(&captured.files["style.css"]),"layout_sha256":sha256(LAYOUT.as_bytes()),
        "provider":provider_evidence,"runtime_qualified":false,
        "files":outputs.iter().map(|(path, bytes)| json!({"path":path,"sha256":sha256(bytes),"size_bytes":bytes.len()})).collect::<Vec<_>>()
    });
    check_cancelled(&cancellation)?;
    // create_dir is the final no-overwrite admission, including concurrent callers.
    fs::create_dir(&request.output_dir).context("presentation.output.create")?;
    for (name, bytes) in &outputs {
        check_cancelled(&cancellation)?;
        write_new(&request.output_dir.join(name), bytes)?;
    }
    check_cancelled(&cancellation)?;
    write_new(
        &request.output_dir.join("bundle.json"),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

pub(crate) fn check_cancelled(cancellation: &ProcessCancellationToken) -> Result<()> {
    ensure!(!cancellation.is_cancelled(), "presentation.cancelled");
    Ok(())
}

pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("presentation.output.parent")?)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub(crate) fn read_tree(
    root: &Path,
    cancellation: &ProcessCancellationToken,
) -> Result<BTreeMap<String, Vec<u8>>> {
    fn walk(
        root: &Path,
        here: &Path,
        files: &mut BTreeMap<String, Vec<u8>>,
        total: &mut usize,
        entries: &mut usize,
        depth: usize,
        cancellation: &ProcessCancellationToken,
    ) -> Result<()> {
        check_cancelled(cancellation)?;
        ensure!(
            depth <= 24 && fs::symlink_metadata(here)?.is_dir(),
            "presentation.web.tree: invalid directory"
        );
        for entry in fs::read_dir(here)? {
            check_cancelled(cancellation)?;
            *entries += 1;
            ensure!(
                *entries <= MAX_BUNDLE_FILES,
                "presentation.web.entries.limit"
            );
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "presentation.web.tree: symlinks refused"
            );
            if metadata.is_dir() {
                walk(root, &path, files, total, entries, depth + 1, cancellation)?;
            } else {
                ensure!(
                    files.len() < MAX_BUNDLE_FILES,
                    "presentation.web.files.limit"
                );
                let bytes = read_file(&path, MAX_BUNDLE_BYTES.saturating_sub(*total))?;
                *total += bytes.len();
                let name = path
                    .strip_prefix(root)?
                    .to_str()
                    .context("presentation.web.path.utf8")?
                    .replace('\\', "/");
                super::relative_path(&name)?;
                files.insert(name, bytes);
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    let mut total = 0;
    let mut entries = 0;
    walk(
        root,
        root,
        &mut files,
        &mut total,
        &mut entries,
        0,
        cancellation,
    )?;
    Ok(files)
}

fn zip_files(files: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    ensure!(
        files.len() <= MAX_BUNDLE_FILES
            && files.values().map(Vec::len).sum::<usize>() <= MAX_BUNDLE_BYTES,
        "presentation.package.limit"
    );
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (path, bytes) in files {
        super::relative_path(path)?;
        writer.start_file(path, options)?;
        writer.write_all(bytes)?;
    }
    Ok(writer.finish()?.into_inner())
}

fn inspect_pdf(bytes: &[u8], expected_pages: usize) -> Result<()> {
    let document = lopdf::Document::load_mem(bytes).context("presentation.pdf.parse")?;
    let pages = document.get_pages();
    ensure!(
        pages.len() == expected_pages,
        "presentation.pdf.pages: expected one page per slide"
    );
    for page in pages.values() {
        let mut current = *page;
        let mut media_box = None;
        for _ in 0..32 {
            let dictionary = document.get_object(current)?.as_dict()?;
            if let Ok(value) = dictionary.get(b"MediaBox") {
                media_box = Some(value);
                break;
            }
            current = dictionary.get(b"Parent")?.as_reference()?;
        }
        let array = media_box
            .context("presentation.pdf.geometry: missing MediaBox")?
            .as_array()?;
        ensure!(
            array.len() == 4,
            "presentation.pdf.geometry: invalid MediaBox"
        );
        let number = |index: usize| -> Result<f64> {
            match &array[index] {
                lopdf::Object::Integer(value) => Ok(*value as f64),
                lopdf::Object::Real(value) => Ok(*value as f64),
                _ => anyhow::bail!("presentation.pdf.geometry: expected numeric box"),
            }
        };
        let width = number(2)? - number(0)?;
        let height = number(3)? - number(1)?;
        ensure!(
            width.is_finite()
                && height.is_finite()
                && width > 0.0
                && height > 0.0
                && (width / height - 16.0 / 9.0).abs() < 0.02,
            "presentation.pdf.geometry: expected landscape 16:9"
        );
    }
    Ok(())
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .replace('\n', "&#10;")
        .replace('\t', "&#9;")
}

fn slide_html(slide: &super::Slide, heading: u8) -> String {
    let mut html = format!(
        "<section v-pre lang=\"{}\" aria-labelledby=\"{}\">\n",
        "en",
        escape(&slide.id)
    );
    if let Some(section) = &slide.section {
        html.push_str(&format!(
            "<p class=\"rf-section\">{}</p>\n",
            escape(section)
        ));
    }
    html.push_str(&format!(
        "<h{heading} id=\"{}\">{}</h{heading}>\n",
        escape(&slide.id),
        escape(&slide.title)
    ));
    for paragraph in &slide.paragraphs {
        html.push_str(&format!("<p>{}</p>\n", escape(paragraph)));
    }
    if !slide.bullets.is_empty() {
        html.push_str("<ul>\n");
        for bullet in &slide.bullets {
            html.push_str(&format!("<li>{}</li>\n", escape(bullet)));
        }
        html.push_str("</ul>\n");
    }
    for image in &slide.images {
        html.push_str(&format!(
            "<figure><img src=\"{}\" alt=\"{}\" />",
            escape(&image.path),
            escape(&image.alt)
        ));
        if let Some(caption) = &image.caption {
            html.push_str(&format!("<figcaption>{}</figcaption>", escape(caption)));
        }
        html.push_str("</figure>\n");
    }
    if !slide.citations.is_empty() {
        html.push_str("<ul class=\"rf-citations\" aria-label=\"Sources\">\n");
        for citation in &slide.citations {
            html.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>\n",
                escape(&citation.url),
                escape(&citation.label)
            ));
        }
        html.push_str("</ul>\n");
    }
    html.push_str("</section>\n");
    html
}

fn slidev_markdown(model: &Presentation) -> Result<String> {
    let head = json!({"theme":"none","title":model.title,"info":"", "layout":"default",
        "aspectRatio":16.0/9.0,"canvasWidth":1280,"transition":"none", "colorSchema":if model.theme == Theme::Night { "dark" } else { "light" },
        "presenter":false,"record":false,"download":false,"browserExporter":false,"monaco":false,
        "mdc":false,"routerMode":"hash","fonts":{"provider":"none","webfonts":[],"local":["sans-serif"],"sans":"sans-serif"},
        "htmlAttrs":{"lang":model.language},"drawings":{"enabled":false}});
    let mut out = format!("---\n{}---\n\n", serde_yaml_ng::to_string(&head)?);
    for (index, slide) in model.slides.iter().enumerate() {
        if index > 0 {
            out.push_str(&format!(
                "\n---\n{}---\n\n",
                serde_yaml_ng::to_string(&json!({"layout":"default","title":slide.title}))?
            ));
        }
        out.push_str(&slide_html(slide, 1).replace(
            "lang=\"en\"",
            &format!("lang=\"{}\"", escape(&model.language)),
        ));
        // v-pre prevents user text from being interpreted as Vue expressions.
        // Speaker notes use escaped text, not Slidev's executable Markdown input.
        if let Some(notes) = &slide.speaker_notes {
            out.push_str(&format!(
                "\n<!--\n{}\n-->\n",
                escape(notes).replace("--", "&#45;&#45;")
            ));
        }
    }
    Ok(out)
}

fn transcript(model: &Presentation) -> String {
    let theme = match model.theme {
        Theme::Paper => "paper",
        Theme::Night => "night",
    };
    let mut out = format!("<!doctype html>\n<html lang=\"{}\" data-theme=\"{theme}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><link rel=\"stylesheet\" href=\"style.css\"></head><body><main><h1>{}</h1>\n", escape(&model.language), escape(&model.title), escape(&model.title));
    out.push_str(&format!(
        "<p>As of {}. Source revision: {}.</p>\n",
        escape(&model.as_of),
        escape(&model.source.revision)
    ));
    for slide in &model.slides {
        out.push_str(&slide_html(slide, 2).replace(" v-pre", "").replace(
            "lang=\"en\"",
            &format!("lang=\"{}\"", escape(&model.language)),
        ));
    }
    out.push_str(&format!("<footer><p>{}</p><p>Review state: {:?} (author declaration).</p></footer></main></body></html>\n", escape(&model.rights), model.review_state));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(root: &Path) -> PresentationRequest {
        let source =
            include_bytes!("../../../../examples/presentation/synthetic.presentation.json");
        fs::write(root.join("deck.json"), source).unwrap();
        PresentationRequest {
            input: root.join("deck.json"),
            output_dir: root.join("out"),
            targets: vec![PresentationTarget::Source],
            provider: None,
            include_speaker_notes: false,
            allow_unqualified: false,
        }
    }

    #[test]
    fn source_package_is_deterministic_and_omits_notes_by_default() {
        let root = tempfile::tempdir().unwrap();
        let request = request(root.path());
        let left = capture(&request).unwrap();
        let right = capture(&request).unwrap();
        assert_eq!(
            zip_files(&left.files).unwrap(),
            zip_files(&right.files).unwrap()
        );
        assert!(!String::from_utf8(left.files["presentation.json"].clone())
            .unwrap()
            .contains("Private rehearsal"));
        assert!(!String::from_utf8(left.files["slides.md"].clone())
            .unwrap()
            .contains("Private rehearsal"));
    }

    #[test]
    fn literal_text_cannot_insert_vue_or_html_and_language_is_retained() {
        let root = tempfile::tempdir().unwrap();
        let request = request(root.path());
        let mut model = capture(&request).unwrap().model;
        model.language = "fr".into();
        model.slides[0].paragraphs =
            vec!["<script>alert(1)</script> {{ process.env }}\n---\ntheme: attacker".into()];
        let markdown = slidev_markdown(&model).unwrap();
        assert!(markdown.contains("v-pre lang=\"fr\""));
        assert!(!markdown.contains("<script>"));
        assert!(markdown.contains("&lt;script&gt;"));
        assert!(markdown.contains("&#10;---&#10;theme: attacker"));
    }

    #[test]
    fn source_build_refuses_overwrite_and_cancelled_publication() {
        let root = tempfile::tempdir().unwrap();
        let request = request(root.path());
        let token = ProcessCancellationToken::new();
        token.cancel();
        assert!(build(&request, token).is_err());
        assert!(!request.output_dir.exists());
        build(&request, ProcessCancellationToken::new()).unwrap();
        assert!(build(&request, ProcessCancellationToken::new()).is_err());
    }

    #[test]
    fn native_targets_require_explicit_provider_and_unqualified_opt_in() {
        let root = tempfile::tempdir().unwrap();
        let mut request = request(root.path());
        request.targets = vec![PresentationTarget::Web];
        assert!(plan(&request).unwrap().provider_required);
        assert!(build(&request, ProcessCancellationToken::new()).is_err());
        assert!(!request.output_dir.exists());
    }
}
