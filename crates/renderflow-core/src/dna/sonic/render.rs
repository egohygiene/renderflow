use serde_json::{json,Value};
use crate::publication::local;
use super::facts::{Confidence,SonicFact};
use super::semantic::{DescriptionCandidate,DescriptionSection,EvidenceText};

fn value(value:&Value)->String {value.as_str().map(str::to_string).unwrap_or_else(||value.to_string())}
fn topic(dimension:&str)->&'static str {
    if dimension=="sonic.arrangement.declared_stem_relationship" {return "lineage";}
    if dimension=="sonic.score.inferred_note" {return "note_candidates";}
    if dimension.starts_with("sonic.technical.") {"technical"}
    else if dimension.starts_with("sonic.rhythm.") {"rhythm"}
    else if dimension.starts_with("sonic.harmony.") {"harmony"}
    else if dimension.starts_with("sonic.arrangement.") {"arrangement"}
    else if dimension.starts_with("sonic.lyrics.") {"lyrics"}
    else {"other"}
}

pub(super) const DESCRIPTION_REVISION:&str="sonic-evidence-description/2";
pub(super) const GUIDANCE_REVISION:&str="renderflow.sonic-guidance/2";
fn shorten(text:&str,maximum:usize)->String {const END:&str=" [excerpt; complete fact retained in Sonic DNA]";if text.len()<=maximum{return text.to_owned();}let mut result=String::new();for c in text.chars(){if result.len()+c.len_utf8()>maximum-END.len(){break;}result.push(c);}result.push_str(END);result}
fn excerpt(value:&Value)->String {shorten(&value.as_str().map(str::to_owned).unwrap_or_else(||value.to_string()),192)}
fn confidence(fact:&SonicFact)->String {match &fact.confidence {
    Confidence::Calibrated {score}=>format!("analyzer-declared calibrated confidence {score}"),
    Confidence::Uncalibrated {score}=>format!("uncalibrated score {score}; not an accuracy probability"),
    Confidence::Unavailable {..}=>"confidence unavailable".into(),
    Confidence::NotApplicable=>"no probabilistic confidence applies".into(),
}}
pub(super) fn deterministic(facts:&[SonicFact])->DescriptionCandidate {
    let mut sections=Vec::new();let mut displayed=0;
    for family in ["technical","rhythm","harmony","arrangement","note_candidates","lineage","lyrics","other"] {
        let selected=facts.iter().filter(|f|topic(&f.dimension)==family).take(3).collect::<Vec<_>>();
        if selected.is_empty(){continue;}displayed+=selected.len();
        let mut text=match family {
            "rhythm"|"harmony"=>"Reported values remain separate; no preferred estimate or correction is selected. ",
            "note_candidates"=>"These are projected unreviewed note candidates, not an authored score or identified instrument. ",
            "lineage"=>"Declared stem relationships do not establish original-mix sample alignment or instrument/voice identity. ",
            "lyrics"=>"Supplied timed-text structure is not word accuracy, alignment or lyrical meaning. ",
            _=>"Selected evidence facts with their original uncertainty and scope. ",
        }.to_owned();
        for fact in &selected {
            let mut line=format!("{}: {}. Origin {:?}, class {:?}; {}.",fact.dimension,excerpt(&fact.value),fact.origin,fact.class,confidence(fact));
            if let Some(original)=&fact.correction_of{line.push_str(&format!(" Separately declared correction of {original}; original retained."));}
            // Preserve the original channel/stem context without merging scopes.
            if let Some(channels)=fact.evidence.get("channel_scope").or_else(||fact.value.get("channel_scope")){line.push_str(&format!(" Channel scope {}.",excerpt(channels)));}
            if let Some(stem)=fact.evidence.get("stem_id").or_else(||fact.value.get("stem_id")){line.push_str(&format!(" Stem scope {}.",excerpt(stem)));}
            text.push_str(&shorten(&line,1000));
            text.push(' ');
        }
        sections.push(DescriptionSection {topic:if matches!(family,"lineage"|"note_candidates"){"arrangement"}else{family}.into(),text,evidence_ids:selected.iter().map(|f|f.id.clone()).collect()});
    }
    DescriptionCandidate {summary:EvidenceText {text:"A supplied native WAV snapshot has verified structural metadata. Selected source-reported musical context is grouped with its original scope and uncertainty; no listening or human review was performed.".into(),evidence_ids:facts.iter().filter(|f|f.dimension.starts_with("sonic.technical.")).take(8).map(|f|f.id.clone()).collect()},sections,
        uncertainty:vec!["No model heard the audio. Instrumentation, vocal identity, mood, lyrical meaning and score semantics were not inferred.".into(),
            "Hash/source binding does not authenticate an analyzer or prove estimates accurate. Distinct values and declared corrections remain separate.".into(),
            "Note velocities/activations are not calibrated confidence; stem duration relationships are not original-mix sample alignment.".into(),
            format!("This view displays {displayed} of {} admitted facts, at most three per family; complete facts remain in Sonic DNA.",facts.len())],review_required:true}
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
    let mut selected=facts.iter().enumerate().filter(|(_,f)|!f.dimension.starts_with("sonic.technical.")).collect::<Vec<_>>();
    selected.sort_by(|(left,a),(right,b)|weights.get(&b.dimension).copied().unwrap_or(1.0).total_cmp(&weights.get(&a.dimension).copied().unwrap_or(1.0)).then_with(||left.cmp(right)));
    json!({"schema_version":"renderflow.sonic-prompt-guidance/v1","profile":"sonic-evidence-v1",
        "source_dna_sha256":dna_sha256,"target_modality":"audio","purpose":"related_original_asset",
        "creative_brief":"Use only the selected descriptive dimensions as starting constraints for an original asset. Preserve uncertainty and review musical suitability.",
        "components":selected.iter().take(64).map(|(_,fact)|json!({
            "dimension":fact.dimension,"value":fact.value,"evidence_ids":[fact.id],"confidence":fact.confidence,
            "importance":weights.get(&fact.dimension).copied().unwrap_or(1.0),"correction_of":fact.correction_of})).collect::<Vec<_>>(),
        "compact_tags":selected.iter().take(16)
            .map(|(_,fact)|format!("{}: {}",fact.dimension,value(&fact.value))).collect::<Vec<_>>(),
        "avoid":["Direct imitation of a named artist, work, franchise or brand","Copying source lyrics, melodies, recordings or voice identity"],
        "selection_limits":{"maximum_components":64,"maximum_tags":16,"available_facts":facts.len()},
        "importance_is_confidence":false,"review_required":true,"direct_imitation_allowed":false,"legal_clearance_claimed":false,
        "source_dna_digest_algorithm":"sha256","guidance_revision":local::sha256(GUIDANCE_REVISION.as_bytes())})
}

#[cfg(test)]mod tests;
