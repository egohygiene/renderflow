//! Confined, bounded and pinned snapshots for the page composition profile.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::evidence::DigestEvidence;
use crate::font::{FontDiagnosticSeverity, FontFormat, FontRegistry, LoadedFontRegistry};

use super::model::*;

const MANIFEST_LIMIT: u64 = 2 * 1024 * 1024;
const CONTENT_LIMIT: u64 = 1024 * 1024;
const ASSET_LIMIT: u64 = 16 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct LoadedAsset {
    pub declaration: CompositionAsset,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct LoadedContent {
    pub document: PageContent,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

/// Input identity deliberately excludes local paths. Bytes remain pinned by the
/// manifest; evidence is stable when a complete input directory is relocated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionInputEvidence {
    pub kind: String,
    pub id: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct LoadedComposition {
    pub(crate) manifest: PageComposition,
    pub(crate) manifest_bytes: Vec<u8>,
    pub(crate) manifest_sha256: String,
    pub(crate) assets: BTreeMap<String, LoadedAsset>,
    pub(crate) contents: BTreeMap<String, LoadedContent>,
    pub(crate) fonts: LoadedFontRegistry,
    pub(crate) diagnostics: Vec<CompositionDiagnostic>,
    pub(crate) inputs: Vec<CompositionInputEvidence>,
    /// Keys are `kind:id`, matching `inputs`. These are the exact captured
    /// bytes, including provenance originals and observed license artifacts.
    pub(crate) input_bytes: BTreeMap<String, Vec<u8>>,
}

impl LoadedComposition {
    pub fn manifest(&self) -> &PageComposition { &self.manifest }
    pub fn manifest_bytes(&self) -> &[u8] { &self.manifest_bytes }
    pub fn manifest_sha256(&self) -> &str { &self.manifest_sha256 }
    pub fn assets(&self) -> &BTreeMap<String, LoadedAsset> { &self.assets }
    pub fn content(&self, locale: &str) -> Option<&LoadedContent> { self.contents.get(locale) }
    pub fn fonts(&self) -> &LoadedFontRegistry { &self.fonts }
    pub fn diagnostics(&self) -> &[CompositionDiagnostic] { &self.diagnostics }
    pub fn inputs(&self) -> &[CompositionInputEvidence] { &self.inputs }
    pub fn input_bytes(&self, kind: &str, id: &str) -> Option<&[u8]> { self.input_bytes.get(&format!("{kind}:{id}")).map(Vec::as_slice) }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = absolute_no_symlinks(path.as_ref())?;
        let root = path.parent().context("composition manifest requires a parent directory")?;
        let manifest_bytes = read_regular(&path, MANIFEST_LIMIT)?;
        let manifest: PageComposition = parse_json(&manifest_bytes)?;
        validate_manifest(&manifest)?;
        let manifest_sha256 = sha256(&manifest_bytes);
        let mut inputs = vec![input_evidence("manifest", &manifest.composition_id, &manifest_bytes)];
        let mut input_bytes = BTreeMap::from([(format!("manifest:{}", manifest.composition_id), manifest_bytes.clone())]);
        let mut total = manifest_bytes.len() as u64;
        let mut assets = BTreeMap::new();
        for declaration in &manifest.assets {
            let bytes = read_reference(root, &declaration.file, ASSET_LIMIT, &mut total)?;
            validate_asset(declaration.format, &bytes)
                .with_context(|| format!("composition asset '{}' is outside the candidate profile", declaration.id))?;
            inputs.push(input_evidence("asset", &declaration.id, &bytes));
            input_bytes.insert(format!("asset:{}", declaration.id), bytes.clone());
            assets.insert(declaration.id.clone(), LoadedAsset { declaration: declaration.clone(), bytes });
        }
        let mut contents = BTreeMap::new();
        for (locale, reference) in &manifest.locales {
            let bytes = read_reference(root, reference, CONTENT_LIMIT, &mut total)?;
            let document: PageContent = parse_json(&bytes)
                .with_context(|| format!("invalid content document for '{locale}'"))?;
            ensure!(document.schema == PAGE_CONTENT_SCHEMA_V1, "unsupported page content schema");
            ensure!(document.composition_id == manifest.composition_id, "content composition_id does not match manifest");
            ensure!(&document.locale == locale, "content locale does not match manifest locale key");
            ensure!(is_stable_id(&document.revision) && is_stable_id(&document.source_revision), "content revision fields must be stable IDs");
            ensure!(document.content.len() <= 256, "at most 256 content entries are supported per locale");
            for (id, text) in &document.content {
                ensure!(is_stable_id(id), "content IDs must be stable ASCII IDs");
                ensure!(text.len() <= 64 * 1024, "a content entry exceeds the 64 KiB limit");
                ensure!(!text.chars().any(|c| (c.is_control() && c != '\n') || matches!(c, '\u{fffe}' | '\u{ffff}')), "content contains characters outside the XML text profile");
            }
            inputs.push(input_evidence("content", locale, &bytes));
            input_bytes.insert(format!("content:{locale}"), bytes.clone());
            contents.insert(locale.clone(), LoadedContent { document, sha256: sha256(&bytes), bytes });
        }
        // Originals are preserved and pinned as evidence, never extracted or
        // rewritten. Their format does not imply a layer-separation capability.
        for (index, reference) in manifest.provenance.originals.iter().enumerate() {
            let bytes = read_reference(root, reference, ASSET_LIMIT, &mut total)?;
            inputs.push(input_evidence("original", &index.to_string(), &bytes));
            input_bytes.insert(format!("original:{index}"), bytes);
        }
        let registry_bytes = read_reference(root, &manifest.font_registry, MANIFEST_LIMIT, &mut total)?;
        let registry: FontRegistry = parse_json(&registry_bytes).context("composition font registry must be JSON")?;
        ensure!(registry.assets.len() <= 64, "composition font registry supports at most 64 assets");
        let registry_path = confined_path(root, &manifest.font_registry.path)?;
        let registry_root = registry_path.parent().context("font registry requires a parent directory")?;
        let mut font_assets = BTreeMap::new();
        let mut license_files = BTreeSet::new();
        inputs.push(input_evidence("font_registry", &registry.registry_id, &registry_bytes));
        input_bytes.insert(format!("font_registry:{}", registry.registry_id), registry_bytes.clone());
        for asset in &registry.assets {
            ensure!(is_stable_id(&asset.id), "font asset ID must be a stable ASCII ID");
            ensure!(!font_assets.contains_key(&asset.id), "duplicate font asset ID");
            ensure!(asset.digest.algorithm == "sha256", "font asset digest must use sha256");
            let reference = FileReference { path: asset.path.clone(), sha256: asset.digest.value.clone() };
            let bytes = read_reference(registry_root, &reference, ASSET_LIMIT, &mut total)?;
            ensure!(font_signature(asset.format, &bytes), "font asset '{}' has an invalid container signature", asset.id);
            inputs.push(input_evidence("font", &asset.id, &bytes));
            input_bytes.insert(format!("font:{}", asset.id), bytes.clone());
            font_assets.insert(asset.id.clone(), bytes);
            if license_files.insert(asset.license.license_file.clone()) {
                let license_path = confined_path(registry_root, &asset.license.license_file)?;
                let bytes = read_regular(&license_path, CONTENT_LIMIT)?;
                add_total(&mut total, bytes.len())?;
                // Registry v1 licenses have no declared digest; capture their
                // observed digest explicitly rather than claiming they are pinned.
                inputs.push(input_evidence("font_license_observed", &asset.id, &bytes));
                input_bytes.insert(format!("font_license_observed:{}", asset.id), bytes);
            }
        }
        let fonts = LoadedFontRegistry::from_snapshot(
            registry,
            registry_path,
            DigestEvidence { algorithm: "sha256".to_string(), value: sha256(&registry_bytes) },
            font_assets,
            license_files,
        );
        let font_validation = fonts.validate();
        ensure!(font_validation.valid, "invalid composition font registry: {}", font_validation.diagnostics.iter().filter(|d| d.severity == FontDiagnosticSeverity::Error).map(|d| d.message.as_str()).collect::<Vec<_>>().join("; "));
        let diagnostics = content_diagnostics(&manifest, &contents);
        Ok(Self { manifest, manifest_bytes, manifest_sha256, assets, contents, fonts, diagnostics, inputs, input_bytes })
    }
}

pub fn load_composition(path: impl AsRef<Path>) -> Result<LoadedComposition> {
    LoadedComposition::load(path)
}

fn validate_manifest(manifest: &PageComposition) -> Result<()> {
    ensure!(manifest.schema == PAGE_COMPOSITION_SCHEMA_V1, "unsupported page composition schema");
    ensure!(is_stable_id(&manifest.composition_id) && is_stable_id(&manifest.revision), "composition_id and revision must be stable ASCII IDs");
    ensure!(!manifest.locales.is_empty() && manifest.locales.len() <= 32, "composition requires between 1 and 32 locales");
    ensure!(manifest.locales.contains_key(&manifest.default_locale) && manifest.locales.contains_key(&manifest.source_locale), "source_locale and default_locale must be declared");
    ensure!(manifest.locales.keys().all(|locale| is_locale(locale)), "locale IDs must use bounded ASCII language subtags");
    ensure!(manifest.locales.keys().map(|locale| locale.to_ascii_lowercase()).collect::<BTreeSet<_>>().len() == manifest.locales.len(),
        "locale IDs must remain distinct on case-insensitive filesystems");
    ensure!(positive(manifest.page.width_mm, 5000.0) && positive(manifest.page.height_mm, 5000.0), "page dimensions must be finite, positive and at most 5000 mm");
    valid_rect(manifest.page.trim)?;
    valid_rect(manifest.page.bleed)?;
    valid_rect(manifest.page.safe)?;
    ensure!(contains(manifest.page.bleed, manifest.page.trim) && contains(manifest.page.trim, manifest.page.safe), "bleed must contain trim, and trim must contain the safe rectangle");
    ensure!(manifest.page.gutter_mm.is_finite() && manifest.page.gutter_mm >= 0.0 && manifest.page.gutter_mm <= manifest.page.width_mm * manifest.page.trim.width / 2.0, "gutter must be finite, nonnegative and no wider than half the trim");
    ensure!(manifest.assets.len() <= 64, "at most 64 composition assets are supported");
    ensure!(!manifest.layers.is_empty() && manifest.layers.len() <= 256, "composition requires between 1 and 256 layers");
    let mut asset_ids = BTreeMap::new();
    for asset in &manifest.assets {
        ensure!(is_stable_id(&asset.id), "asset IDs must be stable ASCII IDs");
        ensure!(asset_ids.insert(asset.id.as_str(), asset.format).is_none(), "duplicate asset ID");
        ensure!(asset.alt.len() <= 4096 && !asset.alt.chars().any(|c| c.is_control() || matches!(c, '\u{fffe}' | '\u{ffff}')), "asset alternative text exceeds limits or contains invalid XML characters");
    }
    let mut layer_ids = BTreeSet::new();
    let mut text_ids = BTreeSet::new();
    let mut content_ids = BTreeSet::new();
    for layer in &manifest.layers {
        ensure!(is_stable_id(&layer.id) && layer_ids.insert(layer.id.as_str()), "layer IDs must be unique stable ASCII IDs");
        valid_rect(layer.bounds)?;
        if let Some(clip) = layer.clip { valid_rect(clip)?; }
        ensure!(layer.opacity.is_finite() && (0.0..=1.0).contains(&layer.opacity), "layer opacity must be between zero and one");
        match &layer.content {
            LayerContent::Raster { asset_id } => ensure!(matches!(asset_ids.get(asset_id.as_str()), Some(AssetFormat::Png | AssetFormat::Jpeg)), "raster layer references an unknown or non-raster asset"),
            LayerContent::Vector { asset_id } => ensure!(asset_ids.get(asset_id.as_str()) == Some(&AssetFormat::Svg), "vector layer references an unknown or non-SVG asset"),
            LayerContent::Shape { fill, stroke, stroke_width_pt, .. } => {
                ensure!(is_color(fill) && stroke.as_ref().is_none_or(|value| is_color(value)), "shape colors must be six/eight-digit hex or 'none'");
                ensure!(stroke_width_pt.is_finite() && (0.0..=72.0).contains(stroke_width_pt), "shape stroke width must be between 0 and 72 points");
            }
            LayerContent::Text { content_id, typography } => {
                ensure!(is_stable_id(content_id) && content_ids.insert(content_id.as_str()), "each text layer requires a unique stable content ID");
                ensure!(positive(typography.font_size_pt, 512.0) && positive(typography.min_font_size_pt, typography.font_size_pt), "text sizes must be positive, bounded and no smaller than the declared minimum");
                ensure!(typography.line_height.is_finite() && (1.0..=5.0).contains(&typography.line_height), "line height must be between 1 and 5");
                ensure!(typography.padding_pt.is_finite() && (0.0..=512.0).contains(&typography.padding_pt), "text padding must be between 0 and 512 points");
                ensure!(is_color(&typography.color) && typography.color != "none", "text color requires a six/eight-digit hex color");
                text_ids.insert(layer.id.as_str());
            }
        }
    }
    let reading_ids = manifest.reading_order.iter().map(String::as_str).collect::<BTreeSet<_>>();
    ensure!(reading_ids.len() == manifest.reading_order.len() && reading_ids == text_ids, "reading_order must contain every text layer exactly once and no other layers");
    ensure!(manifest.provenance.originals.len() <= 16, "at most 16 immutable original references are supported");
    ensure!(!manifest.provenance.originals.is_empty(), "provenance requires at least one pinned immutable original");
    ensure!(manifest.provenance.notes.len() <= 8192, "provenance notes exceed the 8192-byte limit");
    if manifest.provenance.kind == ProvenanceKind::Reconstructed && !manifest.provenance.separation_reviewed {
        ensure!(manifest.provenance.requires_human_review, "unreviewed reconstruction must retain requires_human_review");
    }
    for (locale, overrides) in &manifest.locale_overrides {
        ensure!(manifest.locales.contains_key(locale), "locale override names an undeclared locale");
        ensure!(overrides.len() <= 256, "at most 256 overrides are supported per locale");
        let mut seen = BTreeSet::new();
        for item in overrides {
            ensure!(seen.insert(item.layer_id.as_str()), "duplicate layer override for a locale");
            let layer = manifest.layers.iter().find(|layer| layer.id == item.layer_id).context("locale override references an unknown layer")?;
            ensure!(!item.reason.trim().is_empty() && item.reason.len() <= 2048 && !item.author.trim().is_empty() && item.author.len() <= 256, "locale overrides require bounded author and reason declarations");
            ensure!(item.bounds.is_some() || item.font_size_pt.is_some(), "locale override must change bounds or font size");
            if let Some(bounds) = item.bounds { valid_rect(bounds)?; }
            if let Some(size) = item.font_size_pt {
                let LayerContent::Text { typography, .. } = &layer.content else { bail!("font-size overrides apply only to text layers"); };
                ensure!(positive(size, 512.0) && size >= typography.min_font_size_pt, "locale override cannot reduce text below its minimum size");
            }
        }
    }
    Ok(())
}

fn content_diagnostics(manifest: &PageComposition, contents: &BTreeMap<String, LoadedContent>) -> Vec<CompositionDiagnostic> {
    let mut diagnostics = Vec::new();
    let source_revision = contents.get(&manifest.source_locale).map(|content| content.document.revision.as_str());
    let expected = manifest.layers.iter().filter_map(|layer| match &layer.content { LayerContent::Text { content_id, .. } => Some(content_id.as_str()), _ => None }).collect::<BTreeSet<_>>();
    for (locale, loaded) in contents {
        let content = &loaded.document;
        if !content.reviewed {
            diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error, "composition.content.unreviewed", "Locale content has not been marked reviewed by its author", Some(locale), None));
        }
        if Some(content.source_revision.as_str()) != source_revision {
            diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error, "composition.content.stale", "Locale content does not reference the current source content revision", Some(locale), None));
        }
        for layer in &manifest.layers {
            if let LayerContent::Text { content_id, .. } = &layer.content {
                if content.content.get(content_id).is_none_or(|text| text.trim().is_empty()) {
                    diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error, "composition.content.missing", format!("Missing or empty localized content '{content_id}'"), Some(locale), Some(&layer.id)));
                }
            }
        }
        for id in content.content.keys().filter(|id| !expected.contains(id.as_str())) {
            diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error, "composition.content.unknown", format!("Content '{id}' does not correspond to a text layer"), Some(locale), None));
        }
        if manifest.provenance.kind == ProvenanceKind::FlattenedOriginal && locale != &manifest.source_locale {
            diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error, "composition.provenance.flattened_localization", "A flattened original is not a textless plate; localization requires a reviewed reconstructed/textless composition", Some(locale), None));
        }
    }
    if manifest.provenance.requires_human_review || (manifest.provenance.kind == ProvenanceKind::Reconstructed && !manifest.provenance.separation_reviewed) {
        diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Warning, "composition.provenance.review_required", "Composition provenance requires human review; generated candidates do not approve separation or reconstruction", None, None));
    }
    diagnostics
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn is_locale(value: &str) -> bool {
    !value.is_empty() && value.len() <= 35 && value.split('-').all(|part| !part.is_empty() && part.len() <= 8 && part.bytes().all(|c| c.is_ascii_alphanumeric()))
}

