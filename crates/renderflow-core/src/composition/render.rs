//! Deterministic byte serialization for the bounded composition preview profile.
//!
//! This is not a shaping engine. Explicit author-supplied line breaks and sizes
//! are preserved; neither glyph coverage nor horizontal ink bounds are measured.
//! All resources are drawn from the immutable, validated input snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;

use crate::font::{FontFormat, FontResolutionReport, FontRole, FontTarget, ResolvedFont};

use super::model::{
    CompositionDiagnostic, DiagnosticSeverity, Layer, LayerContent, NormalizedRect,
    PageContent, ShapeKind, TextAlignment, TextDirection, WritingMode,
};
use super::{CompositionOutput, LoadedComposition};

pub const COMPOSITION_RENDERER_ID: &str = "renderflow.layered-svg";
pub const COMPOSITION_RENDERER_VERSION: u32 = 1;
pub const PAGE_TRANSCRIPT_SCHEMA_V1: &str = "renderflow.page-transcript/v1";
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const POINTS_PER_MM: f64 = 72.0 / 25.4;

#[derive(Debug)]
pub struct RenderedComposition {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
    pub extension: &'static str,
    pub diagnostics: Vec<CompositionDiagnostic>,
}

/// Source-derived checks only: passing these checks is not a visual proof.
///
/// The planner consumes these diagnostics before scheduling any output. Render
/// repeats them against the same immutable snapshot to refuse direct SDK calls
/// that would otherwise bypass a known unsupported layout or line-box overflow.
pub fn diagnostics(snapshot: &LoadedComposition, locale: &str) -> Vec<CompositionDiagnostic> {
    let mut result = Vec::new();
    let Some(content) = snapshot.contents.get(locale).map(|item| &item.document) else {
        result.push(diagnostic(DiagnosticSeverity::Error, "composition.locale.missing",
            "the selected locale has no captured content document", locale, None));
        return result;
    };
    if content.direction != TextDirection::Ltr || content.writing_mode != WritingMode::HorizontalTb {
        result.push(diagnostic(DiagnosticSeverity::Error, "composition.layout.unsupported",
            "this preview profile supports only left-to-right horizontal text; no bidi or vertical shaping is implemented", locale, None));
    }
    let fonts = match snapshot.fonts.resolve(FontTarget::Html) {
        Ok(fonts) => Some(fonts),
        Err(error) => {
            result.push(diagnostic(DiagnosticSeverity::Error, "composition.font.resolve",
                format!("captured font registry cannot resolve an embeddable HTML font set: {error}"), locale, None));
            None
        }
    };
    let page_width = snapshot.manifest.page.width_mm * POINTS_PER_MM;
    let page_height = snapshot.manifest.page.height_mm * POINTS_PER_MM;
    if snapshot.manifest.page.gutter_mm > 0.0 {
        result.push(diagnostic(DiagnosticSeverity::Warning, "composition.page.gutter_unresolved",
            "a nonzero gutter is declared without a binding side; no left/right gutter inset is inferred or applied, and binding clearance requires human review", locale, None));
    }
    for layer in &snapshot.manifest.layers {
        if matches!(&layer.content, LayerContent::Raster { .. }) {
            result.push(diagnostic(DiagnosticSeverity::Warning, "composition.raster.flattened_unverified",
                "raster bytes remain a flattened plate; no editable internal layers are recovered, and image decoding and source color behavior are unverified", locale, Some(&layer.id)));
        }
        let LayerContent::Text { content_id, typography } = &layer.content else { continue };
        let (bounds, font_size) = effective_geometry(snapshot, locale, layer, typography.font_size_pt);
        if !contains(snapshot.manifest.page.safe, bounds) {
            result.push(diagnostic(DiagnosticSeverity::Error, "composition.text.outside_safe_area",
                "effective text bounds extend outside the declared page safe rectangle; move or resize the text container, or explicitly correct the safe rectangle before rendering", locale, Some(&layer.id)));
        }
        if layer.clip.is_some_and(|clip| !contains(clip, bounds)) {
            result.push(diagnostic(DiagnosticSeverity::Error, "composition.text.clip_excludes_bounds",
                "the declared clip rectangle excludes part of the effective text container; expand or remove the clip, or explicitly move or resize the container before rendering", locale, Some(&layer.id)));
        }
        if let Some(fonts) = &fonts {
            match fonts.resolved.get(&typography.role) {
                Some(font) if !matches!(font.style.as_str(), "normal" | "italic" | "oblique") => {
                    result.push(diagnostic(DiagnosticSeverity::Error, "composition.font.style_unsupported",
                        "font styles must be exactly normal, italic, or oblique in this preview profile", locale, Some(&layer.id)));
                }
                Some(font) if snapshot.fonts.captured_asset_bytes(&font.asset_id).is_none() => {
                    result.push(diagnostic(DiagnosticSeverity::Error, "composition.font.snapshot_missing",
                        "the resolved font has no captured bytes", locale, Some(&layer.id)));
                }
                None => result.push(diagnostic(DiagnosticSeverity::Error, "composition.font.role_missing",
                    format!("no captured font resolves the {} role", typography.role), locale, Some(&layer.id))),
                _ => {}
            }
        }
        let Some(text) = content.content.get(content_id) else {
            result.push(diagnostic(DiagnosticSeverity::Error, "composition.content.missing",
                format!("content ID {content_id} is absent from the selected locale"), locale, Some(&layer.id)));
            continue;
        };
        let available_width = bounds.width * page_width - 2.0 * typography.padding_pt;
        let available_height = bounds.height * page_height - 2.0 * typography.padding_pt;
        let line_count = text.split('\n').count();
        let line_height = font_size * typography.line_height;
        let requested_height = line_count as f64 * line_height;
        if available_width <= 0.0 || available_height <= 0.0 {
            result.push(diagnostic(DiagnosticSeverity::Error, "composition.text.padding_exhausts_bounds",
                "text padding leaves no positive content area", locale, Some(&layer.id)));
        } else if requested_height > available_height {
            result.push(diagnostic(DiagnosticSeverity::Error, "composition.text.vertical_capacity",
                format!("{line_count} explicit line boxes require {} pt; the padded container allows {} pt; no automatic shrinking or line removal is performed", number(requested_height), number(available_height)), locale, Some(&layer.id)));
        }
        result.push(diagnostic(DiagnosticSeverity::Warning, "composition.text.layout_unverified",
            "explicit line breaks and font size are retained; horizontal overflow, actual glyph ink bounds, shaping and browser font loading have not been measured", locale, Some(&layer.id)));
        result.push(diagnostic(DiagnosticSeverity::Warning, "composition.font.glyph_coverage_unverified",
            "font bytes and license declarations are pinned; supported glyphs and actual fallback behavior are not verified by this renderer", locale, Some(&layer.id)));
    }
    result
}

