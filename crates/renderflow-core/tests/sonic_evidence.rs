//! Authored/unrun #397 conformance cases. Silence, analyzer assertions and AI
//! responses are synthetic fixtures; no model/analyzer was actually qualified.

use anyhow::Result;
use renderflow::ai::{
    AiAvailability, AiCapabilities, AiLocality, AiModel, AiModelCatalog, AiProvider, AiRequest,
    AiResponse, AiSkillRegistry, AiSkillRuntime,
};
use renderflow::dna::sonic::{
    self, Companion, CompanionKind, Correction, FileInput, SemanticMode, SonicRecipe, SonicRequest,
};
use renderflow::dna::{ArtifactDna, DnaProtectedReferenceRule, DnaReviewState};
use renderflow::process::ProcessCancellationToken;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn token() -> ProcessCancellationToken {
    ProcessCancellationToken::new()
}
fn wave() -> Vec<u8> {
    let size = 3200_u32;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(16000_u32.to_le_bytes());
    bytes.extend(32000_u32.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    bytes.resize(44 + size as usize, 0);
    bytes
}
fn fixture(root: &Path) -> (PathBuf, SonicRecipe) {
    fs::write(root.join("source.wav"), wave()).unwrap();
    fs::write(
        root.join("analysis.json"),
        include_bytes!("../../../examples/sonic-dna/analysis.json"),
    )
    .unwrap();
    fs::write(
        root.join("fixture-evidence.txt"),
        include_bytes!("../../../examples/sonic-dna/fixture-evidence.txt"),
    )
    .unwrap();
    let bytes = include_bytes!("../../../examples/sonic-dna/sonic.json");
    let recipe = sonic::parse(bytes).unwrap();
    let input = root.join("sonic.json");
    fs::write(&input, bytes).unwrap();
    (input, recipe)
}
fn save(path: &Path, recipe: &SonicRecipe) {
    fs::write(path, serde_json::to_vec_pretty(recipe).unwrap()).unwrap();
}
fn request(input: &Path, output: &Path) -> SonicRequest {
    SonicRequest {
        input: input.into(),
        output_dir: output.into(),
    }
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn mutate_analysis(
    root: &Path,
    input: &Path,
    recipe: &mut SonicRecipe,
    change: impl FnOnce(&mut Value),
) {
    let path = root.join("analysis.json");
    let mut value = read(&path);
    change(&mut value);
    let bytes = serde_json::to_vec_pretty(&value).unwrap();
    fs::write(path, &bytes).unwrap();
    recipe.analyses[0].file.sha256 = hash(&bytes);
    save(input, recipe);
}

#[path = "sonic_evidence/context.rs"]
mod context;
#[path = "sonic_evidence/native_wav.rs"]
mod native_wav;
#[path = "sonic_evidence/note_stem.rs"]
mod note_stem;

#[test]
fn plan_observes_bindings_without_creating_artifacts_or_model_results() {
    let root = tempfile::tempdir().unwrap();
    let (input, _) = fixture(root.path());
    let before = fs::read(&input).unwrap();
    let plan = sonic::plan(&input, None, token()).unwrap();
    assert_eq!(plan.technical["duration_ms"], 100);
    assert_eq!(plan.inputs.len(), 3);
    assert_eq!(plan.semantic.status, "disabled");
    assert!(!plan.runtime_qualified);
    assert_eq!(plan.direct_audio_inference, "unsupported_by_this_profile");
    assert!(!root.path().join("sonic-dna.json").exists());
    assert_eq!(fs::read(input).unwrap(), before);
}

#[test]
fn baseline_package_keeps_native_confidence_tags_and_source_bytes() {
    let root = tempfile::tempdir().unwrap();
    let (input, recipe) = fixture(root.path());
    let output = root.path().join("bundle");
    let result = sonic::build(&request(&input, &output), None, token()).unwrap();
    assert_eq!(result["state"], "complete");
    assert_eq!(result["semantic"]["status"], "disabled");
    let dna = ArtifactDna::load(output.join("sonic-dna.json")).unwrap();
    assert_eq!(dna.approval.state, DnaReviewState::Candidate);
    assert!(dna.approval.human_review_required);
    let rows = dna.provider_extensions["renderflow.sonic"]["facts"]
        .as_array()
        .unwrap();
    assert_eq!(rows[0]["confidence"]["kind"], "uncalibrated");
    assert_eq!(rows[0]["confidence"]["score"], 0.75);
    assert_eq!(rows[1]["confidence"]["kind"], "unavailable");
    assert!(rows[1]["confidence"].get("score").is_none());
    assert_eq!(
        hash(&fs::read(root.path().join("source.wav")).unwrap()),
        recipe.audio.sha256
    );
    assert_eq!(result["source_inputs_included"], false);
    assert!(!output.join("source.wav").exists());
}

#[test]
fn native_source_clock_reference_and_evidence_mismatches_are_refused() {
    for changed in ["source", "clock", "reference", "evidence"] {
        let root = tempfile::tempdir().unwrap();
        let (input, mut recipe) = fixture(root.path());
        mutate_analysis(root.path(), &input, &mut recipe, |value| match changed {
            "source" => value["source"]["artifact"]["sha256"] = json!("0".repeat(64)),
            "clock" => value["source"]["frame_count"] = json!(1601),
            "reference" => {
                value["observations"][0]["provenance"]["provider_evidence_id"] = json!("absent")
            }
            _ => value["artifacts"][0]["sha256"] = json!("0".repeat(64)),
        });
        assert!(
            sonic::plan(&input, None, token()).is_err(),
            "accepted {changed}"
        );
    }
}

#[test]
fn deterministic_only_recipe_does_not_require_analysis_or_ai() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    recipe.analyses.clear();
    save(&input, &recipe);
    let output = root.path().join("baseline");
    let result = sonic::build(&request(&input, &output), None, token()).unwrap();
    assert_eq!(result["state"], "complete");
    let description = read(&output.join("audio-description.json"));
    assert_eq!(description["mode"], "deterministic_evidence_render");
    assert!(
        description["candidate"]["summary"]["evidence_ids"]
            .as_array()
            .unwrap()
            .len()
            > 0
    );
}

#[test]
fn optional_missing_runtime_and_required_missing_runtime_keep_distinct_states() {
    for (mode, state) in [
        (SemanticMode::Optional, "complete"),
        (SemanticMode::Required, "partial"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (input, mut recipe) = fixture(root.path());
        recipe.semantic.mode = mode;
        recipe.semantic.source_approved_for_ai = true;
        recipe.policy.allow_ai = true;
        save(&input, &recipe);
        let output = root.path().join("bundle");
        let result = sonic::build(&request(&input, &output), None, token()).unwrap();
        assert_eq!(result["state"], state);
        assert_eq!(result["semantic"]["status"], "not_configured");
        assert!(output.join("sonic-dna.json").is_file() && output.join("sonic-run.json").is_file());
    }
}

#[test]
fn invalid_confidence_never_becomes_certainty_or_a_fabricated_zero() {
    for confidence in [
        json!({"kind":"uncalibrated","score":2.0}),
        json!({"kind":"not_applicable"}),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (input, mut recipe) = fixture(root.path());
        mutate_analysis(root.path(), &input, &mut recipe, |value| {
            value["observations"][0]["provenance"]["confidence"] = confidence
        });
        assert!(sonic::plan(&input, None, token()).is_err());
    }
}

#[test]
fn declared_human_correction_preserves_original_estimate_and_review_state() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    let plan = sonic::plan(&input, None, token()).unwrap();
    let id = plan
        .fact_ids
        .iter()
        .find(|id| id.starts_with("analysis."))
        .unwrap()
        .clone();
    recipe.corrections.push(Correction {
        fact_id: id.clone(),
        value: json!(122.0),
        review_reference: "synthetic correction declaration, not human approval".into(),
    });
    save(&input, &recipe);
    let output = root.path().join("bundle");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    let dna = read(&output.join("sonic-dna.json"));
    let facts = dna["provider_extensions"]["renderflow.sonic"]["facts"]
        .as_array()
        .unwrap();
    assert_eq!(
        facts.iter().find(|fact| fact["id"] == id).unwrap()["value"],
        120.0
    );
    let corrected = facts
        .iter()
        .find(|fact| fact["correction_of"] == id)
        .unwrap();
    assert_eq!(corrected["value"], 122.0);
    assert_eq!(corrected["origin"], "human_provided");
    assert_eq!(corrected["confidence"]["kind"], "unavailable");
    assert_eq!(dna["approval"]["state"], "candidate");
}

#[test]
fn companion_authorities_stay_distinct_and_payloads_stay_out_of_model_input() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    for (index, kind) in [
        CompanionKind::ReviewedLyrics,
        CompanionKind::TranscriptCandidate,
        CompanionKind::ArtistMidi,
        CompanionKind::InferredMidi,
    ]
    .into_iter()
    .enumerate()
    {
        let bytes =
            format!("Synthetic reference bytes {index}; authority is a fixture declaration only.");
        let path = format!("companion-{index}.txt");
        fs::write(root.path().join(&path), bytes.as_bytes()).unwrap();
        recipe.companions.push(Companion {
            file: FileInput {
                id: format!("companion-{index}"),
                path,
                sha256: hash(bytes.as_bytes()),
            },
            source_sha256: recipe.audio.sha256.clone(),
            kind,
            authority_reference:
                "synthetic declared authority; not authenticated review or authorship".into(),
            timed_text_context: None,
        });
    }
    save(&input, &recipe);
    let output = root.path().join("bundle");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    let dna = read(&output.join("sonic-dna.json"));
    let companions = dna["provider_extensions"]["renderflow.sonic"]["companions"]
        .as_array()
        .unwrap();
    assert_eq!(companions.len(), 4);
    assert_eq!(companions[0]["kind"], "reviewed_lyrics");
    assert_eq!(companions[1]["kind"], "transcript_candidate");
    assert_eq!(companions[2]["kind"], "artist_midi");
    assert_eq!(companions[3]["kind"], "inferred_midi");
    assert!(companions
        .iter()
        .all(|row| row["payload_exposed_to_model"] == false
            && row["validation"] == "byte_binding_only"));
}

