//! Authored/unrun local-profile cases. Synthetic review references are declarations,
//! not human approval or provider qualification. No voice/model service is used.

use renderflow::process::ProcessCancellationToken;
use renderflow::publication::briefing::{
    self, Briefing, BriefingRequest, Feed, Narration, NarrationOrigin, ReviewClaim,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn token() -> ProcessCancellationToken {
    ProcessCancellationToken::new()
}
fn fixture(root: &Path) -> (PathBuf, Briefing) {
    fs::write(
        root.join("canonical.txt"),
        include_bytes!("../../../examples/briefing/canonical.txt"),
    )
    .unwrap();
    let bytes = include_bytes!("../../../examples/briefing/briefing.json");
    let model = briefing::parse(bytes).unwrap();
    let input = root.join("briefing.json");
    fs::write(&input, bytes).unwrap();
    (input, model)
}
fn save(input: &Path, model: &Briefing) {
    fs::write(input, serde_json::to_vec_pretty(model).unwrap()).unwrap();
}
fn request(input: &Path, output: &Path, draft: bool) -> BriefingRequest {
    BriefingRequest {
        input: input.into(),
        output_dir: output.into(),
        allow_draft: draft,
    }
}

#[test]
fn planning_is_read_only_and_expected_files_are_not_publications() {
    let root = tempfile::tempdir().unwrap();
    let (input, _) = fixture(root.path());
    let before = fs::read(&input).unwrap();
    let plan = briefing::plan(&input, token()).unwrap();
    assert_eq!(plan.freshness, "current_local_snapshot");
    assert!(!plan.reviews_declared && !plan.runtime_qualified);
    assert_eq!(plan.publication_approval, "not_granted");
    assert!(plan
        .expected_files
        .iter()
        .any(|subject| subject.path == "transcript.html"));
    assert!(!root.path().join("transcript.html").exists());
    assert_eq!(fs::read(input).unwrap(), before);
}

#[test]
fn explicit_draft_build_retains_candidates_and_exact_source_bytes() {
    let root = tempfile::tempdir().unwrap();
    let (input, _) = fixture(root.path());
    let output = root.path().join("bundle");
    assert!(briefing::build(&request(&input, &output, false), token()).is_err());
    assert!(!output.exists());
    let result = briefing::build(&request(&input, &output, true), token()).unwrap();
    assert_eq!(result["state"], "complete");
    assert_eq!(result["review_state"], "candidate");
    assert_eq!(result["publication_approval"], "not_granted");
    assert_eq!(result["provider_executed"], false);
    assert_eq!(
        fs::read(output.join("sources/overview.txt")).unwrap(),
        fs::read(root.path().join("canonical.txt")).unwrap()
    );
    assert!(output.join("briefing-run.json").is_file());
    for item in result["files"].as_array().unwrap() {
        let bytes = fs::read(output.join(item["path"].as_str().unwrap())).unwrap();
        assert_eq!(hash(&bytes), item["sha256"].as_str().unwrap());
    }
}

#[test]
fn stale_sources_and_changed_script_reviews_cannot_build_current_bundles() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut model) = fixture(root.path());
    let output = root.path().join("bundle");
    let original = fs::read_to_string(root.path().join("canonical.txt")).unwrap();
    fs::write(
        root.path().join("canonical.txt"),
        original.replace("tide", "wind"),
    )
    .unwrap();
    assert_eq!(briefing::plan(&input, token()).unwrap().freshness, "stale");
    assert!(briefing::build(&request(&input, &output, true), token()).is_err());
    assert!(!output.exists());
    fs::write(root.path().join("canonical.txt"), original).unwrap();
    model.script_review = Some(ReviewClaim {
        sha256: briefing::plan(&input, token()).unwrap().script_sha256,
        reference: "synthetic declaration; no human approval".into(),
    });
    model.chapters[0].text.push_str(" Changed text.");
    save(&input, &model);
    assert!(briefing::plan(&input, token()).is_err());
}

#[test]
fn exact_review_declarations_never_grant_publication_or_audio_approval() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut model) = fixture(root.path());
    model.sources[0].review = Some(ReviewClaim {
        sha256: model.sources[0].sha256.clone(),
        reference: "synthetic fixture declaration".into(),
    });
    save(&input, &model);
    model.script_review = Some(ReviewClaim {
        sha256: briefing::plan(&input, token()).unwrap().script_sha256,
        reference: "synthetic script declaration".into(),
    });
    save(&input, &model);
    let result = briefing::build(
        &request(&input, &root.path().join("bundle"), false),
        token(),
    )
    .unwrap();
    assert_eq!(result["plan"]["reviews_declared"], true);
    assert_eq!(result["review_state"], "candidate");
    assert_eq!(result["remote_publication_performed"], false);
    assert_eq!(result["publication_approval"], "not_granted");
}

