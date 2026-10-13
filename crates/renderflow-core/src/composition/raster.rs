//! Captured-resource PNG previews from the already measured composition SVG.
//!
//! Fonts and text shaping are disabled in the SVG provider. Every image must
//! match a captured asset and pass strict, bounded preflight before a provider
//! that can otherwise silently omit undecodable resources is allowed to draw.
//! Pixel bytes use an explicit assumed-sRGB preview policy, not print proofing.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufReader, Cursor};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};

use anyhow::{bail, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use resvg::{tiny_skia, usvg};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::model::{AssetFormat, LayerContent};
use super::LoadedComposition;

pub const RASTER_SCHEMA_V1: &str = "renderflow.composition-raster/v1";
pub const RASTER_PROVIDER_PROFILE: &str = "renderflow.composition-raster/v1;resvg=0.48.1;usvg=0.48.1;tiny-skia=0.12.0;png=0.18.1;zune-jpeg=0.5.15;imagesize=0.15.0;svg-dpi=72;text=outlines-only;resources=captured-data-only;pixel-format=rgba8;alpha=straight;color=assumed-srgb;png-compression=fast;png-filter=sub";
const MAX_AXIS: u32 = 8192;
const MAX_PIXELS: u64 = 16_777_216;
const MAX_IMAGE_INSTANCE_PIXELS: u64 = 33_554_432;
const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 64 * 1024 * 1024;
const MAX_CHUNKS: usize = 16_384;
const MAX_SVG_NODES: usize = 32_768;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterSettings {
    /// Integer requested sampling density, independent of any source image DPI.
    pub dpi: u32,
    /// `transparent` or an opaque six-digit hex color such as `#ffffff`.
    pub background: String,
}

impl Default for RasterSettings {
    fn default() -> Self { Self { dpi: 144, background: "transparent".to_owned() } }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterPlan {
    pub schema: String,
    pub provider_profile: String,
    pub settings: RasterSettings,
    pub width_px: u32,
    pub height_px: u32,
    pub pixel_format: String,
    pub alpha: String,
    pub color_policy: String,
    pub runtime_qualified: bool,
}

/// Derive a bounded canvas and inspect source metadata without rendering pixels.
/// Full compressed-image decoding occurs before actual publication, and again
/// when admitting a cached output. This function never opens a file or tool.
pub fn plan(snapshot: &LoadedComposition, settings: &RasterSettings) -> Result<RasterPlan> {
    ensure!((36..=600).contains(&settings.dpi), "composition.raster.dpi: DPI must be an integer from 36 through 600");
    let background = normalize_background(&settings.background)?;
    let width = (snapshot.manifest.page.width_mm * f64::from(settings.dpi) / 25.4).ceil();
    let height = (snapshot.manifest.page.height_mm * f64::from(settings.dpi) / 25.4).ceil();
    ensure!(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0
        && width <= f64::from(MAX_AXIS) && height <= f64::from(MAX_AXIS),
        "composition.raster.dimensions: requested canvas must have finite positive dimensions no larger than 8192 pixels per axis");
    let (width_px, height_px) = (width as u32, height as u32);
    checked_dimensions(width_px, height_px)?;
    inspect_assets(snapshot)?;
    Ok(RasterPlan {
        schema: RASTER_SCHEMA_V1.to_owned(), provider_profile: RASTER_PROVIDER_PROFILE.to_owned(),
        settings: RasterSettings { dpi: settings.dpi, background }, width_px, height_px,
        pixel_format: "rgba8".to_owned(), alpha: "straight".to_owned(),
        color_policy: "assumed_srgb_no_profile_conversion".to_owned(), runtime_qualified: false,
    })
}

/// Draw the SVG generated from the same in-process typography report as the
/// vector target. This interface is crate-private: arbitrary serialized plans
/// and arbitrary SVG are not a public rendering input format.
pub(crate) fn render_png(snapshot: &LoadedComposition, svg: &[u8], planned: &RasterPlan) -> Result<Vec<u8>> {
    ensure!(plan(snapshot, &planned.settings)? == *planned,
        "composition.raster.plan_mismatch: raster evidence does not match the current captured composition and provider");
    let assets = inspect_assets(snapshot)?;
    let mut allowed = BTreeSet::new();
    for (id, metadata) in &assets {
        let asset = snapshot.assets.get(id).context("composition.raster.asset_missing: captured asset is absent")?;
        match metadata.format {
            AssetFormat::Png => decode_png(&asset.bytes, metadata.width_px, metadata.height_px)?,
            AssetFormat::Jpeg => decode_jpeg(&asset.bytes, metadata.width_px, metadata.height_px)?,
            AssetFormat::Svg => {},
        }
        allowed.insert((metadata.format.media_type().to_owned(), metadata.sha256.clone()));
    }
    inspect_generated_svg(svg, &allowed)?;
    let allowed = Arc::new(allowed);
    let resolution_failed = Arc::new(AtomicBool::new(false));
    let failed_data = resolution_failed.clone();
    let failed_string = resolution_failed.clone();
    let mut options = usvg::Options::default();
    // The authored SVG's point-based viewport must remain in 72-point units.
    // Sampling uses one uniform scale; ceil only adds a fractional pixel at the
    // right/bottom, rather than stretching the page separately on each axis.
    options.dpi = 72.0;
    options.resources_dir = None;
    options.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: Box::new(move |mime, bytes, options| {
            let key = (mime.to_owned(), format!("{:x}", Sha256::digest(bytes.as_slice())));
            let image = if allowed.contains(&key) {
                match mime {
                    "image/png" => Some(usvg::ImageKind::PNG(bytes)),
                    "image/jpeg" => Some(usvg::ImageKind::JPEG(bytes)),
                    "image/svg+xml" => usvg::Tree::from_data_nested(bytes.as_slice(), options)
                        .ok().map(usvg::ImageKind::SVG),
                    _ => None,
                }
            } else { None };
            if image.is_none() { failed_data.store(true, Ordering::Relaxed); }
            image
        }),
        resolve_string: Box::new(move |_, _| {
            failed_string.store(true, Ordering::Relaxed);
            None
        }),
    };
    let tree = usvg::Tree::from_data(svg, &options)
        .context("composition.raster.svg_parse: generated composition SVG could not be parsed")?;
    ensure!(!resolution_failed.load(Ordering::Relaxed),
        "composition.raster.resource_unavailable: a resource was not captured, had an unsupported MIME type, or failed nested SVG parsing");
    let expected_width = (snapshot.manifest.page.width_mm * 72.0 / 25.4) as f32;
    let expected_height = (snapshot.manifest.page.height_mm * 72.0 / 25.4) as f32;
    ensure!((tree.size().width() - expected_width).abs() <= expected_width.abs() * 1e-5 + 0.001
        && (tree.size().height() - expected_height).abs() <= expected_height.abs() * 1e-5 + 0.001,
        "composition.raster.svg_dimensions: SVG viewport differs from the captured page geometry");
    super::raster_tree::validate_tree(&tree, planned.settings.dpi, planned.width_px, planned.height_px)?;
    let mut pixmap = tiny_skia::Pixmap::new(planned.width_px, planned.height_px)
        .context("composition.raster.allocation: bounded output surface could not be allocated")?;
    if let Some([red, green, blue]) = background_rgb(&planned.settings.background)? {
        pixmap.fill(tiny_skia::Color::from_rgba8(red, green, blue, 255));
    }
    let scale = planned.settings.dpi as f32 / 72.0;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let pixels = pixmap.take_demultiplied();
    let output = encode_png(&pixels, planned)?;
    drop(pixels);
    validate_output(&output, planned)?;
    Ok(output)
}

/// Admit only a complete, decodable output with the exact raster plan's canvas,
/// alpha format, sRGB declaration and rounded pixels-per-metre DPI metadata.
/// Used equally for newly encoded pixels and cache candidates.
pub(crate) fn validate_output(bytes: &[u8], planned: &RasterPlan) -> Result<()> {
    ensure!(planned.schema == RASTER_SCHEMA_V1 && planned.provider_profile == RASTER_PROVIDER_PROFILE
        && planned.pixel_format == "rgba8" && planned.alpha == "straight"
        && planned.color_policy == "assumed_srgb_no_profile_conversion" && !planned.runtime_qualified
        && (36..=600).contains(&planned.settings.dpi)
        && normalize_background(&planned.settings.background)? == planned.settings.background,
        "composition.raster.plan_mismatch: output validation requires a canonical current-provider raster plan");
    ensure!(bytes.len() <= MAX_OUTPUT_BYTES, "composition.raster.output_bounds: encoded PNG exceeds 64 MiB");
    checked_dimensions(planned.width_px, planned.height_px)?;
    let metadata = inspect_png(bytes)?;
    ensure!(metadata.width == planned.width_px && metadata.height == planned.height_px
        && metadata.bit_depth == 8 && metadata.color_type == 6,
        "composition.raster.output_format: cached/output PNG must match the planned dimensions and RGBA8 format");
    let ppm = pixels_per_metre(planned.settings.dpi);
    ensure!(metadata.srgb == Some(0) && metadata.pixel_dims == Some((ppm, ppm, 1)),
        "composition.raster.output_metadata: output must carry perceptual sRGB and the planned rounded pixels-per-metre density");
    decode_png(bytes, planned.width_px, planned.height_px)
}

fn encode_png(pixels: &[u8], planned: &RasterPlan) -> Result<Vec<u8>> {
    ensure!(pixels.len() as u64 == u64::from(planned.width_px) * u64::from(planned.height_px) * 4,
        "composition.raster.output_format: output pixel buffer has an unexpected size");
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, planned.width_px, planned.height_px);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_filter(png::Filter::Sub);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let ppm = pixels_per_metre(planned.settings.dpi);
        encoder.set_pixel_dims(Some(png::PixelDimensions { xppu: ppm, yppu: ppm, unit: png::Unit::Meter }));
        let mut writer = encoder.write_header().context("composition.raster.png_encode: cannot encode PNG header")?;
        writer.write_image_data(pixels).context("composition.raster.png_encode: cannot encode PNG pixels")?;
        writer.finish().context("composition.raster.png_encode: cannot finalize PNG")?;
    }
    ensure!(output.len() <= MAX_OUTPUT_BYTES, "composition.raster.output_bounds: encoded PNG exceeds 64 MiB");
    Ok(output)
}