fn positive(value: f64, maximum: f64) -> bool { value.is_finite() && value > 0.0 && value <= maximum }

fn valid_rect(rect: NormalizedRect) -> Result<()> {
    ensure!([rect.x, rect.y, rect.width, rect.height].iter().all(|value| value.is_finite()), "rectangle coordinates must be finite");
    ensure!(rect.x >= 0.0 && rect.y >= 0.0 && rect.width > 0.0 && rect.height > 0.0 && rect.x + rect.width <= 1.0 + 1e-9 && rect.y + rect.height <= 1.0 + 1e-9, "rectangles must be positive and contained within the normalized page canvas");
    Ok(())
}

fn contains(outer: NormalizedRect, inner: NormalizedRect) -> bool {
    inner.x + 1e-9 >= outer.x && inner.y + 1e-9 >= outer.y && inner.x + inner.width <= outer.x + outer.width + 1e-9 && inner.y + inner.height <= outer.y + outer.height + 1e-9
}

fn is_color(value: &str) -> bool {
    value == "none" || (matches!(value.len(), 7 | 9) && value.starts_with('#') && value[1..].bytes().all(|c| c.is_ascii_hexdigit()))
}

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn input_evidence(kind: &str, id: &str, bytes: &[u8]) -> CompositionInputEvidence {
    CompositionInputEvidence { kind: kind.into(), id: id.into(), sha256: sha256(bytes), size_bytes: bytes.len() as u64 }
}

