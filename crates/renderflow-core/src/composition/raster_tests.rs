//! Authored raster regressions. No decoder, renderer, or test was executed as
//! part of this checkpoint; these assertions await the deferred test session.

use std::io::Cursor;
use std::path::Path;

use super::*;
use crate::composition::{render, CompositionOutput, LoadedComposition};

fn fixture() -> LoadedComposition {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    LoadedComposition::load(root.join("examples/page-composition/raster-composition/page.composition.json")).unwrap()
}

fn svg(contents: &str) -> Vec<u8> {
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"50.8mm\" height=\"50.8mm\" viewBox=\"0 0 144 144\">{contents}</svg>")
        .into_bytes()
}

fn decode(bytes: &[u8]) -> (png::OutputInfo, Vec<u8>) {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut decoded).unwrap();
    reader.finish().unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    decoded.truncate(info.buffer_size());
    (info, decoded)
}

fn pixel(bytes: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = usize::try_from((y * width + x) * 4).unwrap();
    bytes[index..index + 4].try_into().unwrap()
}

#[test]
fn raster_plan_uses_explicit_dpi_canvas_dimensions_and_canonical_background() {
    let snapshot = fixture();
    let default = plan(&snapshot, &RasterSettings::default()).unwrap();
    assert_eq!(default.schema, "renderflow.composition-raster/v1");
    assert_eq!(default.settings.dpi, 144);
    assert_eq!(default.settings.background, "transparent");
    assert_eq!((default.width_px, default.height_px), (288, 288));
    assert_eq!(default.pixel_format, "rgba8");
    assert_eq!(default.alpha, "straight");
    assert_eq!(default.color_policy, "assumed_srgb_no_profile_conversion");
    assert!(!default.runtime_qualified);
    for (dpi, dimension) in [(36, 72), (72, 144), (300, 600), (600, 1200)] {
        let output = plan(&snapshot, &RasterSettings { dpi, background: "#A1B2C3".into() }).unwrap();
        assert_eq!((output.width_px, output.height_px), (dimension, dimension));
        assert_eq!(output.settings.background, "#a1b2c3");
    }
}

#[test]
fn raster_limits_refuse_oversized_axes_area_and_invalid_settings() {
    let snapshot = fixture();
    for settings in [
        RasterSettings { dpi: 35, background: "transparent".into() },
        RasterSettings { dpi: 601, background: "transparent".into() },
        RasterSettings { dpi: 144, background: "red".into() },
        RasterSettings { dpi: 144, background: "#123".into() },
        RasterSettings { dpi: 144, background: "#12345678".into() },
        RasterSettings { dpi: 144, background: "#gg0000".into() },
    ] {
        assert!(plan(&snapshot, &settings).is_err());
    }
    let mut axis = fixture();
    axis.manifest.page.width_mm = 3000.0;
    assert!(plan(&axis, &RasterSettings::default()).is_err());
    let mut area = fixture();
    // Each axis remains below 8192 pixels; their product exceeds 16,777,216.
    area.manifest.page.width_mm = 1000.0;
    area.manifest.page.height_mm = 1000.0;
    assert!(plan(&area, &RasterSettings::default()).is_err());
    let mut fractional = fixture();
    fractional.manifest.page.width_mm = 50.9;
    let fractional = plan(&fractional, &RasterSettings::default()).unwrap();
    assert_eq!((fractional.width_px, fractional.height_px), (289, 288));
}

#[test]
fn png_pixels_preserve_alpha_and_composite_the_requested_background() {
    let snapshot = fixture();
    let source = svg("<rect width=\"72\" height=\"144\" fill=\"#ff0000\" fill-opacity=\"0.5\"/>");
    let transparent = plan(&snapshot, &RasterSettings::default()).unwrap();
    let transparent_bytes = render_png(&snapshot, &source, &transparent).unwrap();
    let (info, pixels) = decode(&transparent_bytes);
    assert_eq!((info.width, info.height), (288, 288));
    let red = pixel(&pixels, info.width, 10, 10);
    assert_eq!(&red[..3], &[255, 0, 0]);
    assert!((127..=128).contains(&red[3]));
    assert_eq!(pixel(&pixels, info.width, 250, 10), [0, 0, 0, 0]);

    let blue = plan(&snapshot, &RasterSettings { dpi: 144, background: "#0000ff".into() }).unwrap();
    let (info, pixels) = decode(&render_png(&snapshot, &source, &blue).unwrap());
    let purple = pixel(&pixels, info.width, 10, 10);
    assert!((127..=129).contains(&purple[0]));
    assert_eq!(purple[1], 0);
    assert!((126..=128).contains(&purple[2]));
    assert_eq!(purple[3], 255);
    assert_eq!(pixel(&pixels, info.width, 250, 10), [0, 0, 255, 255]);
}

