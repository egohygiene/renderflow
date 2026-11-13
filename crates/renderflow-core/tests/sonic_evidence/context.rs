//! Authored/unrun selected temporal/companion projection checks, not media proof.
use super::*;
use renderflow::dna::sonic::{CompanionTextFormat, TimedTextContext};
fn timeline(value: &mut Value) {
    let provenance = value["observations"][0]["provenance"].clone();
    value["timelines"] = json!([{"id":"structure","capability_id":"aniflow/audio-musical-structure","kind":"disjoint_regions",
        "scope":{"channels":[0],"stem_id":null},"events":[{"id":"region","range":{"start":0,"end":800},"label":"synthetic reported section","provenance":provenance}]}]);
}
fn companion(root: &Path, recipe: &mut SonicRecipe, context: bool) {
    let bytes=b"1\n00:00:00,000 --> 00:00:00,050\nPrivate fixture words\n\n2\n00:00:00,040 --> 00:00:00,100\nRetain authored punctuation!\n";
    fs::write(root.join("words.srt"), bytes).unwrap();
    recipe.companions.push(Companion {
        file: FileInput {
            id: "lyrics".into(),
            path: "words.srt".into(),
            sha256: hash(bytes),
        },
        source_sha256: recipe.audio.sha256.clone(),
        kind: CompanionKind::ReviewedLyrics,
        authority_reference: "synthetic supplied review declaration".into(),
        timed_text_context: context.then_some(TimedTextContext {
            format: CompanionTextFormat::Srt,
            maximum_cues: 8,
        }),
    });
}
#[test]
fn musical_events_keep_exact_sample_scope_original_uncertainty_and_evidence() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    mutate_analysis(root.path(), &input, &mut recipe, timeline);
    let output = root.path().join("bundle");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    let dna = read(&output.join("sonic-dna.json"));
    let rows = dna["provider_extensions"]["renderflow.sonic"]["facts"]
        .as_array()
        .unwrap();
    let event = rows
        .iter()
        .find(|f| f["dimension"] == "sonic.arrangement.reported_event")
        .unwrap();
    assert_eq!(
        event["value"]["source_frame_range"],
        json!({"start":0,"end":800})
    );
    assert_eq!(event["confidence"]["kind"], "uncalibrated");
    assert_eq!(event["evidence"]["pointer"], "/timelines/0/events/0");
    assert_eq!(
        dna["provider_extensions"]["renderflow.sonic"]["analyses"][0]["normalized_timeline_facts"],
        1
    );
}
#[test]
fn invalid_event_clock_marker_membership_and_repeated_ids_refuse() {
    for mutation in 0..5 {
        let root = tempfile::tempdir().unwrap();
        let (input, mut recipe) = fixture(root.path());
        mutate_analysis(root.path(), &input, &mut recipe, |v| {
            timeline(v);
            match mutation {
                0 => v["timelines"][0]["events"][0]["range"]["end"] = json!(1601),
                1 => v["timelines"][0]["kind"] = json!("markers"),
                2 => {
                    v["timelines"][0]["events"][0]["provenance"]["provider_evidence_id"] =
                        json!("unknown")
                }
                3 => v["timelines"][0]["events"][0]["id"] = json!("tempo_candidate"),
                _ => {
                    let mut event = v["timelines"][0]["events"][0].clone();
                    event["id"] = json!("overlap");
                    event["range"] = json!({"start":1,"end":900});
                    v["timelines"][0]["events"]
                        .as_array_mut()
                        .unwrap()
                        .push(event);
                }
            }
        });
        assert!(sonic::plan(&input, None, token()).is_err());
    }
}
#[test]
fn unknown_timeline_families_stay_unprojected_without_invented_meaning() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    mutate_analysis(root.path(), &input, &mut recipe, |v| {
        timeline(v);
        v["timelines"][0]["capability_id"] = json!("aniflow/audio-midi-extraction");
    });
    let plan = sonic::plan(&input, None, token()).unwrap();
    assert_eq!(plan.analyses[0]["unprojected"]["timelines"], 1);
    assert_eq!(plan.analyses[0]["normalized_timeline_facts"], 0);
}
#[test]
fn explicitly_selected_lyrics_structure_does_not_expose_words_or_authenticate_review() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    companion(root.path(), &mut recipe, true);
    save(&input, &recipe);
    let plan = sonic::plan(&input, None, token()).unwrap();
    assert_eq!(
        plan.declared_companions[0]["structural_context"]["cue_count"],
        2
    );
    assert_eq!(
        plan.declared_companions[0]["structural_context"]["overlap_present"],
        true
    );
    let output = root.path().join("bundle");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    for file in [
        "sonic-dna.json",
        "audio-description.json",
        "prompt-guidance.json",
    ] {
        let text = fs::read_to_string(output.join(file)).unwrap();
        assert!(!text.contains("Private fixture words"));
        assert!(!text.contains("Retain authored"));
    }
    let dna = read(&output.join("sonic-dna.json"));
    let rows = dna["provider_extensions"]["renderflow.sonic"]["facts"]
        .as_array()
        .unwrap();
    let fact = rows
        .iter()
        .find(|f| f["dimension"] == "sonic.lyrics.declared_timed_text_structure")
        .unwrap();
    assert_eq!(fact["value"]["source_frame_range"]["end"], 1600);
    assert_eq!(fact["evidence"]["review_authentication"], "not_established");
    assert_eq!(fact["confidence"]["kind"], "not_applicable");
}
#[test]
fn companion_context_requires_explicit_selection_correct_role_and_source_clock() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    companion(root.path(), &mut recipe, false);
    save(&input, &recipe);
    assert_eq!(
        sonic::plan(&input, None, token())
            .unwrap()
            .declared_companions[0]["validation"],
        "byte_binding_only"
    );
    recipe.companions[0].timed_text_context = Some(TimedTextContext {
        format: CompanionTextFormat::Srt,
        maximum_cues: 1,
    });
    save(&input, &recipe);
    assert!(sonic::plan(&input, None, token()).is_err());
    recipe.companions[0]
        .timed_text_context
        .as_mut()
        .unwrap()
        .maximum_cues = 8;
    recipe.companions[0].kind = CompanionKind::InferredMidi;
    save(&input, &recipe);
    assert!(sonic::plan(&input, None, token()).is_err());
    recipe.companions[0].kind = CompanionKind::TranscriptCandidate;
    let bytes = b"1\n00:00:00,000 --> 00:00:00,101\nCandidate\n";
    fs::write(root.path().join("words.srt"), bytes).unwrap();
    recipe.companions[0].file.sha256 = hash(bytes);
    save(&input, &recipe);
    assert!(sonic::plan(&input, None, token()).is_err());
}
#[test]
fn timeline_label_hygiene_precedes_description_and_prompt_use() {
    let root = tempfile::tempdir().unwrap();
    let (input, mut recipe) = fixture(root.path());
    mutate_analysis(root.path(), &input, &mut recipe, |v| {
        timeline(v);
        v["timelines"][0]["events"][0]["label"] = json!("Protected Fixture Artist");
    });
    recipe
        .policy
        .protected_references
        .push(DnaProtectedReferenceRule {
            term: "Protected Fixture Artist".into(),
            descriptive_replacement: Some("declared section".into()),
        });
    save(&input, &recipe);
    let output = root.path().join("bundle");
    sonic::build(&request(&input, &output), None, token()).unwrap();
    for file in [
        "sonic-dna.json",
        "audio-description.json",
        "prompt-guidance.json",
    ] {
        assert!(!fs::read_to_string(output.join(file))
            .unwrap()
            .contains("Protected Fixture Artist"));
    }
}
