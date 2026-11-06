//! Authored, unrun review/export cases. The candidate is synthetic fixture data;
//! its declared model/source identities are not observed runtime qualification.
use super::*;
use crate::ai::{AiLocality, runtime::AiIdentityEvidence};

struct Fixture {
    root: tempfile::TempDir,
    store: ArtifactStore,
    candidate: SummaryCandidate,
    bytes: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let text = "Panel 1: Pip carries a lantern to the pier.\nPanel 2: Vale studies a map.";
        std::fs::write(root.path().join("script.txt"), text).unwrap();
        let recipe = SummaryRecipe {
            schema_version: RECIPE_SCHEMA.into(), id: "synthetic.review".into(),
            sources: vec![SummarySource {
                id: "script".into(), kind: TextKind::AuthoredScript, path: "script.txt".into(),
                sha256: local::sha256(text.as_bytes()), ranges: vec![TextRange {start: 0, end: text.len()}],
                approved_for_ai: true, approval_reference: "synthetic-test-declaration".into(),
                privacy_approved_for_remote: false,
            }],
        };
        let path = root.path().join("recipe.json");
        std::fs::write(&path, serde_json::to_vec(&recipe).unwrap()).unwrap();
        let store = ArtifactStore::new(root.path().join("store")).unwrap();
        let prepared = capture(&path, &store, ProcessCancellationToken::new()).unwrap();
        let hash = local::sha256(b"synthetic-only; no model run");
        let mut candidate = SummaryCandidate {
            schema_version: CANDIDATE_SCHEMA.into(), candidate_id: String::new(),
            recipe_sha256: prepared.recipe_sha256,
            body: SummaryBody {
                synopsis: vec![Statement {text: "Pip brings a lantern to the pier.".into(), evidence_ids: vec![prepared.slices[0].evidence_id.clone()]}],
                uncertainty: vec!["The script does not explain why.".into()], omissions: vec![], review_required: true,
            },
            sources: prepared.sources, slices: prepared.slices,
            processing: ProcessingIdentity {
                profile_sha256: hash.clone(), skill_id: SKILL_ID.into(), skill_version: SKILL_VERSION.into(),
                skill_sha256: hash.clone(), input_sha256: hash.clone(), output_schema_sha256: hash.clone(),
                settings_sha256: hash.clone(), hygiene_sha256: hash.clone(), cache_identity: hash.clone(),
                model: AiIdentityEvidence {
                    provider_id: "synthetic.fixture".into(), adapter: "synthetic".into(), runtime_id: "synthetic.runtime".into(),
                    runtime_revision: None, runtime_digest: None, model_id: "synthetic.model".into(),
                    model_revision: None, weights_digest: None, locality: AiLocality::Local,
                    endpoint_identity: hash, reported_model: None,
                },
            },
            state: AiCandidateState::Candidate, review_required: true,
            grounding: "references_checked_semantics_unverified".into(), coverage_note: COVERAGE.into(),
            reference_validator: "validator.summary.references/v1".into(),
        };
        candidate.candidate_id = candidate_id(&candidate).unwrap();
        validate_candidate(&candidate).unwrap();
        let bytes = serde_json::to_vec_pretty(&candidate).unwrap();
        Self {root, store, candidate, bytes}
    }

    fn decision(&self, action: ReviewAction) -> ReviewDecision {
        ReviewDecision {
            schema_version: DECISION_SCHEMA.into(), candidate_id: self.candidate.candidate_id.clone(),
            candidate_sha256: local::sha256(&self.bytes), action,
            reviewer_reference: "synthetic-test-review; not a human approval".into(),
            semantic_review_completed: action != ReviewAction::Reject, revised_body: None,
        }
    }

    fn apply(&self, action: ReviewAction) -> ReviewedArtifact {
        apply(&self.bytes, &self.decision(action), &self.store, ProcessCancellationToken::new()).unwrap()
    }
}

