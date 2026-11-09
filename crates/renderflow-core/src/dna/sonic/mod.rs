//! Source-bound Sonic DNA and reviewable descriptions over supplied evidence.
//! Aniflow owns extraction; existing DNA/AI services own hygiene and execution.

mod analysis;
mod companion;
mod facts;
mod recipe;
mod render;
mod semantic;
mod wav;

use std::collections::{BTreeMap,BTreeSet};
use std::fs;
use std::path::{Path,PathBuf};
use std::sync::Arc;

use anyhow::{ensure,Context,Result};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};

use crate::ai::AiSkillRuntime;
use crate::artifact::{Artifact,ArtifactDescriptor,ArtifactStorageClass,ArtifactStore,CanonicalFormat,MediaType};
use crate::dna::{ArtifactDna,ArtifactDnaEngine,ArtifactDnaExtractor,DnaDeterminism,DnaEvidenceOrigin,
    DnaExtractionBatch,DnaModality,DnaObservation,DnaObservationEvidence,DnaOmission};
use crate::evidence::DigestEvidence;
use crate::graph::Format;
use crate::process::ProcessCancellationToken;
use crate::publication::local::{self,Capture};
use wav::WavInspection;

pub use facts::{Confidence,SonicFact};
pub use recipe::{parse,AnalysisInput,Companion,CompanionKind,CompanionTextFormat,TimedTextContext,Correction,FileInput,SemanticMode,SemanticPolicy,SonicRecipe,PROFILE,SCHEMA};
pub use semantic::{DescriptionCandidate,DescriptionSection,EvidenceText,SemanticReport};

#[derive(Debug,Clone)]
pub struct SonicRequest {pub input:PathBuf,pub output_dir:PathBuf}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedInput {pub id:String,pub role:String,pub sha256:String,pub size_bytes:u64}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SonicPlan {
    pub schema_version:String,
    pub profile:String,
    pub engine_version:String,
    pub input_sha256:String,
    pub intent_sha256:String,
    pub inputs:Vec<ObservedInput>,
    pub technical:Value,
    pub analyses:Vec<Value>,
    pub declared_companions:Vec<Value>,
    pub fact_ids:Vec<String>,
    pub semantic:SemanticReport,
    pub direct_audio_inference:String,
    pub expected_outputs:Vec<String>,
    pub runtime_qualified:bool,
}

struct CapturedFile {subject:ObservedInput,bytes:Vec<u8>}
struct Captured {recipe:SonicRecipe,files:Vec<CapturedFile>,pcm:WavInspection,
    facts:Vec<SonicFact>,analyses:Vec<Value>,companions:Vec<Value>,input_sha256:String,intent_sha256:String}

fn read(root:&mut Capture,file:&FileInput,id:String,role:&str,maximum:usize)->Result<CapturedFile> {
    let bytes=root.read(&file.path,maximum)?;
    let sha256=local::sha256(&bytes);
    ensure!(sha256==file.sha256,"sonic.input.changed");
    Ok(CapturedFile {subject:ObservedInput {id,role:role.into(),sha256,size_bytes:bytes.len() as u64},bytes})
}