fn pixels_per_metre(dpi: u32) -> u32 { (dpi * 10_000 + 127) / 254 }

fn normalize_background(background: &str) -> Result<String> {
    if background == "transparent" { return Ok(background.to_owned()); }
    ensure!(background.len() == 7 && background.starts_with('#') && background.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit),
        "composition.raster.background: background must be transparent or an opaque #RRGGBB hex color");
    Ok(background.to_ascii_lowercase())
}

fn background_rgb(background: &str) -> Result<Option<[u8; 3]>> {
    let normalized = normalize_background(background)?;
    if normalized == "transparent" { return Ok(None); }
    Ok(Some([
        u8::from_str_radix(&normalized[1..3], 16)?,
        u8::from_str_radix(&normalized[3..5], 16)?,
        u8::from_str_radix(&normalized[5..7], 16)?,
    ]))
}

fn checked_dimensions(width: u32, height: u32) -> Result<()> {
    ensure!(width > 0 && height > 0 && width <= MAX_AXIS && height <= MAX_AXIS
        && u64::from(width) * u64::from(height) <= MAX_PIXELS,
        "composition.raster.dimensions: images and output canvases must fit 8192 pixels per axis and 16777216 pixels total");
    Ok(())
}

#[derive(Clone)]
struct AssetMetadata {
    format: AssetFormat,
    sha256: String,
    width_px: u32,
    height_px: u32,
}

