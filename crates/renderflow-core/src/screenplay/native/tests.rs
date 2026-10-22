use std::io::Write;

use zip::{write::SimpleFileOptions, ZipWriter};

use super::*;

const FOUNTAIN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/screenplay/native-424/strict.fountain"));
const FDX: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/screenplay/native-424/strict.fdx"));
const OSF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/screenplay/native-424/strict.osf"));

fn replace(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    std::str::from_utf8(bytes).unwrap().replace(from, to).into_bytes()
}

fn package(entries: &[(&str, &[u8])], compression: CompressionMethod) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer.start_file(*name, SimpleFileOptions::default().compression_method(compression)).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn original_plain_sources_share_ordered_body_semantics() {
    let fountain = parse_native(FOUNTAIN, NativeScreenplayFormat::Fountain).unwrap();
    for (format, bytes) in [(NativeScreenplayFormat::Fdx, FDX), (NativeScreenplayFormat::Osf, OSF)] {
        let imported = parse_native(bytes, format).unwrap();
        equivalent_semantics(&imported, &fountain).unwrap();
        assert_eq!(imported.elements.len(), 6);
        assert_eq!(imported.elements[3].text, "softly");
        assert_eq!(imported.source.sha256, format!("{:x}", Sha256::digest(bytes)));
        assert_eq!(imported.source.format, format.as_str());
    }
}

#[test]
fn stored_and_deflated_fadein_are_read_without_extraction() {
    for compression in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let bytes = package(&[("document.xml", OSF)], compression);
        let imported = parse_native(&bytes, NativeScreenplayFormat::FadeIn).unwrap();
        let plain = parse_native(OSF, NativeScreenplayFormat::Osf).unwrap();
        equivalent_semantics(&imported, &plain).unwrap();
        assert_eq!(imported.source.sha256, format!("{:x}", Sha256::digest(&bytes)));
        assert_eq!(imported.source.format, "fadein");
    }
}

#[test]
fn unknown_or_missing_native_versions_fail_closed() {
    for bytes in [replace(FDX, "Version=\"1\"", "Version=\"3\""), replace(FDX, "Version=\"1\"", "Version=\"\"")]
    { assert!(parse_native(&bytes, NativeScreenplayFormat::Fdx).is_err()); }
    for version in ["12", "2.0", "30", "40"] {
        assert!(parse_native(&replace(OSF, "version=\"20\"", &format!("version=\"{version}\"")), NativeScreenplayFormat::Osf).is_err());
    }
}

#[test]
fn titles_metadata_unknown_elements_and_styling_are_not_discarded() {
    for bytes in [
        replace(FDX, "</Content>", "</Content><TitlePage><Content/></TitlePage>"),
        replace(FDX, "<Text>A silver", "<Text Style=\"Bold\">A silver"),
        replace(FDX, "<Paragraph Type=\"Action\">", "<Paragraph Type=\"Action\" Number=\"4\">"),
        replace(FDX, "Type=\"Action\"", "Type=\"ScriptNote\""),
        replace(FDX, "<Content>", "<Content><CustomData/>"),
    ] { assert!(parse_native(&bytes, NativeScreenplayFormat::Fdx).is_err()); }
    assert!(parse_native(&replace(OSF, "<paragraphs>", "<info/><paragraphs>"), NativeScreenplayFormat::Osf).is_err());
    assert!(parse_native(&replace(OSF, "<para>", "<para bookmark=\"Action\">"), NativeScreenplayFormat::Osf).is_err());
}

#[test]
fn multiple_text_runs_and_mixed_xml_content_are_refused() {
    for bytes in [
        replace(FDX, "A silver leaf rests on the table.", "A silver</Text><Text>leaf rests on the table."),
        replace(FDX, "<Content>", "<Content>discard me"),
        replace(FDX, "A silver leaf", "A <Bold>silver</Bold> leaf"),
    ] { assert!(parse_native(&bytes, NativeScreenplayFormat::Fdx).is_err()); }
}

