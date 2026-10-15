//! Authored regressions only. Tests, rendering, and decoding remain deferred.

use std::path::Path;

use super::*;
use crate::composition::{plan, CompositionPlan, CompositionRequest, LocaleSelector};

fn fixture() -> LoadedComposition {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    LoadedComposition::load(root.join("examples/page-composition/raster-composition/page.composition.json")).unwrap()
}

fn planned(snapshot: &LoadedComposition, targets: Vec<CompositionOutput>) -> CompositionPlan {
    plan(snapshot, &CompositionRequest {
        locales: LocaleSelector::Exact { locale: "en".into() }, targets,
        ..CompositionRequest::default()
    }).unwrap()
}

fn branch(plan: &CompositionPlan, output: CompositionOutput) -> &CompositionBranchPlan {
    plan.branches.iter().find(|branch| branch.output == output).unwrap()
}

#[test]
fn warmed_svg_cannot_bypass_private_geometry_or_typography_identity() {
    let snapshot = fixture();
    let plan = planned(&snapshot, vec![CompositionOutput::Svg, CompositionOutput::Html]);
    let mut prepared = PreparedLocale::new("en");
    prepared.render(&snapshot, branch(&plan, CompositionOutput::Svg)).unwrap();
    let before = prepared.observations();

    let mut recovered = branch(&plan, CompositionOutput::Html).clone();
    recovered.typography = Some(serde_json::from_slice(
        &serde_json::to_vec(recovered.typography.as_ref().unwrap()).unwrap()).unwrap());
    let error = prepared.render(&snapshot, &recovered).unwrap_err();
    assert!(error.to_string().contains("composition.typography.geometry_missing"));
    assert_eq!(prepared.observations(), before);

    let mut different = branch(&plan, CompositionOutput::Html).clone();
    different.typography.as_mut().unwrap().provider_profile.push_str(";different=true");
    let error = prepared.render(&snapshot, &different).unwrap_err();
    assert!(error.to_string().contains("composition.shared.typography_mismatch"));
    assert_eq!(prepared.observations(), before);
}

#[test]
fn prepared_bytes_are_bound_to_one_snapshot_locale_and_target_attempt() {
    let snapshot = fixture();
    let another_snapshot = snapshot.clone();
    let plan = planned(&snapshot, vec![CompositionOutput::Svg, CompositionOutput::Html]);
    let mut prepared = PreparedLocale::new("en");
    prepared.render(&snapshot, branch(&plan, CompositionOutput::Svg)).unwrap();
    let before = prepared.observations();

    let error = prepared.render(&another_snapshot, branch(&plan, CompositionOutput::Html)).unwrap_err();
    assert!(error.to_string().contains("composition.shared.snapshot_mismatch"));
    let mut other_locale = branch(&plan, CompositionOutput::Html).clone();
    other_locale.locale = "de".into();
    let error = prepared.render(&snapshot, &other_locale).unwrap_err();
    assert!(error.to_string().contains("composition.shared.locale_mismatch"));
    let error = prepared.render(&snapshot, branch(&plan, CompositionOutput::Svg)).unwrap_err();
    assert!(error.to_string().contains("composition.shared.duplicate_target"));
    assert_eq!(prepared.observations(), before);
}

#[test]
fn branch_gates_and_output_settings_are_checked_before_shared_access() {
    let snapshot = fixture();
    let plan = planned(&snapshot, vec![CompositionOutput::Svg, CompositionOutput::Html,
        CompositionOutput::Png, CompositionOutput::Pdf]);
    let mut prepared = PreparedLocale::new("en");
    prepared.render(&snapshot, branch(&plan, CompositionOutput::Svg)).unwrap();
    let before = prepared.observations();

    let mut blocked = branch(&plan, CompositionOutput::Html).clone();
    blocked.state = CompositionBranchState::Blocked;
    assert!(prepared.render(&snapshot, &blocked).is_err());
    blocked.state = CompositionBranchState::Planned;
    blocked.diagnostics.push(diagnostic(DiagnosticSeverity::Error,
        "composition.review.required", "authored approval remains required", "en", None));
    assert!(prepared.render(&snapshot, &blocked).unwrap_err().to_string()
        .contains("composition.review.required"));

    let mut png = branch(&plan, CompositionOutput::Png).clone();
    png.raster.as_mut().unwrap().settings.dpi = 0;
    assert!(prepared.render(&snapshot, &png).unwrap_err().to_string().contains("composition.raster.dpi"));
    let mut pdf = branch(&plan, CompositionOutput::Pdf).clone();
    pdf.pdf.as_mut().unwrap().settings.conformance = "pdf_x_4".into();
    assert!(prepared.render(&snapshot, &pdf).is_err());
    assert_eq!(prepared.observations(), before);
}

