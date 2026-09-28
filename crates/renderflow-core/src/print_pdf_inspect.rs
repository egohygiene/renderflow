//! Independent structural inspection of a deliberately narrow print-interior PDF.
//!
//! This validates the PDF object tree and the single, full-bleed image drawn on
//! each page. The expected image-stream hash is computed from the immutable
//! source by the caller, independently of the PDF. It proves output ordering
//! for the supported passthrough JPEG and PNG IDAT routes, even when every page
//! has the same geometry. It is not a general PDF conformance or rasterization
//! engine: image codec validity and visual color fidelity require separate
//! evidence.

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Result;
use lopdf::{content::Content, Dictionary, Document, LoadOptions, Object, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PRINT_PDF_INSPECTION_SCHEMA_V1: &str = "renderflow.print_pdf_inspection/v1";
pub const MAX_PRINT_PDF_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PRINT_PDF_PAGES: usize = 1_000;
const MAX_DECOMPRESSED_STREAM_BYTES: usize = 16 * 1024 * 1024;
const MAX_PAGE_CONTENT_BYTES: usize = 1024 * 1024;
const BOX_TOLERANCE_PT: f64 = 0.02;

/// Exact, independently obtained source expectations for one ordered page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedPrintPage {
    pub media_width_pt: f64,
    pub media_height_pt: f64,
    pub trim_inset_pt: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
    /// Rotation in clockwise degrees; the initial print route supports zero.
    pub rotation: u16,
    /// Exactly `DeviceRGB` or `DeviceGray` for this route.
    pub color_space: String,
    /// `DCTDecode` for JPEG or `FlateDecode` for direct PNG IDAT embedding.
    pub image_filter: String,
    /// SHA-256 of the bytes the provider must embed as its image stream.
    pub image_stream_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintPdfPageEvidence {
    pub page_number: u32,
    pub media_box_pt: [f64; 4],
    pub bleed_box_pt: [f64; 4],
    pub trim_box_pt: [f64; 4],
    pub crop_box_pt: Option<[f64; 4]>,
    pub rotation: u16,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub color_space: String,
    pub image_filter: String,
    pub image_stream_sha256: String,
    /// PDF current-transformation matrix used to draw the sole page image.
    pub image_draw_matrix: [f64; 6],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintPdfInspection {
    pub schema: String,
    pub inspector: String,
    pub byte_length: u64,
    pub sha256: String,
    pub page_count: usize,
    pub pages: Vec<PrintPdfPageEvidence>,
}

/// Downcastable, stable diagnostic code for a failed PDF inspection.
#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct PrintPdfInspectionError {
    pub code: &'static str,
    pub message: String,
}

fn diagnostic(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    PrintPdfInspectionError {
        code,
        message: message.into(),
    }
    .into()
}

fn require(condition: bool, code: &'static str, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(diagnostic(code, message))
    }
}

fn parse_number(object: &Object) -> Result<f64> {
    let value = match object {
        Object::Integer(value) => *value as f64,
        Object::Real(value) => f64::from(*value),
        _ => {
            return Err(diagnostic(
                "print_pdf.invalid_number",
                "PDF box or matrix has a nonnumeric coordinate",
            ))
        }
    };
    require(
        value.is_finite(),
        "print_pdf.invalid_number",
        "PDF coordinate is not finite",
    )?;
    Ok(value)
}

fn rectangle(doc: &Document, page: &Dictionary, key: &[u8], page_number: u32) -> Result<[f64; 4]> {
    let value = page.get_deref(key, doc).map_err(|_| {
        diagnostic(
            "print_pdf.missing_box",
            format!(
                "page {page_number} is missing /{}",
                String::from_utf8_lossy(key)
            ),
        )
    })?;
    rectangle_value(value, page_number)
}

fn rectangle_value(value: &Object, page_number: u32) -> Result<[f64; 4]> {
    let coordinates = value.as_array().map_err(|_| {
        diagnostic(
            "print_pdf.invalid_box",
            format!("page {page_number} has an invalid box array"),
        )
    })?;
    require(
        coordinates.len() == 4,
        "print_pdf.invalid_box",
        format!("page {page_number} box needs four coordinates"),
    )?;
    let box_coordinates = [
        parse_number(&coordinates[0])?,
        parse_number(&coordinates[1])?,
        parse_number(&coordinates[2])?,
        parse_number(&coordinates[3])?,
    ];
    require(
        box_coordinates[0] < box_coordinates[2] && box_coordinates[1] < box_coordinates[3],
        "print_pdf.invalid_box",
        format!("page {page_number} has an empty or reversed box"),
    )?;
    Ok(box_coordinates)
}