#[test]
fn protected_reference_hygiene_applies_before_descriptions_and_guidance() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    mutate_analysis(root.path(), &input, &mut recipe, |value| {
        value["observations"][1]["value"]["value"] = json!("Example Artist")
    });
    recipe
        .policy
        .protected_references
        .push(DnaProtectedReferenceRule {
            term: "Example Artist".into(),
            descriptive_replacement: Some("a declared tonal characteristic".into()),
        });
    save(&input, &recipe);
    let output = root.path().join("bundle");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    for path in [
        "sonic-dna.json",
        "audio-description.json",
        "audio-description.md",
        "prompt-guidance.json",
    ] {
        assert!(!fs::read_to_string(output.join(path))
            .unwrap()
            .contains("Example Artist"));
    }
    assert_eq!(
        read(&output.join("prompt-guidance.json"))["direct_imitation_allowed"],
        false
    );
}

#[derive(Clone, Copy)]
enum ResponseKind {
    Good,
    UnknownCitation,
    Failure,
    DuplicateKey,
}
struct FixtureProvider {
    calls: AtomicUsize,
    kind: ResponseKind,
}
impl AiProvider for FixtureProvider {
    fn name(&self) -> &str {
        "sonic-fixture"
    }
    fn is_local(&self) -> bool {
        true
    }
    fn capabilities(&self) -> AiCapabilities {
        AiCapabilities::new()
    }
    fn models(&self) -> Vec<AiModel> {
        vec![AiModel::new("fixture-model", true)]
    }
    fn execute(&self, request: &AiRequest) -> Result<AiResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(request.output_schema.is_some());
        assert_eq!(request.params.seed, Some(42));
        assert_eq!(request.timeout_ms, Some(60000));
        if matches!(self.kind, ResponseKind::Failure) {
            anyhow::bail!(
                "synthetic provider failure; private raw details must not enter the receipt"
            );
        }
        if matches!(self.kind, ResponseKind::DuplicateKey) {
            return Ok(AiResponse::new(
                "{\"review_required\":false,\"review_required\":true}",
                "fixture-model",
                "sonic-fixture",
            ));
        }
        let id = if matches!(self.kind, ResponseKind::UnknownCitation) {
            "absent-fact"
        } else {
            "pcm.duration"
        };
        let output = json!({"summary":{"text":"This supplied PCM snapshot is 100 milliseconds long.","evidence_ids":[id]},
            "sections":[{"topic":"technical","text":"This is a structured evidence interpretation; no audio was heard.","evidence_ids":[id]}],
            "uncertainty":["Synthetic response only; no model or analyzer qualification."],"review_required":true});
        Ok(AiResponse::new(
            output.to_string(),
            "fixture-model",
            "sonic-fixture",
        ))
    }
}
fn catalog(availability: AiAvailability) -> AiModelCatalog {
    let mut catalog = AiModelCatalog::bundled().unwrap();
    catalog.providers.truncate(1);
    let provider = &mut catalog.providers[0];
    provider.id = "provider.sonic-fixture".into();
    provider.adapter = "sonic-fixture".into();
    provider.locality = AiLocality::Local;
    provider.runtime.id = "runtime.sonic-fixture".into();
    provider.models.truncate(1);
    provider.models[0].id = "fixture-model".into();
    provider.models[0].availability = availability;
    catalog
}
fn enable_ai(input: &Path, recipe: &mut SonicRecipe) {
    recipe.policy.allow_ai = true;
    recipe.semantic.mode = SemanticMode::Optional;
    recipe.semantic.source_approved_for_ai = true;
    save(input, recipe);
}