fn inspect_assets(snapshot: &LoadedComposition) -> Result<BTreeMap<String, AssetMetadata>> {
    let mut assets = BTreeMap::new();
    let mut image_instance_pixels = 0u64;
    for layer in &snapshot.manifest.layers {
        let id = match &layer.content {
            LayerContent::Raster { asset_id } | LayerContent::Vector { asset_id } => asset_id,
            _ => continue,
        };
        if !assets.contains_key(id) {
            let asset = snapshot.assets.get(id).context("composition.raster.asset_missing: referenced captured asset is unavailable")?;
            ensure!(asset.bytes.len() <= MAX_ASSET_BYTES, "composition.raster.asset_bounds: a captured raster asset exceeds 16 MiB");
            let (width_px, height_px) = match asset.declaration.format {
                AssetFormat::Png => {
                    let metadata = inspect_png(&asset.bytes).with_context(|| format!("composition.raster.asset_metadata: PNG asset {id}"))?;
                    (metadata.width, metadata.height)
                },
                AssetFormat::Jpeg => inspect_jpeg(&asset.bytes).with_context(|| format!("composition.raster.asset_metadata: JPEG asset {id}"))?,
                AssetFormat::Svg => {
                    super::raster_svg::inspect(&asset.bytes).with_context(|| format!("composition.raster.asset_metadata: SVG asset {id}"))?;
                    (0, 0)
                },
            };
            if asset.declaration.format != AssetFormat::Svg {
                // Usvg consults this exact header parser after the resolver
                // returns an ImageKind. A codec success alone must not allow
                // the later header lookup to silently omit a captured plate.
                let intrinsic = imagesize::blob_size(&asset.bytes)
                    .context("composition.raster.asset_dimensions: the SVG provider cannot read captured image dimensions")?;
                ensure!(intrinsic.width == width_px as usize && intrinsic.height == height_px as usize,
                    "composition.raster.asset_dimensions: SVG provider and strict preflight disagree about image dimensions");
            }
            assets.insert(id.clone(), AssetMetadata {
                format: asset.declaration.format, sha256: format!("{:x}", Sha256::digest(&asset.bytes)),
                width_px, height_px,
            });
        }
        let metadata = assets.get(id).context("composition.raster.asset_missing: inspected asset is unavailable")?;
        image_instance_pixels = image_instance_pixels.saturating_add(u64::from(metadata.width_px) * u64::from(metadata.height_px));
        ensure!(image_instance_pixels <= MAX_IMAGE_INSTANCE_PIXELS,
            "composition.raster.asset_work_bounds: raster image instances exceed 33554432 decoded pixels; repeated uses count separately");
    }
    Ok(assets)
}