#[test]
fn xml_active_or_opaque_markup_is_refused() {
    let body = std::str::from_utf8(FDX).unwrap().split_once('\n').unwrap().1;
    for prefix in ["<!DOCTYPE FinalDraft>", "<!DOCTYPE FinalDraft [<!ENTITY external SYSTEM \"file:///tmp/secret\">]>", "<!--comment-->", "<?worker instruction?>"] {
        assert!(parse_native(format!("{prefix}{body}").as_bytes(), NativeScreenplayFormat::Fdx).is_err());
    }
    assert!(parse_native(&replace(FDX, "A silver leaf", "<![CDATA[A silver leaf]]>"), NativeScreenplayFormat::Fdx).is_err());
    assert!(parse_native(&replace(FDX, "<Content>", "<Content xmlns=\"urn:unhandled\">"), NativeScreenplayFormat::Fdx).is_err());
}

#[test]
fn xml_depth_is_bounded() {
    let nested = format!("{}text{}", "<node>".repeat(12), "</node>".repeat(12));
    let error = parse_native(nested.as_bytes(), NativeScreenplayFormat::Fdx).unwrap_err();
    assert_eq!(error.code, "screenplay.native.bounds");
}

#[test]
fn native_text_and_input_byte_limits_are_enforced() {
    let long = "a".repeat(MAX_TEXT_BYTES + 1);
    assert!(parse_native(&replace(FDX, "A silver leaf rests on the table.", &long), NativeScreenplayFormat::Fdx).is_err());
    let bytes = vec![b'x'; MAX_INPUT_BYTES + 1];
    assert_eq!(parse_native(&bytes, NativeScreenplayFormat::Fountain).unwrap_err().code, "screenplay.native.bounds");
}

#[test]
fn empty_fdx_spacing_is_distinguished_from_nonempty_data() {
    let spaced = replace(FDX, "<Content>", "<Content><Paragraph Type=\"Empty\"><Text>  \n</Text></Paragraph>");
    equivalent_semantics(&parse_native(&spaced, NativeScreenplayFormat::Fdx).unwrap(), &parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap()).unwrap();
    let data = replace(FDX, "<Content>", "<Content><Paragraph Type=\"Empty\"><Text>lost text</Text></Paragraph>");
    assert!(parse_native(&data, NativeScreenplayFormat::Fdx).is_err());
}

#[test]
fn lowercase_or_whitespace_changes_are_never_hidden_in_comparison() {
    let source = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    let lower = parse_native(&replace(FDX, "GLASSHOUSE", "Glasshouse"), NativeScreenplayFormat::Fdx).unwrap();
    assert_eq!(equivalent_semantics(&source, &lower).unwrap_err().code, "screenplay.native.semantic_mismatch");
    assert!(parse_native(&replace(FDX, "A silver leaf", " A silver leaf"), NativeScreenplayFormat::Fdx).is_err());
}

#[test]
fn reordered_or_retyped_body_is_a_semantic_mismatch() {
    let source = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    let changed = parse_native(&replace(FDX, "Type=\"Transition\"", "Type=\"Action\""), NativeScreenplayFormat::Fdx).unwrap();
    assert!(equivalent_semantics(&source, &changed).is_err());
    let mut reordered = source.clone();
    reordered.elements.swap(0, 1);
    assert!(equivalent_semantics(&source, &reordered).is_err());
}

#[test]
fn semantic_comparison_ignores_generated_ids_but_resolves_owner_positions() {
    let source = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    let mut renamed = source.clone();
    let owner = renamed.elements[2].id.clone();
    renamed.elements[2].id = "different-character-id".to_owned();
    for element in &mut renamed.elements {
        if element.character_id.as_deref() == Some(owner.as_str()) { element.character_id = Some("different-character-id".to_owned()); }
    }
    equivalent_semantics(&source, &renamed).unwrap();
    renamed.elements[3].character_id = Some("not-a-character".to_owned());
    assert!(equivalent_semantics(&source, &renamed).is_err());
}

