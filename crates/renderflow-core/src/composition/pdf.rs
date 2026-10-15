//! One-page, opaque RGB PDFs from the already measured composition SVG.
//!
//! This is a rasterized review/export profile, not a color-managed print proof.
//! The canonical layered package and transcripts retain editability and text.
//! No fonts, ICC profiles, output intents, PDF/X, or accessibility tags are
//! synthesized. All PDF objects and byte lengths come from captured inputs.
//!
//! Cached output admission compares the exact current-provider PDF envelope.
//! It never feeds arbitrary cached bytes to a general PDF parser. Pixel bytes
//! are opaque data of a fixed, bounded length; the executor separately verifies
//! the artifact digest and cache identity.

use std::io::{BufReader, Cursor, Write};

use anyhow::{ensure, Context, Result};
use lopdf::{content::{Content, Operation}, dictionary, xref::XrefType, Document, Object, Stream};

use super::raster;
use super::LoadedComposition;

pub use super::pdf_plan::{plan, BindingSide, PdfPlan, PdfSettings, PDF_PROVIDER_PROFILE, PDF_SCHEMA_V1};

const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_RGBA_BYTES: usize = 64 * 1024 * 1024;
const MAX_RGB_BYTES: usize = 48 * 1024 * 1024;
const MAX_ENVELOPE_BYTES: usize = 16 * 1024;

/// Render the same positioned glyph outlines and captured artwork as PNG,
/// then flatten its opaque RGBA frame into a single DeviceRGB image XObject.
pub(crate) fn render_pdf(
    snapshot: &LoadedComposition,
    svg: &[u8],
    planned: &PdfPlan,
) -> Result<Vec<u8>> {
    ensure!(plan(snapshot, &planned.settings)? == *planned,
        "composition.pdf.plan_mismatch: PDF evidence differs from the captured composition and current provider");
    let png = raster::render_png(snapshot, svg, &planned.raster)?;
    let rgb = decode_opaque_png(&png, planned)?;
    drop(png);
    let output = encode_pdf(rgb, planned)?;
    // encode_pdf consumes and drops the RGB document before validation builds
    // its bounded zero-filled reference document.
    validate_output(&output, planned)?;
    Ok(output)
}

/// Validate only this provider's canonical PDF envelope. This is intentionally
/// narrower than general PDF conformance or independent visual qualification.
///
/// The reference document uses at most 48 MiB of zero RGB samples. Its writer
/// retains only the small structural prefix/suffix; no second PDF-sized byte
/// vector or decompression buffer is allocated during admission.
pub(crate) fn validate_output(bytes: &[u8], planned: &PdfPlan) -> Result<()> {
    super::pdf_plan::validate_plan(planned)?;
    ensure!(bytes.len() <= MAX_OUTPUT_BYTES,
        "composition.pdf.output_bounds: PDF output exceeds 64 MiB");
    let expected = envelope(planned)?;
    ensure!(bytes.len() == expected.position
        && bytes.starts_with(&expected.prefix)
        && bytes.ends_with(&expected.suffix),
        "composition.pdf.output_contract: PDF differs from the exact planned boxes, image, placement, resources, or canonical structure");
    Ok(())
}

fn rgb_length(planned: &PdfPlan) -> Result<usize> {
    let count = u64::from(planned.raster.width_px)
        .checked_mul(u64::from(planned.raster.height_px))
        .and_then(|pixels| pixels.checked_mul(3))
        .context("composition.pdf.pixel_bounds: RGB byte length overflow")?;
    ensure!(count > 0 && count <= MAX_RGB_BYTES as u64,
        "composition.pdf.pixel_bounds: RGB image exceeds 48 MiB");
    usize::try_from(count).context("composition.pdf.pixel_bounds: RGB byte length is not addressable")
}