fn add_total(total: &mut u64, size: usize) -> Result<()> {
    *total = total.checked_add(size as u64).context("composition size overflow")?;
    ensure!(*total <= TOTAL_LIMIT, "composition snapshot exceeds the 128 MiB total input limit");
    Ok(())
}

fn read_reference(root: &Path, reference: &FileReference, limit: u64, total: &mut u64) -> Result<Vec<u8>> {
    ensure!(reference.sha256.len() == 64 && reference.sha256.bytes().all(|c| c.is_ascii_hexdigit()), "input sha256 must contain 64 hexadecimal digits");
    let path = confined_path(root, &reference.path)?;
    let bytes = read_regular(&path, limit)?;
    add_total(total, bytes.len())?;
    ensure!(sha256(&bytes).eq_ignore_ascii_case(&reference.sha256), "composition input '{}' does not match its pinned sha256", reference.path);
    Ok(bytes)
}

fn confined_path(root: &Path, value: &str) -> Result<PathBuf> {
    ensure!(!value.is_empty() && value.len() <= 1024 && !value.contains(['\\', ':', '\0']), "input references must be bounded local relative paths");
    let relative = Path::new(value);
    ensure!(relative.components().all(|part| matches!(part, Component::Normal(_))), "input references cannot be absolute, traversal, or dot paths");
    absolute_no_symlinks(&root.join(relative))
}