#[test]
fn acceptance_preserves_exact_candidate_and_creates_separate_review() {
    let fixture = Fixture::new(); let reviewed = fixture.apply(ReviewAction::Accept);
    let original = observed_artifact(&fixture.store, &reviewed.record.candidate_artifact, 131072, ProcessCancellationToken::new()).unwrap();
    assert_eq!(original, fixture.bytes);
    assert_eq!(parse_candidate(&original).unwrap().state, AiCandidateState::Candidate);
    assert_ne!(reviewed.artifact.id(), reviewed.record.candidate_artifact.id());
    assert_eq!(reviewed.record.output_state, Some(AiCandidateState::Approved));
    assert!(!reviewed.record.publication_approved);
    assert!(reviewed.record.authority_kind.contains("not authenticated"));
}

#[test]
fn same_semantics_with_different_bytes_require_a_new_exact_subject_decision() {
    let fixture = Fixture::new(); let mut bytes = fixture.bytes.clone(); bytes.push(b'\n');
    assert!(apply(&bytes, &fixture.decision(ReviewAction::Accept), &fixture.store, ProcessCancellationToken::new()).is_err());
    let mut decision = fixture.decision(ReviewAction::Accept); decision.candidate_id.push('x');
    assert!(apply(&fixture.bytes, &decision, &fixture.store, ProcessCancellationToken::new()).is_err());
}

#[test]
fn revision_requires_semantic_review_and_references_to_original_evidence() {
    let fixture = Fixture::new(); let mut decision = fixture.decision(ReviewAction::Revise);
    decision.revised_body = Some(fixture.candidate.body.clone());
    decision.revised_body.as_mut().unwrap().synopsis[0].text = "A lantern reaches the pier with Pip.".into();
    decision.semantic_review_completed = false;
    assert!(apply(&fixture.bytes, &decision, &fixture.store, ProcessCancellationToken::new()).is_err());
    decision.semantic_review_completed = true;
    let reviewed = apply(&fixture.bytes, &decision, &fixture.store, ProcessCancellationToken::new()).unwrap();
    assert_eq!(reviewed.record.accepted_body, decision.revised_body);
    decision.revised_body.as_mut().unwrap().synopsis[0].evidence_ids = vec!["invented".into()];
    assert!(apply(&fixture.bytes, &decision, &fixture.store, ProcessCancellationToken::new()).is_err());
}

#[test]
fn rejection_is_recorded_but_cannot_export() {
    let fixture = Fixture::new(); let reviewed = fixture.apply(ReviewAction::Reject);
    assert!(reviewed.record.accepted_body.is_none() && reviewed.record.output_state.is_none());
    let bytes = serde_json::to_vec_pretty(&reviewed.record).unwrap(); let output = fixture.root.path().join("rejected");
    assert!(export(&bytes, &local::sha256(&bytes), &fixture.store, &output, ProcessCancellationToken::new()).is_err());
    assert!(!output.exists());
}

#[test]
fn altered_review_or_expected_hash_is_refused_before_export() {
    let fixture = Fixture::new(); let reviewed = fixture.apply(ReviewAction::Accept);
    let mut record = reviewed.record.clone(); record.accepted_body.as_mut().unwrap().synopsis[0].text = "An invented ending.".into();
    let bytes = serde_json::to_vec(&record).unwrap(); let output = fixture.root.path().join("tampered");
    assert!(export(&bytes, &local::sha256(&bytes), &fixture.store, &output, ProcessCancellationToken::new()).is_err());
    let bytes = serde_json::to_vec(&reviewed.record).unwrap();
    assert!(export(&bytes, &"0".repeat(64), &fixture.store, &output, ProcessCancellationToken::new()).is_err());
    assert!(!output.exists());
    let mut artifact = serde_json::to_value(&reviewed.record.candidate_artifact).unwrap();
    artifact["id"] = json!("artifact:sha256:forged");
    let mut record = reviewed.record.clone(); record.candidate_artifact = serde_json::from_value(artifact).unwrap();
    record.review_id = id(&record).unwrap();
    assert!(validate_record(&record, &fixture.store, ProcessCancellationToken::new()).is_err());
}