/// Serialize a single locale without opening files, probing tools or networking.
pub fn render(snapshot: &LoadedComposition, locale: &str, format: CompositionOutput) -> Result<RenderedComposition> {
    let mut diagnostics = snapshot.diagnostics.iter().filter(|item|
        item.locale.as_deref().is_none_or(|value| value == locale)).cloned().collect::<Vec<_>>();
    if format.visual() { diagnostics.extend(self::diagnostics(snapshot, locale)); }
    if let Some(error) = diagnostics.iter().find(|item| item.severity == DiagnosticSeverity::Error) {
        bail!("{}: {}", error.code, error.message);
    }
    let transcript = transcript(snapshot, locale)?;
    let (bytes, media_type, extension) = match format {
        CompositionOutput::Svg => (render_svg(snapshot, locale, &transcript)?.into_bytes(), "image/svg+xml", "svg"),
        CompositionOutput::Html => (render_html(snapshot, locale, &transcript)?.into_bytes(), "text/html", "html"),
        CompositionOutput::TranscriptJson => {
            let mut bytes = serde_json::to_vec_pretty(&transcript)?;
            bytes.push(b'\n');
            (bytes, "application/json", "transcript.json")
        }
        CompositionOutput::TranscriptText => (render_transcript_text(&transcript).into_bytes(), "text/plain", "transcript.txt"),
        _ => bail!("composition.output.unsupported: this renderer only provides SVG, HTML, transcript JSON, and transcript text"),
    };
    if bytes.len() > MAX_OUTPUT_BYTES {
        bail!("composition.output.bounds: serialized output exceeds 64 MiB");
    }
    Ok(RenderedComposition { bytes, media_type, extension, diagnostics })
}