fn capture(input:&Path,token:ProcessCancellationToken)->Result<Captured> {
    ensure!(!token.is_cancelled(),"sonic.cancelled");
    let bytes=local::read_file(input,local::MAX_JSON)?;
    let recipe=parse(&bytes)?;
    let parent=input.parent().filter(|path|!path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let maximum_capture=if recipe.policy.max_source_bytes>32*1024*1024 {recipe.policy.max_source_bytes+local::MAX_TOTAL}else{local::MAX_TOTAL};
    let mut root=Capture::with_maximum_total(parent,token.clone(),maximum_capture)?;
    let audio=read(&mut root,&recipe.audio,"audio".into(),"audio",recipe.policy.max_source_bytes as usize)?;
    let pcm=wav::inspect(&audio.bytes,&token)?;
    let source_size=audio.bytes.len() as u64;
    let mut files=vec![audio];
    let mut facts=Vec::new();
    let mut analyses=Vec::new();
    for analysis in &recipe.analyses {
        let file=read(&mut root,&analysis.file,format!("analysis.{}",analysis.file.id),"analysis",local::MAX_JSON)?;
        let mut retained=BTreeMap::new();
        for evidence in &analysis.evidence_files {
            let captured=read(&mut root,evidence,format!("analysis.{}.evidence.{}",analysis.file.id,evidence.id),"analysis_evidence",8*1024*1024)?;
            retained.insert(evidence.id.clone(),(captured.subject.sha256.clone(),captured.subject.size_bytes));
            files.push(captured);
        }
        let projected=analysis::project(&file.bytes,&recipe.audio.sha256,source_size,pcm,&retained)?;
        analyses.push(json!({"manifest_sha256":file.subject.sha256,"status":projected.status,
            "normalized_scalar_facts":projected.scalar_count,"normalized_timeline_facts":projected.timeline_count,"unprojected":projected.unprojected,
            "native_validation":"bounded_projection_only","provider_authentication":"not_established",
            "contract_revision":analysis::CONTRACT_PIN}));
        ensure!(facts.len()+projected.facts.len()<=500,"sonic.facts.total_count");
        facts.extend(projected.facts);
        files.push(file);
    }
    let mut companions=Vec::new();
    for (index,companion) in recipe.companions.iter().enumerate() {
        let maximum=if companion.timed_text_context.is_some() {1_048_576}else{8*1024*1024};
        let file=read(&mut root,&companion.file,format!("companion.{}",companion.file.id),"companion",maximum)?;
        let context=companion::project(companion,&file.bytes,index,pcm)?;
        companions.push(json!({"sha256":file.subject.sha256,"size_bytes":file.subject.size_bytes,"kind":companion.kind,
            "source_sha256":companion.source_sha256,"authority_reference":companion.authority_reference,
            "authority_is_declared":true,"validation":if context.is_some() {"syntax_and_source_clock_only"}else{"byte_binding_only"},
            "structural_context":context.as_ref().map(|fact|&fact.value),"payload_exposed_to_model":false}));
        if let Some(fact)=context {facts.push(fact);}
        files.push(file);
    }
    for (index,correction) in recipe.corrections.iter().enumerate() {
        let original=facts.iter().find(|fact|fact.id==correction.fact_id).context("sonic.correction.fact_missing")?;
        ensure!(matches!(original.dimension.as_str(),"sonic.rhythm.tempo_bpm"|"sonic.harmony.key"),"sonic.correction.structural_observation");
        if original.dimension=="sonic.rhythm.tempo_bpm" {
            ensure!(correction.value.as_f64().is_some_and(|value|value>0.0 && value<=1000.0),"sonic.correction.tempo");
        } else {local::text(correction.value.as_str().context("sonic.correction.key")?,256)?;}
        let corrected=SonicFact {id:format!("correction.{index}"),dimension:original.dimension.clone(),value:correction.value.clone(),
            origin:DnaEvidenceOrigin::HumanProvided,class:DnaDeterminism::Heuristic,
            confidence:Confidence::Unavailable {reason:"Human correction declaration; no calibrated score supplied.".into()},
            evidence:json!({"request_sha256":local::sha256(&bytes),"pointer":format!("/corrections/{index}"),
                "review_reference":correction.review_reference,"authority_is_declared":true}),correction_of:Some(original.id.clone())};
        facts.push(corrected);
    }
    ensure!(facts.len()<=500,"sonic.facts.total_count");
    ensure!(serde_json::to_vec(&facts)?.len()<=local::MAX_JSON,"sonic.facts.expanded_byte_limit");
    let mut names=BTreeSet::new();
    for file in &files {ensure!(names.insert(&file.subject.id),"sonic.input.ambiguous_id");}
    let known=facts.iter().map(|fact|fact.dimension.as_str()).chain([
        "sonic.technical.duration_ms","sonic.technical.frame_count","sonic.technical.sample_rate_hz",
        "sonic.technical.channels","sonic.technical.bits_per_sample","sonic.technical.sample_encoding"]).collect::<BTreeSet<_>>();
    for dimension in recipe.dimension_weights.keys() {ensure!(known.contains(dimension.as_str()),"sonic.weight.unknown_dimension");}
    root.check()?;
    let input_sha256=local::sha256(&bytes);
    let intent_sha256=local::sha256(&serde_json::to_vec(&json!({"schema_version":"renderflow.sonic-intent/v1",
        "profile":PROFILE,"engine_version":env!("CARGO_PKG_VERSION"),"request_sha256":input_sha256,
        "inputs":files.iter().map(|file|&file.subject).collect::<Vec<_>>(),"aniflow_contract_revision":analysis::CONTRACT_PIN,
        "normalizer_revision":"sonic-evidence-mappings/3",
        "semantic_skill_sha256":local::sha256(semantic::SKILL_SOURCE.as_bytes())}))?);
    Ok(Captured {recipe,files,pcm,facts,analyses,companions,input_sha256,intent_sha256})
}

fn plan_captured(captured:&Captured,runtime:Option<&AiSkillRuntime<'_>>)->Result<SonicPlan> {
    let request=semantic::request(&captured.recipe,&captured.facts,None,None)?;
    Ok(SonicPlan {schema_version:"renderflow.sonic-plan/v1".into(),profile:PROFILE.into(),engine_version:env!("CARGO_PKG_VERSION").into(),
        input_sha256:captured.input_sha256.clone(),intent_sha256:captured.intent_sha256.clone(),
        inputs:captured.files.iter().map(|file|file.subject.clone()).collect(),
        technical:json!({"profile":"native_music_wav/v1","sample_rate_hz":captured.pcm.sample_rate_hz,"channels":captured.pcm.channels,"bits_per_sample":captured.pcm.bits_per_sample,"sample_encoding":captured.pcm.encoding,"frame_count":captured.pcm.frame_count,
            "duration_ms":captured.pcm.duration_ms,"timestamp_origin":"selected_input_sample_frames_from_zero","sample_conversion":"none","maximum_source_bytes":captured.recipe.policy.max_source_bytes,"maximum_capture_bytes":if captured.recipe.policy.max_source_bytes>32*1024*1024 {captured.recipe.policy.max_source_bytes+local::MAX_TOTAL}else{local::MAX_TOTAL}}),
        analyses:captured.analyses.clone(),declared_companions:captured.companions.iter().map(|companion|json!({
            "kind":companion["kind"],"sha256":companion["sha256"],"authority_is_declared":true,
            "validation":companion["validation"],"structural_context":companion["structural_context"],"payload_exposed_to_model":false})).collect(),
        fact_ids:["pcm.duration","pcm.frames","pcm.sample-rate","pcm.channels","pcm.bits","pcm.encoding"].into_iter().map(str::to_string)
            .chain(captured.facts.iter().map(|fact|fact.id.clone())).collect(),
        semantic:semantic::availability(&captured.recipe,&request,runtime),direct_audio_inference:"unsupported_by_this_profile".into(),
        expected_outputs:vec!["sonic-dna.json".into(),"audio-description.json".into(),"audio-description.md".into(),"prompt-guidance.json".into(),"lineage.json".into()],
        runtime_qualified:false})
}

/// Observe the exact input snapshots and declared model compatibility. No output,
/// artifact store, tool, provider request or model preparation is performed.
pub fn plan(input:&Path,runtime:Option<&AiSkillRuntime<'_>>,token:ProcessCancellationToken)->Result<SonicPlan> {
    plan_captured(&capture(input,token)?,runtime)
}

struct SonicExtractor {source_sha256:String,pcm:WavInspection,facts:Vec<SonicFact>,analyses:Vec<Value>,companions:Vec<Value>}
impl ArtifactDnaExtractor for SonicExtractor {
    fn id(&self)->&str {"renderflow.sonic.evidence-normalizer"}
    fn version(&self)->&str {"3"}
    fn locality(&self)->crate::dna::DnaProviderLocality {crate::dna::DnaProviderLocality::Local}
    fn determinism(&self)->DnaDeterminism {DnaDeterminism::Deterministic}
    fn ai_assisted(&self)->bool {false}
    fn requires_network(&self)->bool {false}
    fn supports(&self,artifact:&Artifact)->bool {artifact.format().as_str()=="wav"}
    fn extract(&self,artifact:&Artifact,store:&ArtifactStore)->Result<DnaExtractionBatch> {
        store.verify(artifact)?;
        ensure!(artifact.digest().value()==self.source_sha256,"sonic.extractor.source.changed");
        let mut batch=DnaExtractionBatch {modalities:vec![DnaModality::Sonic],..DnaExtractionBatch::default()};
        for (id,dimension,value) in [
            ("pcm.duration","sonic.technical.duration_ms",self.pcm.duration_ms),
            ("pcm.frames","sonic.technical.frame_count",self.pcm.frame_count),
            ("pcm.sample-rate","sonic.technical.sample_rate_hz",u64::from(self.pcm.sample_rate_hz)),
            ("pcm.channels","sonic.technical.channels",u64::from(self.pcm.channels)),
            ("pcm.bits","sonic.technical.bits_per_sample",u64::from(self.pcm.bits_per_sample)),
        ] {
            batch.observations.push(DnaObservation {id:id.into(),modality:DnaModality::Sonic,dimension:dimension.into(),value:json!(value),description:None,
                evidence:DnaObservationEvidence {origin:DnaEvidenceOrigin::ProviderObserved,provider_id:self.id().into(),provider_version:self.version().into(),
                    determinism:DnaDeterminism::Deterministic,confidence:1.0,scope:BTreeMap::from([("confidence_scope".into(),json!("structural byte inspection, not musical accuracy"))])},
                similarity_eligible:false});
        }
        batch.observations.push(DnaObservation {id:"pcm.encoding".into(),modality:DnaModality::Sonic,dimension:"sonic.technical.sample_encoding".into(),value:json!(self.pcm.encoding),description:None,
            evidence:DnaObservationEvidence {origin:DnaEvidenceOrigin::ProviderObserved,provider_id:self.id().into(),provider_version:self.version().into(),determinism:DnaDeterminism::Deterministic,confidence:1.0,scope:BTreeMap::from([("confidence_scope".into(),json!("structural byte inspection, not musical accuracy"))])},similarity_eligible:false});
        batch.provider_extensions.insert("renderflow.sonic".into(),json!({"schema_version":"renderflow.sonic-evidence/v1",
            "facts":self.facts,"analyses":self.analyses,"companions":self.companions,
            "confidence_contract":"Unknown/not-applicable/uncalibrated values retain their tags; no musical confidence is synthesized."}));
        batch.omissions.push(DnaOmission {code:"sonic.direct_audio.not_executed".into(),class:"unsupported_input_modality".into(),
            message:"No model listened to audio. Selected musical events and explicitly selected timed-text structure are projected; other native semantic/excerpt and companion word payloads remain uninterpreted.".into()});
        Ok(batch)
    }
}

pub fn build(request:&SonicRequest,runtime:Option<&AiSkillRuntime<'_>>,token:ProcessCancellationToken)->Result<Value> {
    ensure!(!token.is_cancelled(),"sonic.cancelled");
    match fs::symlink_metadata(&request.output_dir) {
        Ok(_)=>anyhow::bail!("sonic.output.exists"),
        Err(error) if error.kind()==std::io::ErrorKind::NotFound=>{},
        Err(error)=>return Err(error.into()),
    }
    let parent=request.output_dir.parent().filter(|path|!path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    ensure!(parent.is_dir(),"sonic.output.parent");
    let captured=capture(&request.input,token.clone())?;
    let plan=plan_captured(&captured,runtime)?;
    let temporary=tempfile::Builder::new().prefix("renderflow-sonic-").tempdir_in(parent)?;
    let store=ArtifactStore::new(temporary.path().join("artifacts"))?;
    let source=store.put_bytes(&captured.files[0].bytes,ArtifactDescriptor::for_format(Format::Wav,ArtifactStorageClass::Source))?;
    let mut selected=vec![source.clone()];
    for file in captured.files.iter().skip(1) {
        let descriptor=ArtifactDescriptor::new(CanonicalFormat::new("sonic-evidence-input")?,
            MediaType::new("application/octet-stream")?,ArtifactStorageClass::Source).with_source(source.id().clone());
        selected.push(store.put_bytes(&file.bytes,descriptor)?);
    }
    ensure!(!token.is_cancelled(),"sonic.cancelled");
    let engine=ArtifactDnaEngine::with_builtins().with_extractor(Arc::new(SonicExtractor {source_sha256:captured.recipe.audio.sha256.clone(),
        pcm:captured.pcm,facts:captured.facts,analyses:captured.analyses,companions:captured.companions}));
    let outcome=engine.extract(&source,&store,&captured.recipe.policy)?;
    let mut dna=outcome.dna.context("sonic.hygiene_or_policy.refused")?;
    dna.approval.human_review_required=true;
    dna.provenance.configuration_digest=DigestEvidence {algorithm:"sha256".into(),value:captured.intent_sha256.clone()};
    dna.validate()?;
    let dna_bytes=serde_json::to_vec_pretty(&dna)?;
    let dna_artifact=store.put_bytes(&dna_bytes,ArtifactDescriptor::new(CanonicalFormat::new(crate::dna::ARTIFACT_DNA_FORMAT_V1)?,
        MediaType::new(crate::dna::ARTIFACT_DNA_MEDIA_TYPE_V1)?,ArtifactStorageClass::Terminal)
        .with_sources(selected.iter().map(|artifact|artifact.id().clone())).with_metadata("renderflow.sonic.profile",PROFILE))?;
    let facts=facts::collect(&dna)?;
    let (semantic,candidate)=semantic::execute(&captured.recipe,&dna,&dna_artifact,&facts,runtime,&store,&token)?;
    let mode=if candidate.is_some() {"model_interpretation_of_structured_evidence"} else {"deterministic_evidence_render"};
    let description=candidate.unwrap_or_else(||render::deterministic(&facts));
    let mut checked_description=serde_json::to_value(description)?;
    let mut description_findings=Vec::new();
    ensure!(crate::dna::sanitize_value(&mut checked_description,&captured.recipe.policy,&mut description_findings),"sonic.description.hygiene");
    let description:DescriptionCandidate=serde_json::from_value(checked_description)?;
    semantic::validate(&description,&facts)?;
    let description_value=json!({"schema_version":"renderflow.sonic-description/v1","source_sha256":source.digest().value(),
        "source_dna_sha256":dna_artifact.digest().value(),"mode":mode,"candidate":description,
        "input_modality":"structured_evidence","review_state":"candidate","human_review_required":true,
        "grounding_scope":"Known evidence IDs and schema only; semantic accuracy is not certified."});
    let mut guidance=render::guidance(&facts,dna_artifact.digest().value(),&captured.recipe.dimension_weights);
    let mut findings=Vec::new();
    ensure!(crate::dna::sanitize_value(&mut guidance,&captured.recipe.policy,&mut findings),"sonic.guidance.hygiene");
    let description_bytes=serde_json::to_vec_pretty(&description_value)?;
    let guidance_bytes=serde_json::to_vec_pretty(&guidance)?;
    let description_artifact=store.put_bytes(&description_bytes,ArtifactDescriptor::for_format(Format::Json,ArtifactStorageClass::Terminal)
        .with_source(dna_artifact.id().clone()).with_metadata("renderflow.sonic.review_required",true))?;
    let guidance_artifact=store.put_bytes(&guidance_bytes,ArtifactDescriptor::for_format(Format::Json,ArtifactStorageClass::Terminal)
        .with_source(dna_artifact.id().clone()).with_metadata("renderflow.sonic.review_required",true))?;
    let mut files=BTreeMap::from([("sonic-dna.json".into(),dna_bytes),("audio-description.json".into(),description_bytes),
        ("audio-description.md".into(),render::markdown(&description).into_bytes()),("prompt-guidance.json".into(),guidance_bytes)]);
    let produced=vec![dna_artifact,description_artifact,guidance_artifact];
    for artifact in &produced {
        store.verify(artifact)?;
        let path=artifact.payload().relative_path().to_str().context("sonic.object.path")?.replace('\\',"/");
        files.insert(path,store.read_bytes(artifact)?);
    }
    files.insert("lineage.json".into(),serde_json::to_vec_pretty(&json!({"schema_version":"renderflow.sonic-lineage/v1",
        "external_selected_inputs":selected.iter().map(|artifact|json!({"artifact":artifact,"payload_included":false,"authority":"selected_input"})).collect::<Vec<_>>(),
        "produced":produced,"review_state":"candidate","source_payloads_retained":false}))?);
    let partial=captured.recipe.semantic.mode==SemanticMode::Required && semantic.status!="produced";
    let receipt=json!({"schema_version":"renderflow.sonic-run/v1","profile":PROFILE,"state":if partial {"partial"} else {"complete"},
        "plan":plan,"semantic":semantic,"source_sha256":source.digest().value(),"intent_sha256":captured.intent_sha256,
        "normalization_status":outcome.status,"normalization_diagnostics":outcome.diagnostics,
        "review_state":"candidate","publication_approval":"not_granted","source_inputs_included":false,
        "model_provider_candidate_payload_included":false,"runtime_qualified":false,
        "scope":"Structural byte observations, supplied analyzer projection and review-required descriptions; no direct audio interpretation or released provider qualification."});
    local::publish(&request.output_dir,&files,"sonic-run.json",receipt,token)
}
