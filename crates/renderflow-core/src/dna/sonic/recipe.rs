use std::collections::{BTreeMap, BTreeSet};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::ai::AiExecutionPreferenceV1;
use crate::dna::DnaExtractionPolicy;
use crate::evidence::files;
use crate::publication::local;

pub const SCHEMA: &str = "renderflow.sonic-request/v1";
pub const PROFILE: &str = "sonic-evidence-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileInput {pub id:String, pub path:String, pub sha256:String}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisInput {pub file:FileInput, pub evidence_files:Vec<FileInput>}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum CompanionKind {ReviewedLyrics, TranscriptCandidate, ArtistMidi, InferredMidi, StemManifest, VisualDna, Metadata}

#[derive(Debug,Clone,Copy,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum CompanionTextFormat {Srt,Webvtt}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedTextContext {pub format:CompanionTextFormat,pub maximum_cues:usize}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Companion {
    pub file:FileInput,
    pub source_sha256:String,
    pub kind:CompanionKind,
    /// A declared authority record; its authenticity is not inferred from bytes.
    pub authority_reference:String,
    /// Explicit structural inspection only; never expose words or infer approval.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub timed_text_context:Option<TimedTextContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Correction {
    pub fact_id:String,
    pub value:Value,
    pub review_reference:String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum SemanticMode {Disabled, Optional, Required}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPolicy {
    pub mode:SemanticMode,
    pub source_approved_for_ai:bool,
    pub preference:AiExecutionPreferenceV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SonicRecipe {
    pub schema_version:String,
    pub id:String,
    pub audio:FileInput,
    pub analyses:Vec<AnalysisInput>,
    pub companions:Vec<Companion>,
    pub corrections:Vec<Correction>,
    pub policy:DnaExtractionPolicy,
    pub semantic:SemanticPolicy,
    /// Importance only; these weights never mean calibrated confidence.
    pub dimension_weights:BTreeMap<String,f64>,
}

pub fn parse(bytes:&[u8])->Result<SonicRecipe> {
    let recipe:SonicRecipe=local::parse(bytes)?;
    ensure!(recipe.schema_version==SCHEMA,"sonic.schema");
    local::identifier(&recipe.id)?;
    validate_file(&recipe.audio)?;
    ensure!(recipe.analyses.len()<=8 && recipe.companions.len()<=32 && recipe.corrections.len()<=64,"sonic.inputs.count");
    ensure!(1+recipe.analyses.iter().map(|analysis|1+analysis.evidence_files.len()).sum::<usize>()+recipe.companions.len()<=256,"sonic.inputs.total_count");
    ensure!(recipe.policy.enabled && recipe.policy.max_source_bytes>0 && recipe.policy.max_source_bytes<=32*1024*1024
        && recipe.policy.max_observations>=16 && recipe.policy.max_observations<=1024,"sonic.policy.bounds");
    local::text(&recipe.policy.policy_id,256)?;
    ensure!(recipe.policy.policy_id.bytes().all(|ch|ch.is_ascii_alphanumeric() || matches!(ch,b'.'|b'-'|b'_'|b'/')),"sonic.policy.id");
    ensure!(recipe.policy.protected_references.len()<=64,"sonic.policy.references.count");
    for rule in &recipe.policy.protected_references {
        local::text(&rule.term,256)?;
        if let Some(replacement)=&rule.descriptive_replacement {local::text(replacement,1024)?;}
    }
    let mut ids=BTreeSet::from([recipe.audio.id.as_str()]);
    let mut digests=BTreeSet::new();
    for analysis in &recipe.analyses {
        validate_file(&analysis.file)?;
        ensure!(ids.insert(&analysis.file.id),"sonic.input.duplicate");
        ensure!(digests.insert(&analysis.file.sha256),"sonic.analysis.repeated_content");
        ensure!(analysis.evidence_files.len()<=128,"sonic.evidence.count");
        let mut evidence=BTreeSet::new();
        for file in &analysis.evidence_files {
            validate_file(file)?;
            ensure!(evidence.insert(&file.id),"sonic.evidence.duplicate");
        }
    }
    for companion in &recipe.companions {
        validate_file(&companion.file)?;
        ensure!(ids.insert(&companion.file.id),"sonic.input.duplicate");
        ensure!(companion.source_sha256==recipe.audio.sha256,"sonic.companion.source.changed");
        local::text(&companion.authority_reference,4096)?;
        if let Some(context)=&companion.timed_text_context {
            ensure!(matches!(companion.kind,CompanionKind::ReviewedLyrics|CompanionKind::TranscriptCandidate)
                &&(1..=256).contains(&context.maximum_cues),"sonic.companion.context.policy");
        }
    }
    let mut corrections=BTreeSet::new();
    for correction in &recipe.corrections {
        local::identifier(&correction.fact_id)?;
        ensure!(corrections.insert(&correction.fact_id),"sonic.correction.duplicate");
        local::text(&correction.review_reference,4096)?;
        ensure!(correction.value.is_number() || correction.value.is_string(),"sonic.correction.value");
    }
    ensure!(recipe.dimension_weights.len()<=64,"sonic.weights.count");
    for (dimension,weight) in &recipe.dimension_weights {
        local::identifier(dimension)?;
        ensure!(dimension.starts_with("sonic.") && weight.is_finite() && (0.0..=1.0).contains(weight),"sonic.weight");
    }
    ensure!(!recipe.policy.allow_remote || recipe.policy.allow_network,"sonic.policy.remote.network");
    Ok(recipe)
}

fn validate_file(file:&FileInput)->Result<()> {
    local::identifier(&file.id)?;
    files::components(&file.path,64)?;
    files::digest(&file.sha256)?;
    Ok(())
}