#[test]
fn comparison_does_not_silently_ignore_opaque_extensions() {
    let source = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    let mut extended = source.clone();
    extended.extensions.insert("fixture:unhandled".to_owned(), serde_json::json!("meaning"));
    assert_eq!(equivalent_semantics(&source, &extended).unwrap_err().code, "screenplay.native.unsupported");
}

#[test]
fn parenthetical_syntax_is_decoded_without_arbitrary_trimming() {
    let wrapped = replace(OSF, "<text>softly</text>", "<text>(softly)</text>");
    equivalent_semantics(&parse_native(&wrapped, NativeScreenplayFormat::Osf).unwrap(), &parse_native(OSF, NativeScreenplayFormat::Osf).unwrap()).unwrap();
    for value in ["( softly)", "((softly))", "softly)", "(softly", "()"] {
        assert!(parse_native(&replace(OSF, "<text>softly</text>", &format!("<text>{value}</text>")), NativeScreenplayFormat::Osf).is_err());
    }
}

#[test]
fn orphan_dialogue_and_empty_documents_are_rejected() {
    assert!(parse_native(b"", NativeScreenplayFormat::Fountain).is_err());
    assert!(parse_native(b"<FinalDraft DocumentType=\"Script\" Template=\"No\" Version=\"1\"><Content/></FinalDraft>", NativeScreenplayFormat::Fdx).is_err());
    assert!(parse_native(&replace(FDX, "Type=\"Character\"", "Type=\"Action\""), NativeScreenplayFormat::Fdx).is_err());
}

#[test]
fn fountain_authoring_constructs_are_explicitly_out_of_profile() {
    for line in ["*styled action*", "[[note]]", "# Section", "= Synopsis", "@NOVA", "!Forced action", ".Forced scene", ">Centered<", "NOVA ^", "~lyric", "A \\*literal marker"] {
        let input = format!("INT. GLASSHOUSE - MORNING\n\n{line}\n");
        assert!(parse_native(input.as_bytes(), NativeScreenplayFormat::Fountain).is_err(), "accepted {line}");
    }
    assert!(parse_native(b"Title: hidden metadata\n\nINT. GLASSHOUSE - MORNING\n", NativeScreenplayFormat::Fountain).is_err());
}

#[test]
fn fountain_multiline_or_incomplete_character_blocks_are_refused() {
    for body in ["NOVA\n(softly)", "NOVA\nHello.\nAnother dialogue line.", "First action.\nSecond action.", "(orphan)"] {
        assert!(parse_native(format!("INT. GLASSHOUSE - MORNING\n\n{body}\n").as_bytes(), NativeScreenplayFormat::Fountain).is_err());
    }
}

#[test]
fn fountain_encoding_and_controls_are_refused() {
    for bytes in [replace(FOUNTAIN, "\n", "\r\n"), replace(FOUNTAIN, "A silver", "\tA silver"), replace(FOUNTAIN, "A silver", "A\0silver")] {
        assert!(parse_native(&bytes, NativeScreenplayFormat::Fountain).is_err());
    }
    assert!(parse_native(&[0xff], NativeScreenplayFormat::Fountain).is_err());
}

#[test]
fn raw_xml_is_not_a_fadein_package() {
    assert!(parse_native(OSF, NativeScreenplayFormat::FadeIn).is_err());
}

#[test]
fn fadein_extra_members_and_unsafe_paths_are_refused() {
    for entries in [vec![("document.xml", OSF), ("other.xml", OSF)], vec![("../document.xml", OSF)], vec![("/document.xml", OSF)], vec![("nested/document.xml", OSF)]] {
        assert!(parse_native(&package(&entries, CompressionMethod::Stored), NativeScreenplayFormat::FadeIn).is_err());
    }
}

#[test]
fn fadein_symlinks_are_refused() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer.add_symlink("document.xml", "outside.xml", SimpleFileOptions::default()).unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    assert!(parse_native(&bytes, NativeScreenplayFormat::FadeIn).is_err());
}

