//! Captured-font shaping and unhinted outline extraction for composition.
//!
//! The caller admits the supported script and chooses a complete font for a
//! text layer. This module never consults installed fonts or performs I/O.

use std::{collections::BTreeSet, fmt::Write};

use anyhow::{anyhow, bail, ensure, Context, Result};
use skrifa::{
    instance::{LocationRef, Size},
    outline::{DrawSettings, Hinting, OutlinePen},
    raw::TableProvider,
    FontRef, GlyphId, MetadataProvider,
};

const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;
const MAX_LINE_GLYPHS: usize = 4096;
const MAX_PATH_BYTES: usize = 4 * 1024 * 1024;
const MAX_PATH_COMMANDS: usize = 131_072;
const MAX_DRAW_MEMORY: usize = 8 * 1024 * 1024;
const MAX_COORDINATE: f64 = 1_000_000.0;

/// Conservative control-point hull, in unscaled font units with positive Y up.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProviderBounds {
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}

/// A shaped glyph and the exact unhinted outline used for output.
#[derive(Debug, Clone)]
pub(crate) struct ProviderGlyph {
    pub glyph_id: u32,
    pub cluster_utf8: u32,
    pub x_advance: f64,
    pub y_advance: f64,
    pub x_offset: f64,
    pub y_offset: f64,
    pub path: String,
    pub bounds: Option<ProviderBounds>,
}

/// One authored line. All measurements and path coordinates are font units.
#[derive(Debug, Clone)]
pub(crate) struct ProviderLine {
    pub glyphs: Vec<ProviderGlyph>,
    pub ascender: f64,
    pub descender: f64,
    pub units_per_em: f64,
}

