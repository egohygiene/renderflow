use super::*;
use serde_json::json;

fn element(id: &str, kind: ScreenplayElementKind, text: &str) -> ScreenplayElement {
    ScreenplayElement {
        id: id.to_owned(), kind, text: text.to_owned(), character_id: None,
        scene_number: None, section_level: None, extensions: BTreeMap::new(),
    }
}

fn sample() -> Screenplay {
    let mut left_dialogue = element("dialogue-left", ScreenplayElementKind::Dialogue, "café, Łódź, 星.");
    left_dialogue.character_id = Some("character-left".to_owned());
    let mut parenthetical = element("quietly", ScreenplayElementKind::Parenthetical, "quietly");
    parenthetical.character_id = Some("character-left".to_owned());
    let mut right_dialogue = element("dialogue-right", ScreenplayElementKind::Dialogue, "I can hear you.");
    right_dialogue.character_id = Some("character-right".to_owned());
    let mut screenplay = Screenplay::new(
        ScreenplaySource { sha256: "a".repeat(64), format: "fountain".to_owned(), extensions: BTreeMap::new() },
        vec![
            element("scene", ScreenplayElementKind::SceneHeading, "INT. STATION - NIGHT"),
            element("character-left", ScreenplayElementKind::Character, "MIRA"),
            parenthetical,
            left_dialogue,
            element("character-right", ScreenplayElementKind::Character, "ORI"),
            right_dialogue,
            element("action", ScreenplayElementKind::Action, "A lantern brightens."),
        ],
    );
    screenplay.elements[0].scene_number = Some("1A".to_owned());
    screenplay.title_page = vec![
        TitlePageField { id: "title".to_owned(), key: "Title".to_owned(), value: "Lantern".to_owned(), extensions: BTreeMap::new() },
        TitlePageField { id: "author-1".to_owned(), key: "Author".to_owned(), value: "First author".to_owned(), extensions: BTreeMap::new() },
        TitlePageField { id: "author-2".to_owned(), key: "Author".to_owned(), value: "Second author".to_owned(), extensions: BTreeMap::new() },
    ];
    screenplay.dual_dialogue = vec![DualDialogue { id: "pair".to_owned(), left_character_id: "character-left".to_owned(), right_character_id: "character-right".to_owned(), extensions: BTreeMap::new() }];
    screenplay
}

fn parse_value(value: &Value) -> Result<Screenplay, ScreenplayError> {
    parse_json(&serde_json::to_vec(value).expect("test JSON"))
}

#[test]
fn canonical_roundtrip_preserves_order_ids_unicode_relationships_and_extensions() {
    let mut screenplay = sample();
    screenplay.extensions.insert("fdx:opaque".to_owned(), json!({"raw": "<Revision color=\"blue\"/>", "revision": 3}));
    let mut section = element("section", ScreenplayElementKind::Section, "Act one");
    section.section_level = Some(1);
    screenplay.elements.push(section);
    for (id, kind) in [
        ("transition", ScreenplayElementKind::Transition), ("lyrics", ScreenplayElementKind::Lyrics),
        ("centered", ScreenplayElementKind::Centered), ("synopsis", ScreenplayElementKind::Synopsis),
        ("note", ScreenplayElementKind::Note),
    ] { screenplay.elements.push(element(id, kind, "Retained text")); }
    let mut unknown = element("unknown", ScreenplayElementKind::Unknown, "");
    unknown.extensions.insert("source:unrecognized".to_owned(), json!({"raw": "opaque node"}));
    screenplay.elements.push(unknown);
    let rendered = render_json(&screenplay).expect("render valid model");
    assert_eq!(parse_json(&rendered).expect("parse rendered model"), screenplay);
    assert_eq!(render_json(&screenplay).expect("deterministic rendering"), rendered);
    assert_eq!(screenplay.title_page[1].key, screenplay.title_page[2].key);
}

