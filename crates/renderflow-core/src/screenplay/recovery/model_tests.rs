use super::*;

const SAMPLE: &str = "INT. LAB - DAY\n\nA lamp glows.\n\n        ALAN\n    (quietly)\n    First line.\n    Second line.\n\nCUT TO:\n";

fn plain(text: &str) -> RecoveryCandidate {
    let pages = text_pages(text.as_bytes()).unwrap();
    recover_pages(&pages, &format!("{:x}", Sha256::digest(text.as_bytes())), RecoveryInputFormat::Text).unwrap()
}

#[test]
fn deterministic_body_recovery_preserves_every_nonblank_source_line() {
    let candidate = plain(SAMPLE);
    assert_eq!(candidate.screenplay.elements.iter().map(|element| element.kind).collect::<Vec<_>>(), vec![
        ScreenplayElementKind::SceneHeading,
        ScreenplayElementKind::Action,
        ScreenplayElementKind::Character,
        ScreenplayElementKind::Parenthetical,
        ScreenplayElementKind::Dialogue,
        ScreenplayElementKind::Dialogue,
        ScreenplayElementKind::Transition,
    ]);
    assert_eq!(candidate.elements.iter().map(|element| element.source_text.as_str()).collect::<Vec<_>>(),
        SAMPLE.split('\n').filter(|line| !line.trim().is_empty()).collect::<Vec<_>>());
    assert_eq!(candidate.screenplay.elements[3].text, "quietly");
    assert_eq!(candidate.elements[3].source_text, "    (quietly)");
    assert_eq!(candidate.screenplay.elements[4].character_id.as_deref(), Some(candidate.screenplay.elements[2].id.as_str()));
    assert_eq!(candidate.elements[2].source.line_start, 5);
    assert_eq!(candidate.elements[2].confidence, RecoveryConfidence::Medium);
    assert_eq!(candidate, plain(SAMPLE));
    validate_candidate(&candidate).unwrap();
}

#[test]
fn all_results_remain_lossy_and_review_required_with_ordinal_evidence() {
    let candidate = plain(SAMPLE);
    assert!(candidate.lossy && candidate.review_required);
    assert_eq!(candidate.schema_version, CANDIDATE_SCHEMA_V1);
    assert_eq!(candidate.heuristic_version, RECOVERY_HEURISTIC_V1);
    assert_eq!(candidate.screenplay.source.sha256, candidate.source_sha256);
    assert_eq!(candidate.screenplay.source.format, "text");
    assert_eq!(candidate.screenplay.losses.len(), 4);
    assert!(candidate.screenplay.losses.iter().all(|loss| loss.direction == ScreenplayLossDirection::Import));
    let extension = &candidate.screenplay.extensions["renderflow:recovery"];
    assert_eq!(extension["review_required"], true);
    assert_eq!(extension["lossy"], true);
    assert_eq!(extension["confidence_semantics"], "ordinal_formatting_cues_not_measured_probabilities");
    let canonical = crate::screenplay::render_json(&candidate.screenplay).unwrap();
    assert_eq!(crate::screenplay::parse_json(&canonical).unwrap(), candidate.screenplay);
}

#[test]
fn page_boundaries_blank_pages_and_terminators_are_preserved() {
    let pages = text_pages(b"INT. LAB - DAY\r\n\r\nA lamp glows.\x0c\x0cEXT. ROAD - NIGHT\n\nRain falls.\x0c").unwrap();
    assert_eq!(pages.len(), 3);
    assert_eq!(pages[0].text, "INT. LAB - DAY\n\nA lamp glows.");
    assert_eq!(pages[1].text, "");
    assert_eq!(pages[2].page_number, 3);
    let candidate = recover_pages(&pages, &"a".repeat(64), RecoveryInputFormat::Text).unwrap();
    assert_eq!(candidate.pages[1].line_count, 0);
    assert_eq!(candidate.pages[1].nonblank_line_count, 0);
    assert_eq!(candidate.elements[2].source.page_number, 3);
    assert_eq!(candidate.elements[2].source.line_start, 1);
}

#[test]
fn dialogue_is_not_joined_across_pages_or_blank_lines() {
    let candidate = plain("INT. LAB - DAY\n\nALAN\nHello.\x0cContinued words.\n\n(aside)\n\nMore words.");
    for element in candidate.screenplay.elements.iter().skip(3) {
        assert_eq!(element.kind, ScreenplayElementKind::Action);
        assert!(element.character_id.is_none());
    }
    assert!(candidate.elements[4].reason_codes.contains(&"orphan_parenthetical_retained".to_owned()));
}