// /Rotate and /CropBox may be inherited from a Pages ancestor. Refusing an
// unnoticed inherited value is necessary to verify effective page geometry.
fn inherited_page_entry<'a>(
    doc: &'a Document,
    page_id: ObjectId,
    key: &[u8],
) -> Result<Option<&'a Object>> {
    let mut current = page_id;
    let mut visited = HashSet::new();
    for _ in 0..32 {
        require(
            visited.insert(current),
            "print_pdf.invalid_structure",
            "PDF page tree contains a parent cycle",
        )?;
        let node = doc
            .get_dictionary(current)
            .map_err(|error| diagnostic("print_pdf.invalid_structure", error.to_string()))?;
        if let Ok(value) = node.get(key) {
            return doc
                .dereference(value)
                .map(|(_, value)| Some(value))
                .map_err(|error| diagnostic("print_pdf.invalid_structure", error.to_string()));
        }
        match node.get(b"Parent") {
            Ok(parent) => {
                current = parent.as_reference().map_err(|_| {
                    diagnostic(
                        "print_pdf.invalid_structure",
                        "PDF page Parent is not indirect",
                    )
                })?;
            }
            Err(_) => return Ok(None),
        }
    }
    Err(diagnostic(
        "print_pdf.invalid_structure",
        "PDF page tree exceeds the inspector depth limit",
    ))
}

fn same_box(actual: [f64; 4], expected: [f64; 4]) -> bool {
    actual
        .iter()
        .zip(expected.iter())
        .all(|(actual, expected)| (actual - expected).abs() <= BOX_TOLERANCE_PT)
}

fn checked_expected(expected: &[ExpectedPrintPage]) -> Result<()> {
    require(
        !expected.is_empty(),
        "print_pdf.invalid_expectation",
        "at least one page is required",
    )?;
    require(
        expected.len() <= MAX_PRINT_PDF_PAGES,
        "print_pdf.page_limit",
        "expected page count exceeds the inspector limit",
    )?;
    for (index, page) in expected.iter().enumerate() {
        require(
            page.media_width_pt.is_finite()
                && page.media_height_pt.is_finite()
                && page.trim_inset_pt.is_finite()
                && page.media_width_pt > 0.0
                && page.media_height_pt > 0.0
                && page.trim_inset_pt >= 0.0
                && page.trim_inset_pt * 2.0 < page.media_width_pt
                && page.trim_inset_pt * 2.0 < page.media_height_pt
                && page.pixel_width > 0
                && page.pixel_height > 0,
            "print_pdf.invalid_expectation",
            format!(
                "expected page {} has invalid dimensions or trim inset",
                index + 1
            ),
        )?;
        require(
            page.rotation == 0,
            "print_pdf.unsupported_rotation",
            "the proven print route supports only unrotated pages",
        )?;
        require(
            matches!(page.color_space.as_str(), "DeviceRGB" | "DeviceGray")
                && matches!(page.image_filter.as_str(), "DCTDecode" | "FlateDecode"),
            "print_pdf.invalid_expectation",
            format!(
                "expected page {} has an unsupported image color space or filter",
                index + 1
            ),
        )?;
        require(
            page.image_stream_sha256.len() == 64
                && page
                    .image_stream_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "print_pdf.invalid_expectation",
            format!(
                "expected page {} has an invalid image-stream SHA-256",
                index + 1
            ),
        )?;
    }
    Ok(())
}