#[test]
fn shared_runtime_mock_creates_cited_review_required_structured_candidate() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    enable_ai(&input, &mut recipe);
    let provider = FixtureProvider {
        calls: AtomicUsize::new(0),
        kind: ResponseKind::Good,
    };
    let catalog = catalog(AiAvailability::Available);
    let skills = AiSkillRegistry::bundled().unwrap();
    let runtime = AiSkillRuntime::new(&catalog, &skills, vec![&provider]);
    let output = root.path().join("bundle");
    let result = sonic::build(&request(&input, &output), Some(&runtime), token()).unwrap();
    assert_eq!(result["semantic"]["status"], "produced");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        read(&output.join("audio-description.json"))["mode"],
        "model_interpretation_of_structured_evidence"
    );
    assert_eq!(
        result["semantic"]["execution"]["raw_prompts_retained"],
        false
    );
    assert_eq!(result["publication_approval"], "not_granted");
}

#[test]
fn unknown_model_citation_is_invalid_and_retains_baseline_instead_of_approval() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    enable_ai(&input, &mut recipe);
    let provider = FixtureProvider {
        calls: AtomicUsize::new(0),
        kind: ResponseKind::UnknownCitation,
    };
    let catalog = catalog(AiAvailability::Available);
    let skills = AiSkillRegistry::bundled().unwrap();
    let runtime = AiSkillRuntime::new(&catalog, &skills, vec![&provider]);
    let output = root.path().join("bundle");
    let result = sonic::build(&request(&input, &output), Some(&runtime), token()).unwrap();
    assert_eq!(result["semantic"]["status"], "invalid");
    assert_eq!(
        read(&output.join("audio-description.json"))["mode"],
        "deterministic_evidence_render"
    );
    assert_eq!(result["model_provider_candidate_payload_included"], false);
}