#[test]
fn fadein_uncompressed_bound_applies_before_member_read() {
    let huge = vec![b' '; MAX_INPUT_BYTES + 1];
    let bytes = package(&[("document.xml", &huge)], CompressionMethod::Deflated);
    assert_eq!(parse_native(&bytes, NativeScreenplayFormat::FadeIn).unwrap_err().code, "screenplay.native.bounds");
}

#[test]
fn fadein_corrupt_member_fails_complete_read() {
    let mut bytes = package(&[("document.xml", OSF)], CompressionMethod::Stored);
    let content_start = bytes.windows(OSF.len()).position(|window| window == OSF).unwrap();
    bytes[content_start] ^= 1;
    assert!(parse_native(&bytes, NativeScreenplayFormat::FadeIn).is_err());
}

#[test]
fn strict_body_exports_are_deterministic_and_preserve_all_ordered_semantics() {
    let body = parse_native(FOUNTAIN, NativeScreenplayFormat::Fountain).unwrap();
    let unchanged = body.clone();
    for format in [NativeScreenplayFormat::Fountain, NativeScreenplayFormat::Fdx,
        NativeScreenplayFormat::Osf, NativeScreenplayFormat::FadeIn, NativeScreenplayFormat::Trelby]
    {
        let bytes = export_body(&body, format).unwrap();
        assert_eq!(export_body(&body, format).unwrap(), bytes);
        equivalent_semantics(&body, &parse_native(&bytes, format).unwrap()).unwrap();
        assert_eq!(body, unchanged);
    }
}

#[test]
fn original_body_export_refuses_metadata_instead_of_clearing_the_model() {
    let source = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    let mut variants = Vec::new();
    let mut body = source.clone();
    body.title_page.push(crate::screenplay::TitlePageField {
        id: "original-title".to_owned(), key: "Title".to_owned(), value: "Keep this title".to_owned(),
        extensions: BTreeMap::new(),
    });
    variants.push(body);
    let mut body = source.clone();
    body.extensions.insert("fixture:meaning".to_owned(), serde_json::json!("retain me"));
    variants.push(body);
    let mut body = source.clone();
    body.source.extensions.insert("fixture:provenance".to_owned(), serde_json::json!("retain me"));
    variants.push(body);
    let mut body = source.clone();
    body.elements[1].extensions.insert("fixture:styling".to_owned(), serde_json::json!("retain me"));
    variants.push(body);
    let mut body = source.clone();
    body.elements[0].scene_number = Some("42".to_owned());
    variants.push(body);
    let mut body = source.clone();
    body.losses.push(crate::screenplay::ScreenplayLossEvidence {
        direction: crate::screenplay::ScreenplayLossDirection::Import,
        format: "fdx".to_owned(), kind: crate::screenplay::ScreenplayLossKind::Layout,
        detail: "Earlier layout loss must remain visible".to_owned(),
    });
    variants.push(body);
    for body in variants {
        let unchanged = body.clone();
        for format in [NativeScreenplayFormat::Fountain, NativeScreenplayFormat::Fdx,
            NativeScreenplayFormat::Osf, NativeScreenplayFormat::FadeIn, NativeScreenplayFormat::Trelby]
        {
            assert_eq!(export_body(&body, format).unwrap_err().code, "screenplay.native.export.unsupported");
            assert_eq!(body, unchanged);
        }
    }
}

#[test]
fn body_export_keeps_unicode_and_escapes_xml_as_text() {
    let mut body = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    body.elements[1].text = "Żółw & кошка see <a>, 'apostrophes', \"quotes\" and 🌌.".to_owned();
    for format in [NativeScreenplayFormat::Fdx, NativeScreenplayFormat::Osf,
        NativeScreenplayFormat::FadeIn, NativeScreenplayFormat::Trelby]
    {
        let bytes = export_body(&body, format).unwrap();
        let parsed = parse_native(&bytes, format).unwrap();
        equivalent_semantics(&body, &parsed).unwrap();
        assert_eq!(parsed.elements[1].text, body.elements[1].text);
    }
}

