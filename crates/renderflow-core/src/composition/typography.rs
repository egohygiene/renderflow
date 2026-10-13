//! Captured-byte typography for the deliberately narrow Latin composition profile.
//!
//! The same unhinted outlines supply both conservative control-point bounds and
//! the renderer's paths. No browser fonts, system fonts, wrapping, source-content
//! rewriting, shrinking, synthetic styles, or per-character font fallback are
//! involved. The shaping provider may normalize clusters internally; retained
//! source text and its UTF-8 cluster references are not rewritten.
//! Runtime and visual qualification are deliberately separate from these authored
//! measurements. A successful report is not approval of a translated page.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::evidence::DigestEvidence;
use crate::font::{FontFormat, FontRole, FontTarget, ResolvedFont};

use super::model::{
    CompositionDiagnostic, DiagnosticSeverity, LayerContent, TextAlignment,
    TextDirection, WritingMode,
};
use super::typography_provider::{self, ProviderLine};
use super::LoadedComposition;

pub const TYPOGRAPHY_SCHEMA_V1: &str = "renderflow.composition-typography/v1";
pub const TYPOGRAPHY_PROVIDER_PROFILE: &str = "renderflow.latin-static-outlines/v1;harfrust=0.14.0;skrifa=0.48.0;script=latn;direction=ltr;features=default;hinting=none;variation=none;bounds=control-point-hull";
const POINTS_PER_MM: f64 = 72.0 / 25.4;
const MAX_GLYPHS: usize = 4096;
const MAX_LINES: usize = 256;
const MAX_OUTLINE_BYTES: usize = 4 * 1024 * 1024;
const MAX_REPORT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CANDIDATE_ATTEMPTS: usize = 128;
const MAX_FONT_WORK_BYTES: usize = 128 * 1024 * 1024;
const GEOMETRY_TOLERANCE: f64 = 1e-6;

/// Portable measurement evidence. Private paths are intentionally absent from
/// JSON, while each layer's digest commits to their precise positions and bytes.
/// A deserialized report has no renderable geometry and is evidence only.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyReport {
    pub schema: String,
    pub provider_profile: String,
    pub locale: String,
    pub layers: Vec<TypographyLayer>,
    pub diagnostics: Vec<CompositionDiagnostic>,
    pub runtime_qualified: bool,
    #[serde(skip)]
    outline_paths: Arc<BTreeMap<String, Vec<PositionedOutline>>>,
}