fn absolute_no_symlinks(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    let mut current = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => continue,
            Component::ParentDir => bail!("composition paths may not traverse parents"),
            _ => current.push(part.as_os_str()),
        }
        let metadata = fs::symlink_metadata(&current).with_context(|| format!("cannot inspect composition input '{}'", current.display()))?;
        ensure!(!metadata.file_type().is_symlink(), "composition input paths cannot traverse symlinks");
    }
    Ok(current)
}

fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    ensure!(before.is_file() && !before.file_type().is_symlink() && before.len() <= limit, "composition input must be a bounded regular file");
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    ensure!(same_file(&before, &opened), "composition input changed while opening");
    let mut bytes = Vec::new();
    file.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    ensure!(bytes.len() as u64 <= limit && bytes.len() as u64 == opened.len() && same_file(&opened, &after) && same_file(&opened, &named_after) && !named_after.file_type().is_symlink(), "composition input changed or exceeded its size bound while reading");
    // Recheck the complete ancestry before the captured bytes leave the loader.
    absolute_no_symlinks(path)?;
    Ok(bytes)
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    let common = left.is_file() == right.is_file() && left.len() == right.len() && left.modified().ok() == right.modified().ok();
    #[cfg(unix)]
    { use std::os::unix::fs::MetadataExt; common && left.dev() == right.dev() && left.ino() == right.ino() }
    #[cfg(not(unix))]
    { common }
}

