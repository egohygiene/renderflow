//! Explicit single-page PDF geometry over an unchanged canonical composition.
//!
//! Gutter is minimum total clearance from the selected trim edge. It is never
//! added to margins or applied as a translation. PDF numbers are recorded at
//! the f32 precision supported by the adopted writer, without changing inputs.

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

use super::model::{NormalizedRect, PageGeometry};
use super::raster::{self, RasterPlan, RasterSettings};
use super::LoadedComposition;

pub const PDF_SCHEMA_V1: &str = "renderflow.composition-pdf/v1";
pub const PDF_PROVIDER_PROFILE: &str = "renderflow.composition-pdf/v1;lopdf=0.45.0;pdf=1.4;image=opaque-rgb8;compression=none;placement=uniform-dpi-top-left;boxes=explicit-f32;output-intent=none;fonts=flattened";
const COLOR_POLICY: &str = "assumed_srgb_no_profile_conversion";
const POINTS_PER_MM: f64 = 72.0 / 25.4;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingSide {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfSettings {
    /// Sampling density for this PDF only; independent of PNG output settings.
    pub dpi: u32,
    /// Opaque six-digit hex color. Transparency is not admitted in this profile.
    pub background: String,
    /// Explicit physical binding edge of this page; no parity inference.
    pub binding_side: BindingSide,
    /// Only assumed_srgb_no_profile_conversion is implemented.
    pub color_policy: String,
    /// Only none is implemented. No PDF/X, PDF/A, or print certification.
    pub conformance: String,
}

impl Default for PdfSettings {
    fn default() -> Self {
        Self {
            dpi: 300,
            background: "#ffffff".to_owned(),
            binding_side: BindingSide::None,
            color_policy: COLOR_POLICY.to_owned(),
            conformance: "none".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfPlan {
    pub schema: String,
    pub provider_profile: String,
    pub settings: PdfSettings,
    pub raster: RasterPlan,
    /// PDF bottom-left point coordinates [left, bottom, right, top].
    /// CropBox equals this full-canvas MediaBox in the bounded provider.
    pub media_box_pt: [f64; 4],
    pub trim_box_pt: [f64; 4],
    pub bleed_box_pt: [f64; 4],
    /// Evidence only: PDF has no SafeBox page entry.
    pub safe_box_pt: [f64; 4],
    pub gutter_pt: f64,
    /// Uniform nominal-DPI placement, anchored at the full canvas top-left.
    /// Rounded final pixels may extend past the right/bottom canvas clip.
    pub image_draw_matrix: [f64; 6],
    pub runtime_qualified: bool,
}

/// Plan from captured composition bytes, without files, tools or rendering.
/// A declared safe area must already honor the selected binding clearance.
/// Per-locale effective text containment is enforced by shared layout checks.
pub fn plan(snapshot: &LoadedComposition, settings: &PdfSettings) -> Result<PdfPlan> {
    let settings = canonical_settings(settings)?;
    let geometry = geometry(&snapshot.manifest.page, settings.binding_side)?;
    let raster = raster::plan(snapshot, &RasterSettings {
        dpi: settings.dpi,
        background: settings.background.clone(),
    })?;
    let image_draw_matrix = draw_matrix(
        raster.width_px, raster.height_px, settings.dpi, geometry.media[3],
    )?;
    let planned = PdfPlan {
        schema: PDF_SCHEMA_V1.to_owned(),
        provider_profile: PDF_PROVIDER_PROFILE.to_owned(),
        settings,
        raster,
        media_box_pt: geometry.media,
        trim_box_pt: geometry.trim,
        bleed_box_pt: geometry.bleed,
        safe_box_pt: geometry.safe,
        gutter_pt: geometry.gutter,
        image_draw_matrix,
        runtime_qualified: false,
    };
    validate_plan(&planned)?;
    Ok(planned)
}

/// Check the serialized contract before fresh or cached output admission.
/// Rendering also replans against the exact captured source before drawing.
pub(crate) fn validate_plan(planned: &PdfPlan) -> Result<()> {
    ensure!(planned.schema == PDF_SCHEMA_V1
        && planned.provider_profile == PDF_PROVIDER_PROFILE
        && !planned.runtime_qualified,
        "composition.pdf.plan_mismatch: current unqualified PDF provider evidence is required");
    ensure!(canonical_settings(&planned.settings)? == planned.settings,
        "composition.pdf.plan_mismatch: PDF settings must be canonical");
    let raster = &planned.raster;
    ensure!(raster.schema == raster::RASTER_SCHEMA_V1
        && raster.provider_profile == raster::RASTER_PROVIDER_PROFILE
        && raster.settings.dpi == planned.settings.dpi
        && raster.settings.background == planned.settings.background
        && raster.pixel_format == "rgba8" && raster.alpha == "straight"
        && raster.color_policy == COLOR_POLICY && !raster.runtime_qualified,
        "composition.pdf.plan_mismatch: PDF raster evidence differs from its settings or provider");
    ensure!(raster.width_px > 0 && raster.height_px > 0
        && raster.width_px <= 8192 && raster.height_px <= 8192
        && u64::from(raster.width_px) * u64::from(raster.height_px) <= 16_777_216,
        "composition.pdf.pixel_bounds: PDF sampling exceeds the bounded raster canvas");
    validate_geometry(
        planned.media_box_pt, planned.trim_box_pt, planned.bleed_box_pt,
        planned.safe_box_pt, planned.gutter_pt, planned.settings.binding_side,
    )?;
    let expected = draw_matrix(
        raster.width_px, raster.height_px, planned.settings.dpi, planned.media_box_pt[3],
    )?;
    ensure!(planned.image_draw_matrix == expected,
        "composition.pdf.plan_mismatch: PDF image placement differs from the uniform DPI contract");
    let epsilon = geometry_epsilon(planned.media_box_pt);
    let pixel_pt = 72.0 / f64::from(planned.settings.dpi);
    for (extent, media_extent) in [
        (expected[0], planned.media_box_pt[2]),
        (expected[3], planned.media_box_pt[3]),
    ] {
        ensure!(extent + epsilon >= media_extent && extent - media_extent <= pixel_pt + epsilon,
            "composition.pdf.plan_mismatch: sampling canvas must cover the media canvas by less than one rounded pixel");
    }
    Ok(())
}

fn canonical_settings(settings: &PdfSettings) -> Result<PdfSettings> {
    ensure!((36..=600).contains(&settings.dpi),
        "composition.pdf.dpi: DPI must be an integer from 36 through 600");
    let background = &settings.background;
    ensure!(background.len() == 7 && background.starts_with('#')
        && background.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit),
        "composition.pdf.background: PDF requires an opaque #RRGGBB background");
    ensure!(settings.color_policy == COLOR_POLICY,
        "composition.pdf.color_policy: only assumed_srgb_no_profile_conversion is supported; ICC conversion and output intents are unavailable");
    ensure!(settings.conformance == "none",
        "composition.pdf.conformance: PDF/X, PDF/A, and other conformance claims are unavailable");
    let mut result = settings.clone();
    result.background = background.to_ascii_lowercase();
    Ok(result)
}

struct Geometry {
    media: [f64; 4],
    trim: [f64; 4],
    bleed: [f64; 4],
    safe: [f64; 4],
    gutter: f64,
}

fn geometry(page: &PageGeometry, binding_side: BindingSide) -> Result<Geometry> {
    ensure!(page.width_mm.is_finite() && page.height_mm.is_finite()
        && page.width_mm > 0.0 && page.height_mm > 0.0
        && page.width_mm <= 5000.0 && page.height_mm <= 5000.0,
        "composition.pdf.geometry: page dimensions must be finite, positive, and at most 5000 mm");
    ensure!(page.gutter_mm.is_finite() && page.gutter_mm >= 0.0,
        "composition.pdf.gutter: gutter must be finite and nonnegative");
    ensure!(page.gutter_mm == 0.0 || binding_side != BindingSide::None,
        "composition.pdf.binding_side: a nonzero gutter requires an explicit left or right binding side");
    // Match normalized source containment tolerance; no inferred margin is added.
    let clearance = match binding_side {
        BindingSide::None => 0.0,
        BindingSide::Left => page.safe.x - page.trim.x,
        BindingSide::Right => page.trim.x + page.trim.width - page.safe.x - page.safe.width,
    };
    ensure!(page.gutter_mm == 0.0
        || clearance + 1e-9 >= page.gutter_mm / page.width_mm,
        "composition.pdf.gutter_clearance: declared safe area does not clear the requested gutter from the selected trim edge; correct the authored geometry");
    let width = page.width_mm * POINTS_PER_MM;
    let height = page.height_mm * POINTS_PER_MM;
    let result = Geometry {
        media: [0.0, 0.0, pdf_number(width)?, pdf_number(height)?],
        trim: pdf_box(page.trim, width, height)?,
        bleed: pdf_box(page.bleed, width, height)?,
        safe: pdf_box(page.safe, width, height)?,
        gutter: pdf_number(page.gutter_mm * POINTS_PER_MM)?,
    };
    ensure!(page.gutter_mm == 0.0 || result.gutter > 0.0,
        "composition.pdf.geometry_precision: nonzero gutter collapses at PDF numeric precision");
    validate_geometry(result.media, result.trim, result.bleed, result.safe, result.gutter, binding_side)?;
    Ok(result)
}

fn pdf_box(rect: NormalizedRect, width: f64, height: f64) -> Result<[f64; 4]> {
    let result = [
        pdf_number(rect.x * width)?,
        pdf_number((1.0 - rect.y - rect.height) * height)?,
        pdf_number((rect.x + rect.width) * width)?,
        pdf_number((1.0 - rect.y) * height)?,
    ];
    checked_box(result)?;
    Ok(result)
}

fn pdf_number(value: f64) -> Result<f64> {
    let rounded = value as f32;
    ensure!(value.is_finite() && rounded.is_finite(),
        "composition.pdf.geometry_precision: coordinates must remain finite at PDF numeric precision");
    Ok(if rounded == 0.0 { 0.0 } else { f64::from(rounded) })
}

fn checked_box(rect: [f64; 4]) -> Result<()> {
    ensure!(rect.iter().all(|value| value.is_finite() && f64::from(*value as f32) == *value)
        && rect[2] > rect[0] && rect[3] > rect[1],
        "composition.pdf.geometry_precision: page boxes must retain positive area at PDF numeric precision");
    Ok(())
}

fn contains(outer: [f64; 4], inner: [f64; 4]) -> bool {
    inner[0] >= outer[0] && inner[1] >= outer[1]
        && inner[2] <= outer[2] && inner[3] <= outer[3]
}

fn geometry_epsilon(media: [f64; 4]) -> f64 {
    // Only writer precision allowance; source clearance is checked separately.
    f64::from(f32::EPSILON) * media[2].max(media[3]).max(1.0) * 4.0
}

fn validate_geometry(
    media: [f64; 4], trim: [f64; 4], bleed: [f64; 4], safe: [f64; 4],
    gutter: f64, binding_side: BindingSide,
) -> Result<()> {
    for rect in [media, trim, bleed, safe] { checked_box(rect)?; }
    ensure!(media[0] == 0.0 && media[1] == 0.0
        && media[2] <= f64::from((5000.0 * POINTS_PER_MM) as f32)
        && media[3] <= f64::from((5000.0 * POINTS_PER_MM) as f32)
        && contains(media, bleed) && contains(bleed, trim) && contains(trim, safe),
        "composition.pdf.geometry: media must contain bleed, bleed must contain trim, and trim must contain safe at PDF numeric precision");
    let epsilon = geometry_epsilon(media);
    ensure!(gutter.is_finite() && gutter >= 0.0 && f64::from(gutter as f32) == gutter
        && gutter <= (trim[2] - trim[0]) / 2.0 + epsilon,
        "composition.pdf.gutter: gutter must be nonnegative and no wider than half the trim");
    ensure!(gutter == 0.0 || binding_side != BindingSide::None,
        "composition.pdf.binding_side: a nonzero gutter requires an explicit binding side");
    let clearance = match binding_side {
        BindingSide::None => 0.0,
        BindingSide::Left => safe[0] - trim[0],
        BindingSide::Right => trim[2] - safe[2],
    };
    ensure!(gutter == 0.0 || clearance + epsilon >= gutter,
        "composition.pdf.gutter_clearance: safe area does not honor binding clearance at PDF numeric precision");
    Ok(())
}

fn draw_matrix(width: u32, height: u32, dpi: u32, media_height: f64) -> Result<[f64; 6]> {
    let image_width = pdf_number(f64::from(width) * 72.0 / f64::from(dpi))?;
    let image_height = pdf_number(f64::from(height) * 72.0 / f64::from(dpi))?;
    Ok([image_width, 0.0, 0.0, image_height, 0.0, pdf_number(media_height - image_height)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> PageGeometry {
        PageGeometry {
            width_mm: 254.0, height_mm: 508.0,
            trim: NormalizedRect { x: 0.1, y: 0.2, width: 0.7, height: 0.6 },
            bleed: NormalizedRect { x: 0.05, y: 0.1, width: 0.9, height: 0.85 },
            safe: NormalizedRect { x: 0.2, y: 0.3, width: 0.5, height: 0.4 },
            gutter_mm: 0.0,
        }
    }

    #[test]
    fn asymmetric_boxes_preserve_full_canvas_and_invert_y() {
        let geometry = geometry(&page(), BindingSide::None).unwrap();
        assert_eq!(geometry.media, [0.0, 0.0, 720.0, 1440.0]);
        assert_eq!(geometry.trim, [72.0, 288.0, 576.0, 1152.0]);
        assert_eq!(geometry.bleed, [36.0, 72.0, 684.0, 1296.0]);
        assert_eq!(geometry.safe, [144.0, 432.0, 504.0, 1008.0]);
    }

    #[test]
    fn gutter_requires_explicit_edge_and_existing_clearance() {
        let mut page = page();
        page.gutter_mm = 20.0;
        assert!(geometry(&page, BindingSide::None).is_err());
        assert!(geometry(&page, BindingSide::Left).is_ok());
        assert!(geometry(&page, BindingSide::Right).is_ok());
        page.safe.x = 0.15;
        page.safe.width = 0.6;
        assert!(geometry(&page, BindingSide::Left).is_err());
        assert!(geometry(&page, BindingSide::Right).is_err());
    }

    #[test]
    fn collapsed_box_is_refused_at_writer_precision() {
        let mut page = page();
        page.safe.x = 0.5;
        page.safe.width = 1e-12;
        assert!(geometry(&page, BindingSide::None).is_err());
    }

    #[test]
    fn opaque_background_and_explicit_color_policy_are_required() {
        let mut settings = PdfSettings::default();
        settings.background = "#ABCDEF".to_owned();
        assert_eq!(canonical_settings(&settings).unwrap().background, "#abcdef");
        settings.background = "transparent".to_owned();
        assert!(canonical_settings(&settings).is_err());
        settings.background = "#ffffff".to_owned();
        settings.color_policy = "icc_cmyk".to_owned();
        assert!(canonical_settings(&settings).is_err());
        settings.color_policy = COLOR_POLICY.to_owned();
        settings.conformance = "pdf_x_4".to_owned();
        assert!(canonical_settings(&settings).is_err());
    }

    #[test]
    fn pixel_padding_is_clipped_without_anisotropic_page_fit() {
        let matrix = draw_matrix(143, 284, 36, 566.9291381835938).unwrap();
        assert_eq!(matrix[0], 286.0);
        assert_eq!(matrix[3], 568.0);
        assert!(matrix[5] < 0.0);
        assert_eq!(matrix[4], 0.0);
    }
}