fn decode_opaque_png(bytes: &[u8], planned: &PdfPlan) -> Result<Vec<u8>> {
    raster::validate_output(bytes, &planned.raster)?;
    let expected_rgb = rgb_length(planned)?;
    let expected_rgba = expected_rgb / 3 * 4;
    ensure!(expected_rgba <= MAX_RGBA_BYTES,
        "composition.pdf.pixel_bounds: RGBA image exceeds 64 MiB");
    let mut limits = png::Limits::default();
    limits.bytes = MAX_RGBA_BYTES;
    let decoder = png::Decoder::new_with_limits(BufReader::new(Cursor::new(bytes)), limits);
    let mut reader = decoder.read_info()
        .context("composition.pdf.png_decode: cannot read composed PNG")?;
    ensure!(reader.info().width == planned.raster.width_px
        && reader.info().height == planned.raster.height_px
        && reader.info().color_type == png::ColorType::Rgba
        && reader.info().bit_depth == png::BitDepth::Eight
        && reader.info().animation_control.is_none()
        && reader.info().frame_control.is_none()
        && reader.output_buffer_size() == Some(expected_rgba),
        "composition.pdf.png_decode: composed frame differs from the bounded RGBA8 plan");
    let mut rgba = vec![0; expected_rgba];
    let frame = reader.next_frame(&mut rgba)
        .context("composition.pdf.png_decode: cannot decode composed PNG pixels")?;
    ensure!(frame.width == planned.raster.width_px
        && frame.height == planned.raster.height_px
        && frame.color_type == png::ColorType::Rgba
        && frame.bit_depth == png::BitDepth::Eight
        && frame.buffer_size() == expected_rgba,
        "composition.pdf.png_decode: decoded frame differs from the planned RGBA8 canvas");
    reader.finish().context("composition.pdf.png_decode: invalid PNG trailing chunks")?;
    opaque_rgb(&rgba, expected_rgb)
}

fn opaque_rgb(rgba: &[u8], expected_rgb: usize) -> Result<Vec<u8>> {
    ensure!(expected_rgb > 0 && expected_rgb <= MAX_RGB_BYTES && expected_rgb % 3 == 0
        && rgba.len() == expected_rgb / 3 * 4,
        "composition.pdf.pixel_bounds: RGBA sample length differs from the planned image");
    ensure!(rgba.chunks_exact(4).all(|pixel| pixel[3] == 255),
        "composition.pdf.alpha: PDF requires an opaque composed frame; transparency is not silently discarded");
    let mut rgb = Vec::with_capacity(expected_rgb);
    for pixel in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&pixel[..3]);
    }
    Ok(rgb)
}

fn encode_pdf(rgb: Vec<u8>, planned: &PdfPlan) -> Result<Vec<u8>> {
    let length = rgb_length(planned)?;
    ensure!(rgb.len() == length,
        "composition.pdf.pixel_bounds: RGB sample length differs from the planned image");
    let mut document = document(rgb, length, planned)?;
    let mut output = Vec::with_capacity(length + MAX_ENVELOPE_BYTES);
    document.save_to(&mut output).context("composition.pdf.encode: cannot serialize composed PDF")?;
    ensure!(output.len() <= MAX_OUTPUT_BYTES,
        "composition.pdf.output_bounds: PDF output exceeds 64 MiB");
    Ok(output)
}

fn rectangle(values: [f64; 4]) -> Object {
    Object::Array(values.into_iter().map(|value| Object::Real(value as f32)).collect())
}

fn content(planned: &PdfPlan) -> Result<Vec<u8>> {
    let width = planned.media_box_pt[2];
    let height = planned.media_box_pt[3];
    Content {
        operations: vec![
            Operation::new("q", Vec::new()),
            // Keep the uniformly sampled image's subpixel right/bottom excess
            // outside the physical canvas, rather than stretching either axis.
            Operation::new("re", vec![0.into(), 0.into(), Object::Real(width as f32), Object::Real(height as f32)]),
            Operation::new("W", Vec::new()),
            Operation::new("n", Vec::new()),
            Operation::new("cm", planned.image_draw_matrix.into_iter()
                .map(|value| Object::Real(value as f32)).collect()),
            Operation::new("Do", vec![Object::Name(b"Im0".to_vec())]),
            Operation::new("Q", Vec::new()),
        ],
    }.encode().context("composition.pdf.encode: cannot serialize the planned page drawing")
}