#[test]
fn unavailable_catalog_or_missing_source_permission_never_invokes_provider() {
    for available in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let (input, mut recipe) = fixture(root.path());
        enable_ai(&input, &mut recipe);
        if available {
            recipe.semantic.source_approved_for_ai = false;
            save(&input, &recipe);
        }
        let provider = FixtureProvider {
            calls: AtomicUsize::new(0),
            kind: ResponseKind::Good,
        };
        let catalog = catalog(if available {
            AiAvailability::Available
        } else {
            AiAvailability::Unverified
        });
        let skills = AiSkillRegistry::bundled().unwrap();
        let runtime = AiSkillRuntime::new(&catalog, &skills, vec![&provider]);
        let output = root.path().join("bundle");
        let result = sonic::build(&request(&input, &output), Some(&runtime), token()).unwrap();
        assert_ne!(result["semantic"]["status"], "produced");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn provider_failure_is_typed_without_raw_details_or_remote_fallback() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    enable_ai(&input, &mut recipe);
    let provider = FixtureProvider {
        calls: AtomicUsize::new(0),
        kind: ResponseKind::Failure,
    };
    let catalog = catalog(AiAvailability::Available);
    let skills = AiSkillRegistry::bundled().unwrap();
    let runtime = AiSkillRuntime::new(&catalog, &skills, vec![&provider]);
    let result = sonic::build(
        &request(&input, &root.path().join("bundle")),
        Some(&runtime),
        token(),
    )
    .unwrap();
    assert_eq!(result["semantic"]["status"], "failed");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert!(!result.to_string().contains("private raw details"));
}