#[test]
fn fountain_body_export_refuses_reclassification_and_never_joins_original_dialogue() {
    let source = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    let mut body = source.clone();
    body.elements[1].text = "CUT TO:".to_owned();
    assert_eq!(export_body(&body, NativeScreenplayFormat::Fountain).unwrap_err().code,
        "screenplay.native.export.semantic_mismatch");
    let mut body = source;
    let mut second_dialogue = body.elements[4].clone();
    second_dialogue.id = "original-second-dialogue".to_owned();
    second_dialogue.text = "A separate authored paragraph.".to_owned();
    body.elements.insert(5, second_dialogue);
    let unchanged = body.clone();
    assert!(export_body(&body, NativeScreenplayFormat::Fountain).is_err());
    assert!(export_body(&body, NativeScreenplayFormat::Trelby).is_err());
    for format in [NativeScreenplayFormat::Fdx, NativeScreenplayFormat::Osf, NativeScreenplayFormat::FadeIn] {
        let parsed = parse_native(&export_body(&body, format).unwrap(), format).unwrap();
        equivalent_semantics(&body, &parsed).unwrap();
        assert_eq!(parsed.elements.len(), body.elements.len());
    }
    assert_eq!(body, unchanged);
}

#[test]
fn body_export_refuses_controls_and_ambiguous_parenthetical_payloads() {
    let source = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    for text in ["\tindented", "line\ncontinuation", "line\0control"] {
        let mut body = source.clone();
        body.elements[1].text = text.to_owned();
        assert!(export_body(&body, NativeScreenplayFormat::Fdx).is_err());
        assert!(export_body(&body, NativeScreenplayFormat::Trelby).is_err());
    }
    let mut body = source;
    body.elements[3].text = "quietly (to herself)".to_owned();
    for format in [NativeScreenplayFormat::Fountain, NativeScreenplayFormat::Fdx,
        NativeScreenplayFormat::Osf, NativeScreenplayFormat::FadeIn, NativeScreenplayFormat::Trelby]
    {
        assert!(export_body(&body, format).is_err());
    }
}

#[test]
fn xml_escaping_expansion_is_bounded_before_output_is_returned() {
    let mut body = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    let mut action = body.elements[1].clone();
    action.text = "&".repeat(MAX_TEXT_BYTES);
    body.elements = (0..(MAX_INPUT_BYTES / (MAX_TEXT_BYTES * 5) + 1)).map(|index| {
        let mut element = action.clone();
        element.id = format!("expanding-action-{index:06}");
        element
    }).collect();
    for format in [NativeScreenplayFormat::Fdx, NativeScreenplayFormat::Osf, NativeScreenplayFormat::FadeIn] {
        assert_eq!(export_body(&body, format).unwrap_err().code, "screenplay.native.export.bounds");
    }
}

#[test]
fn trelby_version_three_body_uses_exact_framing_codes_and_parenthetical_text() {
    // Story content is original to Renderflow; only format tokens derive from
    // the documented upstream version-3 format, not its sample screenplay.
    let bytes = "\u{feff}#Version 3\n#Start-Script \n.\\INT. GLASSHOUSE - MORNING\n..A silver leaf rests on the table.\n._NOVA\n.((softly)\n.:The room is ready.\n./CUT TO:\n".as_bytes();
    let body = parse_native(bytes, NativeScreenplayFormat::Trelby).unwrap();
    assert_eq!(body.elements.len(), 6);
    assert_eq!(body.elements[3].text, "softly");
    assert_eq!(body.elements[3].character_id, Some(body.elements[2].id.clone()));
    assert_eq!(body.elements[4].character_id, Some(body.elements[2].id.clone()));
    assert_eq!(body.source.format, "trelby");
    assert_eq!(body.source.sha256, format!("{:x}", Sha256::digest(bytes)));
    assert_eq!(export_body(&body, NativeScreenplayFormat::Trelby).unwrap(), bytes);
}