fn inspect_generated_svg(svg: &[u8], allowed: &BTreeSet<(String, String)>) -> Result<()> {
    ensure!(svg.len() <= MAX_OUTPUT_BYTES, "composition.raster.svg_bounds: generated SVG exceeds 64 MiB");
    let text = std::str::from_utf8(svg).context("composition.raster.svg_parse: generated SVG must be UTF-8")?;
    ensure!(!text.contains("<!DOCTYPE") && !text.contains("<!ENTITY"), "composition.raster.svg_scope: DTDs and entities are not admitted");
    let document = roxmltree::Document::parse(text).context("composition.raster.svg_parse: invalid generated SVG XML")?;
    ensure!(document.descendants().count() <= MAX_SVG_NODES, "composition.raster.svg_bounds: generated SVG has too many XML nodes");
    for node in document.descendants() {
        ensure!(!node.is_pi(), "composition.raster.svg_scope: processing instructions are not admitted");
        if !node.is_element() { continue; }
        ensure!(node.tag_name().namespace() == Some("http://www.w3.org/2000/svg")
            && matches!(node.tag_name().name(), "svg" | "title" | "desc" | "metadata" | "g" | "defs" | "clipPath" | "rect" | "ellipse" | "image" | "path"),
            "composition.raster.svg_scope: only generated graphical elements are admitted; text must already be glyph outlines");
        if node.tag_name().name() == "image" {
            for dimension in ["width", "height"] {
                let size: f32 = node.attribute(dimension)
                    .context("composition.raster.svg_geometry: generated image dimensions are required")?
                    .parse().context("composition.raster.svg_geometry: image dimensions must be numeric user units")?;
                ensure!(size.is_finite() && size > 0.0,
                    "composition.raster.svg_geometry: image dimensions disappear or overflow in the raster provider's floating-point geometry");
            }
            ensure!(node.attributes().any(|attribute| attribute.name() == "href"),
                "composition.raster.resource_unavailable: generated image has no captured href");
        }
        for attribute in node.attributes() {
            ensure!(attribute.name() != "style" && !attribute.name().starts_with("on"),
                "composition.raster.svg_scope: CSS and event attributes are not admitted");
            if attribute.name() == "href" {
                ensure!(node.tag_name().name() == "image", "composition.raster.resource_unavailable: href is only admitted for captured images");
                let (prefix, encoded) = attribute.value().split_once(',')
                    .context("composition.raster.resource_unavailable: images require captured base64 data URLs")?;
                let mime = prefix.strip_prefix("data:").and_then(|value| value.strip_suffix(";base64"))
                    .context("composition.raster.resource_unavailable: external and non-base64 image references are forbidden")?;
                ensure!(matches!(mime, "image/png" | "image/jpeg" | "image/svg+xml"), "composition.raster.resource_unavailable: unsupported data MIME type");
                let bytes = STANDARD.decode(encoded).context("composition.raster.resource_unavailable: malformed captured data URL")?;
                ensure!(bytes.len() <= MAX_ASSET_BYTES && allowed.contains(&(mime.to_owned(), format!("{:x}", Sha256::digest(&bytes)))),
                    "composition.raster.resource_unavailable: image data does not match a referenced captured asset");
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct PngMetadata {
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: u8,
    srgb: Option<u8>,
    pixel_dims: Option<(u32, u32, u8)>,
}

fn inspect_png(bytes: &[u8]) -> Result<PngMetadata> {
    ensure!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "composition.raster.png_header: invalid PNG signature");
    let mut position = 8usize;
    let mut metadata = None;
    let mut chunks = BTreeSet::new();
    let mut chunk_count = 0usize;
    let mut saw_idat = false;
    let mut idat_finished = false;
    let mut saw_end = false;
    let mut srgb = None;
    let mut pixel_dims = None;
    while position < bytes.len() {
        chunk_count += 1;
        ensure!(chunk_count <= MAX_CHUNKS, "composition.raster.png_bounds: too many PNG chunks");
        let header = bytes.get(position..position.saturating_add(8)).context("composition.raster.png_chunks: truncated chunk header")?;
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let kind = [header[4], header[5], header[6], header[7]];
        ensure!(kind.iter().all(u8::is_ascii_alphabetic) && kind[2].is_ascii_uppercase(), "composition.raster.png_chunks: malformed or reserved chunk type");
        let data_start = position + 8;
        let data_end = data_start.checked_add(length).context("composition.raster.png_chunks: chunk range overflow")?;
        let end = data_end.checked_add(4).context("composition.raster.png_chunks: CRC range overflow")?;
        let data = bytes.get(data_start..data_end).context("composition.raster.png_chunks: truncated chunk payload")?;
        let crc_bytes = bytes.get(data_end..end).context("composition.raster.png_chunks: truncated chunk CRC")?;
        let mut crc = crc32fast::Hasher::new();
        crc.update(&kind); crc.update(data);
        ensure!(crc.finalize() == u32::from_be_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]]), "composition.raster.png_crc: chunk CRC mismatch");
        ensure!(metadata.is_some() || &kind == b"IHDR", "composition.raster.png_chunks: IHDR must be first");
        if &kind != b"IDAT" && saw_idat { idat_finished = true; }
        if &kind != b"IDAT" {
            ensure!(chunks.insert(kind) || matches!(&kind, b"tEXt" | b"zTXt" | b"iTXt"), "composition.raster.png_chunks: repeated singleton chunk");
        }
        match &kind {
            b"IHDR" => {
                ensure!(position == 8 && data.len() == 13, "composition.raster.png_header: malformed IHDR");
                let width = read_u32(&data[..4]); let height = read_u32(&data[4..8]);
                checked_dimensions(width, height)?;
                ensure!(data[8] == 8 && matches!(data[9], 0 | 2 | 3 | 4 | 6), "composition.raster.png_color: PNG input must use supported 8-bit gray, RGB, palette, gray-alpha, or RGBA samples");
                ensure!(data[10] == 0 && data[11] == 0 && data[12] <= 1, "composition.raster.png_layout: unsupported compression, filter, or interlace");
                metadata = Some((width, height, data[8], data[9]));
            },
            b"IDAT" => { ensure!(!idat_finished, "composition.raster.png_chunks: IDAT chunks must be contiguous"); saw_idat = true; },
            b"IEND" => { ensure!(data.is_empty() && saw_idat && end == bytes.len(), "composition.raster.png_chunks: invalid IEND or trailing bytes"); saw_end = true; },
            b"iCCP" | b"cICP" | b"mDCv" | b"cLLi" | b"eXIf" | b"acTL" | b"fcTL" | b"fdAT" => bail!("composition.raster.png_color: ICC/HDR profiles, EXIF transforms, and animated PNG require a different explicit raster profile"),
            b"sRGB" => { ensure!(!saw_idat && data.len() == 1 && data[0] <= 3, "composition.raster.png_color: malformed sRGB declaration"); srgb = Some(data[0]); },
            b"gAMA" => ensure!(!saw_idat && data.len() == 4 && read_u32(data) == 45_455, "composition.raster.png_color: only standard sRGB gamma metadata is admitted; no gamma conversion is performed"),
            b"cHRM" => {
                const SRGB: [u32; 8] = [31_270, 32_900, 64_000, 33_000, 30_000, 60_000, 15_000, 6_000];
                ensure!(!saw_idat && data.len() == 32 && data.chunks_exact(4).map(read_u32).eq(SRGB), "composition.raster.png_color: chromaticities differ from standard sRGB");
            },
            b"sBIT" => ensure!(!saw_idat && !data.is_empty() && data.iter().all(|value| *value == 8), "composition.raster.png_color: reduced significant-bit declarations are unsupported"),
            b"pHYs" => {
                ensure!(data.len() == 9 && data[8] <= 1 && read_u32(&data[..4]) == read_u32(&data[4..8]), "composition.raster.png_aspect: malformed or non-square pixel density is unsupported");
                pixel_dims = Some((read_u32(&data[..4]), read_u32(&data[4..8]), data[8]));
            },
            b"PLTE" | b"tRNS" => ensure!(!saw_idat, "composition.raster.png_chunks: palette and transparency must precede image data"),
            _ if kind[0].is_ascii_uppercase() => bail!("composition.raster.png_chunks: unknown critical PNG chunk"),
            _ => {},
        }
        position = end;
        if saw_end { break; }
    }
    ensure!(saw_end, "composition.raster.png_chunks: missing IEND");
    let (width, height, bit_depth, color_type) = metadata.context("composition.raster.png_header: missing IHDR")?;
    Ok(PngMetadata { width, height, bit_depth, color_type, srgb, pixel_dims })
}