#[test]
fn authored_canonical_fixture_has_explicit_relationship_and_unverified_source() {
    let bytes = include_bytes!("../../../../tests/fixtures/screenplay/minimal.screenplay.json");
    let screenplay = parse_json(bytes).expect("authored canonical fixture");
    assert_eq!(screenplay.dual_dialogue.len(), 1);
    let report = inspect_json(bytes).expect("inspect authored model");
    assert!(!report.source_identity_verified);
    assert!(!report.native_adapters_available);
}

#[test]
fn unknown_fields_are_refused_at_every_closed_record() {
    for pointer in ["", "/source", "/title_page/0", "/elements/0", "/dual_dialogue/0"] {
        let mut value = serde_json::to_value(sample()).expect("model value");
        value.pointer_mut(pointer).expect("record").as_object_mut().expect("object").insert("unrecognized".to_owned(), json!(true));
        assert!(parse_value(&value).is_err(), "unexpected unknown-field acceptance at {pointer}");
    }
    let mut value = serde_json::to_value(sample()).expect("model value");
    value["losses"] = json!([{"direction":"import","format":"fdx","kind":"layout","detail":"Declared earlier loss","unexpected":true}]);
    assert!(parse_value(&value).is_err());
}

#[test]
fn duplicate_keys_are_refused_in_root_and_opaque_extension_objects() {
    let rendered = String::from_utf8(render_json(&sample()).expect("model bytes")).expect("UTF-8");
    let duplicate_root = rendered.replacen("\"schema_version\":", "\"schema_version\":\"renderflow.screenplay/v1\",\"schema_version\":", 1);
    assert!(parse_json(duplicate_root.as_bytes()).is_err());
    let extension = rendered.replacen("{", "{\"extensions\":{\"source:opaque\":{\"revision\":1,\"revision\":2}},", 1);
    assert!(parse_json(extension.as_bytes()).is_err());
}

#[test]
fn positional_record_arrays_and_null_extension_bags_are_refused() {
    for pointer in ["", "/source", "/title_page/0", "/elements/0", "/dual_dialogue/0"] {
        let mut value = serde_json::to_value(sample()).expect("model value");
        *value.pointer_mut(pointer).expect("record") = json!([]);
        assert!(parse_value(&value).is_err());
    }
    let mut value = serde_json::to_value(sample()).expect("model value");
    value["extensions"] = Value::Null;
    assert!(parse_value(&value).is_err());
    value["extensions"] = json!({});
    value["elements"][0]["character_id"] = Value::Null;
    assert!(parse_value(&value).is_ok(), "optional null matches the schema");
}

#[test]
fn schema_version_digest_utf8_and_input_limit_are_enforced() {
    let mut screenplay = sample();
    screenplay.schema_version = "renderflow.screenplay/v2".to_owned();
    assert!(validate(&screenplay).is_err());
    screenplay.schema_version = SCREENPLAY_SCHEMA_V1.to_owned();
    screenplay.source.sha256 = "A".repeat(64);
    assert!(validate(&screenplay).is_err());
    assert!(matches!(parse_json(&[0xff]), Err(ScreenplayError::InvalidUtf8(_))));
    assert!(matches!(parse_json(&vec![b' '; MAX_INPUT_BYTES + 1]), Err(ScreenplayError::InputTooLarge)));
}

#[test]
fn stable_ids_are_unique_across_title_elements_and_dual_groups() {
    let mut screenplay = sample();
    screenplay.elements[0].id = screenplay.title_page[0].id.clone();
    assert!(validate(&screenplay).is_err());
    let mut screenplay = sample();
    screenplay.dual_dialogue[0].id = screenplay.elements[0].id.clone();
    assert!(validate(&screenplay).is_err());
}

#[test]
fn dialogue_cannot_reference_missing_forward_or_interrupted_character_blocks() {
    let mut screenplay = sample();
    screenplay.elements[2].character_id = Some("character-right".to_owned());
    assert!(validate(&screenplay).is_err());
    let mut screenplay = sample();
    screenplay.elements[3].character_id = None;
    assert!(validate(&screenplay).is_err());
    let mut screenplay = sample();
    screenplay.elements.insert(3, element("interruption", ScreenplayElementKind::Action, "A pause."));
    assert!(validate(&screenplay).is_err());
}

