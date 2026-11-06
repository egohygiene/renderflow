//! Original #413 fixture recipe and cases in the existing golden harness.
//! Authored and UNRUN; this is fixture code, not a Renderflow publication API.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use anyhow::{ensure, Context, Result};
use renderflow::composition::{CompositionBranchState, CompositionOutput, CompositionRunState, COMPOSITION_PROFILE};
use renderflow::evidence::verification::{self, VerificationLimits};
use renderflow::planning::{execute_composition, resolve_composition, CompositionPlanningRequest};
use renderflow::process::ProcessCancellationToken;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{Corpus, ScenarioEvidence, ScenarioResults};

#[path = "golden_comic_publications.rs"]
mod publications;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin { pub path: String, pub sha256: String }

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackSelection { pub id: String, pub case_id: String }

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentPack {
    pub schema_version: String,
    pub id: String,
    pub version: String,
    pub document_type: String,
    pub source_fixture: String,
    pub generator: Pin,
    pub recipe_dependencies: Vec<Pin>,
    pub toolchain: Pin,
    pub profile: String,
    pub assets: Vec<Pin>,
    pub rights: Pin,
    pub license: String,
    pub provenance: String,
    pub required_capabilities: Vec<String>,
    pub optional_capabilities: Vec<String>,
    pub cases: Vec<PackCase>,
    pub unavailable_mappings: Vec<UnavailableMapping>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableMapping { pub role: String, pub format: String, pub reason: String }

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackCase {
    pub id: String,
    pub tier: String,
    pub exercise: String,
    pub parameters: Parameters,
    pub limits: PackLimits,
    pub outputs: Vec<OutputExpectation>,
    pub negative_cases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub seed: u64, pub interior_pages: usize, pub cover_pages: usize,
    pub width_mm: f64, pub height_mm: f64, pub dpi: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackLimits {
    pub max_files: usize, pub max_source_bytes: u64, pub max_output_bytes: u64,
    pub cooperative_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputExpectation {
    pub role: String, pub format: String, pub media_type: String,
    pub schema: Option<String>, pub validator: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Story {
    title: String, subtitle: String, back_cover: String,
    characters: Vec<String>, beats: Vec<[String; 2]>,
}

#[derive(Debug, Serialize)]
pub struct PackEvidence {
    pub pack_id: String, pub case_id: String, pub status: String,
    pub exercise: String, pub source_sha256: String, pub generator_sha256: String,
    pub toolchain_sha256: String, pub asset_sha256: BTreeMap<String,String>,
    pub recipe_dependency_sha256: BTreeMap<String,String>,
    pub parameters: Parameters, pub validator_ids: Vec<String>,
    pub profile: String, pub interior_page_ids: Vec<String>, pub cover_page_ids: Vec<String>,
    pub observed_pages: usize, pub observed_publications: usize,
    pub output_sha256: BTreeMap<String, String>, pub source_bytes: u64, pub output_bytes: u64,
    pub negative_cases: Vec<String>, pub unavailable_mappings: Vec<Value>,
    pub publication_outputs: Vec<publications::AggregateEvidence>,
    pub rgb_preparation: BTreeMap<String,renderflow::publication::raster::RgbPreparationEvidence>,
    pub runtime_qualified: bool, pub creative_approval_inferred: bool,
}

struct Page { id: String, role: String, manifest: PathBuf, text: [String; 2], alt: String }
struct Generated { source: PathBuf, pages: Vec<Page>, model: Value }

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn repository() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..") }
fn token() -> ProcessCancellationToken { ProcessCancellationToken::new() }

fn pin_bytes(pin: &Pin) -> Result<Vec<u8>> {
    let relative = Path::new(&pin.path);
    ensure!(!pin.path.is_empty() && !pin.path.contains(['\\', ':', '\0'])
        && relative.components().all(|part| matches!(part, Component::Normal(_))), "unsafe pack pin");
    let mut path = repository();
    for part in relative.components() {
        path.push(part.as_os_str());
        ensure!(!fs::symlink_metadata(&path)?.file_type().is_symlink(), "pack pin traverses a symlink");
    }
    let meta = fs::symlink_metadata(&path)?;
    ensure!(meta.is_file() && meta.len() <= 2 * 1024 * 1024, "pack pin must be a small regular file");
    let bytes = fs::read(path)?;
    ensure!(bytes.len() <= 2 * 1024 * 1024 && digest(&bytes) == pin.sha256, "pack pin digest mismatch");
    Ok(bytes)
}

pub fn check(corpus: &Corpus) -> Result<()> {
    let mut ids = BTreeSet::new();
    for pack in &corpus.packs {
        let stable_id = |id:&str| !id.is_empty() && id.len() <= 128
            && id.bytes().all(|byte|byte.is_ascii_alphanumeric() || matches!(byte,b'.'|b'-'|b'_'));
        ensure!(stable_id(&pack.id) && ids.insert(&pack.id) && pack.schema_version == "renderflow.document-fixture-pack/v1"
            && pack.document_type == "comic" && pack.version == "1.1.0"
            && pack.license == "CC0-1.0" && !pack.provenance.is_empty()
            && pack.profile == COMPOSITION_PROFILE, "unsupported or incomplete fixture pack");
        ensure!(corpus.fixtures.iter().any(|fixture| fixture.id == pack.source_fixture), "pack story fixture missing");
        pin_bytes(&pack.generator)?; pin_bytes(&pack.toolchain)?; pin_bytes(&pack.rights)?;
        let required_recipe = BTreeSet::from([
            "crates/renderflow-core/tests/common/golden_comic_publications.rs",
            "crates/renderflow-core/src/publication/raster.rs",
        ]);
        ensure!(pack.recipe_dependencies.iter().map(|pin|pin.path.as_str()).collect::<BTreeSet<_>>() == required_recipe,
            "comic publication recipe dependency pins differ");
        for pin in &pack.recipe_dependencies { pin_bytes(pin)?; }
        ensure!(pack.generator.path == "crates/renderflow-core/tests/common/golden_comic.rs", "unknown fixture generator");
        ensure!(pack.assets.len() == 3, "comic needs font, license and registry pins");
        for asset in &pack.assets { pin_bytes(asset)?; }
        ensure!(!pack.required_capabilities.is_empty() && !pack.optional_capabilities.is_empty(), "capability declarations missing");
        let mut case_ids = BTreeSet::new();
        for case in &pack.cases {
            let p = &case.parameters;
            ensure!(stable_id(&case.id) && case_ids.insert(&case.id) && matches!(case.tier.as_str(), "fast" | "maximal")
                && matches!(case.exercise.as_str(), "execute" | "plan") && p.seed == 0
                && (2..=44).contains(&p.interior_pages) && p.interior_pages.is_multiple_of(2)
                && p.cover_pages == 2 && p.width_mm == 100.0 && p.height_mm == 140.0 && p.dpi == 127,
                "comic recipe parameters exceed the selected finite subset");
            ensure!((1..=256).contains(&case.limits.max_files)
                && (1..=8 * 1024 * 1024).contains(&case.limits.max_source_bytes)
                && case.limits.max_output_bytes <= 128 * 1024 * 1024
                && (1..=600).contains(&case.limits.cooperative_seconds), "invalid pack budget");
            ensure!(case.outputs.len() == 5, "comic output expectations incomplete");
            let mut formats = BTreeSet::new();
            for output in &case.outputs {
                let format: CompositionOutput = output.format.parse()?;
                ensure!(format.supported() && formats.insert(output.format.as_str())
                    && output.role == "page" && output.media_type == format.media_type()
                    && !output.validator.is_empty(), "unsupported pack output expectation");
                let validator = match format {
                    CompositionOutput::Svg => "fixture.comic.svg-structure/v1",
                    CompositionOutput::Html => "fixture.comic.html-structure/v1",
                    CompositionOutput::TranscriptJson => "fixture.comic.transcript/v1",
                    CompositionOutput::Png => "fixture.comic.png-geometry/v1",
                    CompositionOutput::Pdf => "fixture.comic.pdf-geometry-pixels/v1",
                    _ => anyhow::bail!("no selected comic validator for format"),
                };
                ensure!(output.validator == validator, "unknown comic assertion identity");
                ensure!(output.schema.as_deref() == if format == CompositionOutput::TranscriptJson {
                    Some("renderflow.page-transcript/v1") } else { None }, "unexpected output schema");
            }
            ensure!(formats == BTreeSet::from(["svg","html","transcript_json","png","pdf"]), "comic expected format set differs");
        }
        for mapping in &pack.unavailable_mappings {
            ensure!(!mapping.role.is_empty() && !mapping.format.is_empty() && !mapping.reason.is_empty(), "incomplete unavailable mapping");
        }
    }
    let mut selected = BTreeSet::new();
    for scenario in &corpus.scenarios {
        if let Some(selection) = &scenario.pack {
            let pack = corpus.packs.iter().find(|pack| pack.id == selection.id).context("unknown scenario pack")?;
            let case = pack.cases.iter().find(|case| case.id == selection.case_id).context("unknown pack case")?;
            ensure!(selected.insert((&selection.id, &selection.case_id))
                && scenario.tier == case.tier && scenario.fixtures == vec![pack.source_fixture.clone()]
                && scenario.assertion_scope == if case.exercise == "execute" { "native_output" } else { "inventory" },
                "scenario and pack selection disagree");
        }
    }
    ensure!(selected.len() == corpus.packs.iter().map(|pack| pack.cases.len()).sum::<usize>(), "every pack case needs exactly one scenario");
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?; Ok(())
}
fn write_json(path: &Path, value: &Value) -> Result<()> { write(path, &serde_json::to_vec_pretty(value)?) }
fn reference(path: &str, bytes: &[u8]) -> Value { json!({"path": path, "sha256": digest(bytes)}) }
fn rect(x: f64, y: f64, width: f64, height: f64) -> Value { json!({"x":x,"y":y,"width":width,"height":height}) }

fn artwork(index: usize) -> String {
    // The circle courier and square mapmaker are original geometric characters.
    // No text, resources, font, CSS, AI or external SVG reference is involved.
    let dot = 35 + index % 7 * 5;
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"240\" height=\"300\" viewBox=\"0 0 240 300\"><rect width=\"240\" height=\"300\" fill=\"#edf5fb\"/><rect x=\"5\" y=\"5\" width=\"230\" height=\"140\" fill=\"#c3ddec\"/><rect x=\"5\" y=\"155\" width=\"230\" height=\"140\" fill=\"#dbe9d6\"/><circle cx=\"55\" cy=\"70\" r=\"22\" fill=\"#df8b58\"/><rect x=\"150\" y=\"48\" width=\"42\" height=\"44\" fill=\"#687da8\"/><circle cx=\"{dot}\" cy=\"118\" r=\"5\" fill=\"#4274ab\"/><circle cx=\"95\" cy=\"225\" r=\"22\" fill=\"#df8b58\"/><rect x=\"165\" y=\"202\" width=\"42\" height=\"44\" fill=\"#687da8\"/><circle cx=\"138\" cy=\"205\" r=\"10\" fill=\"#f3cf5b\"/></svg>\n")
}

fn materialize(pack: &DocumentPack, case: &PackCase, story_bytes: &[u8], root: &Path) -> Result<Generated> {
    let story: Story = serde_json::from_slice(story_bytes)?;
    ensure!(!story.title.is_empty() && !story.subtitle.is_empty() && !story.back_cover.is_empty()
        && story.characters.len() == 2 && story.beats.len() == 2
        && story.beats.iter().flatten().all(|line| !line.is_empty() && line.len() <= 256), "invalid bounded original story");
    let source = root.join("source"); fs::create_dir(&source)?;
    write(&source.join("story.json"),story_bytes)?;
    write(&source.join("LICENSE"),&pin_bytes(&pack.rights)?)?;
    fs::create_dir(source.join("fonts"))?;
    for asset in &pack.assets {
        let name = Path::new(&asset.path).file_name().context("asset name missing")?;
        write(&source.join("fonts").join(name), &pin_bytes(asset)?)?;
    }
    let registry = fs::read(source.join("fonts/registry.json"))?;
    let mut pages = Vec::new();
    let ids = std::iter::once(("cover-front".to_string(), "cover".to_string()))
        .chain((1..=case.parameters.interior_pages).map(|n| (format!("interior-{n:03}"), "interior".to_string())))
        .chain(std::iter::once(("cover-back".to_string(), "cover".to_string())));
    for (index, (id, role)) in ids.enumerate() {
        let text = if id == "cover-front" { [story.title.clone(), story.subtitle.clone()] }
            else if id == "cover-back" { [story.back_cover.clone(), "An original fixture.\nThe end.".to_string()] }
            else { story.beats[(index - 1) % story.beats.len()].clone() };
        let alt = format!("Original geometric comic {id}: Pip is an orange circle, Vale is a blue square; two panels show a route and a yellow lantern.");
        let art_name = format!("{id}.svg"); let art = artwork(index).into_bytes(); write(&source.join(&art_name), &art)?;
        let content_name = format!("{id}.content.json");
        let content = json!({"schema":"renderflow.page-content/v1","composition_id":id,"locale":"en","revision":"r1","source_revision":"r1","reviewed":true,"direction":"ltr","writing_mode":"horizontal_tb","content":{"panel.one":text[0],"panel.two":text[1]}});
        let content_bytes = serde_json::to_vec_pretty(&content)?; write(&source.join(&content_name), &content_bytes)?;
        let mut layers = vec![json!({"id":"plate","bounds":rect(0.06,0.06,0.88,0.88),"content":{"kind":"vector","asset_id":"plate"}})];
        for (number, y) in [("one",0.34),("two",0.78)] {
            layers.push(json!({"id":format!("panel.{number}.box"),"bounds":rect(0.08,y,0.84,0.14),"content":{"kind":"shape","shape":"rect","fill":"#ffffff","stroke":"#233844","stroke_width_pt":0.5}}));
            layers.push(json!({"id":format!("panel.{number}.text"),"bounds":rect(0.10,y+0.01,0.80,0.12),"content":{"kind":"text","content_id":format!("panel.{number}"),"typography":{"role":"caption","font_size_pt":9,"min_font_size_pt":9,"line_height":1.2,"padding_pt":2,"alignment":"start","color":"#233844"}}}));
        }
        let manifest = json!({"schema":"renderflow.page-composition/v1","composition_id":id,"revision":"r1",
            "page":{"width_mm":case.parameters.width_mm,"height_mm":case.parameters.height_mm,"trim":rect(0.02,0.02,0.96,0.96),"bleed":rect(0.0,0.0,1.0,1.0),"safe":rect(0.04,0.04,0.92,0.92),"gutter_mm":0},
            "source_locale":"en","default_locale":"en","locales":{"en":reference(&content_name,&content_bytes)},
            "font_registry":reference("fonts/registry.json",&registry),"assets":[{"id":"plate","file":reference(&art_name,&art),"format":"svg","alt":alt}],
            "layers":layers,"reading_order":["panel.one.text","panel.two.text"],
            "provenance":{"kind":"textless_original","originals":[reference(&art_name,&art)],"separation_reviewed":true,"requires_human_review":true,
                "notes":"Original CC0 geometric fixture art/text; reviewed flags are synthetic test declarations, never authenticated human or publication approval."},"locale_overrides":{}});
        let path = source.join(format!("{id}.composition.json")); write_json(&path,&manifest)?;
        let spec = json!({"schema":"renderflow/v2","sources":[{"id":"page","kind":"composition","path":format!("{id}.composition.json")}],
            "targets":{"exact":case.outputs.iter().map(|output|json!({"format":output.format})).collect::<Vec<_>>()},
            "execution":{"composition":{"raster":{"dpi":case.parameters.dpi,"background":"#ffffff"},"pdf":{"dpi":case.parameters.dpi,"background":"#ffffff","binding_side":"none","color_policy":"assumed_srgb_no_profile_conversion","conformance":"none"}}},
            "output":{"bundle_root":format!("{id}.bundle")}});
        write_json(&source.join(format!("{id}.renderflow.json")),&spec)?;
        pages.push(Page { id, role, manifest: path, text, alt });
    }
    let model = json!({"pack_id":pack.id,"case_id":case.id,"profile":pack.profile,
        "geometry_mm":{"width":case.parameters.width_mm,"height":case.parameters.height_mm},
        "alt":pages.iter().map(|page|(page.id.clone(),Value::String(page.alt.clone()))).collect::<serde_json::Map<_,_>>(),
        "groups":[{"role":"interior","desired_format":"pdf","page_ids":pages.iter().filter(|page|page.role=="interior").map(|page|&page.id).collect::<Vec<_>>()},
                  {"role":"cover","desired_format":"pdf","page_ids":["cover-front","cover-back"]}],
        "actual_mapping":"per_page_candidates_then_explicit_device_rgb_publication","unavailable_mappings":pack.unavailable_mappings.iter().map(|mapping|json!({"role":mapping.role,"format":mapping.format,"reason":mapping.reason})).collect::<Vec<_>>(),
        "publication_approved":false});
    write_json(&source.join("publication-expectations.json"),&model)?;
    Ok(Generated { source, pages, model })
}

fn snapshot(root: &Path, max_files: usize, max_bytes: u64) -> Result<(BTreeMap<String, String>, u64)> {
    fn walk(root: &Path, here: &Path, files: &mut BTreeMap<String, String>, total: &mut u64,
        max_files: usize, max_bytes: u64) -> Result<()> {
        for entry in fs::read_dir(here)? {
            let entry = entry?; let path = entry.path(); let meta = fs::symlink_metadata(&path)?;
            ensure!(!meta.file_type().is_symlink(), "fixture snapshot refuses symlinks");
            if meta.is_dir() { walk(root,&path,files,total,max_files,max_bytes)?; }
            else {
                *total = total.checked_add(meta.len()).context("pack byte count overflow")?;
                ensure!(meta.is_file() && *total <= max_bytes && files.len() < max_files, "pack file/byte budget");
                let bytes = fs::read(&path)?;
                ensure!(bytes.len() as u64 == meta.len(), "pack file changed during observation");
                files.insert(path.strip_prefix(root)?.to_string_lossy().into_owned(),digest(&bytes));
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new(); let mut total = 0;
    walk(root,root,&mut files,&mut total,max_files,max_bytes)?; Ok((files,total))
}

fn validate_model(generated: &Generated, case: &PackCase, model: &Value) -> Result<()> {
    let interior = model["groups"][0]["page_ids"].as_array().context("fixture.pack.page_count")?;
    ensure!(interior.len() == case.parameters.interior_pages, "fixture.pack.page_count");
    let expected = generated.pages.iter().filter(|page| page.role == "interior").map(|page| &page.id).collect::<Vec<_>>();
    ensure!(model["groups"][0]["page_ids"] == json!(expected)
        && model["groups"][1]["page_ids"] == json!(["cover-front","cover-back"]), "fixture.pack.page_order");
    ensure!(model["geometry_mm"] == json!({"width":case.parameters.width_mm,"height":case.parameters.height_mm}), "fixture.pack.geometry");
    ensure!(generated.pages.iter().all(|page| model["alt"][&page.id].as_str().is_some_and(|alt| !alt.trim().is_empty())), "fixture.pack.accessibility");
    ensure!(model["publication_approved"] == false, "fixture.pack.approval"); Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?; let path = entry.path(); let target = destination.join(entry.file_name());
        let meta = fs::symlink_metadata(&path)?;
        ensure!(!meta.file_type().is_symlink(), "fixture copy refuses symlinks");
        if meta.is_dir() { copy_tree(&path,&target)?; }
        else { ensure!(meta.is_file(), "fixture copy needs regular files"); write(&target,&fs::read(path)?)?; }
    }
    Ok(())
}

fn request(path: &Path, case: &PackCase) -> Result<CompositionPlanningRequest> {
    let mut request = CompositionPlanningRequest::from_path(path);
    request.request.targets = case.outputs.iter().map(|output|output.format.parse()).collect::<std::result::Result<Vec<_>,_>>()?;
    request.request.raster.dpi = case.parameters.dpi; request.request.raster.background = "#ffffff".into();
    request.request.pdf.dpi = case.parameters.dpi;
    Ok(request)
}

fn inspect_page(root: &Path, page: &Page, case: &PackCase) -> Result<BTreeMap<String,String>> {
    let path = root.join("outputs/en/page");
    let transcript: Value = serde_json::from_slice(&fs::read(path.with_extension("transcript.json"))?)?;
    ensure!(transcript["schema"] == "renderflow.page-transcript/v1"
        && transcript["composition_id"] == page.id && transcript["locale"] == "en"
        && transcript["requires_human_review"] == true, "comic transcript identity/review differs");
    let entries = transcript["entries"].as_array().context("comic transcript entries")?;
    ensure!(entries.len() == 2 && entries[0]["content_id"] == "panel.one"
        && entries[1]["content_id"] == "panel.two" && entries[0]["text"] == page.text[0]
        && entries[1]["text"] == page.text[1] && transcript["artwork"][0]["alt"] == page.alt,
        "comic reading order/text/artwork relationship differs");
    let html = fs::read_to_string(path.with_extension("html"))?;
    let svg = fs::read_to_string(path.with_extension("svg"))?;
    ensure!(html.contains("Content-Security-Policy") && html.contains("data-content-id=\"panel.one\"")
        && html.contains("data-content-id=\"panel.two\"") && svg.contains("<path")
        && svg.contains("data-layer-id=\"panel.one.text\""), "comic web output lacks expected structure");
    let png_bytes = fs::read(path.with_extension("png"))?;
    let mut limits = png::Limits::default(); limits.bytes = 4 * 1024 * 1024;
    let mut reader = png::Decoder::new_with_limits(BufReader::new(Cursor::new(&png_bytes)),limits).read_info()?;
    let mut pixels = vec![0;reader.output_buffer_size().context("PNG frame size missing")?];
    let frame = reader.next_frame(&mut pixels)?; pixels.truncate(frame.buffer_size());
    let width = (case.parameters.width_mm * f64::from(case.parameters.dpi) / 25.4).ceil() as u32;
    let height = (case.parameters.height_mm * f64::from(case.parameters.dpi) / 25.4).ceil() as u32;
    ensure!(frame.width == width && frame.height == height && frame.color_type == png::ColorType::Rgba
        && frame.bit_depth == png::BitDepth::Eight && pixels.chunks_exact(4).all(|pixel|pixel[3] == 255),
        "comic PNG geometry/opaque RGBA contract differs");
    let pdf_bytes = fs::read(path.with_extension("pdf"))?;
    let document = lopdf::Document::load_mem_with_options(&pdf_bytes,lopdf::LoadOptions {
        strict:true,max_decompressed_size:Some(2 * 1024 * 1024),..Default::default()
    })?;
    let pages = document.get_pages(); ensure!(pages.len() == 1, "comic native PDF must be one page");
    let dict = document.get_dictionary(*pages.values().next().context("PDF page missing")?)?;
    let boxes = dict.get(b"MediaBox")?.as_array()?;
    let number = |object:&lopdf::Object| -> Result<f64> { Ok(match object {
        lopdf::Object::Integer(value)=>*value as f64,lopdf::Object::Real(value)=>f64::from(*value),
        _=>anyhow::bail!("PDF box is not numeric"),
    }) };
    ensure!(boxes.len() == 4 && number(&boxes[0])? == 0.0 && number(&boxes[1])? == 0.0
        && (number(&boxes[2])? - case.parameters.width_mm*72.0/25.4).abs() < 0.02
        && (number(&boxes[3])? - case.parameters.height_mm*72.0/25.4).abs() < 0.02, "comic PDF page geometry differs");
    let images = document.objects.values().filter_map(|object|object.as_stream().ok())
        .filter(|stream|stream.dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok() == Some(b"Image".as_slice()))
        .collect::<Vec<_>>();
    ensure!(images.len() == 1, "comic PDF image membership differs");
    let image = images[0];
    ensure!(image.dict.get(b"Width")?.as_i64()? == i64::from(width)
        && image.dict.get(b"Height")?.as_i64()? == i64::from(height)
        && image.dict.get(b"ColorSpace")?.as_name()? == b"DeviceRGB"
        && image.dict.get(b"Filter").is_err(), "comic PDF image contract differs");
    let rgb = pixels.chunks_exact(4).flat_map(|pixel|pixel[..3].iter().copied()).collect::<Vec<_>>();
    ensure!(image.content == rgb, "comic PDF pixels differ from same-settings PNG");
    case.outputs.iter().map(|output| {
        let format: CompositionOutput = output.format.parse()?;
        let bytes = fs::read(path.with_extension(format.extension()))?;
        Ok((format.name().to_string(),digest(&bytes)))
    }).collect()
}

fn negative_sources(generated: &Generated, case: &PackCase, kind: &str) -> Result<()> {
    let temporary = tempfile::tempdir()?; let source = temporary.path().join("source"); copy_tree(&generated.source,&source)?;
    let name = generated.pages[0].manifest.file_name().context("page name missing")?;
    let manifest_path = source.join(name); let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let content_path = source.join("cover-front.content.json");
    let expected = match kind {
        "digest_mismatch" => { fs::write(source.join("cover-front.svg"),b"changed")?; "pinned sha256" }
        "missing_asset" => { fs::remove_file(source.join("cover-front.svg"))?; "cannot inspect composition input" }
        "missing_font" => { fs::remove_file(source.join("fonts/DejaVuSans.ttf"))?; "cannot inspect composition input" }
        "unsupported_schema" => { manifest["schema"] = json!("renderflow.page-composition/v999"); "unsupported page composition schema" }
        "stale_content" | "unreviewed_content" => {
            let mut content: Value = serde_json::from_slice(&fs::read(&content_path)?)?;
            if kind == "stale_content" { content["source_revision"] = json!("old"); }
            else { content["reviewed"] = json!(false); }
            let bytes = serde_json::to_vec_pretty(&content)?; fs::write(&content_path,&bytes)?;
            manifest["locales"]["en"]["sha256"] = json!(digest(&bytes));
            fs::write(&manifest_path,serde_json::to_vec_pretty(&manifest)?)?;
            let resolved = resolve_composition(request(&manifest_path,case)?)?;
            let code = if kind == "stale_content" { "composition.content.stale" } else { "composition.content.unreviewed" };
            ensure!(resolved.native_plan().diagnostics.iter().chain(resolved.native_plan().branches.iter().flat_map(|branch|&branch.diagnostics))
                .any(|diagnostic|diagnostic.code == code), "expected typed content diagnostic missing");
            ensure!(resolved.native_plan().branches.iter().all(|branch|branch.state == CompositionBranchState::Blocked), "unreviewed/stale page must block");
            return Ok(());
        }
        _ => anyhow::bail!("unknown source negative case"),
    };
    fs::write(&manifest_path,serde_json::to_vec_pretty(&manifest)?)?;
    match resolve_composition(request(&manifest_path,case)?) {
        Ok(_) => anyhow::bail!("source negative case {kind} was admitted"),
        Err(error) => ensure!(format!("{error:#}").contains(expected), "wrong refusal boundary for {kind}: {error:#}"),
    }
    ensure!(!source.join("cover-front.bundle").exists(), "source refusal published an output"); Ok(())
}

pub fn run(corpus: &Corpus, paths: &BTreeMap<String,PathBuf>, root: &Path,
    requested_rank: usize, results: &mut ScenarioResults) -> Result<Vec<PackEvidence>> {
    let mut evidence = Vec::new();
    let retained = std::env::var_os("RENDERFLOW_GOLDEN_PACK_SOURCES").map(PathBuf::from);
    if let Some(path) = &retained { fs::create_dir(path).context("retained pack source root must be new and its parent must exist")?; }
    for scenario in &corpus.scenarios {
        let Some(selection) = &scenario.pack else { continue; };
        let pack = corpus.packs.iter().find(|pack|pack.id == selection.id).context("pack missing")?;
        let case = pack.cases.iter().find(|case|case.id == selection.case_id).context("case missing")?;
        if case.tier == "maximal" && requested_rank < 2 {
            results.insert((scenario.id.clone(),ScenarioEvidence { id:scenario.id.clone(),status:"excluded".into(),
                assertions:vec!["larger pack source/planning tier was not requested; no generation or output success inferred".into()],providers:Vec::new(),reason:Some("maximal tier required".into()) }));
            continue;
        }
        let started = Instant::now(); let case_root = root.join(format!("{}-{}",pack.id,case.id)); fs::create_dir(&case_root)?;
        let story_bytes = fs::read(&paths[&pack.source_fixture])?;
        let generated = materialize(pack,case,&story_bytes,&case_root)?; validate_model(&generated,case,&generated.model)?;
        let before = snapshot(&generated.source,case.limits.max_files,case.limits.max_source_bytes)?;
        let output_root = case_root.join("outputs"); fs::create_dir(&output_root)?;
        let cache = case_root.join("cache"); let mut hashes = BTreeMap::new(); let mut publications = 0;
        let mut captures = BTreeMap::new();
        let mut first = None;
        for page in &generated.pages {
            ensure!(started.elapsed().as_secs() < case.limits.cooperative_seconds, "pack cooperative budget exceeded");
            let resolved = resolve_composition(request(&page.manifest,case)?)?;
            ensure!(resolved.native_plan().branches.len() == case.outputs.len()
                && resolved.native_plan().branches.iter().all(|branch|branch.state == CompositionBranchState::Planned), "comic page cannot plan requested outputs");
            if page.id == "cover-front" {
                let spec = generated.source.join(format!("{}.renderflow.json",page.id));
                let build = renderflow::planning::build::resolve(renderflow::planning::PlanningRequest::from_path(&spec))?;
                ensure!(build.is_composition() && build.plan().artifact_forest.as_ref().context("ordinary comic build forest missing")?.branches.len() == case.outputs.len(),
                    "retained ordinary build configuration disagrees with the comic recipe");
            }
            if case.exercise == "execute" {
                let destination = output_root.join(&page.id);
                let run = execute_composition(&resolved,&destination,Some(&cache),&token())?;
                ensure!(run.state == CompositionRunState::Complete && run.review_required && !run.runtime_qualified, "comic native execution state differs");
                let common = run.run_manifest()?;
                let verified = verification::verify(&common,&destination,&VerificationLimits::default(),token())?;
                ensure!(verified.len() == case.outputs.len(), "comic actual publication count differs"); publications += verified.len();
                let raster = resolved.native_plan().branches.iter().find(|branch|branch.output == CompositionOutput::Png)
                    .and_then(|branch|branch.raster.clone()).context("comic PNG plan missing")?;
                captures.insert(page.id.clone(),publications::Capture {manifest:common.clone(),root:destination.clone(),raster});
                let observed = inspect_page(&destination,page,case)?;
                for (format,hash) in &observed { hashes.insert(format!("{}/{}",page.id,format),hash.clone()); }
                if first.is_none() { first = Some((common,destination,observed)); }
            }
        }
        let aggregate = publications::run(&generated,case,&case_root,&output_root,&captures,requested_rank)?;
        publications += aggregate.observed;
        hashes.extend(aggregate.hashes);
        let mut negatives = Vec::new();
        for kind in &case.negative_cases {
            ensure!(started.elapsed().as_secs() < case.limits.cooperative_seconds, "pack negative-case cooperative budget exceeded");
            match kind.as_str() {
                "wrong_order" | "wrong_count" | "wrong_geometry" | "missing_accessibility" => {
                    let mut model = generated.model.clone();
                    let expected = match kind.as_str() {
                        "wrong_order" => { model["groups"][0]["page_ids"].as_array_mut().unwrap().reverse(); "fixture.pack.page_order" }
                        "wrong_count" => { model["groups"][0]["page_ids"].as_array_mut().unwrap().pop(); "fixture.pack.page_count" }
                        "wrong_geometry" => { model["geometry_mm"]["width"] = json!(101); "fixture.pack.geometry" }
                        _ => { model["alt"]["cover-front"] = json!(""); "fixture.pack.accessibility" }
                    };
                    let error = validate_model(&generated,case,&model).expect_err("bad fixture expectation must fail");
                    ensure!(error.to_string().contains(expected), "wrong fixture expectation diagnostic");
                }
                "pre_cancel" => {
                    let resolved = resolve_composition(request(&generated.pages[0].manifest,case)?)?;
                    let cancelled = token(); cancelled.cancel(); let destination = case_root.join("cancelled");
                    ensure!(execute_composition(&resolved,&destination,None,&cancelled).is_err()
                        && !destination.exists(), "pre-cancellation must not publish");
                }
                "unavailable_interchange" => {
                    let mut req = request(&generated.pages[0].manifest,case)?;
                    req.request.targets = vec![CompositionOutput::TranscriptJson,CompositionOutput::Ora];
                    let resolved = resolve_composition(req)?;
                    let run = execute_composition(&resolved,&case_root.join("partial"),None,&token())?;
                    ensure!(run.state == CompositionRunState::Partial && run.run_manifest()?.published_targets()?.len() == 1
                        && run.plan.branches.iter().any(|branch|branch.output == CompositionOutput::Ora && branch.state == CompositionBranchState::Unavailable), "unavailable interchange must preserve transcript sibling");
                }
                "missing_output" | "corrupt_output" => {
                    let (common,destination,_) = first.as_ref().context("negative output case requires executed page")?;
                    let path = destination.join("outputs/en/page.svg"); let bytes = fs::read(&path)?;
                    if kind == "missing_output" { fs::remove_file(&path)?; } else { fs::write(&path,b"invalid SVG")?; }
                    ensure!(verification::verify(common,destination,&VerificationLimits::default(),token()).is_err(), "changed/missing payload must fail native file verification");
                    fs::write(path,bytes)?;
                }
                _ => negative_sources(&generated,case,kind)?,
            }
            negatives.push(kind.clone());
        }
        if case.exercise == "execute" {
            let resolved = resolve_composition(request(&generated.pages[0].manifest,case)?)?;
            let warm = case_root.join("warm"); let run = execute_composition(&resolved,&warm,Some(&cache),&token())?;
            ensure!(run.branches.iter().all(|branch|branch.cache == renderflow::evidence::CacheDisposition::Hit), "comic warm cache did not reuse all terminal outputs");
            ensure!(inspect_page(&warm,&generated.pages[0],case)? == first.as_ref().unwrap().2, "comic deterministic output bytes changed");
            let cold = case_root.join("independent-cold");
            let run = execute_composition(&resolved,&cold,None,&token())?;
            ensure!(run.branches.iter().all(|branch|branch.cache == renderflow::evidence::CacheDisposition::Miss)
                && inspect_page(&cold,&generated.pages[0],case)? == first.as_ref().unwrap().2,
                "independent cold rerun must reproduce candidate bytes without shared cache");
        }
        ensure!(snapshot(&generated.source,case.limits.max_files,case.limits.max_source_bytes)? == before, "comic source changed");
        let output_bytes = snapshot(&output_root,2048,case.limits.max_output_bytes)?.1;
        snapshot(&case_root,4096,case.limits.max_source_bytes + 3 * case.limits.max_output_bytes)?;
        ensure!(started.elapsed().as_secs() < case.limits.cooperative_seconds, "pack cooperative budget exceeded");
        if let Some(path) = &retained { copy_tree(&generated.source,&path.join(format!("{}-{}",pack.id,case.id)))?; }
        evidence.push(PackEvidence { pack_id:pack.id.clone(),case_id:case.id.clone(),status:if case.exercise=="execute" {"observed_candidates"} else {"planned_only"}.into(),
            exercise:case.exercise.clone(),source_sha256:digest(&story_bytes),generator_sha256:pack.generator.sha256.clone(),profile:pack.profile.clone(),
            toolchain_sha256:pack.toolchain.sha256.clone(),asset_sha256:pack.assets.iter().chain(std::iter::once(&pack.rights)).map(|asset|(asset.path.clone(),asset.sha256.clone())).collect(),
            recipe_dependency_sha256:pack.recipe_dependencies.iter().map(|pin|(pin.path.clone(),pin.sha256.clone())).collect(),
            parameters:case.parameters.clone(),validator_ids:case.outputs.iter().map(|output|output.validator.clone()).collect(),
            interior_page_ids:generated.pages.iter().filter(|page|page.role=="interior").map(|page|page.id.clone()).collect(),
            cover_page_ids:vec!["cover-front".into(),"cover-back".into()],observed_pages:if case.exercise=="execute" {generated.pages.len()} else {0},
            observed_publications:publications,output_sha256:hashes,source_bytes:before.1,output_bytes,negative_cases:negatives,
            unavailable_mappings:pack.unavailable_mappings.iter().map(|mapping|json!({"role":mapping.role,"format":mapping.format,"status":"unavailable","reason":mapping.reason})).collect(),
            publication_outputs:aggregate.outputs,rgb_preparation:aggregate.preparation,
            runtime_qualified:false,creative_approval_inferred:false });
        results.insert((scenario.id.clone(),ScenarioEvidence {id:scenario.id.clone(),status:"passed".into(),
            assertions:vec![if case.exercise=="execute" {"Per-page candidates, explicit RGB preparation and exact fixed EPUB image/order joins matched; PDF packets require a separately selected full-tier provider. Source immutability and warm/cold identity remain distinct from approval."} else {"Parameterized page model and aggregate recipes only; zero output artifacts or aggregate SDK plans observed."}.into()],
            providers:vec!["renderflow.core-composition".into(),"tool.renderflow-epub".into()],reason:Some("Finite fixture scope; consumer publication and provider/runtime qualification are not inferred".into())}));
    }
    Ok(evidence)
}
