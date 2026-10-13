//! Authored source regressions. Execution is deferred by the current sprint.

use std::path::Path;
use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::composition::model::LocaleOverride;
use crate::composition::typography;
use crate::font::{FontRole, LoadedFontRegistry};

fn fixture() -> LoadedComposition {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    LoadedComposition::load(root.join("examples/page-composition/measured-typography/page.composition.json")).unwrap()
}

fn structural_fixture() -> LoadedComposition {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    LoadedComposition::load(root.join("examples/page-composition/page.composition.json")).unwrap()
}

fn replace_caption(snapshot: &mut LoadedComposition, text: &str) {
    // Unit-only mutation exercises diagnostics. Public snapshots expose no
    // mutation API; normal callers reload a freshly pinned content document.
    snapshot.contents.get_mut("en").unwrap().document.content
        .insert("panel.one.caption".into(), text.into());
}

fn replace_font_bytes(snapshot: &mut LoadedComposition, id: &str, bytes: Vec<u8>) {
    let mut registry = snapshot.fonts.registry.clone();
    let mut assets = registry.assets.iter().map(|asset|
        (asset.id.clone(), snapshot.fonts.captured_asset_bytes(&asset.id).unwrap().to_vec())
    ).collect::<BTreeMap<_, _>>();
    registry.assets.iter_mut().find(|asset| asset.id == id).unwrap().digest.value =
        crate::composition::hash_bytes(&bytes);
    assets.insert(id.to_string(), bytes);
    let licenses = registry.assets.iter().map(|asset| asset.license.license_file.clone()).collect();
    let mut registry_digest = snapshot.fonts.registry_digest.clone();
    registry_digest.value = crate::composition::hash_bytes(&serde_json::to_vec(&registry).unwrap());
    snapshot.fonts = LoadedFontRegistry::from_snapshot(registry, snapshot.fonts.source_path.clone(),
        registry_digest, assets, licenses);
}

/// Fixture-only mutation: remove ASCII A from the bundled TTF's Unicode cmap.
/// Table signatures, outline bytes, and font licensing are left intact. The
/// caller pins the modified bytes, so this isolates nominal glyph coverage from
/// digest failures. Every byte access remains slice-bounds checked.
fn without_capital_a(mut bytes: Vec<u8>) -> Vec<u8> {
    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }
    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }
    let directory = 12;
    let cmap = (0..usize::from(u16_at(&bytes, 4))).map(|index| directory + index * 16)
        .find(|offset| &bytes[*offset..*offset + 4] == b"cmap").unwrap();
    let start = usize::try_from(u32_at(&bytes, cmap + 8)).unwrap();
    let offsets = (0..usize::from(u16_at(&bytes, start + 2)))
        .map(|index| start + usize::try_from(u32_at(&bytes, start + 4 + index * 8 + 4)).unwrap())
        .collect::<BTreeSet<_>>();
    let scalar = u32::from(b'A');
    let mut changed = 0;
    for offset in offsets {
        match u16_at(&bytes, offset) {
            4 => {
                let segments = usize::from(u16_at(&bytes, offset + 6)) / 2;
                let end_codes = offset + 14;
                let start_codes = end_codes + segments * 2 + 2;
                let deltas = start_codes + segments * 2;
                let range_offsets = deltas + segments * 2;
                for index in 0..segments {
                    if u32::from(u16_at(&bytes, start_codes + index * 2)) <= scalar
                        && scalar <= u32::from(u16_at(&bytes, end_codes + index * 2)) {
                        bytes[deltas + index * 2..deltas + index * 2 + 2]
                            .copy_from_slice(&0u16.wrapping_sub(scalar as u16).to_be_bytes());
                        bytes[range_offsets + index * 2..range_offsets + index * 2 + 2]
                            .copy_from_slice(&0u16.to_be_bytes());
                        changed += 1;
                    }
                }
            }
            12 => {
                for index in 0..usize::try_from(u32_at(&bytes, offset + 12)).unwrap() {
                    let group = offset + 16 + index * 12;
                    if u32_at(&bytes, group) <= scalar && scalar <= u32_at(&bytes, group + 4) {
                        bytes[group..group + 4].copy_from_slice(&scalar.to_be_bytes());
                        bytes[group + 4..group + 8].copy_from_slice(&scalar.to_be_bytes());
                        bytes[group + 8..group + 12].copy_from_slice(&0u32.to_be_bytes());
                        changed += 1;
                    }
                }
            }
            _ => {}
        }
    }
    assert!(changed > 0, "bundled fixture must expose format 4 or 12 Unicode cmap");
    bytes
}