#[test]
fn successful_stages_survive_a_downstream_pixel_decode_failure() {
    let mut snapshot = fixture();
    let asset = snapshot.assets.get_mut("raster.png-alpha").unwrap();
    // Unit-only corruption retains valid PNG framing and CRCs, so metadata
    // planning succeeds but complete decode cannot read this zlib stream.
    let mut position = 8usize;
    let mut changed = false;
    while position < asset.bytes.len() {
        let length = u32::from_be_bytes(asset.bytes[position..position + 4].try_into().unwrap()) as usize;
        let data_start = position + 8;
        let data_end = data_start + length;
        if &asset.bytes[position + 4..data_start] == b"IDAT" {
            assert!(length >= 2);
            asset.bytes[data_start..data_end].fill(0);
            let mut crc = crc32fast::Hasher::new();
            crc.update(&asset.bytes[position + 4..data_end]);
            asset.bytes[data_end..data_end + 4].copy_from_slice(&crc.finalize().to_be_bytes());
            changed = true;
        }
        position = data_end + 4;
    }
    assert!(changed);
    asset.declaration.file.sha256 = crate::composition::hash_bytes(&asset.bytes);
    let plan = planned(&snapshot, vec![CompositionOutput::Png, CompositionOutput::Svg,
        CompositionOutput::Html]);
    let png = branch(&plan, CompositionOutput::Png);
    assert_eq!(png.state, CompositionBranchState::Planned);
    let mut prepared = PreparedLocale::new("en");
    assert!(prepared.render(&snapshot, png).unwrap_err().to_string()
        .contains("composition.raster.png_decode"));
    let after_failure = prepared.observations();
    assert_eq!(after_failure.len(), 2);
    assert!(after_failure.iter().all(|stage| stage.materializations == 1
        && stage.reuse_count == 0 && stage.consumer_target_ids == vec![png.target_id.clone()]));

    let svg = branch(&plan, CompositionOutput::Svg);
    let svg_bytes = prepared.render(&snapshot, svg).unwrap().bytes;
    let after_svg = prepared.observations();
    assert_eq!(after_svg[0], after_failure[0]);
    assert_eq!(after_svg[1].reuse_count, 1);
    assert_eq!(after_svg[1].sha256, crate::composition::hash_bytes(&svg_bytes));
    assert_eq!(after_svg[1].size_bytes, svg_bytes.len() as u64);
    let html = branch(&plan, CompositionOutput::Html);
    prepared.render(&snapshot, html).unwrap();
    let after_html = prepared.observations();
    assert_eq!(after_html[0].reuse_count, 1);
    assert_eq!(after_html[1].reuse_count, 2);
    for stage in after_html {
        assert_eq!(stage.reuse_count as usize + 1, stage.consumer_target_ids.len());
        assert!(stage.consumer_target_ids.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn transcript_and_svg_digest_the_materialized_bytes_not_a_terminal_cache_guess() {
    let snapshot = fixture();
    let plan = planned(&snapshot, vec![CompositionOutput::Html, CompositionOutput::TranscriptJson]);
    let mut prepared = PreparedLocale::new("en");
    assert!(prepared.observations().is_empty());
    prepared.render(&snapshot, branch(&plan, CompositionOutput::Html)).unwrap();
    let transcript = prepared.render(&snapshot, branch(&plan, CompositionOutput::TranscriptJson)).unwrap();
    let observations = prepared.observations();
    assert_eq!(observations[0].kind, "transcript");
    assert_eq!(observations[0].sha256, crate::composition::hash_bytes(&transcript.bytes));
    assert_eq!(observations[0].size_bytes, transcript.bytes.len() as u64);
    assert_eq!(observations[0].reuse_count, 1);
    assert_eq!(observations[1].kind, "svg");
    assert_eq!(observations[1].reuse_count, 0);
}
