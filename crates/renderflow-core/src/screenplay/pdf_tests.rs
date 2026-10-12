//! Authored synthetic parser coverage; no test or native PDF rendering was run
//! for this checkpoint. These fixtures do not qualify afterwriting or a viewer.

use super::*;
use lopdf::{dictionary, Stream, StringFormat};

const TEXT: &[u8] = b"1 0 0 -1 0 792 cm q BT 1 0 0 -1 108 72 Tm /F1 12 Tf [(INT. LAB - DAY)] TJ ET Q";

#[derive(Clone, Copy)]
struct FixtureIds {
    pages: ObjectId,
    page: ObjectId,
    font: ObjectId,
    content: ObjectId,
    catalog: ObjectId,
}

fn expected() -> ExpectedScreenplayPdf {
    ExpectedScreenplayPdf { paper_width_pt: 612.0, paper_height_pt: 792.0 }
}

fn fixture(mutate: impl FnOnce(&mut Document, FixtureIds)) -> Vec<u8> {
    let mut doc = Document::with_version("1.3");
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let pages = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier",
        "Encoding" => "WinAnsiEncoding",
    });
    let content = doc.add_object(Stream::new(dictionary! {}, TEXT.to_vec()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "Contents" => content,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
    });
    doc.objects.insert(pages, Object::Dictionary(dictionary! {
        "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
    }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    mutate(&mut doc, FixtureIds { pages, page, font, content, catalog });
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn error_code(bytes: &[u8]) -> &'static str {
    inspect_pdf(bytes, expected()).unwrap_err().downcast_ref::<ScreenplayPdfInspectionError>().unwrap().code
}

fn set_content(doc: &mut Document, ids: FixtureIds, bytes: &[u8]) {
    doc.objects.insert(ids.content, Object::Stream(Stream::new(dictionary! {}, bytes.to_vec())));
}

#[test]
fn passive_courier_text_pdf_has_independent_bounded_evidence() {
    let bytes = fixture(|_, _| {});
    let evidence = inspect_pdf(&bytes, expected()).unwrap();
    assert_eq!(evidence.schema_version, SCREENPLAY_PDF_INSPECTION_SCHEMA_V1);
    assert_eq!(evidence.sha256, digest(&bytes));
    assert_eq!(evidence.byte_length, bytes.len());
    assert_eq!(evidence.page_count, 1);
    assert_eq!(evidence.pages[0].media_box_pt, [0.0, 0.0, 612.0, 792.0]);
    assert_eq!(evidence.pages[0].nonempty_text_operations, 1);
    assert_eq!(evidence.pages[0].fonts[0].base_font, "Courier");
    assert!(evidence.pages[0].fonts[0].used_by_text);
    assert!(!evidence.pages[0].fonts[0].embedded);
    assert!(!evidence.byte_reproducibility_verified);
    assert!(!evidence.source_text_equivalence_verified);
    assert!(!evidence.visual_fidelity_verified);
    assert!(!evidence.limitations.is_empty());
}

#[test]
fn date_and_id_metadata_are_observed_without_normalization() {
    let bytes = fixture(|doc, _| {
        let date = doc.add_object(Object::String(b"D:20261012000000Z".to_vec(), StringFormat::Literal));
        let info = doc.add_object(dictionary! {
            "CreationDate" => date,
            "Creator" => Object::String(b"afterwriting.com".to_vec(), StringFormat::Literal),
        });
        doc.trailer.set("Info", info);
        doc.trailer.set("ID", vec![
            Object::String(vec![1; 16], StringFormat::Hexadecimal),
            Object::String(vec![2; 16], StringFormat::Hexadecimal),
        ]);
    });
    let evidence = inspect_pdf(&bytes, expected()).unwrap();
    assert_eq!(evidence.metadata[0].key, "CreationDate");
    assert_eq!(evidence.metadata[0].utf8_value.as_deref(), Some("D:20261012000000Z"));
    assert_eq!(evidence.trailer_id_sha256, [digest(&[1; 16]), digest(&[2; 16])]);
    assert!(evidence.nondeterminism_notes.iter().any(|note| note.contains("date metadata")));
    assert!(evidence.nondeterminism_notes.iter().any(|note| note.contains("trailer IDs")));
    assert!(!evidence.byte_reproducibility_verified);
}

#[test]
fn emitted_metadata_changes_output_hash_without_claiming_determinism() {
    let first = fixture(|doc, _| {
        let info = doc.add_object(dictionary! { "CreationDate" => Object::string_literal("D:20261012000000Z") });
        doc.trailer.set("Info", info);
    });
    let second = fixture(|doc, _| {
        let info = doc.add_object(dictionary! { "CreationDate" => Object::string_literal("D:20261013000000Z") });
        doc.trailer.set("Info", info);
    });
    let first = inspect_pdf(&first, expected()).unwrap();
    let second = inspect_pdf(&second, expected()).unwrap();
    assert_ne!(first.sha256, second.sha256);
    assert!(!first.byte_reproducibility_verified && !second.byte_reproducibility_verified);
}

#[test]
fn inherited_media_box_is_checked_against_expected_paper() {
    let bytes = fixture(|doc, ids| {
        doc.get_dictionary_mut(ids.page).unwrap().remove(b"MediaBox");
        doc.get_dictionary_mut(ids.pages).unwrap().set("MediaBox", vec![0.into(), 0.into(), 612.into(), 792.into()]);
    });
    assert!(inspect_pdf(&bytes, expected()).is_ok());
    assert!(inspect_pdf(&bytes, ExpectedScreenplayPdf { paper_width_pt: 595.28, paper_height_pt: 841.89 }).is_err());
}

#[test]
fn inherited_rotation_and_crop_cannot_hide_paper_changes() {
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.pages).unwrap().set("Rotate", 90); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.paper_mismatch");
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.pages).unwrap().set("CropBox", vec![0.into(), 0.into(), 600.into(), 792.into()]); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.paper_mismatch");
}