fn diagnostic(severity: DiagnosticSeverity, code: &str, message: impl Into<String>, locale: &str, layer_id: Option<&str>) -> CompositionDiagnostic {
    CompositionDiagnostic::new(severity, code, message, Some(locale), layer_id)
}

fn effective_geometry(snapshot: &LoadedComposition, locale: &str, layer: &Layer, font_size: f64) -> (NormalizedRect, f64) {
    let mut bounds = layer.bounds;
    let mut font_size = font_size;
    if let Some(overrides) = snapshot.manifest.locale_overrides.get(locale) {
        for item in overrides.iter().filter(|item| item.layer_id == layer.id) {
            if let Some(value) = item.bounds { bounds = value; }
            if let Some(value) = item.font_size_pt { font_size = value; }
        }
    }
    (bounds, font_size)
}

fn contains(outer: NormalizedRect, inner: NormalizedRect) -> bool {
    // Match the loader's geometric tolerance for normalized source coordinates.
    inner.x + 1e-9 >= outer.x && inner.y + 1e-9 >= outer.y
        && inner.x + inner.width <= outer.x + outer.width + 1e-9
        && inner.y + inner.height <= outer.y + outer.height + 1e-9
}

#[derive(Debug, Serialize)]
struct PageTranscript {
    schema: &'static str,
    composition_id: String,
    composition_revision: String,
    locale: String,
    content_revision: String,
    source_revision: String,
    translation_reviewed: bool,
    requires_human_review: bool,
    qualification: &'static str,
    direction: TextDirection,
    writing_mode: WritingMode,
    /// Artwork descriptions retain paint order separately from text reading order.
    artwork: Vec<ArtworkDescription>,
    entries: Vec<TranscriptEntry>,
}

#[derive(Debug, Serialize)]
struct ArtworkDescription {
    layer_id: String,
    asset_id: String,
    alt: String,
}

#[derive(Debug, Serialize)]
struct TranscriptEntry {
    layer_id: String,
    content_id: String,
    text: String,
}

fn selected_content<'a>(snapshot: &'a LoadedComposition, locale: &str) -> Result<&'a PageContent> {
    snapshot.contents.get(locale).map(|item| &item.document)
        .with_context(|| format!("composition.locale.missing: no captured content for {locale}"))
}

fn transcript(snapshot: &LoadedComposition, locale: &str) -> Result<PageTranscript> {
    let content = selected_content(snapshot, locale)?;
    let mut artwork = Vec::new();
    let layers = snapshot.manifest.layers.iter().map(|item| (item.id.as_str(), item)).collect::<BTreeMap<_, _>>();
    for layer in &snapshot.manifest.layers {
        let asset_id = match &layer.content {
            LayerContent::Raster { asset_id } | LayerContent::Vector { asset_id } => asset_id,
            _ => continue,
        };
        let asset = snapshot.assets.get(asset_id)
            .with_context(|| format!("composition.asset.missing: no captured asset {asset_id}"))?;
        artwork.push(ArtworkDescription { layer_id: layer.id.clone(), asset_id: asset_id.clone(), alt: asset.declaration.alt.clone() });
    }
    let mut entries = Vec::new();
    for layer_id in &snapshot.manifest.reading_order {
        let layer = layers.get(layer_id.as_str()).context("composition.reading_order.missing: layer is absent")?;
        let LayerContent::Text { content_id, .. } = &layer.content else {
            bail!("composition.reading_order.kind: reading order must contain text layers only");
        };
        let text = content.content.get(content_id)
            .with_context(|| format!("composition.content.missing: no text for {content_id}"))?;
        entries.push(TranscriptEntry { layer_id: layer_id.clone(), content_id: content_id.clone(), text: text.clone() });
    }
    Ok(PageTranscript {
        schema: PAGE_TRANSCRIPT_SCHEMA_V1,
        composition_id: snapshot.manifest.composition_id.clone(),
        composition_revision: snapshot.manifest.revision.clone(),
        locale: locale.to_owned(),
        content_revision: content.revision.clone(),
        source_revision: content.source_revision.clone(),
        translation_reviewed: content.reviewed,
        requires_human_review: true,
        qualification: "unqualified",
        direction: content.direction,
        writing_mode: content.writing_mode,
        artwork,
        entries,
    })
}