impl TypographyReport {
    /// Only reports measured in this process retain drawable paths.
    pub(crate) fn outlines(&self, layer_id: &str) -> Option<&[PositionedOutline]> {
        self.outline_paths.get(layer_id).map(Vec::as_slice)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyLayer {
    pub layer_id: String,
    pub content_id: String,
    pub role: FontRole,
    pub font_size_pt: f64,
    pub selected_font: Option<TypographyFont>,
    pub rejected_candidates: Vec<RejectedCandidate>,
    pub lines: Vec<TypographyLine>,
    pub outline_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyFont {
    pub asset_id: String,
    pub family: String,
    pub format: FontFormat,
    pub digest: DigestEvidence,
    /// Original declared role order, even though candidates are resolved alone.
    pub fallback_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RejectedCandidate {
    pub asset_id: String,
    pub fallback_index: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyLine {
    pub index: usize,
    pub source_byte_start: usize,
    pub source_byte_end: usize,
    pub baseline_y_pt: f64,
    pub origin_x_pt: f64,
    pub advance_pt: f64,
    pub conservative_outline_bounds: Option<InkBounds>,
    pub glyphs: Vec<TypographyGlyph>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyGlyph {
    pub glyph_id: u32,
    /// UTF-8 byte index in the complete layer text, never a character index.
    pub cluster_utf8: usize,
    pub x_pt: f64,
    pub y_pt: f64,
    pub x_advance_pt: f64,
    pub y_advance_pt: f64,
    pub conservative_outline_bounds: Option<InkBounds>,
}

/// Page-point bounds with a downward y axis. Bezier control points are included,
/// so the result may overestimate the visible curve but must not underestimate it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InkBounds {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}

impl InkBounds {
    fn union(self, other: Self) -> Self {
        Self {
            x_min: self.x_min.min(other.x_min),
            y_min: self.y_min.min(other.y_min),
            x_max: self.x_max.max(other.x_max),
            y_max: self.y_max.max(other.y_max),
        }
    }

    fn finite(self) -> bool {
        [self.x_min, self.y_min, self.x_max, self.y_max]
            .into_iter().all(f64::is_finite)
    }
}

/// Font-unit path, transformed with `translate(x_pt,y_pt) scale(scale,-scale)`.
/// The positive scale converts font units to page points; the minus sign flips
/// the font's upward y axis into the SVG page's downward y axis.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PositionedOutline {
    pub glyph_id: u32,
    pub path: String,
    pub x_pt: f64,
    pub y_pt: f64,
    pub scale: f64,
}

/// Measure one locale from captured inputs; no filesystem or process access.
///
/// Only static, single-face sfnt fonts and the documented Latin character set
/// are accepted. A candidate must cover and shape the entire text layer, so a
/// combining cluster can never be split across primary and fallback fonts.
pub fn measure(snapshot: &LoadedComposition, locale: &str) -> TypographyReport {
    let mut report = TypographyReport {
        schema: TYPOGRAPHY_SCHEMA_V1.to_owned(),
        provider_profile: TYPOGRAPHY_PROVIDER_PROFILE.to_owned(),
        locale: locale.to_owned(),
        layers: Vec::new(),
        diagnostics: Vec::new(),
        runtime_qualified: false,
        outline_paths: Arc::new(BTreeMap::new()),
    };
    let Some(content) = snapshot.contents.get(locale).map(|entry| &entry.document) else {
        push_error(&mut report, "composition.locale.missing", "the selected locale has no captured content document", None);
        return report;
    };
    if content.direction != TextDirection::Ltr || content.writing_mode != WritingMode::HorizontalTb {
        push_error(&mut report, "composition.layout.unsupported", "measured outlines support horizontal left-to-right Latin text only; bidi and vertical layout remain unavailable", None);
        return report;
    }
    let mut paths = BTreeMap::new();
    let mut glyph_count = 0usize;
    let mut outline_bytes = 0usize;
    let mut character_count = 0usize;
    let mut line_count = 0usize;
    let mut candidate_attempts = 0usize;
    let mut remaining_font_work = MAX_FONT_WORK_BYTES;
    let mut policy_cache = BTreeMap::<(FontRole, String), Result<ResolvedFont, String>>::new();
    // One snapshot clone per locale. Candidate isolation below restricts
    // validation to one captured asset, and cached policy outcomes prevent
    // hashing that asset again for every text layer using the same role.
    let mut isolated = snapshot.fonts.clone();
    isolated.registry.roles.clear();
    let page_width = snapshot.manifest.page.width_mm * POINTS_PER_MM;
    let page_height = snapshot.manifest.page.height_mm * POINTS_PER_MM;

    'layers: for layer in &snapshot.manifest.layers {
        let LayerContent::Text { content_id, typography } = &layer.content else { continue };
        let (bounds, size) = super::render::effective_geometry(snapshot, locale, layer, typography.font_size_pt);
        let mut evidence = TypographyLayer {
            layer_id: layer.id.clone(), content_id: content_id.clone(), role: typography.role,
            font_size_pt: size, selected_font: None, rejected_candidates: Vec::new(),
            lines: Vec::new(), outline_digest: None,
        };
        let Some(text) = content.content.get(content_id) else {
            push_error(&mut report, "composition.content.missing", format!("content ID {content_id} is absent from the selected locale"), Some(&layer.id));
            report.layers.push(evidence);
            continue;
        };
        character_count = character_count.saturating_add(text.chars().count());
        line_count = line_count.saturating_add(text.split('\n').count());
        if character_count > MAX_GLYPHS || line_count > MAX_LINES {
            push_error(&mut report, "composition.typography.text_bounds", "a locale may contain at most 4096 text code points and 256 authored lines in this measurement profile", Some(&layer.id));
            report.layers.push(evidence);
            break;
        }
        if let Some((code, reason)) = text_scope_error(text) {
            push_error(&mut report, code, reason, Some(&layer.id));
            report.layers.push(evidence);
            continue;
        }
        let inner = InkBounds {
            x_min: bounds.x * page_width + typography.padding_pt,
            y_min: bounds.y * page_height + typography.padding_pt,
            x_max: (bounds.x + bounds.width) * page_width - typography.padding_pt,
            y_max: (bounds.y + bounds.height) * page_height - typography.padding_pt,
        };
        let line_height = size * typography.line_height;
        if !inner.finite() || !size.is_finite() || size <= 0.0 || !line_height.is_finite()
            || line_height <= 0.0 || inner.x_max <= inner.x_min || inner.y_max <= inner.y_min {
            push_error(&mut report, "composition.text.padding_exhausts_bounds", "effective text geometry must leave a finite positive content area and font size", Some(&layer.id));
            report.layers.push(evidence);
            continue;
        }
        let Some(binding) = snapshot.fonts.registry.roles.get(&typography.role) else {
            push_error(&mut report, "composition.typography.font_unavailable", "the text layer's semantic role has no declared font binding", Some(&layer.id));
            report.layers.push(evidence);
            continue;
        };

        let mut selected = None;
        for (index, id) in std::iter::once(&binding.primary).chain(binding.fallbacks.iter()).enumerate() {
            candidate_attempts += 1;
            if candidate_attempts > MAX_CANDIDATE_ATTEMPTS {
                push_error(&mut report, "composition.typography.budget", "a locale may attempt at most 128 whole-layer font candidates; simplify the page or fallback declarations", Some(&layer.id));
                report.layers.push(evidence);
                break 'layers;
            }
            let candidate = (|| -> anyhow::Result<(ResolvedFont, Vec<ProviderLine>)> {
                let bytes = snapshot.fonts.captured_asset_bytes(id)
                    .ok_or_else(|| anyhow::anyhow!("composition.typography.snapshot_missing: selected font has no captured bytes"))?;
                let key = (typography.role, id.clone());
                if !policy_cache.contains_key(&key) {
                    charge_font_work(&mut remaining_font_work, bytes.len().saturating_mul(2))?;
                    let mut candidate_binding = binding.clone();
                    candidate_binding.primary = id.clone();
                    candidate_binding.fallbacks.clear();
                    isolated.registry.roles.clear();
                    isolated.registry.roles.insert(typography.role, candidate_binding);
                    isolated.registry.assets = snapshot.fonts.registry.assets.iter()
                        .filter(|asset| asset.id == *id).cloned().collect();
                    let resolved = isolated.resolve(FontTarget::Html)
                        .and_then(|mut resolved| resolved.resolved.remove(&typography.role)
                            .ok_or_else(|| anyhow::anyhow!("composition.typography.role_missing: isolated role was not resolved")))
                        .map_err(|error| match isolated.registry.assets.first() {
                            Some(asset) => format!(
                                "{error}; candidate declares style={}, weight={}, embedding={:?}, redistribution={:?}; this role requires style={}, weight={}, embedding=Allowed, redistribution=Allowed",
                                asset.style, asset.weight, asset.license.embedding, asset.license.redistribution,
                                binding.style, binding.weight),
                            None => error.to_string(),
                        });
                    policy_cache.insert(key.clone(), resolved);
                }
                let font = policy_cache.get(&key)
                    .ok_or_else(|| anyhow::anyhow!("composition.typography.policy_missing: candidate policy outcome is unavailable"))?
                    .clone().map_err(anyhow::Error::msg)?;
                if !matches!(font.format, FontFormat::Ttf | FontFormat::Otf) {
                    anyhow::bail!("composition.typography.font_format: measured outlines require a static TTF or OTF; WOFF and WOFF2 are not decoded");
                }
                let lines = shape_candidate(bytes, text, locale, MAX_GLYPHS - glyph_count,
                    MAX_OUTLINE_BYTES - outline_bytes, &mut remaining_font_work)?;
                Ok((font, lines))
            })();
            match candidate {
                Ok((font, lines)) => { selected = Some((font, lines, index)); break; }
                Err(error) => evidence.rejected_candidates.push(RejectedCandidate {
                    asset_id: id.clone(), fallback_index: index,
                    reason: error.to_string().chars().take(512).collect(),
                }),
            }
        }
        let Some((font, measured_lines, fallback_index)) = selected else {
            push_error(&mut report, "composition.typography.font_unavailable", "no declared font candidate can safely shape this complete text layer; inspect rejected_candidates for policy, font-format, glyph-coverage, or resource-limit reasons", Some(&layer.id));
            report.layers.push(evidence);
            if report_too_large(&report) { truncate_report(&mut report); paths.clear(); break; }
            continue;
        };
        let mut font_digest = font.digest;
        font_digest.value.make_ascii_lowercase();
        evidence.selected_font = Some(TypographyFont {
            asset_id: font.asset_id, family: font.family, format: font.format,
            digest: font_digest, fallback_index,
        });
        if fallback_index > 0 {
            report.diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Warning,
                "composition.typography.fallback_selected",
                format!("declared fallback at index {fallback_index} was selected for the entire text layer; no combining cluster was divided between fonts"),
                Some(locale), Some(&layer.id)));
        }
        let mut layer_paths = Vec::new();
        let mut byte_start = 0usize;
        let mut horizontal_overflow = false;
        let mut vertical_overflow = measured_lines.len() as f64 * line_height > inner.y_max - inner.y_min + GEOMETRY_TOLERANCE;
        let mut geometry_invalid = false;

        for (index, (source_line, measured)) in text.split('\n').zip(measured_lines).enumerate() {
            let scale = size / measured.units_per_em;
            let advance_pt = measured.glyphs.iter().map(|glyph| glyph.x_advance).sum::<f64>() * scale;
            let natural_height = (measured.ascender - measured.descender) * scale;
            let baseline_y = inner.y_min + index as f64 * line_height
                + (line_height - natural_height) / 2.0 + measured.ascender * scale;
            let spare_width = inner.x_max - inner.x_min - advance_pt;
            let origin_x = inner.x_min + match typography.alignment {
                TextAlignment::Start => 0.0,
                TextAlignment::Center => spare_width / 2.0,
                TextAlignment::End => spare_width,
            };
            if ![scale, advance_pt, natural_height, baseline_y, origin_x].into_iter().all(f64::is_finite)
                || scale <= 0.0 || advance_pt < 0.0 {
                geometry_invalid = true;
                break;
            }
            horizontal_overflow |= advance_pt > inner.x_max - inner.x_min + GEOMETRY_TOLERANCE;
            let mut line = TypographyLine {
                index, source_byte_start: byte_start, source_byte_end: byte_start + source_line.len(),
                baseline_y_pt: baseline_y, origin_x_pt: origin_x, advance_pt,
                conservative_outline_bounds: None, glyphs: Vec::new(),
            };
            let mut cursor_x = 0.0;
            let mut cursor_y = 0.0;
            for glyph in measured.glyphs {
                let x = origin_x + (cursor_x + glyph.x_offset) * scale;
                let y = baseline_y - (cursor_y + glyph.y_offset) * scale;
                let ink = glyph.bounds.map(|value| InkBounds {
                    x_min: x + value.x_min * scale, y_min: y - value.y_max * scale,
                    x_max: x + value.x_max * scale, y_max: y - value.y_min * scale,
                });
                let cluster = glyph.cluster_utf8 as usize;
                if ![x, y, glyph.x_advance * scale, glyph.y_advance * scale].into_iter().all(f64::is_finite)
                    || ink.is_some_and(|value| !value.finite())
                    || cluster >= source_line.len() || !source_line.is_char_boundary(cluster) {
                    geometry_invalid = true;
                    break;
                }
                if let Some(ink) = ink {
                    horizontal_overflow |= ink.x_min < inner.x_min - GEOMETRY_TOLERANCE
                        || ink.x_max > inner.x_max + GEOMETRY_TOLERANCE;
                    vertical_overflow |= ink.y_min < inner.y_min - GEOMETRY_TOLERANCE
                        || ink.y_max > inner.y_max + GEOMETRY_TOLERANCE;
                    line.conservative_outline_bounds = Some(match line.conservative_outline_bounds {
                        Some(existing) => existing.union(ink), None => ink,
                    });
                }
                line.glyphs.push(TypographyGlyph {
                    glyph_id: glyph.glyph_id, cluster_utf8: byte_start + cluster,
                    x_pt: x, y_pt: y, x_advance_pt: glyph.x_advance * scale,
                    y_advance_pt: -glyph.y_advance * scale,
                    conservative_outline_bounds: ink,
                });
                glyph_count += 1;
                outline_bytes += glyph.path.len();
                if !glyph.path.is_empty() {
                    layer_paths.push(PositionedOutline {
                        glyph_id: glyph.glyph_id, path: glyph.path, x_pt: x, y_pt: y, scale,
                    });
                }
                cursor_x += glyph.x_advance;
                cursor_y += glyph.y_advance;
            }
            if geometry_invalid { break; }
            evidence.lines.push(line);
            byte_start += source_line.len() + 1;
        }
        if geometry_invalid {
            push_error(&mut report, "composition.typography.invalid_geometry", "shaping produced invalid, nonfinite, or out-of-range metrics or UTF-8 clusters; this layer cannot be published", Some(&layer.id));
            evidence.lines.clear();
            layer_paths.clear();
        } else {
            if horizontal_overflow {
                push_error(&mut report, "composition.typography.horizontal_overflow", "a shaped advance or conservative outline bound exceeds the padded text container; author a new line break, size, or locale override; no automatic wrapping or shrinking is performed", Some(&layer.id));
            }
            if vertical_overflow {
                push_error(&mut report, "composition.typography.vertical_overflow", "authored line boxes or conservative outline bounds exceed the padded text container; author a new layout or size before visual publication", Some(&layer.id));
            }
            match serde_json::to_vec(&layer_paths) {
                Ok(bytes) => {
                    evidence.outline_digest = Some(format!("{:x}", Sha256::digest(bytes)));
                    paths.insert(layer.id.clone(), layer_paths);
                }
                Err(_) => push_error(&mut report, "composition.typography.invalid_geometry", "outline evidence could not be serialized as finite canonical geometry", Some(&layer.id)),
            }
        }
        report.layers.push(evidence);
        if report_too_large(&report) { truncate_report(&mut report); paths.clear(); break; }
    }
    if report_too_large(&report) { truncate_report(&mut report); paths.clear(); }
    report.outline_paths = Arc::new(paths);
    report
}

fn shape_candidate(bytes: &[u8], text: &str, locale: &str, remaining_glyphs: usize, remaining_paths: usize, remaining_font_work: &mut usize) -> anyhow::Result<Vec<ProviderLine>> {
    let mut lines = Vec::new();
    let mut glyphs = 0usize;
    let mut paths = 0usize;
    for line in text.split('\n') {
        charge_font_work(remaining_font_work, bytes.len())?;
        let measured = typography_provider::shape_line(bytes, line, locale)?;
        if ![measured.units_per_em, measured.ascender, measured.descender].into_iter().all(f64::is_finite)
            || measured.units_per_em <= 0.0 || measured.ascender <= measured.descender {
            anyhow::bail!("composition.typography.invalid_metrics: font metrics must be finite with positive units-per-em and ascent above descent");
        }
        glyphs = glyphs.saturating_add(measured.glyphs.len());
        paths = measured.glyphs.iter().fold(paths, |count, glyph| count.saturating_add(glyph.path.len()));
        if glyphs > remaining_glyphs || paths > remaining_paths {
            anyhow::bail!("composition.typography.budget: a locale may contain at most 4096 shaped glyphs and 4 MiB of outline paths");
        }
        lines.push(measured);
    }
    Ok(lines)
}

fn charge_font_work(remaining: &mut usize, bytes: usize) -> anyhow::Result<()> {
    if bytes > *remaining {
        anyhow::bail!("composition.typography.budget: cumulative font bytes hashed or parsed exceed the 128 MiB per-locale work budget");
    }
    *remaining -= bytes;
    Ok(())
}

fn text_scope_error(text: &str) -> Option<(&'static str, String)> {
    for line in text.split('\n') {
        if line.chars().next().is_some_and(is_combining_mark) {
            return Some(("composition.typography.unsafe_line_start",
                "an authored line starts with a combining mark; retain the complete base-and-mark cluster on one line".to_owned()));
        }
        for character in line.chars() {
            if !supported_character(character) {
                return Some(("composition.typography.unsupported_character", format!(
                    "U+{:04X} is outside the bounded Latin profile; control characters, soft hyphens, bidi controls, joiners, variation selectors, non-Latin scripts and emoji require another shaping profile", character as u32)));
            }
        }
    }
    None
}

fn is_combining_mark(character: char) -> bool {
    matches!(character, '\u{0300}'..='\u{036f}')
}

/// Explicit code-point scope, independent of the host Unicode or locale data.
/// Common punctuation and spacing are accepted; this is not a Unicode-wide
/// script itemizer, source-content normalization layer, line breaker, or language
/// detector. The shaping provider may normalize admitted clusters internally.
fn supported_character(character: char) -> bool {
    character != '\u{034f}' && matches!(character,
        '\u{0020}'..='\u{007e}' | '\u{00a0}'..='\u{00ac}' | '\u{00ae}'..='\u{024f}'
        | '\u{0300}'..='\u{036f}' | '\u{1e00}'..='\u{1eff}'
        | '\u{2000}'..='\u{200a}' | '\u{2010}'..='\u{2027}' | '\u{202f}'..='\u{205f}'
        | '\u{20ac}' | '\u{2122}')
}

fn push_error(report: &mut TypographyReport, code: &str, message: impl Into<String>, layer_id: Option<&str>) {
    report.diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
        code, message, Some(&report.locale), layer_id));
}

fn report_too_large(report: &TypographyReport) -> bool {
    serde_json::to_vec(report).map_or(true, |bytes| bytes.len() > MAX_REPORT_BYTES)
}

fn truncate_report(report: &mut TypographyReport) {
    report.layers.clear();
    report.diagnostics.clear();
    push_error(report, "composition.typography.budget", "typography evidence exceeds the 2 MiB canonical report limit; split the page or simplify the declared font/layout choices", None);
}