fn read_u32(bytes: &[u8]) -> u32 { u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) }

fn decode_png(bytes: &[u8], width: u32, height: u32) -> Result<()> {
    checked_dimensions(width, height)?;
    let mut limits = png::Limits::default();
    limits.bytes = MAX_DECODED_BYTES;
    let mut decoder = png::Decoder::new_with_limits(BufReader::new(Cursor::new(bytes)), limits);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().context("composition.raster.png_decode: PNG metadata cannot be decoded")?;
    ensure!(reader.info().width == width && reader.info().height == height
        && reader.info().animation_control.is_none() && reader.info().frame_control.is_none(),
        "composition.raster.png_decode: decoded PNG dimensions or animation state differs from preflight");
    let required = reader.output_buffer_size().context("composition.raster.png_decode: PNG output buffer dimensions overflow")?;
    ensure!(required <= MAX_DECODED_BYTES, "composition.raster.png_bounds: decoded PNG exceeds 64 MiB");
    let mut pixels = vec![0; required];
    let frame = reader.next_frame(&mut pixels).context("composition.raster.png_decode: invalid or truncated PNG pixel stream")?;
    ensure!(frame.width == width && frame.height == height && frame.bit_depth == png::BitDepth::Eight,
        "composition.raster.png_decode: decoded PNG format differs from the admitted frame");
    reader.finish().context("composition.raster.png_decode: PNG trailing chunks are invalid")?;
    Ok(())
}