#[test]
fn page_tree_counts_and_duplicate_page_references_are_refused() {
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.pages).unwrap().set("Count", 2); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.page_count_mismatch");
    let bytes = fixture(|doc, ids| {
        doc.get_dictionary_mut(ids.pages).unwrap().set("Count", 2);
        doc.get_dictionary_mut(ids.pages).unwrap().set("Kids", vec![Object::Reference(ids.page), Object::Reference(ids.page)]);
    });
    assert_eq!(error_code(&bytes), "screenplay_pdf.invalid_structure");
}

#[test]
fn cyclic_page_tree_and_parent_mismatch_are_refused() {
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.pages).unwrap().set("Kids", vec![Object::Reference(ids.pages)]); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.invalid_structure");
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.page).unwrap().set("Parent", ids.catalog); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.invalid_structure");
}

#[test]
fn catalog_actions_and_unreachable_script_objects_are_refused() {
    let bytes = fixture(|doc, ids| {
        doc.get_dictionary_mut(ids.catalog).unwrap().set("OpenAction", dictionary! { "S" => "JavaScript", "JS" => Object::string_literal("app.alert(1)") });
    });
    assert_eq!(error_code(&bytes), "screenplay_pdf.active_content");
    let bytes = fixture(|doc, _| { doc.add_object(dictionary! { "Type" => "Action", "S" => "JavaScript", "JS" => Object::string_literal("app.alert(1)") }); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.active_content");
}

#[test]
fn annotations_attachments_and_external_streams_are_refused() {
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.page).unwrap().set("Annots", Vec::<Object>::new()); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.active_content");
    let bytes = fixture(|doc, _| { doc.add_object(dictionary! { "Type" => "Filespec", "F" => Object::string_literal("payload") }); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.active_content");
    let bytes = fixture(|doc, ids| {
        doc.objects.insert(ids.content, Object::Stream(Stream::new(dictionary! { "F" => Object::string_literal("external.txt") }, TEXT.to_vec())));
    });
    assert_eq!(error_code(&bytes), "screenplay_pdf.external_stream");
}

#[test]
fn blank_and_whitespace_only_pages_are_refused() {
    for content in [b"q Q".as_slice(), b"BT /F1 12 Tf (   ) Tj ET".as_slice(), b"BT /F1 12 Tf [] TJ ET".as_slice()] {
        let bytes = fixture(|doc, ids| set_content(doc, ids, content));
        assert_eq!(error_code(&bytes), "screenplay_pdf.empty_page");
    }
}

#[test]
fn undeclared_and_missing_text_fonts_are_refused() {
    for content in [b"BT /Missing 12 Tf (text) Tj ET".as_slice(), b"BT (text) Tj ET".as_slice()] {
        let bytes = fixture(|doc, ids| set_content(doc, ids, content));
        assert_eq!(error_code(&bytes), "screenplay_pdf.invalid_font");
    }
}

#[test]
fn different_or_customized_font_resources_are_refused() {
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.font).unwrap().set("BaseFont", "Helvetica"); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.font_mismatch");
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.font).unwrap().set("Widths", vec![Object::Integer(1)]); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.font_mismatch");
    let bytes = fixture(|doc, ids| { doc.get_dictionary_mut(ids.font).unwrap().set("Encoding", "MacRomanEncoding"); });
    assert_eq!(error_code(&bytes), "screenplay_pdf.font_mismatch");
}

#[test]
fn image_resources_and_invisible_text_are_refused() {
    let bytes = fixture(|doc, ids| {
        doc.get_dictionary_mut(ids.page).unwrap().set("Resources", dictionary! { "Font" => dictionary! { "F1" => ids.font }, "XObject" => dictionary! {} });
    });
    assert_eq!(error_code(&bytes), "screenplay_pdf.unsupported_resource");
    let bytes = fixture(|doc, ids| set_content(doc, ids, b"BT /F1 12 Tf 3 Tr (text) Tj ET"));
    assert_eq!(error_code(&bytes), "screenplay_pdf.invalid_content");
}

#[test]
fn control_and_non_ascii_glyph_bytes_are_refused() {
    for content in [b"BT /F1 12 Tf <00> Tj ET".as_slice(), b"BT /F1 12 Tf <e9> Tj ET".as_slice()] {
        let bytes = fixture(|doc, ids| set_content(doc, ids, content));
        assert_eq!(error_code(&bytes), "screenplay_pdf.unsupported_text");
    }
}

#[test]
fn malformed_or_unbalanced_content_operators_are_refused() {
    for content in [
        b"BT /F1 12 Tf (text) Tj".as_slice(),
        b"q BT /F1 12 Tf (text) Tj ET".as_slice(),
        b"BT /F1 12 Tf [(text) true] TJ ET".as_slice(),
        b"BT /F1 0 Tf (text) Tj ET".as_slice(),
        b"BT /F1 12 Tf (text) Tj ET /Im0 Do".as_slice(),
    ] {
        let bytes = fixture(|doc, ids| set_content(doc, ids, content));
        assert!(inspect_pdf(&bytes, expected()).is_err());
    }
}

#[test]
fn decoded_page_stream_limit_is_enforced() {
    use std::io::Write;
    let bytes = fixture(|doc, ids| {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&vec![b' '; MAX_DECOMPRESSED_STREAM_BYTES + 1]).unwrap();
        doc.objects.insert(ids.content, Object::Stream(Stream::new(dictionary! { "Filter" => "FlateDecode" }, encoder.finish().unwrap())));
    });
    assert!(inspect_pdf(&bytes, expected()).is_err());
}

#[test]
fn invalid_headers_truncation_and_expectations_are_refused() {
    let bytes = fixture(|_, _| {});
    assert_eq!(error_code(b"not a PDF"), "screenplay_pdf.invalid_structure");
    assert!(inspect_pdf(&bytes[..bytes.len() - 10], expected()).is_err());
    assert!(inspect_pdf(&bytes, ExpectedScreenplayPdf { paper_width_pt: f64::NAN, paper_height_pt: 792.0 }).is_err());
}

#[test]
fn oversized_file_and_metadata_are_refused() {
    assert_eq!(error_code(&vec![b' '; MAX_SCREENPLAY_PDF_BYTES + 1]), "screenplay_pdf.byte_limit");
    let bytes = fixture(|doc, _| {
        let info = doc.add_object(dictionary! { "Title" => Object::String(vec![b'x'; 4_097], StringFormat::Literal) });
        doc.trailer.set("Info", info);
    });
    assert_eq!(error_code(&bytes), "screenplay_pdf.metadata_limit");
}
