use super::*;
use crate::dna::{DnaDeterminism, DnaEvidenceOrigin};
fn fact(id: &str, dimension: &str, value: Value) -> SonicFact {
    SonicFact {
        id: id.into(),
        dimension: dimension.into(),
        value,
        origin: DnaEvidenceOrigin::SourceReported,
        class: DnaDeterminism::Heuristic,
        confidence: Confidence::Uncalibrated { score: 0.7 },
        evidence: json!({"channel_scope":[0,1],"stem_id":null}),
        correction_of: None,
    }
}
#[test]
fn distinct_tempos_and_corrections_remain_separately_cited() {
    let base = fact("pcm", "sonic.technical.channels", json!(2));
    let a = fact("tempo-a", "sonic.rhythm.tempo_bpm", json!(120));
    let mut b = fact("tempo-b", "sonic.rhythm.tempo_bpm", json!(144));
    b.correction_of = Some(a.id.clone());
    b.confidence = Confidence::Unavailable {
        reason: "declared correction".into(),
    };
    let facts = vec![base, a, b];
    let d = deterministic(&facts);
    super::super::semantic::validate(&d, &facts).unwrap();
    let rhythm = d.sections.iter().find(|s| s.topic == "rhythm").unwrap();
    assert!(
        rhythm.text.contains("120")
            && rhythm.text.contains("144")
            && rhythm.text.contains("confidence unavailable")
    );
    assert_eq!(rhythm.evidence_ids, vec!["tempo-a", "tempo-b"]);
    assert!(d.review_required);
}
#[test]
fn selected_context_families_survive_large_event_lists_and_unicode_values() {
    let mut facts = vec![fact("pcm", "sonic.technical.channels", json!(2))];
    for i in 0..40 {
        facts.push(fact(
            &format!("event-{i}"),
            "sonic.arrangement.reported_event",
            json!("音".repeat(500)),
        ));
    }
    facts.push(fact(
        "stem",
        "sonic.arrangement.declared_stem_relationship",
        json!({"alignment":"not_established"}),
    ));
    facts.push(fact(
        "note",
        "sonic.score.inferred_note",
        json!({"pitch_midi":60}),
    ));
    let d = deterministic(&facts);
    super::super::semantic::validate(&d, &facts).unwrap();
    assert!(d
        .sections
        .iter()
        .any(|s| s.evidence_ids.contains(&"stem".into())));
    assert!(d
        .sections
        .iter()
        .any(|s| s.evidence_ids.contains(&"note".into())));
    assert!(d
        .sections
        .iter()
        .all(|s| s.text.len() <= 4096 && s.evidence_ids.len() <= 3));
}
#[test]
fn explicit_importance_changes_guidance_order_without_changing_confidence() {
    let facts = vec![
        fact("tempo", "sonic.rhythm.tempo_bpm", json!(120)),
        fact("key", "sonic.harmony.key", json!("C")),
    ];
    let weights = std::collections::BTreeMap::from([
        ("sonic.rhythm.tempo_bpm".into(), 0.1),
        ("sonic.harmony.key".into(), 0.9),
    ]);
    let g = guidance(&facts, &"a".repeat(64), &weights);
    assert_eq!(g["components"][0]["dimension"], "sonic.harmony.key");
    assert_eq!(g["components"][0]["confidence"]["score"], 0.7);
    assert_eq!(g["importance_is_confidence"], false);
    assert_eq!(g["review_required"], true);
}