/// Shape one admitted Latin line and capture the same glyph outlines measured.
///
/// Failures include malformed/unsupported fonts, missing glyphs, missing ink for
/// non-whitespace clusters, and resource bounds. This is not a complete font
/// validator; only tables and outlines needed for this request are parsed.
pub(crate) fn shape_line(bytes: &[u8], text: &str, locale: &str) -> Result<ProviderLine> {
    validate_sfnt(bytes)?;
    ensure!(text.chars().count() <= MAX_LINE_GLYPHS, "typography_resource_limit: authored line exceeds 4096 characters");
    ensure!(!text.chars().any(char::is_control), "typography_unsupported_text: line contains a control character");
    let face = FontRef::new(bytes).context("typography_invalid_font: invalid SFNT face")?;
    face.head().context("typography_invalid_font: invalid head table")?;
    face.hhea().context("typography_invalid_font: invalid hhea table")?;
    face.hmtx().context("typography_invalid_font: invalid hmtx table")?;
    face.maxp().context("typography_invalid_font: invalid maxp table")?;
    face.cmap().context("typography_invalid_font: invalid cmap table")?;
    // Do not silently downgrade malformed top-level OpenType layout tables to
    // an unshaped font. The provider still parses nested lookup data on demand.
    if face.table_data(skrifa::Tag::new(b"GSUB")).is_some() {
        face.gsub().context("typography_invalid_font: invalid GSUB table")?;
    }
    if face.table_data(skrifa::Tag::new(b"GPOS")).is_some() {
        face.gpos().context("typography_invalid_font: invalid GPOS table")?;
    }
    if face.table_data(skrifa::Tag::new(b"GDEF")).is_some() {
        face.gdef().context("typography_invalid_font: invalid GDEF table")?;
    }
    if face.table_data(skrifa::Tag::new(b"kern")).is_some() {
        face.kern().context("typography_invalid_font: invalid kern table")?;
    }
    let metrics = face.metrics(Size::unscaled(), LocationRef::default());
    ensure!((16..=16_384).contains(&metrics.units_per_em), "typography_invalid_font: units per em outside 16..16384");
    ensure!(metrics.glyph_count > 0, "typography_invalid_font: font has no glyphs");
    for value in [f64::from(metrics.ascent), f64::from(metrics.descent)] {
        ensure!(valid_coordinate(value), "typography_invalid_metrics: font metrics are not finite and bounded");
    }
    ensure!(metrics.ascent > metrics.descent, "typography_invalid_metrics: font ascent must exceed descent");

    let charmap = face.charmap();
    for ch in text.chars() {
        let mapped = charmap.map(ch);
        ensure!(mapped.is_some_and(|id| id.to_u32() != 0), "typography_missing_glyph: U+{:04X} has no nominal glyph", ch as u32);
    }

    let font = harfrust::Font::new(bytes.to_vec(), 0)
        .ok_or_else(|| anyhow!("typography_invalid_font: shaping provider rejected SFNT face"))?;
    ensure!(font.units_per_em() == metrics.units_per_em, "typography_invalid_metrics: provider units per em disagree");
    let shaper = harfrust::ShaperFont::new(&font);
    let mut buffer = harfrust::Buffer::new();
    buffer.push_str(text);
    buffer.set_direction(harfrust::Direction::LeftToRight);
    buffer.set_script(Some(harfrust::Script::LATIN));
    buffer.set_language(harfrust::Language::new(locale));
    buffer.set_cluster_level(harfrust::ClusterLevel::MonotoneGraphemes);
    ensure!(buffer.allocation_successful(), "typography_resource_limit: shaping input allocation failed");
    harfrust::shape(&shaper, &mut buffer, harfrust::ShapeOptions::default())
        .context("typography_shaping_failed: provider could not shape authored line")?;
    ensure!(buffer.allocation_successful(), "typography_resource_limit: shaping exceeded provider allocation or operation limits");
    let infos = buffer.glyph_infos();
    let positions = buffer.glyph_positions();
    ensure!(infos.len() == positions.len() && infos.len() <= MAX_LINE_GLYPHS, "typography_resource_limit: invalid or excessive shaped glyph count");
    ensure!(text.is_empty() || !infos.is_empty(), "typography_shaping_failed: nonempty text produced no glyphs");
    let outlines = face.outline_glyphs();
    let mut glyphs = Vec::with_capacity(infos.len());
    let mut remaining_bytes = MAX_PATH_BYTES;
    let mut remaining_commands = MAX_PATH_COMMANDS;
    for (index, (info, position)) in infos.iter().zip(positions).enumerate() {
        ensure!(info.glyph_id > 0 && info.glyph_id < u32::from(metrics.glyph_count), "typography_missing_glyph: shaped glyph is missing or outside the font");
        let cluster = info.cluster as usize;
        ensure!(cluster < text.len() && text.is_char_boundary(cluster), "typography_shaping_failed: invalid UTF-8 cluster offset");
        if index > 0 {
            ensure!(infos[index - 1].cluster <= info.cluster, "typography_shaping_failed: Latin clusters are not monotone");
        }
        let cluster_end = infos[index + 1..].iter()
            .find(|next| next.cluster > info.cluster)
            .map_or(text.len(), |next| next.cluster as usize);
        let cluster_text = text.get(cluster..cluster_end)
            .ok_or_else(|| anyhow!("typography_shaping_failed: invalid cluster range"))?;
        let whitespace = !cluster_text.is_empty() && cluster_text.chars().all(char::is_whitespace);
        let mut pen = BoundedOutline::new(remaining_bytes, remaining_commands);
        if let Some(glyph) = outlines.get(GlyphId::new(info.glyph_id)) {
            ensure!(glyph.draw_memory_size(Hinting::None) <= MAX_DRAW_MEMORY, "typography_resource_limit: outline scratch memory exceeds 8 MiB");
            glyph.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen)
                .context("typography_invalid_outline: provider could not draw glyph")?;
        } else {
            ensure!(whitespace, "typography_invalid_outline: non-whitespace glyph has no outline");
        }
        ensure!(!pen.failed, "typography_resource_limit: outline is nonfinite or exceeds bounded coordinates, commands, or bytes");
        ensure!(whitespace || (pen.has_drawing && pen.bounds.is_some()), "typography_invalid_outline: non-whitespace glyph has an empty outline");
        remaining_bytes -= pen.path.len();
        remaining_commands -= pen.commands;
        let values = [position.x_advance, position.y_advance, position.x_offset, position.y_offset];
        ensure!(values.iter().all(|value| valid_coordinate(f64::from(*value))), "typography_invalid_metrics: shaped position exceeds bounded coordinates");
        ensure!(position.y_advance == 0, "typography_unsupported_text: horizontal shaping produced a vertical advance");
        glyphs.push(ProviderGlyph {
            glyph_id: info.glyph_id,
            cluster_utf8: info.cluster,
            x_advance: f64::from(position.x_advance),
            y_advance: f64::from(position.y_advance),
            x_offset: f64::from(position.x_offset),
            y_offset: f64::from(position.y_offset),
            path: pen.path,
            bounds: pen.bounds,
        });
    }
    Ok(ProviderLine {
        glyphs,
        ascender: f64::from(metrics.ascent),
        descender: f64::from(metrics.descent),
        units_per_em: f64::from(metrics.units_per_em),
    })
}