#[test]
fn trelby_body_refuses_unknown_versions_config_and_incomplete_records() {
    let valid = "\u{feff}#Version 3\n#Start-Script \n..A leaf opens.\n";
    for bytes in [
        valid.trim_start_matches('\u{feff}').as_bytes().to_vec(),
        valid.replace("#Version 3", "#Version 2").into_bytes(),
        valid.replace("#Version 3", "#Version 30").into_bytes(),
        valid.replace('\n', "\r\n").into_bytes(),
        valid.replace("#Start-Script \n", "#Start-Script\n").into_bytes(),
        valid.replace("#Start-Script \n", "#Begin-Config \n#End-Config \n#Start-Script \n").into_bytes(),
        valid.trim_end_matches('\n').as_bytes().to_vec(),
        format!("{valid}#End-Script \n").into_bytes(),
        format!("{TRELBY_BODY_PREFIX}\n").into_bytes(),
        format!("{TRELBY_BODY_PREFIX}.\n").into_bytes(),
        format!("{TRELBY_BODY_PREFIX}..\n").into_bytes(),
        format!("{TRELBY_BODY_PREFIX}>.Wrapped continuation\n").into_bytes(),
        format!("{TRELBY_BODY_PREFIX}.%A note cannot be discarded.\n").into_bytes(),
        format!("{TRELBY_BODY_PREFIX}.=A shot cannot be retyped.\n").into_bytes(),
        format!("{TRELBY_BODY_PREFIX}.@An act break cannot be retyped.\n").into_bytes(),
    ] {
        assert!(parse_native(&bytes, NativeScreenplayFormat::Trelby).is_err());
    }
    assert!(parse_native(&[0xff], NativeScreenplayFormat::Trelby).is_err());
}

#[test]
fn trelby_paragraph_neighbors_and_nonliteral_line_separators_are_refused() {
    for body in [
        "._NOVA\n..This interrupts the character.\n",
        "._NOVA\n.:First paragraph.\n.:Second paragraph.\n",
        "._NOVA\n.((quietly)\n.((again)\n.:Words.\n",
        "..A line\u{2028}separator.\n",
        "..A paragraph\u{2029}separator.\n",
        "..A tab\tchanges upstream.\n",
        "..A form\u{000c}feed disappears upstream.\n",
    ] {
        assert!(parse_native(format!("{TRELBY_BODY_PREFIX}{body}").as_bytes(), NativeScreenplayFormat::Trelby).is_err());
    }
    let mut body = parse_native(FDX, NativeScreenplayFormat::Fdx).unwrap();
    for separator in ['\u{2028}', '\u{2029}'] {
        body.elements[1].text = format!("A hidden{separator}line break.");
        assert_eq!(export_body(&body, NativeScreenplayFormat::Trelby).unwrap_err().code,
            "screenplay.native.export.text");
    }
}

#[test]
fn trelby_body_limits_apply_before_unbounded_record_collection() {
    let oversized_text = format!("{TRELBY_BODY_PREFIX}..{}\n", "a".repeat(MAX_TEXT_BYTES + 1));
    assert!(parse_native(oversized_text.as_bytes(), NativeScreenplayFormat::Trelby).is_err());
    let oversized_input = vec![b'x'; MAX_INPUT_BYTES + 1];
    assert_eq!(parse_native(&oversized_input, NativeScreenplayFormat::Trelby).unwrap_err().code,
        "screenplay.native.bounds");
    let oversized_count = format!("{TRELBY_BODY_PREFIX}{}", "..A leaf opens.\n".repeat(MAX_ELEMENTS + 1));
    assert_eq!(parse_native(oversized_count.as_bytes(), NativeScreenplayFormat::Trelby).unwrap_err().code,
        "screenplay.native.bounds");
}