fn document(rgb: Vec<u8>, declared_length: usize, planned: &PdfPlan) -> Result<Document> {
    let mut document = Document::with_version("1.4");
    // Document::new defaults to an xref stream even for a 1.4 version string.
    // The fixed profile explicitly uses a classic table and no object streams.
    document.reference_table.cross_reference_type = XrefType::CrossReferenceTable;
    let pages_id = document.new_object_id();
    let mut image = Stream::new(dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => i64::from(planned.raster.width_px),
        "Height" => i64::from(planned.raster.height_px),
        "ColorSpace" => "DeviceRGB",
        "BitsPerComponent" => 8,
        "Interpolate" => false,
    }, rgb);
    // The empty reference skeleton uses the actual declared length so its
    // payload offset has exactly the same numeral width as the final PDF.
    image.dict.set("Length", declared_length as i64);
    image.allows_compression = false;
    let image_id = document.add_object(image);
    ensure!(image_id == (2, 0),
        "composition.pdf.provider_contract: fixed image object identity changed");
    let mut drawing = Stream::new(dictionary! {}, content(planned)?);
    drawing.allows_compression = false;
    let drawing_id = document.add_object(drawing);
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => rectangle(planned.media_box_pt),
        "CropBox" => rectangle(planned.media_box_pt),
        "BleedBox" => rectangle(planned.bleed_box_pt),
        "TrimBox" => rectangle(planned.trim_box_pt),
        "Rotate" => 0,
        "Resources" => dictionary! {
            "XObject" => dictionary! { "Im0" => image_id },
        },
        "Contents" => drawing_id,
    });
    document.objects.insert(pages_id, Object::Dictionary(dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => 1,
    }));
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);
    Ok(document)
}

fn envelope(planned: &PdfPlan) -> Result<EnvelopeWriter> {
    let length = rgb_length(planned)?;
    let mut skeleton = document(Vec::new(), length, planned)?;
    let mut bytes = Vec::new();
    skeleton.save_to(&mut bytes)
        .context("composition.pdf.provider_contract: cannot serialize reference skeleton")?;
    ensure!(bytes.len() <= MAX_ENVELOPE_BYTES,
        "composition.pdf.provider_contract: reference skeleton exceeds 16 KiB");
    let object_start = find_bytes(&bytes, b"2 0 obj\n")
        .context("composition.pdf.provider_contract: missing reference image object")?;
    let marker = b">>stream\n";
    let relative = find_bytes(&bytes[object_start..], marker)
        .context("composition.pdf.provider_contract: missing reference image payload")?;
    ensure!(relative <= 1024,
        "composition.pdf.provider_contract: reference image dictionary is too large");
    let pixel_start = object_start + relative + marker.len();
    ensure!(bytes[pixel_start..].starts_with(b"\nendstream \nendobj\n"),
        "composition.pdf.provider_contract: reference image must contain an empty raw stream");
    let pixel_end = pixel_start.checked_add(length)
        .context("composition.pdf.pixel_bounds: image payload range overflow")?;
    let mut expected = EnvelopeWriter {
        pixel_start, pixel_end, position: 0, prefix: Vec::new(), suffix: Vec::new(),
    };
    // The pinned writer computes every offset and trailer byte itself. The
    // sink discards the zero image samples while retaining the entire envelope.
    let mut reference = document(vec![0; length], length, planned)?;
    reference.save_to(&mut expected)
        .context("composition.pdf.provider_contract: cannot serialize reference envelope")?;
    ensure!(expected.prefix.len() == pixel_start
        && expected.position >= pixel_end
        && expected.suffix.len() == expected.position - pixel_end,
        "composition.pdf.provider_contract: reference image span is inconsistent");
    Ok(expected)
}

fn find_bytes(bytes: &[u8], needle: &[u8]) -> Option<usize> {
    bytes.windows(needle.len()).position(|window| window == needle)
}

/// Records only the structural bytes around a known raw image interval.
/// Neither input PDF bytes nor pixel values influence its boundaries.
struct EnvelopeWriter {
    pixel_start: usize,
    pixel_end: usize,
    position: usize,
    prefix: Vec<u8>,
    suffix: Vec<u8>,
}

