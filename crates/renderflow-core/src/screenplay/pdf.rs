//! Independent, bounded structural inspection for the afterwriting Courier profile.
//!
//! This deliberately verifies a much smaller claim than visual screenplay fidelity:
//! a passive PDF with correctly sized pages and nonempty text that references only
//! the four standard Courier fonts. Standard PDF fonts are not embedded, so readers
//! can substitute their own font programs. No text extraction, rasterization, font
//! program qualification, PDF/A conformance, or byte reproducibility is implied.

use std::collections::{BTreeSet, HashSet};

use anyhow::Result;
use lopdf::{content::Content, Dictionary, Document, LoadOptions, Object, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SCREENPLAY_PDF_INSPECTION_SCHEMA_V1: &str = "renderflow.screenplay-pdf-inspection/v1";
pub const MAX_SCREENPLAY_PDF_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_SCREENPLAY_PDF_PAGES: usize = 1_000;
const MAX_OBJECTS: usize = 100_000;
const MAX_OBJECT_VALUES: usize = 500_000;
const MAX_OBJECT_DEPTH: usize = 32;
const MAX_DECOMPRESSED_STREAM_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL_CONTENT_BYTES: usize = 32 * 1024 * 1024;
const MAX_PAGE_OPERATIONS: usize = 100_000;
const BOX_TOLERANCE_PT: f64 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedScreenplayPdf {
    pub paper_width_pt: f64,
    pub paper_height_pt: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayPdfFontEvidence {
    pub resource_name: String,
    pub base_font: String,
    pub subtype: String,
    pub encoding: String,
    /// The adopted base-14 profile deliberately does not embed font programs.
    pub embedded: bool,
    pub used_by_text: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayPdfPageEvidence {
    pub page_number: u32,
    pub media_box_pt: [f64; 4],
    pub crop_box_pt: Option<[f64; 4]>,
    pub rotation: i64,
    pub decoded_content_bytes: usize,
    pub nonempty_text_operations: usize,
    pub fonts: Vec<ScreenplayPdfFontEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayPdfMetadataEntry {
    pub key: String,
    /// Present only when the exact stored bytes are UTF-8 (not PDFDocEncoding).
    pub utf8_value: Option<String>,
    pub byte_length: usize,
    pub value_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayPdfInspection {
    pub schema_version: String,
    pub inspector: String,
    pub byte_length: usize,
    pub sha256: String,
    pub page_count: usize,
    pub pages: Vec<ScreenplayPdfPageEvidence>,
    pub metadata: Vec<ScreenplayPdfMetadataEntry>,
    pub trailer_id_sha256: Vec<String>,
    pub metadata_stream_sha256: Option<String>,
    pub byte_reproducibility_verified: bool,
    pub source_text_equivalence_verified: bool,
    pub visual_fidelity_verified: bool,
    pub nondeterminism_notes: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct ScreenplayPdfInspectionError {
    pub code: &'static str,
    pub message: String,
}

fn diagnostic(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    ScreenplayPdfInspectionError { code, message: message.into() }.into()
}

fn require(condition: bool, code: &'static str, message: impl Into<String>) -> Result<()> {
    if condition { Ok(()) } else { Err(diagnostic(code, message)) }
}

fn structure(error: impl std::fmt::Display) -> anyhow::Error {
    diagnostic("screenplay_pdf.invalid_structure", error.to_string())
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn resolve<'a>(doc: &'a Document, mut value: &'a Object) -> Result<&'a Object> {
    let mut seen = HashSet::new();
    for _ in 0..MAX_OBJECT_DEPTH {
        match value {
            Object::Reference(id) => {
                require(seen.insert(*id), "screenplay_pdf.invalid_structure", "cyclic PDF object reference")?;
                value = doc.objects.get(id).ok_or_else(|| structure("dangling PDF object reference"))?;
            }
            _ => return Ok(value),
        }
    }
    Err(structure("PDF object reference chain exceeds the depth limit"))
}

fn dictionary<'a>(doc: &'a Document, object: &'a Object) -> Result<&'a Dictionary> {
    resolve(doc, object)?.as_dict().map_err(structure)
}

fn number(object: &Object) -> Result<f64> {
    let value = match object {
        Object::Integer(value) => *value as f64,
        Object::Real(value) => f64::from(*value),
        _ => return Err(diagnostic("screenplay_pdf.invalid_number", "PDF coordinate is not numeric")),
    };
    require(value.is_finite(), "screenplay_pdf.invalid_number", "PDF coordinate is not finite")?;
    Ok(value)
}

fn rectangle(value: &Object) -> Result<[f64; 4]> {
    let values = value.as_array().map_err(structure)?;
    require(values.len() == 4, "screenplay_pdf.invalid_geometry", "PDF box must contain four coordinates")?;
    let result = [number(&values[0])?, number(&values[1])?, number(&values[2])?, number(&values[3])?];
    require(result[0] < result[2] && result[1] < result[3], "screenplay_pdf.invalid_geometry", "PDF box is empty or reversed")?;
    Ok(result)
}

fn same_box(actual: [f64; 4], expected: [f64; 4]) -> bool {
    actual.iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() <= BOX_TOLERANCE_PT)
}

// Scan every parsed object, including unreachable objects, without following
// references recursively. Stream payloads are not decompressed by this scan.
fn inspect_object(doc: &Document, object: &Object, depth: usize, budget: &mut usize) -> Result<()> {
    require(depth <= MAX_OBJECT_DEPTH && *budget < MAX_OBJECT_VALUES, "screenplay_pdf.object_limit", "PDF exceeds the object nesting or value limit")?;
    *budget += 1;
    match object {
        Object::Dictionary(value) => inspect_dictionary(doc, value, depth, budget)?,
        Object::Stream(value) => {
            require(!value.dict.has(b"F") && !value.dict.has(b"FFilter") && !value.dict.has(b"FDecodeParms"), "screenplay_pdf.external_stream", "external PDF streams are outside the screenplay profile")?;
            inspect_dictionary(doc, &value.dict, depth, budget)?;
        }
        Object::Array(values) => {
            for value in values { inspect_object(doc, value, depth + 1, budget)?; }
        }
        Object::Reference(id) => require(doc.objects.contains_key(id), "screenplay_pdf.invalid_structure", "PDF contains a dangling reference")?,
        Object::Real(value) => require(value.is_finite(), "screenplay_pdf.invalid_number", "PDF contains a nonfinite number")?,
        _ => {}
    }
    Ok(())
}

fn inspect_dictionary(doc: &Document, value: &Dictionary, depth: usize, budget: &mut usize) -> Result<()> {
    const FORBIDDEN_KEYS: &[&[u8]] = &[
        b"OpenAction", b"AA", b"A", b"JS", b"JavaScript", b"AcroForm", b"XFA",
        b"Annots", b"Names", b"EmbeddedFiles", b"EF", b"AF", b"Collection",
        b"RichMedia", b"RichMediaContent", b"Movie", b"Sound", b"Launch",
        b"URI", b"SubmitForm", b"ImportData", b"OCProperties", b"PresSteps",
    ];
    for (key, object) in value.iter() {
        require(!FORBIDDEN_KEYS.contains(&key.as_slice()), "screenplay_pdf.active_content", format!("PDF contains unsupported /{}", String::from_utf8_lossy(key)))?;
        inspect_object(doc, object, depth + 1, budget)?;
    }
    if let Ok(kind) = value.get(b"Type").and_then(Object::as_name) {
        require(![b"Action".as_slice(), b"Filespec", b"EmbeddedFile", b"Annot"].contains(&kind), "screenplay_pdf.active_content", "PDF contains an action, attachment, or annotation object")?;
        require(!matches!(kind, b"ObjStm" | b"XRef"), "screenplay_pdf.invalid_structure", "compressed object and cross-reference streams are outside the adopted PDFKit profile")?;
    }
    Ok(())
}

fn collect_pages(
    doc: &Document,
    id: ObjectId,
    parent: Option<ObjectId>,
    depth: usize,
    seen: &mut HashSet<ObjectId>,
    pages: &mut Vec<ObjectId>,
) -> Result<usize> {
    require(depth < MAX_OBJECT_DEPTH && seen.insert(id), "screenplay_pdf.invalid_structure", "PDF page tree is too deep, cyclic, or repeats a node")?;
    let object = doc.objects.get(&id).ok_or_else(|| structure("missing page tree object"))?;
    let node = object.as_dict().map_err(structure)?;
    match parent {
        Some(parent) => require(node.get(b"Parent").and_then(Object::as_reference).ok() == Some(parent), "screenplay_pdf.invalid_structure", "PDF page tree Parent does not match its containing node")?,
        None => require(!node.has(b"Parent"), "screenplay_pdf.invalid_structure", "root Pages node has a Parent")?,
    }
    match node.get(b"Type").and_then(Object::as_name).map_err(structure)? {
        b"Page" => {
            require(parent.is_some() && !node.has(b"Kids") && !node.has(b"Count"), "screenplay_pdf.invalid_structure", "invalid leaf Page node")?;
            require(pages.len() < MAX_SCREENPLAY_PDF_PAGES, "screenplay_pdf.page_limit", "PDF exceeds the page limit")?;
            pages.push(id);
            Ok(1)
        }
        b"Pages" => {
            let count = node.get(b"Count").and_then(Object::as_i64).map_err(structure)?;
            require(count > 0 && count <= MAX_SCREENPLAY_PDF_PAGES as i64, "screenplay_pdf.page_limit", "PDF page tree declares an empty or excessive page count")?;
            let children = node.get(b"Kids").and_then(Object::as_array).map_err(structure)?;
            require(!children.is_empty() && children.len() <= MAX_SCREENPLAY_PDF_PAGES, "screenplay_pdf.page_limit", "PDF page tree has an empty or excessive Kids array")?;
            let mut actual = 0;
            for child in children {
                actual += collect_pages(doc, child.as_reference().map_err(structure)?, Some(id), depth + 1, seen, pages)?;
            }
            require(actual == count as usize, "screenplay_pdf.page_count_mismatch", "PDF page tree Count disagrees with reachable pages")?;
            Ok(actual)
        }
        _ => Err(structure("page tree object is neither Page nor Pages")),
    }
}

fn inherited<'a>(doc: &'a Document, mut id: ObjectId, key: &[u8]) -> Result<Option<&'a Object>> {
    let mut seen = HashSet::new();
    for _ in 0..MAX_OBJECT_DEPTH {
        require(seen.insert(id), "screenplay_pdf.invalid_structure", "PDF inheritance cycle")?;
        let node = doc.objects.get(&id).ok_or_else(|| structure("missing page ancestor"))?.as_dict().map_err(structure)?;
        if let Ok(value) = node.get(key) { return resolve(doc, value).map(Some); }
        match node.get(b"Parent") {
            Ok(parent) => id = parent.as_reference().map_err(structure)?,
            Err(_) => return Ok(None),
        }
    }
    Err(structure("PDF inheritance exceeds depth limit"))
}

fn fonts(doc: &Document, resources: &Dictionary) -> Result<Vec<ScreenplayPdfFontEvidence>> {
    for (key, _) in resources.iter() {
        require(matches!(key.as_slice(), b"Font" | b"ProcSet"), "screenplay_pdf.unsupported_resource", format!("resource /{} is outside the text-only screenplay profile", String::from_utf8_lossy(key)))?;
    }
    let fonts = dictionary(doc, resources.get(b"Font").map_err(structure)?)?;
    require(!fonts.is_empty() && fonts.len() <= 4, "screenplay_pdf.invalid_font", "the screenplay profile requires between one and four Courier font resources")?;
    let mut evidence = Vec::new();
    for (name, font) in fonts.iter() {
        let font = dictionary(doc, font)?;
        require(font.get(b"Type").and_then(Object::as_name).ok() == Some(b"Font") && font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type1"), "screenplay_pdf.invalid_font", "screenplay font is not a standard Type1 font")?;
        let base = font.get(b"BaseFont").and_then(Object::as_name).map_err(structure)?;
        require(matches!(base, b"Courier" | b"Courier-Bold" | b"Courier-Oblique" | b"Courier-BoldOblique"), "screenplay_pdf.font_mismatch", "screenplay PDF references a font outside the declared Courier family")?;
        require(font.get(b"Encoding").and_then(Object::as_name).ok() == Some(b"WinAnsiEncoding"), "screenplay_pdf.font_mismatch", "Courier font must use WinAnsiEncoding")?;
        for (key, _) in font.iter() {
            require(matches!(key.as_slice(), b"Type" | b"Subtype" | b"BaseFont" | b"Encoding"), "screenplay_pdf.font_mismatch", "custom font widths, mappings, descriptors, or programs are outside the base-14 profile")?;
        }
        evidence.push(ScreenplayPdfFontEvidence {
            resource_name: std::str::from_utf8(name).map_err(structure)?.to_owned(),
            base_font: std::str::from_utf8(base).map_err(structure)?.to_owned(),
            subtype: "Type1".to_owned(), encoding: "WinAnsiEncoding".to_owned(),
            embedded: false, used_by_text: false,
        });
    }
    evidence.sort_by(|left, right| left.resource_name.cmp(&right.resource_name));
    Ok(evidence)
}

fn inspect_content(bytes: &[u8], fonts: &mut [ScreenplayPdfFontEvidence]) -> Result<usize> {
    let content = Content::decode_strict(bytes).map_err(|error| diagnostic("screenplay_pdf.invalid_content", error.to_string()))?;
    require(content.operations.len() <= MAX_PAGE_OPERATIONS, "screenplay_pdf.content_limit", "PDF page exceeds the operation limit")?;
    let mut graphics_depth = 0_usize;
    let mut in_text = false;
    let mut font = None;
    let mut nonempty_text = 0;
    for operation in content.operations {
        let operands = &operation.operands;
        match operation.operator.as_str() {
            "q" if operands.is_empty() && !in_text => {
                require(graphics_depth < MAX_OBJECT_DEPTH, "screenplay_pdf.invalid_content", "PDF graphics state nesting exceeds limit")?;
                graphics_depth += 1;
            }
            "Q" if operands.is_empty() && !in_text && graphics_depth > 0 => { graphics_depth -= 1; font = None; }
            "BT" if operands.is_empty() && !in_text => { in_text = true; font = None; }
            "ET" if operands.is_empty() && in_text => { in_text = false; }
            "Tf" if in_text && operands.len() == 2 => {
                let name = operands[0].as_name().map_err(structure)?;
                let size = number(&operands[1])?;
                require(size > 0.0 && size <= 1_000.0, "screenplay_pdf.invalid_content", "PDF uses an invalid text size")?;
                font = Some(fonts.iter().position(|entry| entry.resource_name.as_bytes() == name).ok_or_else(|| diagnostic("screenplay_pdf.invalid_font", "text references an undeclared font resource"))?);
            }
            "Tj" | "TJ" if in_text && operands.len() == 1 => {
                let font = font.ok_or_else(|| diagnostic("screenplay_pdf.invalid_font", "text has no explicitly selected font"))?;
                let mut visible = false;
                match (&operation.operator[..], &operands[0]) {
                    ("Tj", Object::String(text, _)) => visible = nonempty_ascii_text(text)?,
                    ("TJ", Object::Array(values)) => {
                        for value in values {
                            match value {
                                Object::String(text, _) => visible |= nonempty_ascii_text(text)?,
                                Object::Integer(_) | Object::Real(_) => { number(value)?; }
                                _ => return Err(diagnostic("screenplay_pdf.invalid_content", "invalid TJ text array")),
                            }
                        }
                    }
                    _ => return Err(diagnostic("screenplay_pdf.invalid_content", "invalid text show operands")),
                }
                if visible { nonempty_text += 1; fonts[font].used_by_text = true; }
            }
            "Tr" if in_text && operands.len() == 1 => {
                require(operands[0].as_i64().ok() == Some(0), "screenplay_pdf.invalid_content", "screenplay text must use filled, non-clipping rendering")?;
            }
            "Tm" if in_text => numeric_operands(operands, 6)?,
            "Td" | "TD" if in_text => numeric_operands(operands, 2)?,
            "Tc" | "Tw" | "Tz" | "TL" | "Ts" if in_text => numeric_operands(operands, 1)?,
            "T*" if in_text && operands.is_empty() => {}
            "cm" if !in_text => numeric_operands(operands, 6)?,
            "m" | "l" if !in_text => numeric_operands(operands, 2)?,
            "c" if !in_text => numeric_operands(operands, 6)?,
            "v" | "y" | "re" if !in_text => numeric_operands(operands, 4)?,
            "w" | "J" | "j" | "M" | "i" => numeric_operands(operands, 1)?,
            "g" | "G" => numeric_operands(operands, 1)?,
            "rg" | "RG" => numeric_operands(operands, 3)?,
            "k" | "K" => numeric_operands(operands, 4)?,
            "cs" | "CS" if operands.len() == 1 => {
                require(matches!(operands[0].as_name().ok(), Some(b"DeviceGray" | b"DeviceRGB" | b"DeviceCMYK")), "screenplay_pdf.invalid_content", "unsupported PDF color space")?;
            }
            "sc" | "SC" | "scn" | "SCN" if matches!(operands.len(), 1 | 3 | 4) => {
                for operand in operands { number(operand)?; }
            }
            "d" if operands.len() == 2 => {
                for dash in operands[0].as_array().map_err(structure)? { number(dash)?; }
                number(&operands[1])?;
            }
            "h" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" | "W" | "W*" if !in_text && operands.is_empty() => {}
            _ => return Err(diagnostic("screenplay_pdf.unsupported_content", format!("PDF operation {} is malformed or outside the adopted text profile", operation.operator))),
        }
    }
    require(!in_text && graphics_depth == 0, "screenplay_pdf.invalid_content", "PDF text or graphics state is unbalanced")?;
    require(nonempty_text > 0, "screenplay_pdf.empty_page", "PDF page contains no nonempty text operations")?;
    Ok(nonempty_text)
}

fn numeric_operands(operands: &[Object], count: usize) -> Result<()> {
    require(operands.len() == count, "screenplay_pdf.invalid_content", "PDF operator has the wrong operand count")?;
    for operand in operands { number(operand)?; }
    Ok(())
}

fn nonempty_ascii_text(bytes: &[u8]) -> Result<bool> {
    require(bytes.iter().all(|byte| (b' '..=b'~').contains(byte)), "screenplay_pdf.unsupported_text", "PDF text contains bytes outside the adopted printable ASCII profile")?;
    Ok(bytes.iter().any(|byte| *byte != b' '))
}

fn inspect_page(doc: &Document, id: ObjectId, index: usize, expected: ExpectedScreenplayPdf, total_content: &mut usize) -> Result<ScreenplayPdfPageEvidence> {
    let page = doc.objects.get(&id).ok_or_else(|| structure("missing page"))?.as_dict().map_err(structure)?;
    let media = rectangle(inherited(doc, id, b"MediaBox")?.ok_or_else(|| diagnostic("screenplay_pdf.invalid_geometry", "page has no MediaBox"))?)?;
    let expected_box = [0.0, 0.0, expected.paper_width_pt, expected.paper_height_pt];
    require(same_box(media, expected_box), "screenplay_pdf.paper_mismatch", format!("page {} MediaBox differs from the configured paper", index + 1))?;
    let crop = inherited(doc, id, b"CropBox")?.map(rectangle).transpose()?;
    if let Some(crop) = crop { require(same_box(crop, expected_box), "screenplay_pdf.paper_mismatch", "PDF CropBox changes the configured paper area")?; }
    for key in [b"TrimBox".as_slice(), b"BleedBox", b"ArtBox"] {
        if let Ok(value) = page.get(key) {
            require(same_box(rectangle(resolve(doc, value)?)?, expected_box), "screenplay_pdf.paper_mismatch", "PDF page has a nonstandard print box")?;
        }
    }
    let rotation = inherited(doc, id, b"Rotate")?.map(Object::as_i64).transpose().map_err(structure)?.unwrap_or(0);
    require(rotation == 0, "screenplay_pdf.paper_mismatch", "rotated PDF pages are outside the screenplay profile")?;
    if let Ok(unit) = page.get(b"UserUnit") { require(number(resolve(doc, unit)?)? == 1.0, "screenplay_pdf.paper_mismatch", "nonstandard PDF UserUnit")?; }
    let resources = inherited(doc, id, b"Resources")?.ok_or_else(|| diagnostic("screenplay_pdf.invalid_font", "page has no resources"))?.as_dict().map_err(structure)?;
    let mut fonts = fonts(doc, resources)?;
    let contents = page.get(b"Contents").map_err(structure)?;
    // The adopted PDFKit profile emits one indirect content stream per page.
    let stream_id = contents.as_reference().map_err(structure)?;
    let stream = doc.objects.get(&stream_id).ok_or_else(|| structure("missing content stream"))?.as_stream().map_err(structure)?;
    if let Ok(filter) = stream.dict.get(b"Filter") {
        require(filter.as_name().ok() == Some(b"FlateDecode") && !stream.dict.has(b"DecodeParms"), "screenplay_pdf.unsupported_content", "page content uses an unsupported stream filter")?;
    }
    let content = doc.get_page_content_with_limit(id, MAX_DECOMPRESSED_STREAM_BYTES).map_err(|error| diagnostic("screenplay_pdf.invalid_content", error.to_string()))?;
    *total_content = total_content.checked_add(content.len()).ok_or_else(|| diagnostic("screenplay_pdf.content_limit", "PDF decoded content size overflow"))?;
    require(*total_content <= MAX_TOTAL_CONTENT_BYTES, "screenplay_pdf.content_limit", "PDF exceeds the total decoded content limit")?;
    let nonempty_text_operations = inspect_content(&content, &mut fonts)?;
    Ok(ScreenplayPdfPageEvidence { page_number: (index + 1) as u32, media_box_pt: media, crop_box_pt: crop, rotation, decoded_content_bytes: content.len(), nonempty_text_operations, fonts })
}

fn metadata(doc: &Document) -> Result<Vec<ScreenplayPdfMetadataEntry>> {
    let Ok(info) = doc.trailer.get(b"Info") else { return Ok(Vec::new()); };
    let info = dictionary(doc, info)?;
    require(info.len() <= 32, "screenplay_pdf.metadata_limit", "PDF Info dictionary has too many entries")?;
    let mut result = Vec::new();
    for (key, value) in info.iter() {
        let value = resolve(doc, value)?.as_str().map_err(structure)?;
        require(value.len() <= 4_096, "screenplay_pdf.metadata_limit", "PDF metadata value exceeds the limit")?;
        result.push(ScreenplayPdfMetadataEntry {
            key: std::str::from_utf8(key).map_err(structure)?.to_owned(),
            utf8_value: std::str::from_utf8(value).ok().map(str::to_owned),
            byte_length: value.len(), value_sha256: digest(value),
        });
    }
    result.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(result)
}

/// Validate the passive Courier-only PDF subset independently of the provider.
///
/// The caller supplies paper dimensions from its own configuration. Returned
/// evidence never claims source-text equivalence, visual fidelity, or identical
/// bytes across runs. Dates and IDs are observed without rewriting the PDF.
pub fn inspect_pdf(bytes: &[u8], expected: ExpectedScreenplayPdf) -> Result<ScreenplayPdfInspection> {
    require(expected.paper_width_pt.is_finite() && expected.paper_height_pt.is_finite() && expected.paper_width_pt > 0.0 && expected.paper_height_pt > 0.0 && expected.paper_width_pt <= 2_000.0 && expected.paper_height_pt <= 2_000.0, "screenplay_pdf.invalid_expectation", "configured paper dimensions are invalid")?;
    require(!bytes.is_empty() && bytes.len() <= MAX_SCREENPLAY_PDF_BYTES, "screenplay_pdf.byte_limit", "PDF is empty or exceeds the byte limit")?;
    require(bytes.starts_with(b"%PDF-"), "screenplay_pdf.invalid_structure", "PDF header is missing")?;
    let end = bytes.iter().rposition(|byte| !byte.is_ascii_whitespace()).map_or(0, |position| position + 1);
    require(bytes[..end].ends_with(b"%%EOF"), "screenplay_pdf.invalid_structure", "PDF EOF marker is missing")?;
    // lopdf bounds each parser-decompressed stream here. Its object-count and
    // aggregate allocation limits are not configurable; our object/value limits
    // apply after loading. This is not a hard process-wide parser memory cap.
    let doc = Document::load_mem_with_options(bytes, LoadOptions { strict: true, max_decompressed_size: Some(MAX_DECOMPRESSED_STREAM_BYTES), ..LoadOptions::default() }).map_err(structure)?;
    require(!doc.is_encrypted() && !doc.trailer.has(b"Encrypt"), "screenplay_pdf.encrypted", "encrypted PDFs cannot be independently inspected")?;
    require(!doc.trailer.has(b"Prev") && !doc.trailer.has(b"XRefStm"), "screenplay_pdf.invalid_structure", "incremental and hybrid PDF updates are outside the screenplay profile")?;
    require(doc.objects.len() <= MAX_OBJECTS, "screenplay_pdf.object_limit", "PDF exceeds the object limit")?;
    let mut budget = 0;
    inspect_dictionary(&doc, &doc.trailer, 0, &mut budget)?;
    for object in doc.objects.values() { inspect_object(&doc, object, 0, &mut budget)?; }
    let catalog = dictionary(&doc, doc.trailer.get(b"Root").map_err(structure)?)?;
    require(catalog.get(b"Type").and_then(Object::as_name).ok() == Some(b"Catalog"), "screenplay_pdf.invalid_structure", "PDF Root is not a Catalog")?;
    let root = catalog.get(b"Pages").and_then(Object::as_reference).map_err(structure)?;
    let mut page_ids = Vec::new();
    collect_pages(&doc, root, None, 0, &mut HashSet::new(), &mut page_ids)?;
    let mut total_content = 0;
    let mut pages = Vec::new();
    for (index, id) in page_ids.into_iter().enumerate() { pages.push(inspect_page(&doc, id, index, expected, &mut total_content)?); }
    let metadata = metadata(&doc)?;
    let mut trailer_id_sha256 = Vec::new();
    if let Ok(ids) = doc.trailer.get(b"ID") {
        let ids = resolve(&doc, ids)?.as_array().map_err(structure)?;
        require(ids.len() == 2, "screenplay_pdf.invalid_structure", "PDF trailer ID must contain two strings")?;
        for id in ids {
            let id = resolve(&doc, id)?.as_str().map_err(structure)?;
            require(!id.is_empty() && id.len() <= 128, "screenplay_pdf.metadata_limit", "PDF trailer ID length is invalid")?;
            trailer_id_sha256.push(digest(id));
        }
    }
    let metadata_stream_sha256 = catalog.get(b"Metadata").ok().map(|value| -> Result<String> {
        let stream = resolve(&doc, value)?.as_stream().map_err(structure)?;
        require(stream.content.len() <= MAX_DECOMPRESSED_STREAM_BYTES, "screenplay_pdf.metadata_limit", "PDF metadata stream is too large")?;
        Ok(digest(&stream.content))
    }).transpose()?;
    let keys: BTreeSet<&str> = metadata.iter().map(|entry| entry.key.as_str()).collect();
    let mut nondeterminism_notes = vec!["A single observed PDF cannot establish byte reproducibility; no repeated rendering or metadata normalization was performed.".to_owned()];
    if keys.contains("CreationDate") || keys.contains("ModDate") { nondeterminism_notes.push("PDF Info contains date metadata that can change between invocations.".to_owned()); }
    if !trailer_id_sha256.is_empty() { nondeterminism_notes.push("PDF trailer IDs are present; their hashes are recorded without assuming deterministic generation.".to_owned()); }
    if metadata_stream_sha256.is_some() { nondeterminism_notes.push("An opaque metadata stream is present; only its encoded bytes were hashed.".to_owned()); }
    Ok(ScreenplayPdfInspection {
        schema_version: SCREENPLAY_PDF_INSPECTION_SCHEMA_V1.to_owned(),
        inspector: "renderflow.screenplay-pdf-inspector/v1+lopdf-0.45.0".to_owned(),
        byte_length: bytes.len(), sha256: digest(bytes), page_count: pages.len(), pages,
        metadata, trailer_id_sha256, metadata_stream_sha256,
        byte_reproducibility_verified: false, source_text_equivalence_verified: false, visual_fidelity_verified: false,
        nondeterminism_notes,
        limitations: vec![
            "Structural inspection does not compare emitted screenplay text, pagination, margins, clipping, line breaks, or visual layout with the source.".to_owned(),
            "The declared base-14 Courier fonts are not embedded; PDF viewers may substitute font programs and render differently.".to_owned(),
            "Font resource names and text operators are checked, but glyph coverage and rendering are not qualified.".to_owned(),
            "This is a bounded passive PDF subset inspection, not PDF/A certification or a general PDF security audit.".to_owned(),
            "The PDF parser has a per-stream decompression limit; aggregate parser allocations occur before the inspector's object-count checks and are not capped by a process-wide memory quota.".to_owned(),
        ],
    })
}

#[cfg(test)]
#[path = "pdf_tests.rs"]
mod tests;