#[test]
fn literal_transcript_and_opt_in_rss_escape_markup_without_creating_audio() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut model) = fixture(root.path());
    model.title = "Tides & <notes>".into();
    model.chapters[0].text = "<script>alert('literal only')</script> & waves".into();
    model.feed = Some(Feed {
        title: "Synthetic notes".into(),
        description: "Original test-only feed candidate".into(),
        site_url: "https://example.invalid/".into(),
        episode_url: "https://example.invalid/r1".into(),
        narration_url: None,
    });
    save(&input, &model);
    let output = root.path().join("bundle");
    briefing::build(&request(&input, &output, true), token()).unwrap();
    let html = fs::read_to_string(output.join("transcript.html")).unwrap();
    assert!(html.contains("&lt;script&gt;") && !html.contains("<script>"));
    let rss = fs::read_to_string(output.join("feed.rss")).unwrap();
    let tree = roxmltree::Document::parse(&rss).unwrap();
    assert!(tree.descendants().any(|node| node.has_tag_name("guid")));
    assert!(!tree
        .descendants()
        .any(|node| node.has_tag_name("enclosure")));
    assert!(!output.join("narration.wav").exists());
}

fn wave() -> Vec<u8> {
    let data = 32_000_u32;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + data).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(16_000_u32.to_le_bytes());
    bytes.extend(32_000_u32.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data.to_le_bytes());
    bytes.resize(44 + data as usize, 0);
    bytes
}

#[test]
fn supplied_wave_and_chapter_positions_require_exact_script_binding() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut model) = fixture(root.path());
    let bytes = wave();
    fs::write(root.path().join("provided.wav"), &bytes).unwrap();
    model.narration = Some(Narration {
        path: "provided.wav".into(),
        sha256: hash(&bytes),
        script_sha256: briefing::plan(&input, token()).unwrap().script_sha256,
        origin: NarrationOrigin::SuppliedSynthetic,
        voice_reference: "synthetic silence, not speech".into(),
        consent_reference: "synthetic fixture; no person".into(),
        provider_reference: Some("authored-zero-PCM".into()),
        model_reference: Some("no model, synthetic test bytes".into()),
        rights: "Original test bytes, MIT".into(),
        review: None,
    });
    model.chapters[0].start_ms = Some(0);
    model.chapters[1].start_ms = Some(500);
    save(&input, &model);
    let changed = briefing::plan(&input, token()).unwrap();
    assert_eq!(changed.narration_script_current, Some(false));
    let output = root.path().join("bundle");
    assert!(briefing::build(&request(&input, &output, true), token()).is_err());
    model.narration.as_mut().unwrap().script_sha256 = changed.script_sha256;
    save(&input, &model);
    let result = briefing::build(&request(&input, &output, true), token()).unwrap();
    assert_eq!(result["plan"]["narration_duration_ms"], 1000);
    assert_eq!(fs::read(output.join("narration.wav")).unwrap(), bytes);
    model.chapters[1].start_ms = Some(1000);
    save(&input, &model);
    assert!(briefing::plan(&input, token()).is_err());
}

#[test]
fn invalid_line_ranges_paths_duplicate_fields_and_object_shapes_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut model) = fixture(root.path());
    model.chapters[0].sources[0].end_line = 99;
    save(&input, &model);
    assert!(briefing::plan(&input, token()).is_err());
    model.sources[0].path = "../outside.txt".into();
    assert!(briefing::validate(&model).is_err());
    let original = include_str!("../../../examples/briefing/briefing.json");
    assert!(briefing::parse(
        original
            .replacen("\"id\":", "\"id\":\"duplicate\",\"id\":", 1)
            .as_bytes()
    )
    .is_err());
    assert!(briefing::parse(b"[]").is_err());
}

#[test]
fn cancellation_and_existing_outputs_preserve_prior_files() {
    let root = tempfile::tempdir().unwrap();
    let (input, _) = fixture(root.path());
    let output = root.path().join("bundle");
    let cancelled = token();
    cancelled.cancel();
    assert!(briefing::build(&request(&input, &output, true), cancelled).is_err());
    assert!(!output.exists());
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep"), b"prior").unwrap();
    assert!(briefing::build(&request(&input, &output, true), token()).is_err());
    assert_eq!(fs::read(output.join("keep")).unwrap(), b"prior");
}

#[cfg(unix)]
#[test]
fn source_symlinks_are_not_captured() {
    let root = tempfile::tempdir().unwrap();
    let (input, _) = fixture(root.path());
    fs::rename(
        root.path().join("canonical.txt"),
        root.path().join("actual.txt"),
    )
    .unwrap();
    std::os::unix::fs::symlink("actual.txt", root.path().join("canonical.txt")).unwrap();
    assert!(briefing::plan(&input, token()).is_err());
}