#[test]
fn page_numbers_and_continuation_markers_are_not_discarded() {
    let candidate = plain("INT. LAB - DAY\n\nALAN\nHello.\n42.\nMore words.\n(MORE)\nPage 2\n(CONTINUED)\n");
    for text in ["42.", "(MORE)", "Page 2", "(CONTINUED)"] {
        let (index, element) = candidate.screenplay.elements.iter().enumerate().find(|(_, element)| element.text == text).unwrap();
        assert_eq!(element.kind, ScreenplayElementKind::Action);
        assert_eq!(candidate.elements[index].confidence, RecoveryConfidence::Low);
        assert!(candidate.elements[index].reason_codes.contains(&"pagination_or_continuation_marker_retained".to_owned()));
    }
    let continuation = candidate.screenplay.elements.iter().find(|element| element.text == "More words.").unwrap();
    assert_eq!(continuation.kind, ScreenplayElementKind::Action);
    assert!(continuation.character_id.is_none());
}

#[test]
fn indentation_change_ends_a_dialogue_block() {
    let candidate = plain("INT. LAB - DAY\n\n        ALAN\n    Hello.\nA door opens.\n");
    let last = candidate.screenplay.elements.last().unwrap();
    assert_eq!(last.kind, ScreenplayElementKind::Action);
    assert!(last.character_id.is_none());
    assert!(candidate.elements.last().unwrap().reason_codes.contains(&"indentation_ended_dialogue_context".to_owned()));
}

#[test]
fn two_distinct_cue_blocks_without_a_heading_remain_explicitly_uncertain() {
    let candidate = plain("ALAN\nHello there.\n\nROSE\nHello back.\n");
    assert_eq!(candidate.screenplay.elements.len(), 4);
    assert!(candidate.elements.iter().all(|element| element.confidence == RecoveryConfidence::Low
        && element.reason_codes.contains(&"missing_scene_heading_requires_review".to_owned())));
}

#[test]
fn ordinary_prose_and_insufficient_structure_are_refused() {
    for text in ["This is an ordinary document.\nIt is not a script.", "ALAN\nHello.", "INT. LAB - DAY\n", "CHAPTER ONE\nText one.\n\nCHAPTER TWO\nText two."] {
        let pages = text_pages(text.as_bytes()).unwrap();
        assert_eq!(recover_pages(&pages, &"a".repeat(64), RecoveryInputFormat::Text).unwrap_err().code,
            "screenplay.recovery.not_screenplay");
    }
    let pages = text_pages(b"\n \t\n").unwrap();
    assert_eq!(recover_pages(&pages, &"a".repeat(64), RecoveryInputFormat::Text).unwrap_err().code, "screenplay.recovery.empty");
}

#[test]
fn uppercase_action_without_dialogue_and_title_text_are_retained() {
    let candidate = plain("SCRIPT TITLE\nWritten by A. Person\n\nINT. LAB - DAY\n\nTHE DOOR\n\nA lamp glows.\n");
    assert_eq!(candidate.screenplay.elements[0].text, "SCRIPT TITLE");
    assert_eq!(candidate.screenplay.elements[3].text, "THE DOOR");
    assert_eq!(candidate.screenplay.elements[3].kind, ScreenplayElementKind::Action);
    assert!(candidate.elements[3].reason_codes.contains(&"ambiguous_uppercase_text_retained".to_owned()));
    assert!(candidate.screenplay.title_page.is_empty());
}

#[test]
fn ocr_page_classification_never_receives_high_confidence() {
    let pages = vec![ExtractedPage { page_number: 1, text: SAMPLE.to_owned(), method: ExtractionMethod::Ocr }];
    let candidate = recover_pages(&pages, &"a".repeat(64), RecoveryInputFormat::Pdf).unwrap();
    assert_eq!(candidate.pages[0].method, ExtractionMethod::Ocr);
    assert!(candidate.elements.iter().all(|element| element.confidence == RecoveryConfidence::Low
        && element.reason_codes.contains(&"ocr_text_requires_source_review".to_owned())));
    assert_eq!(candidate.screenplay.source.format, "pdf");
}

#[test]
fn mixed_pdf_extraction_methods_and_unicode_are_preserved() {
    let pages = vec![
        ExtractedPage { page_number: 1, text: "INT. CAFÉ - DAY\n\n        ÉMILIE\n    Bonjour, 世界.\n".to_owned(), method: ExtractionMethod::EmbeddedText },
        ExtractedPage { page_number: 2, text: "EXT. ДОРОГА - NIGHT\n\nИдёт дождь.\n".to_owned(), method: ExtractionMethod::Ocr },
    ];
    let candidate = recover_pages(&pages, &"a".repeat(64), RecoveryInputFormat::Pdf).unwrap();
    assert_eq!(candidate.screenplay.elements[2].text, "Bonjour, 世界.");
    assert_eq!(candidate.screenplay.elements[4].text, "Идёт дождь.");
    assert_eq!(candidate.pages[0].method, ExtractionMethod::EmbeddedText);
    assert_eq!(candidate.pages[1].method, ExtractionMethod::Ocr);
    validate_candidate(&candidate).unwrap();
}

