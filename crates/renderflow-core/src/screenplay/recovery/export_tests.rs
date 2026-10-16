use std::io::Read;

use super::*;
use super::super::model::{recover_pages, ExtractedPage, ExtractionMethod, RecoveryInputFormat};

fn candidate(text: &str) -> RecoveryCandidate {
    recover_pages(
        &[ExtractedPage { page_number: 1, text: text.to_owned(), method: ExtractionMethod::PlainText }],
        &"a".repeat(64),
        RecoveryInputFormat::Text,
    ).unwrap()
}

fn dialogue_candidate() -> RecoveryCandidate {
    candidate("INT. LAB - DAY\n\nA lamp glows.\n\nALAN\n(quietly)\nFirst line.\nSecond line.\n\nCUT TO:\n")
}

#[test]
fn all_native_exports_keep_review_required_and_bind_the_unchanged_candidate() {
    let recovered = dialogue_candidate();
    let before = serde_json::to_vec(&recovered).unwrap();
    let canonical = render_json(&recovered.screenplay).unwrap();
    for format in [NativeScreenplayFormat::Fountain, NativeScreenplayFormat::Fdx,
        NativeScreenplayFormat::Osf, NativeScreenplayFormat::FadeIn]
    {
        let result = export(&recovered, format).unwrap();
        assert!(result.evidence.lossy);
        assert!(result.evidence.review_required);
        assert!(!result.evidence.runtime_qualified);
        assert_eq!(result.evidence.source_sha256, recovered.source_sha256);
        assert_eq!(result.evidence.candidate_sha256, digest(&before));
        assert_eq!(result.evidence.canonical_sha256, digest(&canonical));
        assert_eq!(result.evidence.output_sha256, digest(&result.bytes));
        assert_eq!(result.evidence.output_size_bytes, result.bytes.len() as u64);
        assert!(result.evidence.losses.starts_with(&recovered.screenplay.losses));
        assert!(result.evidence.losses.iter().any(|loss| loss.direction == ScreenplayLossDirection::Export
            && loss.kind == ScreenplayLossKind::SourceExtensions));
        validate_export(&recovered, format, &result.bytes, &result.evidence).unwrap();
        assert_eq!(serde_json::to_vec(&recovered).unwrap(), before);
    }
}

#[test]
fn xml_exports_preserve_unicode_and_escape_markup_as_text() {
    let recovered = candidate("INT. LAB - DAY\n\nŻółw & кошка see <a> and \"quoted\" text.\n");
    for format in [NativeScreenplayFormat::Fdx, NativeScreenplayFormat::Osf, NativeScreenplayFormat::FadeIn] {
        let result = export(&recovered, format).unwrap();
        let parsed = parse_native(&result.bytes, format).unwrap();
        assert_eq!(parsed.elements[1].text, "Żółw & кошка see <a> and \"quoted\" text.");
        assert!(result.evidence.dialogue_joins.is_empty());
        assert_eq!(parsed.elements.len(), recovered.screenplay.elements.len());
    }
}

#[test]
fn fountain_joins_only_adjacent_dialogue_for_the_same_character_and_records_each_id() {
    let recovered = dialogue_candidate();
    let result = export(&recovered, NativeScreenplayFormat::Fountain).unwrap();
    let parsed = parse_native(&result.bytes, NativeScreenplayFormat::Fountain).unwrap();
    let original_dialogue = recovered.screenplay.elements.iter()
        .filter(|element| element.kind == ScreenplayElementKind::Dialogue).collect::<Vec<_>>();
    assert_eq!(original_dialogue.len(), 2);
    assert_eq!(result.evidence.dialogue_joins.len(), 1);
    assert_eq!(result.evidence.dialogue_joins[0].source_element_ids,
        original_dialogue.iter().map(|element| element.id.clone()).collect::<Vec<_>>());
    assert_eq!(result.evidence.dialogue_joins[0].separator, " ");
    assert_eq!(parsed.elements.iter().find(|element| element.kind == ScreenplayElementKind::Dialogue).unwrap().text,
        "First line. Second line.");
    assert_eq!(result.evidence.source_element_count, result.evidence.serialized_element_count + 1);
    assert_eq!(recovered.screenplay.elements.iter().filter(|element|
        element.kind == ScreenplayElementKind::Dialogue).count(), 2);
}

#[test]
fn fountain_refuses_styling_ambiguity_without_blocking_escaped_xml_export() {
    let recovered = candidate("INT. LAB - DAY\n\nA snake_case label appears.\n");
    let error = export(&recovered, NativeScreenplayFormat::Fountain).unwrap_err();
    assert_eq!(error.code, "screenplay_recovery.export.unsupported");
    let fdx = export(&recovered, NativeScreenplayFormat::Fdx).unwrap();
    assert_eq!(parse_native(&fdx.bytes, NativeScreenplayFormat::Fdx).unwrap().elements[1].text,
        "A snake_case label appears.");
}