/// Admit a bounded, static, single-face SFNT with monochrome outlines.
fn validate_sfnt(bytes: &[u8]) -> Result<()> {
    ensure!((12..=MAX_FONT_BYTES).contains(&bytes.len()), "typography_invalid_font: font must contain 12 bytes through 16 MiB");
    ensure!(&bytes[..4] == b"\0\x01\0\0" || &bytes[..4] == b"OTTO", "typography_unsupported_font: only static single-face TTF/OTF is supported; WOFF and collections are unavailable");
    let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    ensure!((1..=256).contains(&count), "typography_invalid_font: table count outside 1..256");
    let directory_end = 12 + count * 16;
    ensure!(directory_end <= bytes.len(), "typography_invalid_font: truncated table directory");
    let mut tags = BTreeSet::new();
    let mut ranges = Vec::with_capacity(count);
    for record in bytes[12..directory_end].chunks_exact(16) {
        let tag = [record[0], record[1], record[2], record[3]];
        ensure!(tags.insert(tag), "typography_invalid_font: duplicate table tag");
        let offset = u32::from_be_bytes([record[8], record[9], record[10], record[11]]) as usize;
        let length = u32::from_be_bytes([record[12], record[13], record[14], record[15]]) as usize;
        let end = offset.checked_add(length).ok_or_else(|| anyhow!("typography_invalid_font: table range overflow"))?;
        ensure!(offset >= directory_end && end <= bytes.len(), "typography_invalid_font: table is outside captured font bytes");
        if length > 0 { ranges.push((offset, end)); }
        if matches!(&tag, b"fvar" | b"gvar" | b"avar" | b"HVAR" | b"VVAR" | b"MVAR" | b"CFF2" | b"VARC") {
            bail!("typography_unsupported_font: variable-font tables require an explicit variation profile");
        }
        if matches!(&tag, b"COLR" | b"CPAL" | b"CBDT" | b"CBLC" | b"sbix" | b"SVG " | b"bdat" | b"bloc" | b"EBDT" | b"EBLC") {
            bail!("typography_unsupported_font: color or bitmap glyph tables are unavailable in the monochrome outline profile");
        }
    }
    ranges.sort_unstable();
    ensure!(ranges.windows(2).all(|pair| pair[0].1 <= pair[1].0), "typography_invalid_font: overlapping font tables");
    for required in [*b"head", *b"hhea", *b"hmtx", *b"maxp", *b"cmap"] {
        ensure!(tags.contains(&required), "typography_invalid_font: required metric or character-map table is absent");
    }
    ensure!((tags.contains(b"glyf") && tags.contains(b"loca")) || tags.contains(b"CFF "), "typography_unsupported_font: TrueType or CFF outlines are required");
    Ok(())
}

fn valid_coordinate(value: f64) -> bool {
    value.is_finite() && value.abs() <= MAX_COORDINATE
}

/// The control-point hull contains the actual Bezier ink, so it may reject a
/// near-edge glyph conservatively but cannot understate that glyph's extent.
struct BoundedOutline {
    path: String,
    bounds: Option<ProviderBounds>,
    commands: usize,
    max_bytes: usize,
    max_commands: usize,
    failed: bool,
    has_drawing: bool,
}

impl BoundedOutline {
    fn new(max_bytes: usize, max_commands: usize) -> Self {
        Self { path: String::new(), bounds: None, commands: 0, max_bytes, max_commands, failed: false, has_drawing: false }
    }