#[test]
fn extraction_identity_includes_page_boundaries_and_method() {
    let first = vec![ExtractedPage { page_number: 1, text: "ab".to_owned(), method: ExtractionMethod::EmbeddedText }];
    let separate = vec![
        ExtractedPage { page_number: 1, text: "a".to_owned(), method: ExtractionMethod::EmbeddedText },
        ExtractedPage { page_number: 2, text: "b".to_owned(), method: ExtractionMethod::EmbeddedText },
    ];
    let mut changed = first.clone();
    changed[0].method = ExtractionMethod::Ocr;
    assert_ne!(pages_digest(&first), pages_digest(&separate));
    assert_ne!(pages_digest(&first), pages_digest(&changed));
}

#[test]
fn invalid_digest_pages_methods_and_controls_fail_with_typed_errors() {
    let pages = text_pages(SAMPLE.as_bytes()).unwrap();
    for digest in ["a".repeat(63), "A".repeat(64), "g".repeat(64)] {
        assert_eq!(recover_pages(&pages, &digest, RecoveryInputFormat::Text).unwrap_err().code, "screenplay.recovery.source");
    }
    let mut out_of_order = pages.clone();
    out_of_order[0].page_number = 2;
    assert_eq!(recover_pages(&out_of_order, &"a".repeat(64), RecoveryInputFormat::Text).unwrap_err().code, "screenplay.recovery.pages");
    assert_eq!(recover_pages(&pages, &"a".repeat(64), RecoveryInputFormat::Pdf).unwrap_err().code, "screenplay.recovery.source");
    for bytes in [b"bad\rline".as_slice(), b"bad\0line", b"bad\x1bline", b"\xef\xbb\xbfbom", b"\xff"] {
        assert_eq!(text_pages(bytes).unwrap_err().code, "screenplay.recovery.encoding");
    }
}

#[test]
fn explicit_input_page_line_and_element_bounds_are_enforced() {
    assert_eq!(text_pages(&vec![b'x'; MAX_RECOVERY_INPUT_BYTES + 1]).unwrap_err().code, "screenplay.recovery.bounds");
    assert_eq!(text_pages("\x0c".repeat(MAX_RECOVERY_PAGES + 1).as_bytes()).unwrap_err().code, "screenplay.recovery.bounds");
    assert_eq!(text_pages("x".repeat(MAX_RECOVERY_LINE_BYTES + 1).as_bytes()).unwrap_err().code, "screenplay.recovery.bounds");
    assert_eq!(text_pages("\n".repeat(MAX_RECOVERY_LINES).as_bytes()).unwrap_err().code, "screenplay.recovery.bounds");
    assert_eq!(text_pages("x\n".repeat(MAX_RECOVERY_ELEMENTS + 1).as_bytes()).unwrap_err().code, "screenplay.recovery.bounds");
}

#[test]
fn replay_refuses_mutated_review_state_loss_lineage_and_semantics() {
    let original = plain(SAMPLE);
    let mut mutations = Vec::new();
    let mut changed = original.clone(); changed.review_required = false; mutations.push(changed);
    let mut changed = original.clone(); changed.lossy = false; mutations.push(changed);
    let mut changed = original.clone(); changed.screenplay.losses.clear(); mutations.push(changed);
    let mut changed = original.clone(); changed.screenplay.elements[1].text = "Invented action.".to_owned(); mutations.push(changed);
    let mut changed = original.clone(); changed.elements[1].source.line_start = 99; mutations.push(changed);
    let mut changed = original.clone(); changed.elements[1].confidence = RecoveryConfidence::High; mutations.push(changed);
    let mut changed = original.clone(); changed.pages[0].text_sha256 = "b".repeat(64); mutations.push(changed);
    let mut changed = original.clone(); changed.extracted_text_sha256 = "c".repeat(64); mutations.push(changed);
    let mut changed = original.clone(); changed.elements[1].reason_codes.clear(); mutations.push(changed);
    for changed in mutations {
        assert_eq!(validate_candidate(&changed).unwrap_err().code, "screenplay.recovery.candidate_mismatch");
    }
}

#[test]
fn strict_candidate_json_roundtrip_remains_replayable() {
    let candidate = plain(SAMPLE);
    let encoded = serde_json::to_vec(&candidate).unwrap();
    let decoded: RecoveryCandidate = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(candidate, decoded);
    validate_candidate(&decoded).unwrap();
    let mut value = serde_json::to_value(&candidate).unwrap();
    value["automatically_approved"] = json!(true);
    assert!(serde_json::from_value::<RecoveryCandidate>(value).is_err());
}