#[test]
fn dual_dialogue_rejects_reversed_nonadjacent_and_reused_character_blocks() {
    let mut screenplay = sample();
    screenplay.dual_dialogue[0].left_character_id = "character-right".to_owned();
    screenplay.dual_dialogue[0].right_character_id = "character-left".to_owned();
    assert!(validate(&screenplay).is_err());
    let mut screenplay = sample();
    screenplay.elements.insert(4, element("between", ScreenplayElementKind::Action, "A pause."));
    assert!(validate(&screenplay).is_err());
    let mut screenplay = sample();
    let mut duplicate = screenplay.dual_dialogue[0].clone();
    duplicate.id = "pair-2".to_owned();
    screenplay.dual_dialogue.push(duplicate);
    assert!(validate(&screenplay).is_err());
}

#[test]
fn dual_dialogue_requires_dialogue_on_both_sides() {
    let mut screenplay = sample();
    screenplay.elements[5].kind = ScreenplayElementKind::Parenthetical;
    assert!(validate(&screenplay).is_err());
}

#[test]
fn kind_specific_fields_and_unknown_source_data_are_checked() {
    let mut screenplay = sample();
    screenplay.elements[6].scene_number = Some("2".to_owned());
    assert!(validate(&screenplay).is_err());
    screenplay.elements[6].scene_number = None;
    screenplay.elements[6].kind = ScreenplayElementKind::Section;
    assert!(validate(&screenplay).is_err());
    screenplay.elements[6].section_level = Some(7);
    assert!(validate(&screenplay).is_err());
    screenplay.elements[6].section_level = None;
    screenplay.elements[6].kind = ScreenplayElementKind::Unknown;
    assert!(validate(&screenplay).is_err());
    screenplay.elements[6].extensions.insert("source:raw".to_owned(), json!("Unrecognized paragraph"));
    assert!(validate(&screenplay).is_ok());
}

#[test]
fn semantic_text_is_bounded_by_utf8_bytes_and_refuses_controls() {
    let mut screenplay = sample();
    screenplay.elements[6].text = "星".repeat(MAX_TEXT_BYTES / 3 + 1);
    assert!(validate(&screenplay).is_err());
    screenplay.elements[6].text = "bare\rcarriage-return".to_owned();
    assert!(validate(&screenplay).is_err());
    screenplay.elements[6].text = "literal <markup> & typography\n\tretained".to_owned();
    assert!(validate(&screenplay).is_ok());
}

#[test]
fn extensions_require_namespaces_and_respect_entry_and_value_limits() {
    let mut screenplay = sample();
    screenplay.extensions.insert("unqualified".to_owned(), json!(true));
    assert!(validate(&screenplay).is_err());
    screenplay.extensions.clear();
    screenplay.extensions.insert(":empty".to_owned(), json!(true));
    assert!(validate(&screenplay).is_err());
    screenplay.extensions.clear();
    for index in 0..=MAX_EXTENSION_ENTRIES {
        screenplay.extensions.insert(format!("source:{index}"), Value::Null);
    }
    assert!(validate(&screenplay).is_err());
    screenplay.extensions.clear();
    screenplay.extensions.insert("source:oversized".to_owned(), json!("x".repeat(MAX_EXTENSION_VALUE_BYTES)));
    assert!(validate(&screenplay).is_err(), "encoded string quotes count against value bytes");
}

#[test]
fn nested_extension_depth_and_aggregate_bytes_are_bounded() {
    let mut screenplay = sample();
    let mut value = Value::Null;
    for _ in 0..=MAX_EXTENSION_DEPTH { value = json!([value]); }
    screenplay.extensions.insert("source:deep".to_owned(), value);
    assert!(validate(&screenplay).is_err());
    screenplay.extensions.clear();
    for index in 0..17 {
        screenplay.extensions.insert(format!("source:{index}"), json!("x".repeat(MAX_EXTENSION_VALUE_BYTES - 2)));
    }
    assert!(validate(&screenplay).is_err());
}