#[test]
fn same_snapshot_produces_equal_outline_bytes_and_keeps_review_requirements() {
    let snapshot = fixture();
    let first = render(&snapshot, "en", CompositionOutput::Svg).unwrap();
    let second = render(&snapshot, "en", CompositionOutput::Svg).unwrap();
    assert_eq!(first.bytes, second.bytes);
    let svg = String::from_utf8(first.bytes).unwrap();
    assert!(svg.contains("data:image/svg+xml;base64,"));
    assert!(svg.contains("<path"));
    assert!(!svg.contains("<text"));
    assert!(!svg.contains("<tspan"));
    assert!(!svg.contains("@font-face"));
    assert!(!svg.contains("data:font/"));
    assert!(svg.contains("human review required"));
    assert!(!svg.contains("file://"));
}

#[test]
fn localized_content_is_escaped_and_retains_explicit_lines() {
    let mut snapshot = fixture();
    snapshot.contents.get_mut("en").unwrap().document.content.insert(
        "panel.one.caption".into(), "<script> & \"hello\"\nA second line.".into());
    let svg = String::from_utf8(render(&snapshot, "en", CompositionOutput::Svg).unwrap().bytes).unwrap();
    assert!(!svg.contains("<script>"));
    assert!(svg.contains("&lt;script&gt; &amp; &quot;hello&quot;"));
    assert!(svg.contains("A second line."));
    assert!(!svg.contains("<text"));
    let transcript: serde_json::Value = serde_json::from_slice(
        &render(&snapshot, "en", CompositionOutput::TranscriptJson).unwrap().bytes).unwrap();
    assert_eq!(transcript["entries"][0]["text"], "<script> & \"hello\"\nA second line.");
    let html = String::from_utf8(render(&snapshot, "en", CompositionOutput::Html).unwrap().bytes).unwrap();
    assert!(!html.contains("<script>"));
    assert!(html.contains("Content-Security-Policy"));
    assert!(html.contains("<figure aria-hidden=\"true\">"));
}

#[test]
fn reading_order_does_not_rearrange_paint_order() {
    let mut snapshot = fixture();
    snapshot.manifest.reading_order.reverse();
    let text = render(&snapshot, "en", CompositionOutput::TranscriptJson).unwrap();
    let transcript: serde_json::Value = serde_json::from_slice(&text.bytes).unwrap();
    assert_eq!(transcript["entries"][0]["layer_id"], "caption.text.two");
    assert_eq!(transcript["entries"][1]["layer_id"], "caption.text.one");
    assert_eq!(transcript["requires_human_review"], true);
    let svg = String::from_utf8(render(&snapshot, "en", CompositionOutput::Svg).unwrap().bytes).unwrap();
    assert!(svg.find("data-layer-id=\"caption.text.one\"").unwrap()
        < svg.find("data-layer-id=\"caption.text.two\"").unwrap());
}

#[test]
fn locale_override_changes_only_the_named_layer_and_locale() {
    let mut snapshot = fixture();
    let bounds = NormalizedRect { x: 0.15, y: 0.2, width: 0.5, height: 0.3 };
    snapshot.manifest.locale_overrides.insert("en".into(), vec![LocaleOverride {
        layer_id: "caption.text.one".into(), reason: "Explicit reviewed placement".into(),
        author: "fixture".into(), bounds: Some(bounds), font_size_pt: Some(10.0),
    }]);
    let first = snapshot.manifest.layers.iter().find(|layer| layer.id == "caption.text.one").unwrap();
    let (actual, size) = effective_geometry(&snapshot, "en", first, 12.0);
    assert_eq!(actual.x, 0.15);
    assert_eq!(actual.height, 0.3);
    assert_eq!(size, 10.0);
    let (unchanged, size) = effective_geometry(&snapshot, "ar", first, 12.0);
    assert_eq!(unchanged.x, first.bounds.x);
    assert_eq!(size, 12.0);
}