fn render_svg(snapshot: &LoadedComposition, locale: &str, transcript: &PageTranscript) -> Result<String> {
    let fonts = snapshot.fonts.resolve(FontTarget::Html)?;
    let page = &snapshot.manifest.page;
    let width = page.width_mm * POINTS_PER_MM;
    let height = page.height_mm * POINTS_PER_MM;
    let mut output = String::new();
    writeln!(output, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\" role=\"img\" aria-labelledby=\"rf-title rf-description\" xml:lang=\"{}\">",
        number(page.width_mm), number(page.height_mm), number(width), number(height), escape_xml(locale))?;
    writeln!(output, "<title id=\"rf-title\">{}</title>", escape_xml(&snapshot.manifest.composition_id))?;
    writeln!(output, "<desc id=\"rf-description\">{}</desc>", escape_xml(&accessible_description(transcript)))?;
    writeln!(output, "<metadata>Renderer: {COMPOSITION_RENDERER_ID}/{COMPOSITION_RENDERER_VERSION}; qualification: unqualified; human review required; explicit lines, no glyph or ink-bounds verification.</metadata>")?;
    output.push_str("<defs>\n<style>\n");
    write_font_faces(&mut output, snapshot, &fonts)?;
    output.push_str("</style>\n");
    for (index, layer) in snapshot.manifest.layers.iter().enumerate() {
        if let Some(clip) = layer.clip {
            writeln!(output, "<clipPath id=\"rf-clip-{index}\" clipPathUnits=\"userSpaceOnUse\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                number(clip.x * width), number(clip.y * height), number(clip.width * width), number(clip.height * height))?;
        }
    }
    output.push_str("</defs>\n<g aria-hidden=\"true\">\n");
    for (index, layer) in snapshot.manifest.layers.iter().enumerate() {
        let font_size = match &layer.content { LayerContent::Text { typography, .. } => typography.font_size_pt, _ => 0.0 };
        let (bounds, font_size) = effective_geometry(snapshot, locale, layer, font_size);
        let (x, y, layer_width, layer_height) = (bounds.x * width, bounds.y * height, bounds.width * width, bounds.height * height);
        write!(output, "<g data-layer-id=\"{}\" opacity=\"{}\"", escape_xml(&layer.id), number(layer.opacity))?;
        if layer.clip.is_some() { write!(output, " clip-path=\"url(#rf-clip-{index})\"")?; }
        output.push_str(">\n");
        match &layer.content {
            LayerContent::Raster { asset_id } | LayerContent::Vector { asset_id } => {
                let asset = snapshot.assets.get(asset_id).context("composition.asset.missing: asset absent from snapshot")?;
                // Vector bytes have already passed the loader's inert SVG whitelist.
                // Nesting them as an image also isolates their internal IDs/styles.
                write!(output, "<image data-asset-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"data:{};base64,",
                    escape_xml(asset_id), number(x), number(y), number(layer_width), number(layer_height), asset.declaration.format.media_type())?;
                append_base64(&mut output, &asset.bytes)?;
                output.push_str("\"/>\n");
            }
            LayerContent::Shape { shape, fill, stroke, stroke_width_pt } => {
                match shape {
                    ShapeKind::Rect => write!(output, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"", number(x), number(y), number(layer_width), number(layer_height))?,
                    ShapeKind::Ellipse => write!(output, "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"", number(x + layer_width / 2.0), number(y + layer_height / 2.0), number(layer_width / 2.0), number(layer_height / 2.0))?,
                }
                writeln!(output, " fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>", escape_xml(fill), escape_xml(stroke.as_deref().unwrap_or("none")), number(*stroke_width_pt))?;
            }
            LayerContent::Text { content_id, typography } => {
                let font = fonts.resolved.get(&typography.role).context("composition.font.role_missing: unresolved text role")?;
                let content = selected_content(snapshot, locale)?;
                let text = content.content.get(content_id).context("composition.content.missing: text absent from snapshot")?;
                let (text_x, anchor) = match typography.alignment {
                    TextAlignment::Start => (x + typography.padding_pt, "start"),
                    TextAlignment::Center => (x + layer_width / 2.0, "middle"),
                    TextAlignment::End => (x + layer_width - typography.padding_pt, "end"),
                };
                write!(output, "<text data-content-id=\"{}\" font-family=\"{}\" font-size=\"{}\" font-style=\"{}\" font-weight=\"{}\" font-kerning=\"none\" text-anchor=\"{}\" fill=\"{}\" xml:space=\"preserve\" direction=\"ltr\">",
                    escape_xml(content_id), font_family(font), number(font_size), escape_xml(&font.style), font.weight, anchor, escape_xml(&typography.color))?;
                for (line_index, line) in text.split('\n').enumerate() {
                    let baseline = y + typography.padding_pt + font_size + line_index as f64 * font_size * typography.line_height;
                    write!(output, "<tspan x=\"{}\" y=\"{}\">{}</tspan>", number(text_x), number(baseline), escape_xml(line))?;
                }
                output.push_str("</text>\n");
            }
        }
        output.push_str("</g>\n");
        check_output_bound(output.len())?;
    }
    output.push_str("</g>\n</svg>\n");
    check_output_bound(output.len())?;
    Ok(output)
}

fn write_font_faces(output: &mut String, snapshot: &LoadedComposition, fonts: &FontResolutionReport) -> Result<()> {
    let used_roles = snapshot.manifest.layers.iter().filter_map(|layer| match &layer.content {
        LayerContent::Text { typography, .. } => Some(typography.role),
        _ => None,
    }).collect::<BTreeSet<FontRole>>();
    let mut written = BTreeSet::new();
    for role in used_roles {
        let font = fonts.resolved.get(&role).context("composition.font.role_missing: unresolved text role")?;
        if !written.insert((font.digest.value.clone(), font.weight, font.style.clone())) { continue; }
        let bytes = snapshot.fonts.captured_asset_bytes(&font.asset_id)
            .context("composition.font.snapshot_missing: resolved font bytes not captured")?;
        write!(output, "@font-face{{font-family:\"{}\";src:url(\"data:{};base64,", font_family(font), font.format.media_type())?;
        append_base64(output, bytes)?;
        writeln!(output, "\") format(\"{}\");font-style:{};font-weight:{};font-display:block;}}", font_format_label(font.format), font.style, font.weight)?;
    }
    Ok(())
}

fn font_family(font: &ResolvedFont) -> String {
    // The loader verifies this SHA-256 digest; no declared family name becomes CSS.
    format!("rf_font_{}", font.digest.value)
}

fn font_format_label(format: FontFormat) -> &'static str {
    match format { FontFormat::Ttf => "truetype", FontFormat::Otf => "opentype", FontFormat::Woff => "woff", FontFormat::Woff2 => "woff2" }
}

fn append_base64(output: &mut String, bytes: &[u8]) -> Result<()> {
    let encoded_len = bytes.len().checked_add(2).and_then(|value| value.checked_div(3))
        .and_then(|value| value.checked_mul(4)).context("composition.output.bounds: encoded resource size overflow")?;
    let length = output.len().checked_add(encoded_len).context("composition.output.bounds: output size overflow")?;
    check_output_bound(length)?;
    STANDARD.encode_string(bytes, output);
    Ok(())
}

fn check_output_bound(length: usize) -> Result<()> {
    if length > MAX_OUTPUT_BYTES { bail!("composition.output.bounds: serialized output exceeds 64 MiB"); }
    Ok(())
}

fn accessible_description(transcript: &PageTranscript) -> String {
    let mut description = String::new();
    for artwork in &transcript.artwork {
        if !artwork.alt.is_empty() {
            if !description.is_empty() { description.push('\n'); }
            description.push_str(&artwork.alt);
        }
    }
    for entry in &transcript.entries {
        if !description.is_empty() { description.push('\n'); }
        description.push_str(&entry.text);
    }
    description
}

fn render_html(snapshot: &LoadedComposition, locale: &str, transcript: &PageTranscript) -> Result<String> {
    let svg = render_svg(snapshot, locale, transcript)?;
    let mut output = String::new();
    writeln!(output, "<!doctype html>\n<html lang=\"{}\" dir=\"ltr\">\n<head>\n<meta charset=\"utf-8\">", escape_xml(locale))?;
    output.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; font-src data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\">\n");
    writeln!(output, "<title>{}</title>", escape_xml(&transcript.composition_id))?;
    output.push_str("<style>body{margin:0;padding:2rem;background:#eee;color:#111;font-family:system-ui,sans-serif}main{max-width:72rem;margin:auto}figure{margin:0}figure>svg{display:block;max-width:100%;height:auto;background:white}li{margin-block:1rem}.composition-text{white-space:pre-wrap;overflow-wrap:anywhere}.qualification{border:1px solid #666;padding:1rem}</style>\n</head>\n<body><main>\n");
    writeln!(output, "<h1>{}</h1>", escape_xml(&transcript.composition_id))?;
    output.push_str("<p class=\"qualification\">Preview requires human review. Font loading, glyph coverage, horizontal overflow and actual ink bounds are unverified. No automatic line wrapping or shrinking was applied to the artwork.</p>\n<figure aria-hidden=\"true\">\n");
    output.push_str(&svg);
    output.push_str("</figure>\n<section aria-labelledby=\"artwork-heading\"><h2 id=\"artwork-heading\">Artwork descriptions</h2><ol>\n");
    for artwork in &transcript.artwork {
        writeln!(output, "<li data-layer-id=\"{}\">{}</li>", escape_xml(&artwork.layer_id), escape_xml(&artwork.alt))?;
    }
    output.push_str("</ol></section>\n<section aria-labelledby=\"transcript-heading\"><h2 id=\"transcript-heading\">Text in reading order</h2><ol>\n");
    for entry in &transcript.entries {
        writeln!(output, "<li data-layer-id=\"{}\" data-content-id=\"{}\"><div class=\"composition-text\">{}</div></li>", escape_xml(&entry.layer_id), escape_xml(&entry.content_id), escape_xml(&entry.text))?;
    }
    output.push_str("</ol></section>\n</main></body></html>\n");
    check_output_bound(output.len())?;
    Ok(output)
}

fn render_transcript_text(transcript: &PageTranscript) -> String {
    let mut output = format!("{}\nLocale: {}\nSource revision: {}\nQualification: unqualified; human review required.\n\nArtwork descriptions (paint order):\n", transcript.composition_id, transcript.locale, transcript.source_revision);
    for artwork in &transcript.artwork {
        output.push_str(&format!("[{} / {}]\n{}\n\n", artwork.layer_id, artwork.asset_id, artwork.alt));
    }
    output.push_str("Text (reading order):\n");
    for entry in &transcript.entries {
        output.push_str(&format!("[{} / {}]\n{}\n\n", entry.layer_id, entry.content_id, entry.text));
    }
    output
}

fn number(value: f64) -> String {
    // Shortest round-tripping decimal conversion; no locale-dependent formatting.
    // Normalize negative zero because it has no geometric meaning here.
    if value == 0.0 { "0".to_owned() } else { value.to_string() }
}

fn escape_xml(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        match character {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(character),
        }
    }
    result
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