#[test]
fn extension_numbers_preserve_exact_integer_domain_and_refuse_rounding() {
    let mut screenplay = sample();
    screenplay.extensions.insert("source:integers".to_owned(), json!([i64::MIN, u64::MAX]));
    assert_eq!(parse_json(&render_json(&screenplay).expect("exact integer rendering")).expect("exact integer parsing"), screenplay);
    let rendered = String::from_utf8(render_json(&sample()).expect("model bytes")).expect("UTF-8");
    for number in ["18446744073709551617", "-9223372036854775809", "1.25", "2e0", "1.0"] {
        let input = rendered.replacen("{", &format!("{{\"extensions\":{{\"source:number\":{number}}},"), 1);
        assert!(parse_json(input.as_bytes()).is_err(), "numeric precision risk accepted: {number}");
    }
    screenplay.extensions.insert("source:fraction".to_owned(), json!(1.25));
    assert!(validate(&screenplay).is_err(), "SDK model follows the same exact numeric domain");
}

#[test]
fn inspection_digest_observes_json_bytes_without_reobserving_declared_source() {
    let bytes = render_json(&sample()).expect("model bytes");
    let report = inspect_json(&bytes).expect("inspection");
    assert_eq!(report.input_sha256, format!("{:x}", Sha256::digest(&bytes)));
    assert_eq!(report.source.sha256, "a".repeat(64));
    assert_ne!(report.input_sha256, report.source.sha256);
    assert!(!report.source_identity_verified);
    assert_eq!(report.element_counts.get("dialogue"), Some(&2));
    assert_eq!(report.dual_dialogue_groups, 1);
}

#[test]
fn directional_policies_never_infer_native_support_or_symmetric_roundtrips() {
    let screenplay = sample();
    let canonical = assess_export(&screenplay, ScreenplayExportTarget::ScreenplayJson).expect("canonical policy");
    assert!(canonical.adapter_available);
    assert_eq!(canonical.preservation, ScreenplayPreservation::CanonicalModel);
    assert!(canonical.review_required.is_empty());
    for target in [ScreenplayExportTarget::Fountain, ScreenplayExportTarget::Fdx, ScreenplayExportTarget::FadeIn, ScreenplayExportTarget::Osf, ScreenplayExportTarget::Pdf, ScreenplayExportTarget::Text] {
        let policy = assess_export(&screenplay, target).expect("native policy");
        assert!(!policy.adapter_available);
        assert_eq!(policy.preservation, ScreenplayPreservation::Unassessed);
        assert!(policy.review_required.contains(&ScreenplayLossKind::DualDialogue));
    }
    for source in [ScreenplayImportFormat::Fountain, ScreenplayImportFormat::Fdx, ScreenplayImportFormat::FadeIn, ScreenplayImportFormat::Osf] {
        assert!(!assess_import(source).adapter_available);
        assert_eq!(assess_import(source).preservation, ScreenplayPreservation::Unassessed);
    }
    assert_eq!(serde_json::to_value(ScreenplayExportTarget::FadeIn).expect("target JSON"), json!("fadein"));
}

#[test]
fn prior_directional_losses_are_retained_as_declarations_not_new_findings() {
    let mut screenplay = sample();
    screenplay.losses.push(ScreenplayLossEvidence {
        direction: ScreenplayLossDirection::Import, format: "fdx".to_owned(),
        kind: ScreenplayLossKind::Pagination, detail: "Operator-declared earlier import did not retain page breaks.".to_owned(),
    });
    let report = inspect_json(&render_json(&screenplay).expect("model with evidence")).expect("report");
    assert_eq!(report.prior_losses, screenplay.losses);
    assert!(assess_export(&screenplay, ScreenplayExportTarget::Fountain).expect("policy").review_required.contains(&ScreenplayLossKind::PriorLossEvidence));
}
