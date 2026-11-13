//! Authored/unrun publication RGB contract regressions.

use super::*;
use crate::composition::raster::{RasterSettings, RASTER_PROVIDER_PROFILE, RASTER_SCHEMA_V1};

fn source(alpha: u8) -> (Vec<u8>, RasterPlan) {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: 5000,
            yppu: 5000,
            unit: png::Unit::Meter,
        }));
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[9, 37, 211, alpha]).unwrap();
        writer.finish().unwrap();
    }
    (
        bytes,
        RasterPlan {
            schema: RASTER_SCHEMA_V1.into(),
            provider_profile: RASTER_PROVIDER_PROFILE.into(),
            settings: RasterSettings {
                dpi: 127,
                background: "#ffffff".into(),
            },
            width_px: 1,
            height_px: 1,
            pixel_format: "rgba8".into(),
            alpha: "straight".into(),
            color_policy: "assumed_srgb_no_profile_conversion".into(),
            runtime_qualified: false,
        },
    )
}

#[test]
fn opaque_samples_are_retained_and_the_exact_print_input_contract_accepts_rgb() {
    let (bytes, plan) = source(255);
    let prepared = prepare_composition_rgb(
        &bytes,
        &digest(&bytes),
        &plan,
        RGB_POLICY,
        &ProcessCancellationToken::new(),
    )
    .unwrap();
    let mut reader = png::Decoder::new(BufReader::new(Cursor::new(&prepared.bytes)))
        .read_info()
        .unwrap();
    assert!(reader.info().srgb.is_none() && reader.info().pixel_dims.is_none());
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgb);
    assert_eq!(pixels, [9, 37, 211]);
    reader.finish().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prepared.png");
    std::fs::write(&path, &prepared.bytes).unwrap();
    let image =
        crate::print_pdf_image::inspect_print_image(&path, crate::graph::Format::Png).unwrap();
    assert_eq!(image.width_px, 1);
    assert_eq!(prepared.evidence.source_sha256, digest(&bytes));
    assert_eq!(prepared.evidence.output_sha256, digest(&prepared.bytes));
    assert_eq!(prepared.evidence.rgb_samples_sha256, digest(&[9, 37, 211]));
    assert_eq!(prepared.evidence.fidelity, FidelityDeclaration::Partial);
    assert!(
        prepared.evidence.review_required
            && !prepared.evidence.runtime_qualified
            && !prepared.evidence.losses.is_empty()
    );
}

#[test]
fn transparency_is_refused_without_compositing_or_sample_loss() {
    let (bytes, plan) = source(254);
    let error = prepare_composition_rgb(
        &bytes,
        &digest(&bytes),
        &plan,
        RGB_POLICY,
        &ProcessCancellationToken::new(),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("publication.rgb.alpha"));
}

#[test]
fn source_identity_and_color_loss_acknowledgment_are_required() {
    let (bytes, plan) = source(255);
    for (identity, policy) in [
        ("0".repeat(64), RGB_POLICY),
        (digest(&bytes), "preserve_srgb"),
    ] {
        assert!(prepare_composition_rgb(
            &bytes,
            &identity,
            &plan,
            policy,
            &ProcessCancellationToken::new()
        )
        .is_err());
    }
}

#[test]
fn incompatible_native_plan_and_corrupt_png_are_refused() {
    let (mut bytes, mut plan) = source(255);
    plan.width_px = 2;
    assert!(prepare_composition_rgb(
        &bytes,
        &digest(&bytes),
        &plan,
        RGB_POLICY,
        &ProcessCancellationToken::new()
    )
    .is_err());
    plan.width_px = 1;
    let index = bytes.len() / 2;
    bytes[index] ^= 1;
    assert!(prepare_composition_rgb(
        &bytes,
        &digest(&bytes),
        &plan,
        RGB_POLICY,
        &ProcessCancellationToken::new()
    )
    .is_err());
}

#[test]
fn pre_cancelled_preparation_returns_no_candidate() {
    let (bytes, plan) = source(255);
    let token = ProcessCancellationToken::new();
    token.cancel();
    assert!(
        prepare_composition_rgb(&bytes, &digest(&bytes), &plan, RGB_POLICY, &token)
            .err()
            .unwrap()
            .to_string()
            .contains("publication.rgb.cancelled")
    );
}

#[test]
fn preparation_is_deterministic_and_leaves_the_selected_source_unchanged() {
    let (bytes, plan) = source(255);
    let before = bytes.clone();
    let identity = digest(&bytes);
    let first = prepare_composition_rgb(
        &bytes,
        &identity,
        &plan,
        RGB_POLICY,
        &ProcessCancellationToken::new(),
    )
    .unwrap();
    let second = prepare_composition_rgb(
        &bytes,
        &identity,
        &plan,
        RGB_POLICY,
        &ProcessCancellationToken::new(),
    )
    .unwrap();
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(
        serde_json::to_value(first.evidence).unwrap(),
        serde_json::to_value(second.evidence).unwrap()
    );
    assert_eq!(bytes, before);
}
