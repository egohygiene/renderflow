//! Retained-bundle cases authored for the deferred qualification pass.
use super::*;
use renderflow::dna::sonic::readback::{inspect, ReadbackRequest};
fn selected(root: &Path) -> (PathBuf, ReadbackRequest) {
    let (input, _) = fixture(root);
    let output = root.join("bundle");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    let pin = hash(&fs::read(output.join("sonic-run.json")).unwrap());
    (
        input.clone(),
        ReadbackRequest {
            schema_version: "renderflow.sonic-readback-request/v1".into(),
            directory: output,
            receipt_sha256: pin,
            input: Some(input),
        },
    )
}
#[test]
fn current_bundle_readback_checks_bytes_without_creating_a_store_or_approval() {
    let root = tempfile::tempdir().unwrap();
    let (_, r) = selected(root.path());
    let before = fs::read(r.directory.join("sonic-run.json")).unwrap();
    let observed = inspect(&r, token()).unwrap();
    assert_eq!(observed.source_state, "current");
    assert_eq!(observed.bundle_state, "complete");
    assert_eq!(observed.review_state, "candidate");
    assert!(!observed.runtime_qualified);
    assert_eq!(observed.publication_approval, "not_granted");
    assert!(!r.directory.join("artifacts").exists());
    assert_eq!(
        fs::read(r.directory.join("sonic-run.json")).unwrap(),
        before
    );
}
#[test]
fn changed_original_is_distinct_from_retained_candidate_integrity() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut r) = selected(root.path());
    fs::write(root.path().join("source.wav"), b"changed").unwrap();
    assert_eq!(
        inspect(&r, token()).unwrap().source_state,
        "unavailable_or_changed"
    );
    fs::write(&input, b"changed recipe").unwrap();
    assert_eq!(inspect(&r, token()).unwrap().source_state, "stale");
    fs::remove_file(input).unwrap();
    assert_eq!(inspect(&r, token()).unwrap().source_state, "unavailable");
    r.input = None;
    assert_eq!(inspect(&r, token()).unwrap().source_state, "not_supplied");
}
#[test]
fn external_receipt_pin_and_exact_cas_and_inventory_membership_are_required() {
    let root = tempfile::tempdir().unwrap();
    let (_, mut r) = selected(root.path());
    let pin = r.receipt_sha256.clone();
    r.receipt_sha256 = "0".repeat(64);
    assert!(inspect(&r, token()).is_err());
    r.receipt_sha256 = pin;
    fs::write(r.directory.join("extra-private.txt"), b"unexpected").unwrap();
    assert!(inspect(&r, token()).is_err());
    fs::remove_file(r.directory.join("extra-private.txt")).unwrap();
    let lineage = read(&r.directory.join("lineage.json"));
    let relative = lineage["produced"][0]["payload"]["relative_path"]
        .as_str()
        .unwrap();
    fs::write(r.directory.join(relative), b"changed CAS").unwrap();
    assert!(inspect(&r, token()).is_err());
}
#[test]
fn required_unavailable_semantics_stay_partial_without_provider_resolution() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    recipe.semantic.mode = SemanticMode::Required;
    save(&input, &recipe);
    let output = root.path().join("partial");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    let r = ReadbackRequest {
        schema_version: "renderflow.sonic-readback-request/v1".into(),
        directory: output.clone(),
        receipt_sha256: hash(&fs::read(output.join("sonic-run.json")).unwrap()),
        input: Some(input),
    };
    let result = inspect(&r, token()).unwrap();
    assert_eq!(result.bundle_state, "partial");
    assert_eq!(result.source_state, "current");
    assert_eq!(
        result.provider_state,
        "not_reobserved; retained declarations only"
    );
}
#[test]
fn loose_or_symbolic_partial_folders_cannot_be_promoted() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("sonic-dna.json"), b"{}").unwrap();
    let r = ReadbackRequest {
        schema_version: "renderflow.sonic-readback-request/v1".into(),
        directory: root.path().into(),
        receipt_sha256: "a".repeat(64),
        input: None,
    };
    assert!(inspect(&r, token()).is_err());
    assert!(!root.path().join("sonic-run.json").exists());
    assert_eq!(fs::read(root.path().join("sonic-dna.json")).unwrap(), b"{}");
}