    fn command(&mut self, command: char, points: &[(f32, f32)]) {
        if self.failed { return; }
        if self.commands >= self.max_commands || points.iter().any(|(x, y)| !valid_coordinate(f64::from(*x)) || !valid_coordinate(f64::from(*y))) {
            self.failed = true;
            return;
        }
        // One command is tiny and bounded independently of the accumulated path.
        let mut next = String::new();
        next.push(command);
        for (x, y) in points {
            // Match the f64 measurements below; a shortest f32 decimal can
            // represent a slightly different value when SVG parses it as f64.
            let (x, y) = (coordinate(*x), coordinate(*y));
            let _ = write!(&mut next, "{x},{y} ");
        }
        if self.path.len().saturating_add(next.len()) > self.max_bytes {
            self.failed = true;
            return;
        }
        self.commands += 1;
        self.has_drawing |= matches!(command, 'L' | 'Q' | 'C');
        self.path.push_str(&next);
        for (x, y) in points {
            let (x, y) = (coordinate(*x), coordinate(*y));
            match &mut self.bounds {
                Some(bounds) => {
                    bounds.x_min = bounds.x_min.min(x);
                    bounds.y_min = bounds.y_min.min(y);
                    bounds.x_max = bounds.x_max.max(x);
                    bounds.y_max = bounds.y_max.max(y);
                }
                None => self.bounds = Some(ProviderBounds { x_min: x, y_min: y, x_max: x, y_max: y }),
            }
        }
    }
}

fn coordinate(value: f32) -> f64 {
    if value == 0.0 { 0.0 } else { f64::from(value) }
}

impl OutlinePen for BoundedOutline {
    fn move_to(&mut self, x: f32, y: f32) { self.command('M', &[(x, y)]); }
    fn line_to(&mut self, x: f32, y: f32) { self.command('L', &[(x, y)]); }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) { self.command('Q', &[(cx, cy), (x, y)]); }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) { self.command('C', &[(cx0, cy0), (cx1, cy1), (x, y)]); }
    fn close(&mut self) { self.command('Z', &[]); }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Authored regression sources only; execution is deferred with the sprint's
    // build, test and native qualification gates.
    #[test]
    fn path_decimal_matches_measured_f64_and_normalizes_negative_zero() {
        let mut pen = BoundedOutline::new(256, 4);
        pen.move_to(123456.79_f32, -0.0);
        assert_eq!(pen.path, "M123456.7890625,0 ");
        let bounds = pen.bounds.expect("recorded move point");
        assert_eq!(bounds.x_min, 123456.7890625);
        assert_eq!(bounds.y_min.to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn sfnt_table_escape_is_rejected_before_provider_parsing() {
        let mut bytes = vec![0_u8; 28];
        bytes[..4].copy_from_slice(b"\0\x01\0\0");
        bytes[4..6].copy_from_slice(&1_u16.to_be_bytes());
        bytes[12..16].copy_from_slice(b"head");
        bytes[20..24].copy_from_slice(&28_u32.to_be_bytes());
        bytes[24..28].copy_from_slice(&16_u32.to_be_bytes());
        let error = validate_sfnt(&bytes).expect_err("escaped table");
        assert!(error.to_string().contains("table is outside captured font bytes"));
    }

    #[test]
    fn compressed_collection_variable_and_color_font_surfaces_are_rejected() {
        for signature in [b"wOFF", b"wOF2", b"ttcf"] {
            let mut bytes = vec![0_u8; 12];
            bytes[..4].copy_from_slice(signature);
            assert!(validate_sfnt(&bytes).expect_err("unsupported container")
                .to_string().starts_with("typography_unsupported_font:"));
        }
        for tag in [b"fvar", b"COLR"] {
            let mut bytes = vec![0_u8; 28];
            bytes[..4].copy_from_slice(b"\0\x01\0\0");
            bytes[4..6].copy_from_slice(&1_u16.to_be_bytes());
            bytes[12..16].copy_from_slice(tag);
            bytes[20..24].copy_from_slice(&28_u32.to_be_bytes());
            assert!(validate_sfnt(&bytes).expect_err("unsupported font tables")
                .to_string().starts_with("typography_unsupported_font:"));
        }
    }
}