/// Inspect a bounded PDF against independently measured, ordered page inputs.
///
/// The PDF's image stream must equal each expected source-derived stream hash
/// in sequence. The method checks parsed PDF structure, explicit print boxes,
/// page image resources, pixels, color/filter, and the sole drawing operation.
/// It intentionally refuses page artwork overlays, annotations, nonzero
/// rotation, and non-full-bleed placement because they are not proven here.
pub fn inspect_print_pdf(
    path: &Path,
    expected: &[ExpectedPrintPage],
) -> Result<PrintPdfInspection> {
    checked_expected(expected)?;
    let file =
        File::open(path).map_err(|error| diagnostic("print_pdf.unreadable", error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| diagnostic("print_pdf.unreadable", error.to_string()))?;
    require(
        metadata.len() <= MAX_PRINT_PDF_BYTES,
        "print_pdf.byte_limit",
        "PDF exceeds the inspector byte limit",
    )?;
    let mut pdf_bytes = Vec::new();
    file.take(MAX_PRINT_PDF_BYTES + 1)
        .read_to_end(&mut pdf_bytes)
        .map_err(|error| diagnostic("print_pdf.unreadable", error.to_string()))?;
    require(
        pdf_bytes.len() as u64 <= MAX_PRINT_PDF_BYTES,
        "print_pdf.byte_limit",
        "PDF grew beyond the inspector byte limit",
    )?;
    require(
        pdf_bytes.starts_with(b"%PDF-"),
        "print_pdf.invalid_structure",
        "PDF header is missing",
    )?;
    let output_digest = format!("{:x}", Sha256::digest(&pdf_bytes));
    let doc = Document::load_mem_with_options(
        &pdf_bytes,
        LoadOptions {
            strict: true,
            max_decompressed_size: Some(MAX_DECOMPRESSED_STREAM_BYTES),
            ..LoadOptions::default()
        },
    )
    .map_err(|error| diagnostic("print_pdf.invalid_structure", error.to_string()))?;
    require(
        !doc.is_encrypted(),
        "print_pdf.encrypted",
        "encrypted PDFs cannot be independently inspected",
    )?;

    let catalog = doc
        .catalog()
        .map_err(|error| diagnostic("print_pdf.invalid_structure", error.to_string()))?;
    require(
        ![b"OpenAction".as_slice(), b"AA", b"AcroForm", b"Names"]
            .iter()
            .any(|key| catalog.has(key)),
        "print_pdf.unexpected_content",
        "PDF catalog declares active actions, forms, or name trees outside the print route",
    )?;
    let pages_root = catalog
        .get(b"Pages")
        .and_then(Object::as_reference)
        .and_then(|id| doc.get_dictionary(id))
        .map_err(|error| diagnostic("print_pdf.invalid_structure", error.to_string()))?;
    let declared_count = pages_root
        .get(b"Count")
        .and_then(Object::as_i64)
        .map_err(|error| diagnostic("print_pdf.invalid_structure", error.to_string()))?;
    let pages = doc.get_pages();
    require(
        declared_count >= 0
            && declared_count as usize == pages.len()
            && pages.len() == expected.len(),
        "print_pdf.page_count_mismatch",
        format!(
            "PDF declares {declared_count} pages, parser reached {}, expected {}",
            pages.len(),
            expected.len()
        ),
    )?;
    let unique_pages: HashSet<ObjectId> = pages.values().copied().collect();
    require(
        unique_pages.len() == pages.len(),
        "print_pdf.invalid_structure",
        "PDF page tree repeats a page object",
    )?;

    let mut inspected = Vec::with_capacity(pages.len());
    for (number, id) in pages {
        let source = &expected[(number - 1) as usize];
        inspected.push(inspect_page(&doc, number, id, source)?);
    }
    Ok(PrintPdfInspection {
        schema: PRINT_PDF_INSPECTION_SCHEMA_V1.to_string(),
        inspector: "renderflow.print_pdf_inspector/v1+lopdf-0.45.0".to_string(),
        byte_length: pdf_bytes.len() as u64,
        sha256: output_digest,
        page_count: inspected.len(),
        pages: inspected,
    })
}

fn inspect_page(
    doc: &Document,
    number: u32,
    id: ObjectId,
    expected: &ExpectedPrintPage,
) -> Result<PrintPdfPageEvidence> {
    let page = doc
        .get_dictionary(id)
        .map_err(|error| diagnostic("print_pdf.invalid_structure", error.to_string()))?;
    require(
        !page.has(b"Annots") && !page.has(b"AA"),
        "print_pdf.unexpected_content",
        format!("page {number} has annotations or additional actions"),
    )?;
    if let Ok(unit) = page.get(b"UserUnit") {
        require(
            (parse_number(unit)? - 1.0).abs() <= f64::EPSILON,
            "print_pdf.invalid_geometry",
            format!("page {number} uses nonstandard UserUnit"),
        )?;
    }
    let media = rectangle(doc, page, b"MediaBox", number)?;
    let bleed = rectangle(doc, page, b"BleedBox", number)?;
    let trim = rectangle(doc, page, b"TrimBox", number)?;
    let crop = inherited_page_entry(doc, id, b"CropBox")?
        .map(|value| rectangle_value(value, number))
        .transpose()?;
    let expected_media = [0.0, 0.0, expected.media_width_pt, expected.media_height_pt];
    let inset = expected.trim_inset_pt;
    let expected_trim = [
        inset,
        inset,
        expected.media_width_pt - inset,
        expected.media_height_pt - inset,
    ];
    require(
        same_box(media, expected_media),
        "print_pdf.media_box_mismatch",
        format!("page {number} has unexpected MediaBox {media:?}"),
    )?;
    require(
        same_box(bleed, expected_media),
        "print_pdf.bleed_box_mismatch",
        format!("page {number} has unexpected BleedBox {bleed:?}"),
    )?;
    require(
        same_box(trim, expected_trim),
        "print_pdf.trim_box_mismatch",
        format!("page {number} has unexpected TrimBox {trim:?}"),
    )?;
    if let Some(box_coordinates) = crop {
        require(
            same_box(box_coordinates, expected_media),
            "print_pdf.crop_box_mismatch",
            format!("page {number} has unexpected CropBox {box_coordinates:?}"),
        )?;
    }
    let rotation = inherited_page_entry(doc, id, b"Rotate")?
        .map_or(Ok(0_i64), Object::as_i64)
        .map_err(|error| diagnostic("print_pdf.invalid_rotation", error.to_string()))?;
    require(
        rotation == i64::from(expected.rotation),
        "print_pdf.rotation_mismatch",
        format!(
            "page {number} rotation {rotation} differs from expected {}",
            expected.rotation
        ),
    )?;

    let resources = page
        .get_deref(b"Resources", doc)
        .and_then(Object::as_dict)
        .map_err(|error| {
            diagnostic(
                "print_pdf.missing_image",
                format!("page {number} has no resources: {error}"),
            )
        })?;
    require(
        !resources.has(b"Font"),
        "print_pdf.unexpected_content",
        format!("page {number} has font resources"),
    )?;
    let xobjects = resources
        .get_deref(b"XObject", doc)
        .and_then(Object::as_dict)
        .map_err(|error| {
            diagnostic(
                "print_pdf.missing_image",
                format!("page {number} has no image XObject: {error}"),
            )
        })?;
    require(
        xobjects.len() == 1,
        "print_pdf.image_count_mismatch",
        format!(
            "page {number} has {} XObjects, expected one",
            xobjects.len()
        ),
    )?;
    let (image_name, image_ref) = xobjects.iter().next().expect("checked one XObject");
    let image_id = image_ref.as_reference().map_err(|_| {
        diagnostic(
            "print_pdf.invalid_image",
            format!("page {number} image XObject is not indirect"),
        )
    })?;
    let image = doc
        .get_object(image_id)
        .and_then(Object::as_stream)
        .map_err(|error| {
            diagnostic(
                "print_pdf.invalid_image",
                format!("page {number} image stream is invalid: {error}"),
            )
        })?;
    require(
        image.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"),
        "print_pdf.invalid_image",
        format!("page {number} XObject is not an image"),
    )?;
    require(
        !image.dict.has(b"SMask") && !image.dict.has(b"Mask"),
        "print_pdf.unsupported_image",
        format!("page {number} has image transparency or a mask"),
    )?;
    require(
        ![
            b"Decode".as_slice(),
            b"Interpolate",
            b"Alternates",
            b"ImageMask",
        ]
        .iter()
        .any(|key| image.dict.has(key)),
        "print_pdf.unsupported_image",
        format!("page {number} changes the source image interpretation"),
    )?;
    let width = image
        .dict
        .get(b"Width")
        .and_then(Object::as_i64)
        .map_err(|error| diagnostic("print_pdf.invalid_image", error.to_string()))?;
    let height = image
        .dict
        .get(b"Height")
        .and_then(Object::as_i64)
        .map_err(|error| diagnostic("print_pdf.invalid_image", error.to_string()))?;
    require(
        width == i64::from(expected.pixel_width) && height == i64::from(expected.pixel_height),
        "print_pdf.image_dimensions_mismatch",
        format!(
            "page {number} image is {width}x{height}, expected {}x{}",
            expected.pixel_width, expected.pixel_height
        ),
    )?;
    require(
        image
            .dict
            .get(b"BitsPerComponent")
            .and_then(Object::as_i64)
            .ok()
            == Some(8),
        "print_pdf.unsupported_image",
        format!("page {number} image is not 8-bit"),
    )?;
    let color = image
        .dict
        .get(b"ColorSpace")
        .and_then(Object::as_name)
        .map_err(|_| {
            diagnostic(
                "print_pdf.unsupported_image",
                format!("page {number} image has a non-device color space"),
            )
        })?;
    require(
        color == expected.color_space.as_bytes(),
        "print_pdf.color_space_mismatch",
        format!(
            "page {number} image color space differs from {}",
            expected.color_space
        ),
    )?;
    let filter = image
        .dict
        .get(b"Filter")
        .and_then(Object::as_name)
        .map_err(|_| {
            diagnostic(
                "print_pdf.unsupported_image",
                format!("page {number} image has no single declared filter"),
            )
        })?;
    require(
        filter == expected.image_filter.as_bytes(),
        "print_pdf.image_filter_mismatch",
        format!(
            "page {number} image filter differs from {}",
            expected.image_filter
        ),
    )?;
    let stream_digest = format!("{:x}", Sha256::digest(&image.content));
    require(
        stream_digest == expected.image_stream_sha256,
        "print_pdf.page_order_mismatch",
        format!("page {number} image stream differs from the ordered source"),
    )?;

    let content_ids = doc.get_page_contents(id);
    require(
        content_ids.len() == 1,
        "print_pdf.unexpected_content",
        format!("page {number} needs exactly one content stream"),
    )?;
    let content_bytes = doc
        .get_page_content_with_limit(id, MAX_PAGE_CONTENT_BYTES)
        .map_err(|error| diagnostic("print_pdf.invalid_content", error.to_string()))?;
    let content = Content::decode_strict(&content_bytes)
        .map_err(|error| diagnostic("print_pdf.invalid_content", error.to_string()))?;
    let mut depth = 0_i32;
    let mut matrix = None;
    let mut draw_count = 0;
    for operation in content.operations {
        match operation.operator.as_str() {
            "q" if operation.operands.is_empty() => depth += 1,
            "Q" if operation.operands.is_empty() && depth > 0 => depth -= 1,
            "cm" if operation.operands.len() == 6 && depth == 1 && matrix.is_none() => {
                matrix = Some([
                    parse_number(&operation.operands[0])?,
                    parse_number(&operation.operands[1])?,
                    parse_number(&operation.operands[2])?,
                    parse_number(&operation.operands[3])?,
                    parse_number(&operation.operands[4])?,
                    parse_number(&operation.operands[5])?,
                ]);
            }
            "Do" if operation.operands.len() == 1 && depth == 1 && matrix.is_some() => {
                let name = operation.operands[0].as_name().map_err(|_| {
                    diagnostic(
                        "print_pdf.unexpected_content",
                        format!("page {number} has an invalid image draw"),
                    )
                })?;
                require(
                    name == image_name,
                    "print_pdf.unexpected_content",
                    format!("page {number} draws an unrecognized XObject"),
                )?;
                draw_count += 1;
            }
            _ => {
                return Err(diagnostic(
                    "print_pdf.unexpected_content",
                    format!(
                        "page {number} uses unsupported drawing operation {}",
                        operation.operator
                    ),
                ))
            }
        }
    }
    require(
        depth == 0 && draw_count == 1,
        "print_pdf.unexpected_content",
        format!("page {number} does not draw exactly one balanced image"),
    )?;
    let matrix = matrix.ok_or_else(|| {
        diagnostic(
            "print_pdf.unexpected_content",
            format!("page {number} has no image draw matrix"),
        )
    })?;
    let expected_matrix = [
        expected.media_width_pt,
        0.0,
        0.0,
        expected.media_height_pt,
        0.0,
        0.0,
    ];
    require(
        matrix
            .iter()
            .zip(expected_matrix)
            .all(|(actual, expected)| (actual - expected).abs() <= BOX_TOLERANCE_PT),
        "print_pdf.image_placement_mismatch",
        format!("page {number} image is not drawn full bleed: {matrix:?}"),
    )?;
    Ok(PrintPdfPageEvidence {
        page_number: number,
        media_box_pt: media,
        bleed_box_pt: bleed,
        trim_box_pt: trim,
        crop_box_pt: crop,
        rotation: rotation as u16,
        pixel_width: width as u32,
        pixel_height: height as u32,
        color_space: String::from_utf8_lossy(color).into_owned(),
        image_filter: String::from_utf8_lossy(filter).into_owned(),
        image_stream_sha256: stream_digest,
        image_draw_matrix: matrix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    fn expectation(image_data: &[u8]) -> ExpectedPrintPage {
        ExpectedPrintPage {
            media_width_pt: 100.0,
            media_height_pt: 100.0,
            trim_inset_pt: 5.0,
            pixel_width: 2,
            pixel_height: 2,
            rotation: 0,
            color_space: "DeviceRGB".to_string(),
            image_filter: "FlateDecode".to_string(),
            image_stream_sha256: format!("{:x}", Sha256::digest(image_data)),
        }
    }

    // Structurally representative synthetic PDFs; deliberately fake image
    // compressed bytes are sufficient for this independent object inspector.
    // End-to-end provider tests must inspect actual PNG/JPEG-derived streams.
    fn fixture(
        image_data: &[&[u8]],
        mutate: impl FnOnce(&mut Document, &[ObjectId], &[ObjectId]),
    ) -> tempfile::NamedTempFile {
        let mut document = Document::with_version("1.4");
        let pages_id = document.new_object_id();
        let mut page_ids = Vec::new();
        let mut image_ids = Vec::new();
        for data in image_data {
            let image_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 2,
                    "Height" => 2,
                    "ColorSpace" => "DeviceRGB",
                    "BitsPerComponent" => 8,
                    "Filter" => "FlateDecode",
                },
                data.to_vec(),
            ));
            image_ids.push(image_id);
            let content_id = document.add_object(Stream::new(
                dictionary! {},
                b"q 100 0 0 100 0 0 cm /Im0 Do Q".to_vec(),
            ));
            let page_id = document.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => dictionary! {
                    "XObject" => dictionary! { "Im0" => image_id },
                },
                "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                "BleedBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                "TrimBox" => vec![5.into(), 5.into(), 95.into(), 95.into()],
            });
            page_ids.push(page_id);
        }
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_ids.iter().copied().map(Object::from).collect::<Vec<_>>(),
                "Count" => page_ids.len() as i64,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        mutate(&mut document, &page_ids, &image_ids);
        let file = tempfile::NamedTempFile::new().unwrap();
        document.save(file.path()).unwrap();
        file
    }

    fn code(error: &anyhow::Error) -> &str {
        error
            .downcast_ref::<PrintPdfInspectionError>()
            .expect("typed inspector diagnostic")
            .code
    }

    #[test]
    fn ordered_images_and_print_boxes_are_inspected() {
        let first = b"synthetic image one";
        let second = b"synthetic image two";
        let pdf = fixture(&[first, second], |_, _, _| {});
        let report =
            inspect_print_pdf(pdf.path(), &[expectation(first), expectation(second)]).unwrap();
        assert_eq!(report.page_count, 2);
        assert_eq!(report.pages[0].trim_box_pt, [5.0, 5.0, 95.0, 95.0]);
        assert_eq!(
            report.pages[1].image_stream_sha256,
            expectation(second).image_stream_sha256
        );
        assert_eq!(
            report.pages[0].image_draw_matrix,
            [100.0, 0.0, 0.0, 100.0, 0.0, 0.0]
        );
        assert_eq!(report.sha256.len(), 64);
        assert_eq!(report.schema, PRINT_PDF_INSPECTION_SCHEMA_V1);
    }

    #[test]
    fn same_size_pages_in_wrong_order_are_refused() {
        let first = b"synthetic image one";
        let second = b"synthetic image two";
        let pdf = fixture(&[first, second], |_, _, _| {});
        let error =
            inspect_print_pdf(pdf.path(), &[expectation(second), expectation(first)]).unwrap_err();
        assert_eq!(code(&error), "print_pdf.page_order_mismatch");
    }

    #[test]
    fn page_count_and_geometry_mismatches_are_refused() {
        let pdf = fixture(&[b"page"], |_, _, _| {});
        let input = expectation(b"page");
        assert_eq!(
            code(&inspect_print_pdf(pdf.path(), &[input.clone(), input.clone()]).unwrap_err()),
            "print_pdf.page_count_mismatch"
        );

        let mut wrong_trim = input.clone();
        wrong_trim.trim_inset_pt = 6.0;
        assert_eq!(
            code(&inspect_print_pdf(pdf.path(), &[wrong_trim]).unwrap_err()),
            "print_pdf.trim_box_mismatch"
        );

        let mut wrong_pixels = input;
        wrong_pixels.pixel_width = 3;
        assert_eq!(
            code(&inspect_print_pdf(pdf.path(), &[wrong_pixels]).unwrap_err()),
            "print_pdf.image_dimensions_mismatch"
        );
    }

    #[test]
    fn inherited_rotation_and_crop_cannot_hide_on_page_tree() {
        let rotated = fixture(&[b"page"], |document, page_ids, _| {
            let parent = document
                .get_dictionary(page_ids[0])
                .unwrap()
                .get(b"Parent")
                .unwrap()
                .as_reference()
                .unwrap();
            document
                .get_dictionary_mut(parent)
                .unwrap()
                .set("Rotate", 90);
        });
        assert_eq!(
            code(&inspect_print_pdf(rotated.path(), &[expectation(b"page")]).unwrap_err()),
            "print_pdf.rotation_mismatch"
        );

        let cropped = fixture(&[b"page"], |document, page_ids, _| {
            let parent = document
                .get_dictionary(page_ids[0])
                .unwrap()
                .get(b"Parent")
                .unwrap()
                .as_reference()
                .unwrap();
            document
                .get_dictionary_mut(parent)
                .unwrap()
                .set("CropBox", vec![1.into(), 1.into(), 99.into(), 99.into()]);
        });
        assert_eq!(
            code(&inspect_print_pdf(cropped.path(), &[expectation(b"page")]).unwrap_err()),
            "print_pdf.crop_box_mismatch"
        );
    }

    #[test]
    fn unsupported_images_and_overlay_drawing_are_refused() {
        let pdf = fixture(&[b"page"], |document, _, image_ids| {
            document
                .get_object_mut(image_ids[0])
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set("ColorSpace", "DeviceCMYK");
        });
        assert_eq!(
            code(&inspect_print_pdf(pdf.path(), &[expectation(b"page")]).unwrap_err()),
            "print_pdf.color_space_mismatch"
        );

        let pdf = fixture(&[b"page"], |document, _, image_ids| {
            document
                .get_object_mut(image_ids[0])
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set("Decode", vec![1.into(), 0.into()]);
        });
        assert_eq!(
            code(&inspect_print_pdf(pdf.path(), &[expectation(b"page")]).unwrap_err()),
            "print_pdf.unsupported_image"
        );

        let pdf = fixture(&[b"page"], |document, page_ids, _| {
            let page = document.get_dictionary(page_ids[0]).unwrap();
            let content_id = page.get(b"Contents").unwrap().as_reference().unwrap();
            document
                .get_object_mut(content_id)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .set_content(b"q 100 0 0 100 0 0 cm /Im0 Do 0 0 m 1 1 l S Q".to_vec());
        });
        assert_eq!(
            code(&inspect_print_pdf(pdf.path(), &[expectation(b"page")]).unwrap_err()),
            "print_pdf.unexpected_content"
        );
    }

    #[test]
    fn active_pdf_catalog_content_is_refused() {
        let pdf = fixture(&[b"page"], |document, _, _| {
            document.catalog_mut().unwrap().set(
                "OpenAction",
                dictionary! {
                    "S" => "URI",
                    "URI" => Object::string_literal("https://example.invalid"),
                },
            );
        });
        assert_eq!(
            code(&inspect_print_pdf(pdf.path(), &[expectation(b"page")]).unwrap_err()),
            "print_pdf.unexpected_content"
        );
    }

    #[test]
    fn malformed_and_oversized_inputs_are_refused_before_acceptance() {
        let invalid = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(invalid.path(), b"%PDF-not-a-real-document").unwrap();
        assert_eq!(
            code(&inspect_print_pdf(invalid.path(), &[expectation(b"page")]).unwrap_err()),
            "print_pdf.invalid_structure"
        );

        let oversized = tempfile::NamedTempFile::new().unwrap();
        oversized
            .as_file()
            .set_len(MAX_PRINT_PDF_BYTES + 1)
            .unwrap();
        assert_eq!(
            code(&inspect_print_pdf(oversized.path(), &[expectation(b"page")]).unwrap_err()),
            "print_pdf.byte_limit"
        );
    }
}
