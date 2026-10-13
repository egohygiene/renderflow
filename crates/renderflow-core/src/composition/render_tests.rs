//! Authored source regressions. Execution is deferred by the current sprint.

use std::path::Path;

use super::*;
use crate::composition::model::LocaleOverride;

fn fixture() -> LoadedComposition {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    LoadedComposition::load(root.join("examples/page-composition/page.composition.json")).unwrap()
}

#[test]
fn same_snapshot_produces_equal_bytes_and_keeps_review_diagnostics() {
    let snapshot = fixture();
    let first = render(&snapshot, "en", CompositionOutput::Svg).unwrap();
    let second = render(&snapshot, "en", CompositionOutput::Svg).unwrap();
    assert_eq!(first.bytes, second.bytes);
    assert!(first.diagnostics.iter().any(|item| item.code == "composition.text.layout_unverified"));
    assert!(first.diagnostics.iter().any(|item| item.code == "composition.font.glyph_coverage_unverified"));
    let svg = String::from_utf8(first.bytes).unwrap();
    assert!(svg.contains("data:image/svg+xml;base64,"));
    assert!(svg.contains("data:font/woff2;base64,"));
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
    assert!(svg.contains("&lt;script&gt; &amp; &quot;hello&quot;</tspan><tspan"));
    assert!(svg.contains(">A second line.</tspan>"));
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
    let checks = diagnostics(&snapshot, "en");
    assert!(checks.iter().any(|item| item.code == "composition.text.vertical_capacity"
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
fn raw_font_family_never_becomes_generated_css() {
    let mut snapshot = fixture();
    for asset in &mut snapshot.fonts.registry.assets {
        asset.family = "bad\";} body{background:url(https://invalid.example/)} /*".into();
    }
    let svg = String::from_utf8(render(&snapshot, "en", CompositionOutput::Svg).unwrap().bytes).unwrap();
    assert!(svg.contains("font-family:\"rf_font_"));
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
    let snapshot = fixture();
    let checks = diagnostics(&snapshot, "en");
    assert!(checks.iter().any(|item| item.code == "composition.page.gutter_unresolved"
        && item.severity == DiagnosticSeverity::Warning && item.layer_id.is_none()));
    assert!(!checks.iter().any(|item| item.severity == DiagnosticSeverity::Error));
    assert!(check_output_bound(64 * 1024 * 1024).is_ok());
    assert!(check_output_bound(64 * 1024 * 1024 + 1).is_err());
}