fn inspect_jpeg(bytes: &[u8]) -> Result<(u32, u32)> {
    ensure!(bytes.starts_with(&[0xff, 0xd8]), "composition.raster.jpeg_header: missing JPEG SOI");
    let mut position = 2usize;
    let mut frame = None;
    let mut scans = 0usize;
    let mut markers = 0usize;
    loop {
        markers += 1;
        ensure!(markers <= MAX_CHUNKS, "composition.raster.jpeg_bounds: too many JPEG markers");
        ensure!(bytes.get(position) == Some(&0xff), "composition.raster.jpeg_chunks: expected marker");
        while bytes.get(position) == Some(&0xff) { position += 1; }
        let marker = *bytes.get(position).context("composition.raster.jpeg_chunks: truncated marker")?;
        position += 1;
        if marker == 0xd9 {
            ensure!(scans > 0 && position == bytes.len(), "composition.raster.jpeg_chunks: missing scan or trailing bytes after EOI");
            break;
        }
        ensure!(!matches!(marker, 0 | 0x01 | 0xd0..=0xd8), "composition.raster.jpeg_chunks: unexpected standalone marker");
        let length_bytes = bytes.get(position..position.saturating_add(2)).context("composition.raster.jpeg_chunks: truncated segment length")?;
        let length = usize::from(u16::from_be_bytes([length_bytes[0], length_bytes[1]]));
        ensure!(length >= 2, "composition.raster.jpeg_chunks: invalid segment length");
        let end = position.checked_add(length).context("composition.raster.jpeg_chunks: segment overflow")?;
        let data = bytes.get(position + 2..end).context("composition.raster.jpeg_chunks: truncated segment")?;
        position = end;
        match marker {
            0xe0 if data.starts_with(b"JFIF\0") => {
                ensure!(data.len() >= 14, "composition.raster.jpeg_aspect: truncated JFIF density declaration");
                let x_density = u16::from_be_bytes([data[8], data[9]]);
                let y_density = u16::from_be_bytes([data[10], data[11]]);
                let thumbnail_bytes = usize::from(data[12]) * usize::from(data[13]) * 3;
                ensure!(data[7] <= 2 && x_density > 0 && x_density == y_density && data.len() == 14 + thumbnail_bytes,
                    "composition.raster.jpeg_aspect: malformed or non-square JFIF pixel density is unsupported");
            },
            0xc0 | 0xc2 => {
                ensure!(frame.is_none() && scans == 0 && data.len() >= 6 && data[0] == 8,
                    "composition.raster.jpeg_frame: only one 8-bit baseline/progressive frame is admitted");
                let height = u32::from(u16::from_be_bytes([data[1], data[2]]));
                let width = u32::from(u16::from_be_bytes([data[3], data[4]]));
                checked_dimensions(width, height)?;
                ensure!(matches!(data[5], 1 | 3) && data.len() == 6 + usize::from(data[5]) * 3,
                    "composition.raster.jpeg_color: only grayscale or three-component JPEG is admitted; CMYK/YCCK are unsupported");
                frame = Some((width, height));
            },
            0xc1 | 0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf | 0xdc => bail!("composition.raster.jpeg_frame: unsupported JPEG frame/coding mode"),
            0xe1 => bail!("composition.raster.jpeg_color: EXIF/XMP metadata needs explicit orientation and color handling"),
            0xe2 if data.starts_with(b"ICC_PROFILE\0") => bail!("composition.raster.jpeg_color: ICC color conversion is unavailable"),
            0xee => bail!("composition.raster.jpeg_color: Adobe APP14 color transforms are unsupported"),
            0xda => {
                ensure!(frame.is_some(), "composition.raster.jpeg_frame: scan precedes frame");
                scans += 1;
                ensure!(scans <= 64, "composition.raster.jpeg_bounds: JPEG exceeds 64 scans");
                loop {
                    let byte = *bytes.get(position).context("composition.raster.jpeg_chunks: truncated entropy-coded scan")?;
                    if byte != 0xff { position += 1; continue; }
                    let marker_start = position;
                    while bytes.get(position) == Some(&0xff) { position += 1; }
                    let next = *bytes.get(position).context("composition.raster.jpeg_chunks: truncated scan marker")?;
                    if next == 0 || (0xd0..=0xd7).contains(&next) { position += 1; continue; }
                    position = marker_start;
                    break;
                }
            },
            _ => {},
        }
    }
    frame.context("composition.raster.jpeg_frame: no supported JPEG frame")
}