#[test]
fn known_vertical_overflow_blocks_visual_output_without_shrinking() {
    let mut snapshot = fixture();
    let layer = snapshot.manifest.layers.iter_mut().find(|layer| layer.id == "caption.text.one").unwrap();
    layer.bounds.height = 0.04;
    let checks = typography::measure(&snapshot, "en");
    assert!(checks.diagnostics.iter().any(|item| item.code == "composition.typography.vertical_overflow"
        && item.severity == DiagnosticSeverity::Error));
    assert!(render(&snapshot, "en", CompositionOutput::Svg).is_err());
    // The transcript preserves text even when a visual container cannot fit it.
    assert!(render(&snapshot, "en", CompositionOutput::TranscriptText).is_ok());
}

#[test]
fn unsupported_visual_layout_still_allows_direction_tagged_transcripts() {
    let mut snapshot = fixture();
    // The bundled RTL/vertical copy is deliberately unreviewed. Here only the
    // synthetic approval gate is removed to isolate the layout capability rule.
    for locale in ["ar", "ja"] {
        snapshot.contents.get_mut(locale).unwrap().document.reviewed = true;
    }
    snapshot.diagnostics.retain(|item| item.code != "composition.content.unreviewed");
    assert!(diagnostics(&snapshot, "ar").iter().any(|item| item.code == "composition.layout.unsupported"));
    assert!(render(&snapshot, "ar", CompositionOutput::Svg).is_err());
    assert!(render(&snapshot, "ja", CompositionOutput::Html).is_err());
    let bytes = render(&snapshot, "ar", CompositionOutput::TranscriptJson).unwrap().bytes;
    let transcript: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(transcript["direction"], "rtl");
    assert_eq!(transcript["qualification"], "unqualified");
}

#[test]
fn raw_font_family_never_becomes_generated_markup() {
    let mut snapshot = fixture();
    for asset in &mut snapshot.fonts.registry.assets {
        asset.family = "bad\";} body{background:url(https://invalid.example/)} /*".into();
    }
    let svg = String::from_utf8(render(&snapshot, "en", CompositionOutput::Svg).unwrap().bytes).unwrap();
    assert!(svg.contains("<path"));
    assert!(!svg.contains("font-family"));
    assert!(!svg.contains("invalid.example"));
}

#[test]
fn content_approval_diagnostics_cannot_be_bypassed_by_direct_rendering() {
    let mut snapshot = fixture();
    snapshot.diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
        "composition.content.unreviewed", "not reviewed", Some("en"), None));
    let error = render(&snapshot, "en", CompositionOutput::TranscriptJson).unwrap_err();
    assert!(error.to_string().contains("composition.content.unreviewed"));
}

#[test]
fn effective_locale_bounds_must_stay_inside_safe_area_and_clip() {
    let mut snapshot = fixture();
    let layer = snapshot.manifest.layers.iter_mut().find(|layer| layer.id == "caption.text.one").unwrap();
    layer.clip = Some(layer.bounds);
    snapshot.manifest.locale_overrides.insert("de".into(), vec![LocaleOverride {
        layer_id: "caption.text.one".into(), reason: "Negative geometry fixture".into(),
        author: "fixture".into(), font_size_pt: None,
        bounds: Some(NormalizedRect { x: 0.01, y: 0.09, width: 0.4, height: 0.2 }),
    }]);
    let english = diagnostics(&snapshot, "en");
    assert!(!english.iter().any(|item| matches!(item.code.as_str(),
        "composition.text.outside_safe_area" | "composition.text.clip_excludes_bounds")));
    let german = diagnostics(&snapshot, "de");
    for code in ["composition.text.outside_safe_area", "composition.text.clip_excludes_bounds"] {
        assert!(german.iter().any(|item| item.code == code && item.severity == DiagnosticSeverity::Error
            && item.layer_id.as_deref() == Some("caption.text.one")));
    }
    assert!(render(&snapshot, "de", CompositionOutput::Svg).is_err());
    assert!(render(&snapshot, "de", CompositionOutput::TranscriptText).is_ok());
}

