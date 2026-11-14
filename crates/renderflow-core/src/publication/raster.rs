//! Explicit conversion from the native composition RGBA preview contract to
//! the narrow device-RGB PNG input accepted by ordered PDF/EPUB packaging.
//! Samples are retained, not color-converted. This is not a print proof.

use crate::composition::{raster, RasterPlan};
use crate::evidence::FidelityDeclaration;
use crate::process::ProcessCancellationToken;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufReader, Cursor};

pub const RGB_PREPARATION_V1: &str = "renderflow.publication-rgb/v1";
pub const RGB_POLICY: &str = "assumed_srgb_samples_to_device_rgb_no_conversion";
pub const RGB_PROVIDER: &str = "renderflow.publication-rgb/v1;png=0.18.1;rgb8;compression=fast;filter=sub;metadata=none;alpha=opaque-only";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RgbPreparationEvidence {
    pub schema_version: String,
    pub provider_profile: String,
    pub source_sha256: String,
    pub raster_plan_sha256: String,
    pub output_sha256: String,
    pub rgb_samples_sha256: String,
    pub output_bytes: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub source_dpi: u32,
    pub color_policy: String,
    pub fidelity: FidelityDeclaration,
    pub losses: Vec<String>,
    pub review_required: bool,
    pub runtime_qualified: bool,
}

pub struct PreparedRgb {
    pub bytes: Vec<u8>,
    pub evidence: RgbPreparationEvidence,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn check(token: &ProcessCancellationToken) -> Result<()> {
    ensure!(!token.is_cancelled(), "publication.rgb.cancelled");
    Ok(())
}

/// The caller must supply the observed source digest and explicitly acknowledge
/// device-color metadata loss with RGB_POLICY. A supplied raster plan is a
/// contract to validate, not authenticated provenance or publication approval.
/// Transparent pixels are refused; no hidden background/compositing occurs.
pub fn prepare_composition_rgb(
    source: &[u8],
    expected_sha256: &str,
    plan: &RasterPlan,
    color_policy: &str,
    token: &ProcessCancellationToken,
) -> Result<PreparedRgb> {
    check(token)?;
    ensure!(
        color_policy == RGB_POLICY,
        "publication.rgb.color_policy: explicitly acknowledge device-RGB metadata loss"
    );
    ensure!(source.len() <= 64 * 1024 * 1024 && digest(source) == expected_sha256,
        "publication.rgb.source_identity: source bytes differ from the selected digest or exceed 64 MiB");
    raster::validate_output(source, plan).context("publication.rgb.native_contract")?;
    check(token)?;
    let expected_rgba = u64::from(plan.width_px)
        .checked_mul(u64::from(plan.height_px))
        .and_then(|count| count.checked_mul(4))
        .context("publication.rgb.bounds")?;
    ensure!(
        expected_rgba > 0 && expected_rgba <= 64 * 1024 * 1024,
        "publication.rgb.bounds"
    );
    let mut limits = png::Limits::default();
    limits.bytes = 64 * 1024 * 1024;
    let mut reader = png::Decoder::new_with_limits(BufReader::new(Cursor::new(source)), limits)
        .read_info()
        .context("publication.rgb.decode")?;
    ensure!(
        reader.output_buffer_size() == Some(expected_rgba as usize),
        "publication.rgb.bounds"
    );
    let mut rgba = vec![0; expected_rgba as usize];
    let frame = reader
        .next_frame(&mut rgba)
        .context("publication.rgb.decode")?;
    ensure!(
        frame.width == plan.width_px
            && frame.height == plan.height_px
            && frame.color_type == png::ColorType::Rgba
            && frame.bit_depth == png::BitDepth::Eight
            && frame.buffer_size() == rgba.len(),
        "publication.rgb.decode_contract"
    );
    reader.finish().context("publication.rgb.trailing_data")?;
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for row in rgba.chunks_exact(plan.width_px as usize * 4) {
        check(token)?;
        for pixel in row.chunks_exact(4) {
            ensure!(
                pixel[3] == 255,
                "publication.rgb.alpha: transparent pixels cannot be discarded"
            );
            rgb.extend_from_slice(&pixel[..3]);
        }
    }
    drop(rgba);
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, plan.width_px, plan.height_px);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_filter(png::Filter::Sub);
        // No sRGB/gAMA/ICC/pHYs is copied: the receiving exact device-color
        // route cannot preserve that interpretation. Geometry stays explicit.
        let mut writer = encoder.write_header().context("publication.rgb.encode")?;
        writer
            .write_image_data(&rgb)
            .context("publication.rgb.encode")?;
        writer.finish().context("publication.rgb.encode")?;
    }
    ensure!(
        bytes.len() <= 64 * 1024 * 1024,
        "publication.rgb.output_limit"
    );
    check(token)?;
    let evidence = RgbPreparationEvidence {
        schema_version: RGB_PREPARATION_V1.into(), provider_profile: RGB_PROVIDER.into(),
        source_sha256: expected_sha256.into(), raster_plan_sha256: digest(&serde_json::to_vec(plan)?),
        output_sha256: digest(&bytes), rgb_samples_sha256: digest(&rgb), output_bytes: bytes.len(),
        width_px: plan.width_px, height_px: plan.height_px, source_dpi: plan.settings.dpi,
        color_policy: RGB_POLICY.into(), fidelity: FidelityDeclaration::Partial,
        losses: vec!["opaque alpha channel removed".into(), "embedded sRGB intent and density metadata removed; RGB samples unchanged without color conversion".into()],
        review_required: true, runtime_qualified: false,
    };
    Ok(PreparedRgb { bytes, evidence })
}

#[cfg(test)]
mod tests;