fn font_signature(format: FontFormat, bytes: &[u8]) -> bool {
    match format {
        FontFormat::Ttf => bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"true"),
        FontFormat::Otf => bytes.starts_with(b"OTTO"),
        FontFormat::Woff => bytes.starts_with(b"wOFF"),
        FontFormat::Woff2 => bytes.starts_with(b"wOF2"),
    }
}

fn validate_asset(format: AssetFormat, bytes: &[u8]) -> Result<()> {
    match format {
        AssetFormat::Png => {
            ensure!(bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") && &bytes[12..16] == b"IHDR", "PNG asset has an invalid signature/header");
            let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            bounded_raster_dimensions(width, height)?;
        }
        AssetFormat::Jpeg => {
            ensure!(bytes.len() >= 4 && bytes.starts_with(&[0xff, 0xd8, 0xff]) && bytes.ends_with(&[0xff, 0xd9]), "JPEG asset has an invalid signature");
            let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
            decoder.read_info().context("invalid JPEG metadata")?;
            let info = decoder.info().context("JPEG has no image dimensions")?;
            bounded_raster_dimensions(u32::from(info.width), u32::from(info.height))?;
        }
        AssetFormat::Svg => validate_svg(bytes)?,
    }
    Ok(())
}

fn bounded_raster_dimensions(width: u32, height: u32) -> Result<()> {
    ensure!(width > 0 && height > 0 && width <= 32768 && height <= 32768 && u64::from(width) * u64::from(height) <= 100_000_000, "raster dimensions exceed the 32768-axis/100-megapixel profile");
    Ok(())
}

/// A small inert graphical subset. No CSS, URLs, text, scripts, resource
/// references, foreign content, entity declarations, or executable attributes.
pub(crate) fn validate_svg(bytes: &[u8]) -> Result<()> {
    ensure!(bytes.len() <= 4 * 1024 * 1024, "SVG asset exceeds the 4 MiB limit");
    let text = std::str::from_utf8(bytes).context("SVG asset must be UTF-8")?;
    ensure!(!text.contains("<!DOCTYPE") && !text.contains("<!ENTITY"), "SVG DTD/entity declarations are not supported");
    let document = roxmltree::Document::parse(text).context("invalid SVG asset XML")?;
    let root = document.root_element();
    ensure!(root.tag_name().name() == "svg" && root.tag_name().namespace() == Some("http://www.w3.org/2000/svg"), "SVG requires the standard SVG root namespace");
    ensure!(document.descendants().count() <= 16384, "SVG asset has too many XML nodes");
    for node in document.descendants() {
        ensure!(!node.is_pi(), "SVG processing instructions are not supported");
        if node.is_text() { ensure!(node.text().unwrap_or_default().trim().is_empty(), "SVG graphical assets cannot contain text"); }
        if !node.is_element() { continue; }
        ensure!(node.tag_name().namespace() == Some("http://www.w3.org/2000/svg") && matches!(node.tag_name().name(), "svg" | "g" | "path" | "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon"), "unsupported SVG element");
        ensure!(node == root || node.tag_name().name() != "svg", "nested SVG viewports are not supported");
        for attribute in node.attributes() {
            ensure!(attribute.namespace().is_none(), "namespaced SVG attributes are not supported");
            let name = attribute.name();
            let value = attribute.value();
            ensure!(value.len() <= 262144 && !value.chars().any(char::is_control), "SVG attribute exceeds bounds or contains controls");
            match name {
                "fill" | "stroke" => ensure!(is_color(value), "SVG paints require hex colors or none"),
                "fill-rule" => ensure!(matches!(value, "nonzero" | "evenodd"), "unsupported SVG fill rule"),
                "stroke-linecap" => ensure!(matches!(value, "butt" | "round" | "square"), "unsupported SVG line cap"),
                "stroke-linejoin" => ensure!(matches!(value, "miter" | "round" | "bevel"), "unsupported SVG line join"),
                "preserveAspectRatio" => ensure!(matches!(value, "none" | "xMidYMid meet" | "xMidYMid slice"), "unsupported SVG aspect ratio policy"),
                "d" => ensure!(value.bytes().all(|c| c.is_ascii_digit() || b"MmZzLlHhVvCcSsQqTtAaEe.,+- ".contains(&c)), "unsupported SVG path data"),
                "viewBox" | "points" => ensure!(value.bytes().all(|c| c.is_ascii_digit() || b"Ee.,+- ".contains(&c)), "unsupported SVG coordinate list"),
                "x" | "y" | "x1" | "y1" | "x2" | "y2" | "width" | "height" | "cx" | "cy" | "r" | "rx" | "ry" | "stroke-width" | "stroke-miterlimit" => {
                    let number: f64 = value.parse().context("SVG geometry requires numeric user units")?;
                    ensure!(number.is_finite() && number.abs() <= 1e7, "SVG geometry exceeds bounded numeric range");
                }
                "opacity" | "fill-opacity" | "stroke-opacity" => {
                    let number: f64 = value.parse().context("SVG opacity must be numeric")?;
                    ensure!(number.is_finite() && (0.0..=1.0).contains(&number), "SVG opacity must lie between zero and one");
                }
                _ => bail!("unsupported SVG attribute '{name}'"),
            }
        }
    }
    Ok(())
}

/// Reject duplicate object keys throughout JSON, including dynamic locale and
/// content maps; serde's ordinary map deserializer would silently keep the last.
fn parse_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueJson::deserialize(&mut deserializer)?.0;
    deserializer.end()?;
    Ok(serde_json::from_value(value)?)
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = Value;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { formatter.write_str("JSON without duplicate object keys") }
            fn visit_bool<E: serde::de::Error>(self, value: bool) -> std::result::Result<Value, E> { Ok(Value::Bool(value)) }
            fn visit_i64<E: serde::de::Error>(self, value: i64) -> std::result::Result<Value, E> { Ok(value.into()) }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> std::result::Result<Value, E> { Ok(value.into()) }
            fn visit_f64<E: serde::de::Error>(self, value: f64) -> std::result::Result<Value, E> { serde_json::Number::from_f64(value).map(Value::Number).ok_or_else(|| E::custom("non-finite JSON number")) }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> std::result::Result<Value, E> { Ok(Value::String(value.to_string())) }
            fn visit_string<E: serde::de::Error>(self, value: String) -> std::result::Result<Value, E> { Ok(Value::String(value)) }
            fn visit_none<E: serde::de::Error>(self) -> std::result::Result<Value, E> { Ok(Value::Null) }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Value, E> { Ok(Value::Null) }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> std::result::Result<Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueJson>()? { values.push(value.0); }
                Ok(Value::Array(values))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
                let mut object = serde_json::Map::new();
                while let Some((key, value)) = map.next_entry::<String, UniqueJson>()? {
                    if object.insert(key, value.0).is_some() { return Err(serde::de::Error::custom("duplicate JSON object key")); }
                }
                Ok(Value::Object(object))
            }
        }
        deserializer.deserialize_any(UniqueVisitor).map(UniqueJson)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_json(path: &Path, value: Value) -> Vec<u8> {
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        fs::write(path, &bytes).unwrap();
        bytes
    }

    // Container signatures are sufficient for this loader-only fixture. This
    // does not represent a usable or qualified font, glyph set, or rendered page.
    fn fixture(root: &Path) -> PathBuf {
        let root = root.canonicalize().unwrap();
        let plate = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" fill="#ffffff"/></svg>"##;
        fs::write(root.join("plate.svg"), plate).unwrap();
        let font = [0, 1, 0, 0, 0, 0, 0, 0];
        fs::write(root.join("font.ttf"), font).unwrap();
        fs::write(root.join("LICENSE.txt"), b"Loader-only synthetic font bytes").unwrap();
        let registry = write_json(&root.join("fonts.json"), json!({
            "schema": "renderflow.font-registry/v1", "registry_id": "fixture", "version": "1",
            "assets": [{ "id": "body", "family": "Fixture", "path": "font.ttf", "format": "ttf",
                "digest": {"algorithm":"sha256","value":sha256(&font)},
                "provenance": {"source_url":"https://example.invalid/fixture","source_version":"1"},
                "license": {"spdx_id":"LicenseRef-Fixture","license_file":"LICENSE.txt","redistribution":"allowed","embedding":"allowed"}
            }], "roles": {"body":{"primary":"body"}}
        }));
        let content = write_json(&root.join("en.json"), json!({
            "schema":PAGE_CONTENT_SCHEMA_V1,"composition_id":"fixture","locale":"en","revision":"r1","source_revision":"r1","reviewed":true,"direction":"ltr","writing_mode":"horizontal_tb","content":{"dialogue":"Hello"}
        }));
        write_json(&root.join("page.json"), json!({
            "schema": PAGE_COMPOSITION_SCHEMA_V1,"composition_id":"fixture","revision":"r1",
            "page":{"width_mm":100.0,"height_mm":100.0,"trim":{"x":0.0,"y":0.0,"width":1.0,"height":1.0},"bleed":{"x":0.0,"y":0.0,"width":1.0,"height":1.0},"safe":{"x":0.05,"y":0.05,"width":0.9,"height":0.9},"gutter_mm":0.0},
            "source_locale":"en","default_locale":"en","locales":{"en":{"path":"en.json","sha256":sha256(&content)}},
            "font_registry":{"path":"fonts.json","sha256":sha256(&registry)},
            "assets":[{"id":"plate","file":{"path":"plate.svg","sha256":sha256(plate)},"format":"svg","alt":"Blank synthetic plate"}],
            "layers":[{"id":"art","bounds":{"x":0.0,"y":0.0,"width":1.0,"height":1.0},"content":{"kind":"vector","asset_id":"plate"}},
                {"id":"words","bounds":{"x":0.1,"y":0.1,"width":0.8,"height":0.2},"content":{"kind":"text","content_id":"dialogue","typography":{"role":"body","font_size_pt":12.0,"min_font_size_pt":9.0,"line_height":1.2,"padding_pt":2.0,"alignment":"start","color":"#000000"}}}],
            "reading_order":["words"],"provenance":{"kind":"textless_original","originals":[{"path":"plate.svg","sha256":sha256(plate)}],"separation_reviewed":true,"requires_human_review":false,"notes":"Synthetic loader fixture"}
        }));
        root.join("page.json")
    }

    #[test]
    fn duplicate_dynamic_json_keys_are_refused() {
        assert!(parse_json::<Value>(br#"{"locales":{"en":1,"en":2}}"#).is_err());
        assert!(parse_json::<Value>(br#"{"a":[{"b":1,"b":2}]}"#).is_err());
    }

    #[test]
    fn svg_resources_and_executable_content_are_refused() {
        for payload in [
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.invalid/a.png"/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg"><rect style="fill:red"/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text>Already baked in</text></svg>"#,
            r#"<!DOCTYPE svg [<!ENTITY e "expanded">]><svg xmlns="http://www.w3.org/2000/svg"/>"#,
        ] { assert!(validate_svg(payload.as_bytes()).is_err(), "{payload}"); }
    }

    #[test]
    fn loaded_snapshot_survives_replacement_of_original_inputs() {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture(directory.path());
        let loaded = LoadedComposition::load(path).unwrap();
        let original_plate = loaded.assets["plate"].bytes.clone();
        fs::write(directory.path().join("plate.svg"), b"replacement").unwrap();
        fs::remove_file(directory.path().join("font.ttf")).unwrap();
        fs::remove_file(directory.path().join("LICENSE.txt")).unwrap();
        assert_eq!(loaded.assets["plate"].bytes, original_plate);
        assert_eq!(loaded.input_bytes["asset:plate"], original_plate);
        assert!(loaded.fonts.validate().valid);
        assert!(loaded.fonts.resolve(crate::font::FontTarget::Html).is_ok());
        for input in &loaded.inputs {
            let bytes = &loaded.input_bytes[&format!("{}:{}", input.kind, input.id)];
            assert_eq!(sha256(bytes), input.sha256);
            assert_eq!(bytes.len() as u64, input.size_bytes);
        }
    }

    #[test]
    fn mismatched_pinned_input_is_fatal() {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture(directory.path());
        fs::write(directory.path().join("en.json"), b"{}").unwrap();
        assert!(LoadedComposition::load(path).is_err());
    }

    #[test]
    fn stale_content_is_a_locale_diagnostic() {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture(directory.path());
        let mut loaded = LoadedComposition::load(path).unwrap();
        loaded.contents.get_mut("en").unwrap().document.source_revision = "old".into();
        let diagnostics = content_diagnostics(&loaded.manifest, &loaded.contents);
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == "composition.content.stale" && diagnostic.locale.as_deref() == Some("en")));
    }

    #[test]
    fn traversal_and_symlink_inputs_are_refused() {
        let directory = tempfile::tempdir().unwrap();
        assert!(confined_path(directory.path(), "../outside").is_err());
        assert!(confined_path(directory.path(), "/absolute").is_err());
        assert!(confined_path(directory.path(), "https://example.invalid").is_err());
        #[cfg(unix)]
        {
            let path = fixture(directory.path());
            fs::rename(directory.path().join("plate.svg"), directory.path().join("actual.svg")).unwrap();
            std::os::unix::fs::symlink("actual.svg", directory.path().join("plate.svg")).unwrap();
            assert!(LoadedComposition::load(path).is_err());
        }
    }
}