fn decode_jpeg(bytes: &[u8], width: u32, height: u32) -> Result<()> {
    use zune_jpeg::zune_core::{colorspace::ColorSpace, options::DecoderOptions};
    checked_dimensions(width, height)?;
    let options = DecoderOptions::default().set_max_width(MAX_AXIS as usize).set_max_height(MAX_AXIS as usize)
        .set_strict_mode(true).jpeg_set_max_scans(64).jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(bytes), options);
    decoder.decode_headers().context("composition.raster.jpeg_decode: JPEG headers cannot be decoded")?;
    let info = decoder.info().context("composition.raster.jpeg_decode: JPEG lacks decoded image information")?;
    ensure!(u32::from(info.width) == width && u32::from(info.height) == height
        && decoder.output_colorspace() == Some(ColorSpace::RGBA),
        "composition.raster.jpeg_decode: decoder dimensions or color space differ from preflight");
    let required = decoder.output_buffer_size().context("composition.raster.jpeg_decode: JPEG output buffer dimensions overflow")?;
    ensure!(required <= MAX_DECODED_BYTES && required as u64 == u64::from(width) * u64::from(height) * 4,
        "composition.raster.jpeg_decode: provider output buffer differs from the bounded RGBA canvas");
    let pixels = decoder.decode().context("composition.raster.jpeg_decode: JPEG pixel stream cannot be decoded strictly")?;
    ensure!(pixels.len() as u64 == u64::from(width) * u64::from(height) * 4 && pixels.len() <= MAX_DECODED_BYTES,
        "composition.raster.jpeg_decode: decoded JPEG output has an unexpected size");
    Ok(())
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;