#[test]
fn gutter_intent_is_unresolved_and_output_limit_matches_executor() {
    let mut snapshot = fixture();
    snapshot.manifest.page.gutter_mm = 2.0;
    let checks = diagnostics(&snapshot, "en");
    assert!(checks.iter().any(|item| item.code == "composition.page.gutter_unresolved"
        && item.severity == DiagnosticSeverity::Warning && item.layer_id.is_none()));
    assert!(!checks.iter().any(|item| item.severity == DiagnosticSeverity::Error));
    assert!(check_output_bound(64 * 1024 * 1024).is_ok());
    assert!(check_output_bound(64 * 1024 * 1024 + 1).is_err());
}

#[test]
fn structural_font_stub_blocks_visuals_but_preserves_semantic_transcripts() {
    let snapshot = structural_fixture();
    let report = typography::measure(&snapshot, "en");
    assert!(report.diagnostics.iter().any(|item| item.code == "composition.typography.font_unavailable"
        && item.severity == DiagnosticSeverity::Error));
    assert!(render(&snapshot, "en", CompositionOutput::Svg).is_err());
    assert!(render(&snapshot, "en", CompositionOutput::Html).is_err());
    let bytes = render(&snapshot, "en", CompositionOutput::TranscriptJson).unwrap().bytes;
    let transcript: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(transcript["entries"][0]["text"],
        snapshot.contents["en"].document.content["panel.one.caption"]);
}

#[test]
fn measured_horizontal_overflow_refuses_visual_output_without_rewriting_copy() {
    let mut snapshot = fixture();
    let copy = "W".repeat(100);
    replace_caption(&mut snapshot, &copy);
    let report = typography::measure(&snapshot, "en");
    assert!(report.diagnostics.iter().any(|item| item.code == "composition.typography.horizontal_overflow"
        && item.severity == DiagnosticSeverity::Error
        && item.layer_id.as_deref() == Some("caption.text.one")));
    assert!(render(&snapshot, "en", CompositionOutput::Svg).is_err());
    let transcript: serde_json::Value = serde_json::from_slice(
        &render(&snapshot, "en", CompositionOutput::TranscriptJson).unwrap().bytes).unwrap();
    assert_eq!(transcript["entries"][0]["text"], copy);
    let layer = snapshot.manifest.layers.iter().find(|layer| layer.id == "caption.text.one").unwrap();
    let LayerContent::Text { typography, .. } = &layer.content else { panic!("text fixture required") };
    assert_eq!(typography.font_size_pt, 14.0);
}

#[test]
fn unsafe_line_starts_unhandled_scripts_and_text_budgets_fail_explicitly() {
    for (text, code) in [
        ("First line\n\u{0301}orphan mark".to_string(), "composition.typography.unsafe_line_start"),
        ("\u{0627}".to_string(), "composition.typography.unsupported_character"),
        ("a".repeat(4097), "composition.typography.text_bounds"),
        ("\n".repeat(256), "composition.typography.text_bounds"),
    ] {
        let mut snapshot = fixture();
        replace_caption(&mut snapshot, &text);
        let report = typography::measure(&snapshot, "en");
        assert!(report.diagnostics.iter().any(|item| item.code == code
            && item.severity == DiagnosticSeverity::Error
            && item.layer_id.as_deref() == Some("caption.text.one")), "expected {code}");
        assert!(render(&snapshot, "en", CompositionOutput::Svg).is_err());
        let transcript: serde_json::Value = serde_json::from_slice(
            &render(&snapshot, "en", CompositionOutput::TranscriptJson).unwrap().bytes).unwrap();
        assert_eq!(transcript["entries"][0]["text"], text);
    }
}