#[test]
fn fountain_refuses_a_non_scene_opening_instead_of_inventing_a_heading() {
    let recovered = candidate("A recovered introductory paragraph.\n");
    assert!(export(&recovered, NativeScreenplayFormat::Fountain).is_err());
    let result = export(&recovered, NativeScreenplayFormat::Osf).unwrap();
    let parsed = parse_native(&result.bytes, NativeScreenplayFormat::Osf).unwrap();
    assert_eq!(parsed.elements.len(), 1);
    assert_eq!(parsed.elements[0].kind, ScreenplayElementKind::Action);
}

#[test]
fn fadein_is_a_deterministic_single_stored_document_without_external_members() {
    let recovered = dialogue_candidate();
    let first = export(&recovered, NativeScreenplayFormat::FadeIn).unwrap();
    let second = export(&recovered, NativeScreenplayFormat::FadeIn).unwrap();
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.evidence, second.evidence);
    let mut archive = zip::ZipArchive::new(Cursor::new(&first.bytes)).unwrap();
    assert_eq!(archive.len(), 1);
    let mut member = archive.by_index(0).unwrap();
    assert_eq!(member.name(), "document.xml");
    assert_eq!(member.compression(), CompressionMethod::Stored);
    assert!(!member.encrypted());
    let mut xml = Vec::new();
    member.read_to_end(&mut xml).unwrap();
    assert_eq!(xml, export(&recovered, NativeScreenplayFormat::Osf).unwrap().bytes);
}

#[test]
fn terminal_validation_rejects_native_bytes_or_any_mutated_evidence_claim() {
    let recovered = dialogue_candidate();
    let result = export(&recovered, NativeScreenplayFormat::Fdx).unwrap();
    let mut bytes = result.bytes.clone();
    bytes.push(b'\n');
    assert_eq!(validate_export(&recovered, NativeScreenplayFormat::Fdx, &bytes, &result.evidence)
        .unwrap_err().code, "screenplay_recovery.export.identity");
    let mut claims = Vec::new();
    let mut evidence = result.evidence.clone();
    evidence.review_required = false;
    claims.push(evidence);
    let mut evidence = result.evidence.clone();
    evidence.lossy = false;
    claims.push(evidence);
    let mut evidence = result.evidence.clone();
    evidence.runtime_qualified = true;
    claims.push(evidence);
    let mut evidence = result.evidence.clone();
    evidence.source_sha256 = "b".repeat(64);
    claims.push(evidence);
    let mut evidence = result.evidence.clone();
    evidence.losses.clear();
    claims.push(evidence);
    for evidence in claims {
        assert_eq!(validate_export(&recovered, NativeScreenplayFormat::Fdx, &result.bytes, &evidence)
            .unwrap_err().code, "screenplay_recovery.export.identity");
    }
}

#[test]
fn modified_candidate_cannot_reuse_or_generate_native_export_evidence() {
    let recovered = dialogue_candidate();
    let result = export(&recovered, NativeScreenplayFormat::Fdx).unwrap();
    let mut tampered = recovered.clone();
    tampered.screenplay.elements[1].text = "Different recovered words.".to_owned();
    assert_eq!(export(&tampered, NativeScreenplayFormat::Fdx).unwrap_err().code,
        "screenplay_recovery.export.candidate");
    assert!(validate_export(&tampered, NativeScreenplayFormat::Fdx, &result.bytes, &result.evidence).is_err());
}

#[test]
fn arbitrary_canonical_features_are_never_silently_removed_from_body_projection() {
    let recovered = dialogue_candidate();
    let mut screenplay = recovered.screenplay.clone();
    screenplay.elements[0].scene_number = Some("1".to_owned());
    assert_eq!(body_projection(&screenplay, NativeScreenplayFormat::Fdx).unwrap_err().code,
        "screenplay_recovery.export.unsupported");
    let mut screenplay = recovered.screenplay.clone();
    screenplay.elements[1].text = "First action.\nSecond action.".to_owned();
    assert_eq!(body_projection(&screenplay, NativeScreenplayFormat::Fdx).unwrap_err().code,
        "screenplay_recovery.export.text");
}

#[test]
fn bounded_fountain_join_refuses_oversized_paragraph_instead_of_truncating() {
    let recovered = dialogue_candidate();
    let mut screenplay = recovered.screenplay.clone();
    for element in screenplay.elements.iter_mut().filter(|element| element.kind == ScreenplayElementKind::Dialogue) {
        element.text = "a".repeat(MAX_TEXT_BYTES / 2);
    }
    assert_eq!(body_projection(&screenplay, NativeScreenplayFormat::Fountain).unwrap_err().code,
        "screenplay_recovery.export.bounds");
}

#[test]
fn shared_trelby_codec_does_not_expand_the_recovery_export_contract() {
    let recovered = dialogue_candidate();
    let unchanged = serde_json::to_vec(&recovered).unwrap();
    assert_eq!(export(&recovered, NativeScreenplayFormat::Trelby).unwrap_err().code,
        "screenplay_recovery.export.unsupported");
    let fdx = export(&recovered, NativeScreenplayFormat::Fdx).unwrap();
    assert_eq!(validate_export(&recovered, NativeScreenplayFormat::Trelby, &fdx.bytes, &fdx.evidence)
        .unwrap_err().code, "screenplay_recovery.export.unsupported");
    assert_eq!(serde_json::to_vec(&recovered).unwrap(), unchanged);
}