#[test]
fn native_stereo_snapshot_and_explicit_capture_policy_survive_saved_readback() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    super::native_wav::native(root.path(), &input, &mut recipe);
    recipe.policy.max_source_bytes = 128 * 1024 * 1024;
    save(&input, &recipe);
    let original = fs::read(root.path().join("source.wav")).unwrap();
    let output = root.path().join("native-readback");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    let receipt = fs::read(output.join("sonic-run.json")).unwrap();
    let run: Value = serde_json::from_slice(&receipt).unwrap();
    assert_eq!(
        run["plan"]["technical"]["maximum_capture_bytes"],
        192 * 1024 * 1024
    );
    assert_eq!(run["plan"]["technical"]["sample_rate_hz"], 44_100);
    assert_eq!(run["plan"]["technical"]["channels"], 2);
    assert_eq!(run["plan"]["technical"]["bits_per_sample"], 24);
    assert_eq!(run["plan"]["technical"]["frame_count"], 4410);
    let selected = ReadbackRequest {
        schema_version: "renderflow.sonic-readback-request/v1".into(),
        directory: output.clone(),
        receipt_sha256: hash(&receipt),
        input: Some(input),
    };
    let observed = inspect(&selected, token()).unwrap();
    assert_eq!(observed.source_state, "current");
    assert_eq!(observed.review_state, "candidate");
    assert!(!observed.runtime_qualified);
    assert_eq!(observed.publication_approval, "not_granted");
    assert_eq!(fs::read(root.path().join("source.wav")).unwrap(), original);
    assert_eq!(fs::read(output.join("sonic-run.json")).unwrap(), receipt);
    assert!(!output.join("source.wav").exists());
}

#[test]
fn selected_midi_readback_keeps_uncertainty_and_detects_a_changed_report_selection() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    super::note_stem::midi(root.path(), &input, &mut recipe);
    let output = root.path().join("midi-readback");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    let receipt = fs::read(output.join("sonic-run.json")).unwrap();
    let dna_bytes = fs::read(output.join("sonic-dna.json")).unwrap();
    let dna: Value = serde_json::from_slice(&dna_bytes).unwrap();
    let note = dna["provider_extensions"]["renderflow.sonic"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["dimension"] == "sonic.score.inferred_note")
        .unwrap();
    assert_eq!(note["confidence"]["kind"], "unavailable");
    assert_eq!(note["value"]["activation_is_confidence"], false);
    let selected = ReadbackRequest {
        schema_version: "renderflow.sonic-readback-request/v1".into(),
        directory: output.clone(),
        receipt_sha256: hash(&receipt),
        input: Some(input.clone()),
    };
    assert_eq!(inspect(&selected, token()).unwrap().source_state, "current");
    super::note_stem::replace_report(root.path(), &input, &mut recipe, "midi", |value| {
        value["settings"]["private_marker"] = json!("Changed selected private settings");
    });
    let observed = inspect(&selected, token()).unwrap();
    assert_eq!(observed.source_state, "stale");
    assert_eq!(observed.bundle_state, "complete");
    assert_eq!(
        observed.provider_state,
        "not_reobserved; retained declarations only"
    );
    assert_eq!(fs::read(output.join("sonic-run.json")).unwrap(), receipt);
    assert_eq!(fs::read(output.join("sonic-dna.json")).unwrap(), dna_bytes);
}

#[test]
fn selected_stem_readback_keeps_distinct_clocks_without_sample_alignment_authority() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    super::note_stem::stem(root.path(), &input, &mut recipe);
    let output = root.path().join("stem-readback");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    let receipt = fs::read(output.join("sonic-run.json")).unwrap();
    let dna = read(&output.join("sonic-dna.json"));
    let fact = dna["provider_extensions"]["renderflow.sonic"]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["dimension"] == "sonic.arrangement.declared_stem_relationship")
        .unwrap();
    assert_eq!(fact["value"]["selected_source_sample_rate_hz"], 22_050);
    assert_eq!(fact["value"]["original_mix"]["sample_rate_hz"], 48_000);
    assert_eq!(
        fact["value"]["sample_alignment_to_original_mix"],
        "not_established"
    );
    assert_eq!(fact["value"]["original_mix_bytes_observed"], false);
    let selected = ReadbackRequest {
        schema_version: "renderflow.sonic-readback-request/v1".into(),
        directory: output.clone(),
        receipt_sha256: hash(&receipt),
        input: Some(input),
    };
    let observed = inspect(&selected, token()).unwrap();
    assert_eq!(observed.source_state, "current");
    assert_eq!(observed.review_state, "candidate");
    assert_eq!(observed.publication_approval, "not_granted");
    assert_eq!(fs::read(output.join("sonic-run.json")).unwrap(), receipt);
    assert!(!fs::read_to_string(output.join("sonic-dna.json"))
        .unwrap()
        .contains("Private stem authority payload"));
}