#[test]
fn self_approved_candidate_and_unknown_decision_fields_are_refused() {
    let fixture = Fixture::new(); let mut candidate = serde_json::to_value(&fixture.candidate).unwrap();
    candidate["state"] = json!("approved");
    assert!(parse_candidate(&serde_json::to_vec(&candidate).unwrap()).is_err());
    let mut decision = serde_json::to_value(fixture.decision(ReviewAction::Accept)).unwrap();
    decision["publication_approved"] = json!(true);
    assert!(crate::evidence::json::object::<ReviewDecision>(&serde_json::to_vec(&decision).unwrap(), 65536).is_err());
    let mut decision = fixture.decision(ReviewAction::Accept); decision.semantic_review_completed = false;
    assert!(apply(&fixture.bytes, &decision, &fixture.store, ProcessCancellationToken::new()).is_err());
}

#[test]
fn export_preserves_references_coverage_and_observed_completion_receipt() {
    let fixture = Fixture::new(); let reviewed = fixture.apply(ReviewAction::Accept);
    let bytes = serde_json::to_vec_pretty(&reviewed.record).unwrap(); let output = fixture.root.path().join("export");
    let receipt = export(&bytes, &local::sha256(&bytes), &fixture.store, &output, ProcessCancellationToken::new()).unwrap();
    assert_eq!(receipt["publication_approved"], false); assert_eq!(receipt["files"].as_array().unwrap().len(), 3);
    assert_eq!(std::fs::read_to_string(output.join("summary.txt")).unwrap(), "Pip brings a lantern to the pier.\n");
    let summary: Value = serde_json::from_slice(&std::fs::read(output.join("summary.json")).unwrap()).unwrap();
    assert_eq!(summary["references"][0]["evidence_id"], fixture.candidate.slices[0].evidence_id);
    assert!(summary["references"][0].get("text").is_none()); assert_eq!(summary["coverage_note"], COVERAGE);
    assert_eq!(summary["source_coverage"][0]["unselected_bytes"], 0); assert!(output.join("export-receipt.json").is_file());
}

#[test]
fn existing_output_and_pre_cancelled_export_do_not_replace_authored_files() {
    let fixture = Fixture::new(); let reviewed = fixture.apply(ReviewAction::Accept);
    let bytes = serde_json::to_vec(&reviewed.record).unwrap(); let output = fixture.root.path().join("existing");
    std::fs::create_dir(&output).unwrap(); std::fs::write(output.join("summary.txt"), "Authored original").unwrap();
    assert!(export(&bytes, &local::sha256(&bytes), &fixture.store, &output, ProcessCancellationToken::new()).is_err());
    assert_eq!(std::fs::read_to_string(output.join("summary.txt")).unwrap(), "Authored original");
    let token = ProcessCancellationToken::new(); token.cancel(); let cancelled = fixture.root.path().join("cancelled");
    assert!(export(&bytes, &local::sha256(&bytes), &fixture.store, &cancelled, token).is_err()); assert!(!cancelled.exists());
}

#[test]
fn changed_candidate_store_bytes_and_mismatched_review_bundle_are_refused() {
    let fixture = Fixture::new(); let reviewed = fixture.apply(ReviewAction::Accept);
    let mut mismatch = ReviewedArtifact {record: reviewed.record.clone(), artifact: reviewed.artifact.clone()};
    mismatch.record.decision.reviewer_reference = "different synthetic reference".into(); mismatch.record.review_id = id(&mismatch.record).unwrap();
    let output = fixture.root.path().join("mismatch");
    assert!(publish_review(&mismatch, &fixture.store, &output, ProcessCancellationToken::new()).is_err()); assert!(!output.exists());
    let path = fixture.store.payload_path(&reviewed.record.candidate_artifact).unwrap();
    std::fs::write(path, b"changed").unwrap();
    assert!(validate_record(&reviewed.record, &fixture.store, ProcessCancellationToken::new()).is_err());
}

#[cfg(unix)]
#[test]
fn serialized_artifact_locator_cannot_follow_a_symlink_outside_the_store() {
    let fixture = Fixture::new(); let reviewed = fixture.apply(ReviewAction::Accept);
    let path = fixture.store.payload_path(&reviewed.record.candidate_artifact).unwrap();
    let outside = fixture.root.path().join("outside.json"); std::fs::write(&outside, &fixture.bytes).unwrap();
    std::fs::remove_file(&path).unwrap(); std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(validate_record(&reviewed.record, &fixture.store, ProcessCancellationToken::new()).is_err());
}