#[test]
fn unsupported_primary_has_an_explicit_measured_fallback_path() {
    let mut snapshot = fixture();
    let structural = structural_fixture();
    let stub = structural.fonts.registry.assets[0].clone();
    let mut registry = snapshot.fonts.registry.clone();
    let good_id = registry.assets[0].id.clone();
    registry.assets.push(stub.clone());
    let binding = registry.roles.get_mut(&FontRole::Caption).unwrap();
    binding.primary = stub.id.clone();
    binding.fallbacks = vec![good_id.clone()];
    let assets = BTreeMap::from([
        (good_id.clone(), snapshot.fonts.captured_asset_bytes(&good_id).unwrap().to_vec()),
        (stub.id.clone(), structural.fonts.captured_asset_bytes(&stub.id).unwrap().to_vec()),
    ]);
    let licenses = registry.assets.iter().map(|asset| asset.license.license_file.clone()).collect::<BTreeSet<_>>();
    snapshot.fonts = LoadedFontRegistry::from_snapshot(registry, snapshot.fonts.source_path.clone(),
        snapshot.fonts.registry_digest.clone(), assets, licenses);
    let report = typography::measure(&snapshot, "en");
    assert!(!report.diagnostics.iter().any(|item| item.severity == DiagnosticSeverity::Error));
    assert!(report.diagnostics.iter().any(|item| item.code == "composition.typography.fallback_selected"));
    for layer in &report.layers {
        let selected = layer.selected_font.as_ref().unwrap();
        assert_eq!(selected.asset_id, good_id);
        assert_eq!(selected.fallback_index, 1);
    }
    let svg = String::from_utf8(render(&snapshot, "en", CompositionOutput::Svg).unwrap().bytes).unwrap();
    assert!(svg.contains("<path"));
    assert!(!svg.contains("data:font/"));
}

#[test]
fn an_unmapped_character_is_reported_instead_of_a_notdef_outline() {
    let mut snapshot = fixture();
    replace_caption(&mut snapshot, "A");
    let id = snapshot.fonts.registry.assets[0].id.clone();
    let bytes = without_capital_a(snapshot.fonts.captured_asset_bytes(&id).unwrap().to_vec());
    replace_font_bytes(&mut snapshot, &id, bytes);
    let report = typography::measure(&snapshot, "en");
    let layer = report.layers.iter().find(|layer| layer.layer_id == "caption.text.one").unwrap();
    assert!(layer.selected_font.is_none());
    assert!(layer.rejected_candidates.iter().any(|candidate|
        candidate.asset_id == id && candidate.reason.contains("typography_missing_glyph")));
    assert!(report.diagnostics.iter().any(|item| item.code == "composition.typography.font_unavailable"
        && item.layer_id.as_deref() == Some("caption.text.one")));
    assert!(render(&snapshot, "en", CompositionOutput::Svg).is_err());
    let transcript: serde_json::Value = serde_json::from_slice(
        &render(&snapshot, "en", CompositionOutput::TranscriptJson).unwrap().bytes).unwrap();
    assert_eq!(transcript["entries"][0]["text"], "A");
}

#[test]
fn a_signature_only_ttf_is_not_accepted_as_a_measurable_font() {
    let mut snapshot = fixture();
    let id = snapshot.fonts.registry.assets[0].id.clone();
    let mut stub = vec![0; 64];
    stub[..4].copy_from_slice(b"\0\x01\0\0");
    replace_font_bytes(&mut snapshot, &id, stub);
    let report = typography::measure(&snapshot, "en");
    assert!(report.layers.iter().all(|layer| layer.selected_font.is_none()));
    assert!(report.layers.iter().all(|layer| layer.rejected_candidates.iter()
        .any(|candidate| candidate.reason.contains("typography_invalid_font"))));
    assert!(render(&snapshot, "en", CompositionOutput::Html).is_err());
    assert!(render(&snapshot, "en", CompositionOutput::TranscriptText).is_ok());
}