#[test]
fn duplicate_nested_json_and_traversing_input_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    let path = root.path().join("analysis.json");
    let original = fs::read_to_string(&path).unwrap();
    let duplicate = original.replacen(
        "\"kind\": \"quantity\"",
        "\"kind\": \"label\", \"kind\": \"quantity\"",
        1,
    );
    fs::write(&path, &duplicate).unwrap();
    recipe.analyses[0].file.sha256 = hash(duplicate.as_bytes());
    save(&input, &recipe);
    assert!(sonic::plan(&input, None, token()).is_err());
    recipe.audio.path = "../outside.wav".into();
    save(&input, &recipe);
    assert!(sonic::plan(&input, None, token()).is_err());
}

#[test]
fn ambiguous_model_json_is_refused_by_the_shared_strict_decoder() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    enable_ai(&input, &mut recipe);
    let provider = FixtureProvider {
        calls: AtomicUsize::new(0),
        kind: ResponseKind::DuplicateKey,
    };
    let catalog = catalog(AiAvailability::Available);
    let skills = AiSkillRegistry::bundled().unwrap();
    let runtime = AiSkillRuntime::new(&catalog, &skills, vec![&provider]);
    let result = sonic::build(
        &request(&input, &root.path().join("bundle")),
        Some(&runtime),
        token(),
    )
    .unwrap();
    assert_eq!(result["semantic"]["status"], "failed");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn cancellation_and_existing_destination_leave_sources_and_prior_files_intact() {
    let root = tempfile::tempdir().unwrap();
    let (input, _) = fixture(root.path());
    let output = root.path().join("bundle");
    let cancelled = token();
    cancelled.cancel();
    assert!(sonic::build(&request(&input, &output), None, cancelled).is_err());
    assert!(!output.exists());
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep"), b"prior").unwrap();
    assert!(sonic::build(&request(&input, &output), None, token()).is_err());
    assert_eq!(fs::read(output.join("keep")).unwrap(), b"prior");
    assert_eq!(fs::read(root.path().join("source.wav")).unwrap(), wave());
}

#[path = "sonic_evidence/readback.rs"]
mod readback;