#[test]
fn clipping_and_group_opacity_survive_pixel_composition() {
    let snapshot = fixture();
    let source = svg("<defs><clipPath id=\"cut\"><rect width=\"36\" height=\"144\"/></clipPath></defs><g clip-path=\"url(#cut)\" opacity=\"0.5\"><rect width=\"144\" height=\"144\" fill=\"#ff0000\"/></g>");
    let raster = plan(&snapshot, &RasterSettings::default()).unwrap();
    let (info, pixels) = decode(&render_png(&snapshot, &source, &raster).unwrap());
    assert!((127..=128).contains(&pixel(&pixels, info.width, 10, 10)[3]));
    assert_eq!(pixel(&pixels, info.width, 100, 10), [0, 0, 0, 0]);
}

#[test]
fn composed_fixture_retains_dimensions_background_and_png_resolution_metadata() {
    let snapshot = fixture();
    let settings = RasterSettings { dpi: 144, background: "#204060".into() };
    let first = render::render_with_settings(&snapshot, "en", CompositionOutput::Png, &settings).unwrap();
    let second = render::render_with_settings(&snapshot, "en", CompositionOutput::Png, &settings).unwrap();
    assert_eq!(first.media_type, "image/png");
    assert_eq!(first.extension, "png");
    assert_eq!(first.bytes, second.bytes);
    let (info, pixels) = decode(&first.bytes);
    assert_eq!((info.width, info.height), (288, 288));
    assert_eq!(pixel(&pixels, info.width, 0, 0), [32, 64, 96, 255]);
    assert!(pixels.chunks_exact(4).any(|value| value != [32, 64, 96, 255]));
    let reader = png::Decoder::new(Cursor::new(&first.bytes)).read_info().unwrap();
    let physical = reader.info().pixel_dims.as_ref().unwrap();
    assert_eq!(physical.unit, png::Unit::Meter);
    assert_eq!((physical.xppu, physical.yppu), (5669, 5669));

    let transparent = render::render_with_settings(&snapshot, "en", CompositionOutput::Png,
        &RasterSettings { dpi: 72, background: "transparent".into() }).unwrap();
    let (info, pixels) = decode(&transparent.bytes);
    assert_eq!((info.width, info.height), (144, 144));
    assert_eq!(pixel(&pixels, info.width, 0, 0), [0, 0, 0, 0]);
}

#[test]
fn external_images_are_refused_instead_of_being_omitted_from_pixels() {
    let snapshot = fixture();
    let raster = plan(&snapshot, &RasterSettings::default()).unwrap();
    for href in ["https://invalid.example/plate.png", "file:///missing-plate.png", "plate.png"] {
        let source = svg(&format!("<image href=\"{href}\" width=\"144\" height=\"144\"/>"));
        assert!(render_png(&snapshot, &source, &raster).is_err(), "external href must fail: {href}");
    }
}

#[test]
fn small_output_does_not_admit_a_giant_isolated_off_canvas_surface() {
    let snapshot = fixture();
    let raster = plan(&snapshot, &RasterSettings::default()).unwrap();
    let source = svg("<g opacity=\"0.5\"><rect x=\"-100000\" y=\"-100000\" width=\"200000\" height=\"200000\" fill=\"#112233\"/></g>");
    let error = render_png(&snapshot, &source, &raster).unwrap_err();
    assert!(error.to_string().contains("composition.raster.tree_budget"));
}

#[test]
fn cached_png_admission_checks_dimensions_density_and_complete_image_data() {
    let snapshot = fixture();
    let raster = plan(&snapshot, &RasterSettings::default()).unwrap();
    let bytes = render_png(&snapshot, &svg("<rect width=\"144\" height=\"144\" fill=\"#112233\"/>"), &raster).unwrap();
    assert!(validate_output(&bytes, &raster).is_ok());
    let mut wrong_dimensions = raster.clone();
    wrong_dimensions.width_px += 1;
    assert!(validate_output(&bytes, &wrong_dimensions).is_err());
    let mut wrong_density = raster.clone();
    wrong_density.settings.dpi = 72;
    assert!(validate_output(&bytes, &wrong_density).is_err());
    // A complete decoded frame alone is insufficient when the final PNG
    // framing or integrity data is missing.
    assert!(validate_output(&bytes[..bytes.len() - 12], &raster).is_err());
}