#[test]
fn measured_lines_retain_global_utf8_clusters_and_bounded_glyph_geometry() {
    let mut snapshot = fixture();
    let text = "Café\nA\u{0301}";
    replace_caption(&mut snapshot, text);
    let first = typography::measure(&snapshot, "en");
    let second = typography::measure(&snapshot, "en");
    assert!(!first.runtime_qualified);
    assert!(!first.diagnostics.iter().any(|item| item.severity == DiagnosticSeverity::Error));
    assert_eq!(serde_json::to_vec(&first).unwrap(), serde_json::to_vec(&second).unwrap());
    let layer = first.layers.iter().find(|layer| layer.layer_id == "caption.text.one").unwrap();
    assert_eq!(layer.lines.len(), 2);
    assert_eq!(layer.lines[0].source_byte_start, 0);
    assert_eq!(layer.lines[0].source_byte_end, "Café".len());
    assert_eq!(layer.lines[1].source_byte_start, "Café\n".len());
    assert_eq!(layer.lines[1].source_byte_end, text.len());
    assert_eq!(layer.selected_font.as_ref().unwrap().asset_id, "font.dejavu-sans.regular");
    assert_eq!(layer.outline_digest.as_ref().unwrap().len(), 64);
    for line in &layer.lines {
        assert!(line.origin_x_pt.is_finite() && line.baseline_y_pt.is_finite());
        assert!(line.advance_pt.is_finite() && line.advance_pt > 0.0);
        assert!(!line.glyphs.is_empty());
        let summed_advance = line.glyphs.iter().map(|glyph| glyph.x_advance_pt).sum::<f64>();
        assert!((summed_advance - line.advance_pt).abs() < 1e-6);
        let line_bounds = line.conservative_outline_bounds.unwrap();
        for glyph in &line.glyphs {
            assert_ne!(glyph.glyph_id, 0);
            assert!(glyph.cluster_utf8 >= line.source_byte_start && glyph.cluster_utf8 < line.source_byte_end);
            assert!(text.is_char_boundary(glyph.cluster_utf8));
            assert!([glyph.x_pt, glyph.y_pt, glyph.x_advance_pt, glyph.y_advance_pt]
                .into_iter().all(f64::is_finite));
            if let Some(ink) = glyph.conservative_outline_bounds {
                assert!(ink.x_min.is_finite() && ink.x_max.is_finite()
                    && ink.y_min.is_finite() && ink.y_max.is_finite());
                assert!(ink.x_min <= ink.x_max && ink.y_min <= ink.y_max);
                assert!(line_bounds.x_min <= ink.x_min && line_bounds.x_max >= ink.x_max);
                assert!(line_bounds.y_min <= ink.y_min && line_bounds.y_max >= ink.y_max);
            }
        }
    }
    let glyphs = first.layers.iter().flat_map(|layer| &layer.lines)
        .map(|line| line.glyphs.len()).sum::<usize>();
    assert!(glyphs <= 4096);
    let path_bytes = first.layers.iter().filter_map(|layer| first.outlines(&layer.layer_id))
        .flatten().map(|outline| outline.path.len()).sum::<usize>();
    assert!(path_bytes <= 4 * 1024 * 1024);
    let bytes = serde_json::to_vec(&first).unwrap();
    assert!(bytes.len() <= 2 * 1024 * 1024);
    let recovered: typography::TypographyReport = serde_json::from_slice(&bytes).unwrap();
    assert!(first.outlines("caption.text.one").is_some());
    assert!(recovered.outlines("caption.text.one").is_none());
    assert_eq!(crate::composition::hash_json(&first).unwrap(), crate::composition::hash_json(&recovered).unwrap());
    assert!(render_with_typography(&snapshot, "en", CompositionOutput::Svg, Some(&recovered)).is_err());
}