impl Write for EnvelopeWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self.position.checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("PDF reference byte count overflow"))?;
        if end > MAX_OUTPUT_BYTES {
            return Err(std::io::Error::other("PDF reference exceeds 64 MiB"));
        }
        let prefix_length = self.pixel_start.saturating_sub(self.position).min(bytes.len());
        let suffix_start = self.pixel_end.saturating_sub(self.position).min(bytes.len());
        let retained = self.prefix.len() + self.suffix.len() + prefix_length + bytes.len() - suffix_start;
        if retained > MAX_ENVELOPE_BYTES {
            return Err(std::io::Error::other("PDF reference envelope exceeds 16 KiB"));
        }
        self.prefix.extend_from_slice(&bytes[..prefix_length]);
        self.suffix.extend_from_slice(&bytes[suffix_start..]);
        self.position = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::raster::{RasterPlan, RasterSettings, RASTER_PROVIDER_PROFILE, RASTER_SCHEMA_V1};

    fn tiny_plan() -> PdfPlan {
        let settings = PdfSettings { dpi: 36, ..PdfSettings::default() };
        PdfPlan {
            schema: PDF_SCHEMA_V1.to_owned(),
            provider_profile: PDF_PROVIDER_PROFILE.to_owned(),
            settings: settings.clone(),
            raster: RasterPlan {
                schema: RASTER_SCHEMA_V1.to_owned(),
                provider_profile: RASTER_PROVIDER_PROFILE.to_owned(),
                settings: RasterSettings { dpi: settings.dpi, background: settings.background.clone() },
                width_px: 2, height_px: 2,
                pixel_format: "rgba8".to_owned(), alpha: "straight".to_owned(),
                color_policy: "assumed_srgb_no_profile_conversion".to_owned(),
                runtime_qualified: false,
            },
            media_box_pt: [0.0, 0.0, 4.0, 4.0],
            trim_box_pt: [0.0, 0.0, 4.0, 4.0],
            bleed_box_pt: [0.0, 0.0, 4.0, 4.0],
            safe_box_pt: [0.0, 0.0, 4.0, 4.0],
            gutter_pt: 0.0,
            image_draw_matrix: [4.0, 0.0, 0.0, 4.0, 0.0, 0.0],
            runtime_qualified: false,
        }
    }

    #[test]
    fn exact_envelope_accepts_pixels_and_refuses_structure_changes() {
        let planned = tiny_plan();
        let rgb = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let bytes = encode_pdf(rgb.clone(), &planned).unwrap();
        validate_output(&bytes, &planned).unwrap();
        let expected = envelope(&planned).unwrap();
        assert_eq!(&bytes[expected.pixel_start..expected.pixel_end], rgb.as_slice());
        let mut changed = bytes.clone();
        let position = find_bytes(&changed, b"/TrimBox").unwrap();
        changed[position + 1] = b'C';
        assert!(validate_output(&changed, &planned).is_err());
        assert!(validate_output(&bytes[..bytes.len() - 1], &planned).is_err());
        let mut trailing = bytes;
        trailing.extend_from_slice(b"\n% extra revision");
        assert!(validate_output(&trailing, &planned).is_err());
    }

    #[test]
    fn generated_pdf_has_one_image_and_explicit_page_boxes() {
        let planned = tiny_plan();
        let bytes = encode_pdf(vec![255; 12], &planned).unwrap();
        // This parser sees only the just-generated, envelope-validated fixture.
        validate_output(&bytes, &planned).unwrap();
        let document = Document::load_mem_with_options(&bytes, lopdf::LoadOptions {
            strict: true, max_decompressed_size: Some(1024), ..Default::default()
        }).unwrap();
        assert_eq!(document.objects.len(), 5);
        let page = document.get_dictionary((4, 0)).unwrap();
        for name in [b"MediaBox".as_slice(), b"CropBox", b"BleedBox", b"TrimBox"] {
            let actual = page.get(name).unwrap().as_array().unwrap();
            let values = actual.iter().map(|value| match value {
                Object::Integer(number) => *number as f64,
                Object::Real(number) => f64::from(*number),
                _ => panic!("box coordinates must be numeric"),
            }).collect::<Vec<_>>();
            assert_eq!(values, vec![0.0, 0.0, 4.0, 4.0]);
        }
        let image = document.get_object((2, 0)).unwrap().as_stream().unwrap();
        assert_eq!(image.dict.get(b"ColorSpace").unwrap().as_name().unwrap(), b"DeviceRGB");
        assert!(image.dict.get(b"Filter").is_err());
        assert!(image.dict.get(b"SMask").is_err());
        assert_eq!(image.content, vec![255; 12]);
    }

    #[test]
    fn alpha_is_never_silently_discarded() {
        assert_eq!(opaque_rgb(&[1, 2, 3, 255], 3).unwrap(), vec![1, 2, 3]);
        assert!(opaque_rgb(&[1, 2, 3, 254], 3).is_err());
        assert!(opaque_rgb(&[1, 2, 3, 255], 6).is_err());
    }


    #[test]
    fn validator_refuses_actions_filters_changed_boxes_and_drawing() {
        let planned = tiny_plan();
        for mutation in 0..4 {
            let mut document = document(vec![255; 12], 12, &planned).unwrap();
            match mutation {
                0 => document.get_object_mut((5, 0)).unwrap().as_dict_mut().unwrap()
                    .set("OpenAction", dictionary! { "S" => "JavaScript", "JS" => Object::string_literal("void(0)") }),
                1 => document.get_object_mut((2, 0)).unwrap().as_stream_mut().unwrap()
                    .dict.set("Filter", "FlateDecode"),
                2 => document.get_object_mut((4, 0)).unwrap().as_dict_mut().unwrap()
                    .set("TrimBox", rectangle([0.0, 0.0, 3.0, 4.0])),
                _ => document.get_object_mut((3, 0)).unwrap().as_stream_mut().unwrap()
                    .set_content(b"q\n3 0 0 4 0 0 cm\n/Im0 Do\nQ".to_vec()),
            }
            let mut bytes = Vec::new();
            document.save_to(&mut bytes).unwrap();
            assert!(validate_output(&bytes, &planned).is_err());
        }
    }

    #[test]
    fn validator_refuses_wrong_lengths_xref_and_pixel_extent() {
        let planned = tiny_plan();
        let bytes = encode_pdf(vec![255; 12], &planned).unwrap();
        let expected = envelope(&planned).unwrap();
        let mut shortened = bytes.clone();
        shortened.remove(expected.pixel_start);
        assert!(validate_output(&shortened, &planned).is_err());
        let mut enlarged = bytes.clone();
        enlarged.insert(expected.pixel_start, 0);
        assert!(validate_output(&enlarged, &planned).is_err());
        let mut wrong_length = bytes.clone();
        let length_position = find_bytes(&wrong_length, b"/Length 12").unwrap();
        wrong_length[length_position + b"/Length 1".len()] = b'1';
        assert!(validate_output(&wrong_length, &planned).is_err());
        let mut wrong_xref = bytes;
        let xref_prefix = b"xref\n0 6\n0000000000 65535 f \n";
        let xref_position = find_bytes(&wrong_xref, xref_prefix).unwrap();
        wrong_xref[xref_position + xref_prefix.len()] = b'1';
        assert!(validate_output(&wrong_xref, &planned).is_err());
    }

    #[test]
    fn raw_image_bytes_remain_data_even_when_they_resemble_pdf_syntax() {
        let planned = tiny_plan();
        let bytes = encode_pdf(b"endstream\n()".to_vec(), &planned).unwrap();
        validate_output(&bytes, &planned).unwrap();
    }

    #[test]
    fn envelope_sink_handles_writes_crossing_both_boundaries() {
        let mut sink = EnvelopeWriter {
            pixel_start: 3, pixel_end: 7, position: 0,
            prefix: Vec::new(), suffix: Vec::new(),
        };
        sink.write_all(b"0123456789").unwrap();
        assert_eq!(sink.prefix, b"012");
        assert_eq!(sink.suffix, b"789");
        assert_eq!(sink.position, 10);
    }
}
