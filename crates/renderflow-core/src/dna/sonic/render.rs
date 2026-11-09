use serde_json::{json,Value};
use crate::publication::local;
use super::facts::{Confidence,SonicFact};
use super::semantic::{DescriptionCandidate,DescriptionSection,EvidenceText};

fn value(value:&Value)->String {value.as_str().map(str::to_string).unwrap_or_else(||value.to_string())}
fn topic(dimension:&str)->&'static str {
    if dimension.starts_with("sonic.technical.") {"technical"}
    else if dimension.starts_with("sonic.rhythm.") {"rhythm"}
    else if dimension.starts_with("sonic.harmony.") {"harmony"}
    else if dimension.starts_with("sonic.arrangement.") {"arrangement"}
    else if dimension.starts_with("sonic.lyrics.") {"lyrics"}
    else {"other"}
}

pub(super) fn deterministic(facts:&[SonicFact])->DescriptionCandidate {
    let sections=facts.iter().take(32).map(|fact|{
        let confidence=match &fact.confidence {
            Confidence::Calibrated {score}=>format!("analyzer-declared calibrated confidence {score}"),
            Confidence::Uncalibrated {score}=>format!("uncalibrated score {score}; not an accuracy probability"),
            Confidence::Unavailable {..}=>"confidence unavailable".into(),
            Confidence::NotApplicable=>"no probabilistic confidence applies".into(),
        };
        DescriptionSection {topic:topic(&fact.dimension).into(),text:format!("{}: {}. Evidence origin {:?}, class {:?}; {}.{}",
            fact.dimension,value(&fact.value),fact.origin,fact.class,confidence,
            if fact.correction_of.is_some() {" This is a separately declared correction; the original observation is retained."} else {""}),
            evidence_ids:vec![fact.id.clone()]}
    }).collect();
    DescriptionCandidate {summary:EvidenceText {text:"A supplied PCM audio snapshot has verified structural metadata. Musical estimates, when supplied, remain source-reported observations with their original uncertainty.".into(),
        evidence_ids:facts.iter().filter(|fact|fact.dimension.starts_with("sonic.technical.")).take(8).map(|fact|fact.id.clone()).collect()},
        sections,uncertainty:vec!["No model heard the audio. Instrumentation, vocal identity, mood, lyrical meaning and score semantics were not inferred.".into(),
            "Hash/source binding does not authenticate an analyzer or prove the musical estimates accurate. Conflicting observations remain separate.".into(),
            format!("The evidence description displays at most 32 of {} admitted facts; full sanitized facts remain in Sonic DNA.",facts.len())],review_required:true}
}

pub(super) fn markdown(candidate:&DescriptionCandidate)->String {
    let mut markdown="# Audio evidence description\n\nReview required. Structured evidence interpretation only.\n\n".to_string();
    for line in candidate.summary.text.lines() {markdown.push_str(&format!("    {line}\n"));}
    for section in &candidate.sections {
        markdown.push_str(&format!("\n## {}\n\n",section.topic));
        for line in section.text.lines() {markdown.push_str(&format!("    {line}\n"));}
        markdown.push_str(&format!("\nEvidence IDs: {}\n",section.evidence_ids.join(", ")));
    }
    markdown.push_str("\n## Uncertainty\n\n");
    for note in &candidate.uncertainty {for line in note.lines() {markdown.push_str(&format!("    {line}\n"));}markdown.push('\n');}
    markdown
}

pub(super) fn guidance(facts:&[SonicFact],dna_sha256:&str,weights:&std::collections::BTreeMap<String,f64>)->Value {
    json!({"schema_version":"renderflow.sonic-prompt-guidance/v1","profile":"sonic-evidence-v1",
        "source_dna_sha256":dna_sha256,"target_modality":"audio","purpose":"related_original_asset",
        "creative_brief":"Use only the selected descriptive dimensions as starting constraints for an original asset. Preserve uncertainty and review musical suitability.",
        "components":facts.iter().filter(|fact|!fact.dimension.starts_with("sonic.technical.")).take(64).map(|fact|json!({
            "dimension":fact.dimension,"value":fact.value,"evidence_ids":[fact.id],"confidence":fact.confidence,
            "importance":weights.get(&fact.dimension).copied().unwrap_or(1.0),"correction_of":fact.correction_of})).collect::<Vec<_>>(),
        "compact_tags":facts.iter().filter(|fact|!fact.dimension.starts_with("sonic.technical.")).take(16)
            .map(|fact|format!("{}: {}",fact.dimension,value(&fact.value))).collect::<Vec<_>>(),
        "avoid":["Direct imitation of a named artist, work, franchise or brand","Copying source lyrics, melodies, recordings or voice identity"],
        "selection_limits":{"maximum_components":64,"maximum_tags":16,"available_facts":facts.len()},
        "importance_is_confidence":false,"review_required":true,"direct_imitation_allowed":false,"legal_clearance_claimed":false,
        "source_dna_digest_algorithm":"sha256","guidance_revision":local::sha256(b"renderflow.sonic-guidance/1")})
}
