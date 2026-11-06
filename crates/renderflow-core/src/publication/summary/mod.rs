//! Source-grounded, review-required synopsis candidates over the existing AI
//! skill runtime. Selected text and model output never carry publish authority.
mod model;
pub use model::*;
use std::collections::{BTreeMap,BTreeSet};
use std::path::Path;
use std::time::{Instant,SystemTime,UNIX_EPOCH};
use std::sync::atomic::{AtomicU64,Ordering};
use anyhow::{ensure,Context,Result};
use serde_json::{json,Value};
use crate::ai::{AiCandidateState,AiExecutionPreferenceV1,AiInputArtifactEvidence,AiModelCatalog,AiProvider,AiSkillExecutionRequest,AiSkillRegistry,AiSkillRuntime,AiSkillSpec};
use crate::ai::profile::{RuntimeMode,RuntimeProfile,ProfileStatus};
use crate::artifact::{Artifact,ArtifactDescriptor,ArtifactStorageClass,ArtifactStore};
use crate::graph::Format;
use crate::process::ProcessCancellationToken;
use super::local;

pub const COVERAGE:&str="Selected UTF-8 text slices only; visual/media content was not inspected. Unselected source bytes are recorded separately. Reference checks do not prove paraphrase accuracy.";
static INVOCATIONS:AtomicU64=AtomicU64::new(1);
pub fn skill()->Result<AiSkillSpec> {AiSkillSpec::from_json(include_str!("../../../data/ai/skills/publication-summary-v1.json"))}

pub struct PreparedSummary {
    recipe_sha256:String,sources:Vec<SourceEvidence>,slices:Vec<SourceSlice>,artifacts:Vec<Artifact>,input:Value,
}

fn cancelled(token:&ProcessCancellationToken)->Result<()> {ensure!(!token.is_cancelled(),"summary.cancelled");Ok(())}
fn evidence_id(source:&str,sha:&str,start:usize,end:usize)->String {
    format!("evidence-{}",local::sha256(&serde_json::to_vec(&(source,sha,start,end)).expect("serializable source identity")))
}

/// Freeze a bounded recipe and the exact selected text. Source approval is a
/// separate declared permission, checked again before any runtime invocation.
pub fn capture(path:&Path,store:&ArtifactStore,token:ProcessCancellationToken)->Result<PreparedSummary> {
    cancelled(&token)?;
    let bytes=local::read_file(path,65536)?;let recipe:SummaryRecipe=crate::evidence::json::object(&bytes,65536)?;
    ensure!(recipe.schema_version==RECIPE_SCHEMA&&!recipe.sources.is_empty()&&recipe.sources.len()<=8,"summary.recipe");local::identifier(&recipe.id)?;
    let recipe_sha256=local::sha256(&serde_json::to_vec(&recipe)?);
    let root=path.parent().filter(|path|!path.as_os_str().is_empty()).unwrap_or(Path::new("."));let mut capture=local::Capture::new(root,token.clone())?;
    let mut sources=Vec::new();let mut slices=Vec::new();let mut artifacts=Vec::new();let mut ids=BTreeSet::new();let mut selected=0_usize;let mut total=0_usize;
    for source in &recipe.sources {
        cancelled(&token)?;local::identifier(&source.id)?;ensure!(ids.insert(&source.id),"summary.source.duplicate");
        ensure!(!source.ranges.is_empty()&&source.ranges.len()<=16,"summary.range.count");
        ensure!(!source.privacy_approved_for_remote||source.approved_for_ai,"summary.source.permissions");
        if source.approved_for_ai {local::text(&source.approval_reference,1024)?;}
        let bytes=capture.read(&source.path,1024*1024)?;total+=bytes.len();ensure!(total<=2*1024*1024&&local::sha256(&bytes)==source.sha256,"summary.source.identity_or_limit");
        let text=std::str::from_utf8(&bytes).context("summary.source.utf8")?;
        let artifact=store.put_bytes(&bytes,ArtifactDescriptor::for_format(Format::Text,ArtifactStorageClass::Source)
            .with_metadata("renderflow.summary.source_id",source.id.clone()))?;store.verify(&artifact)?;
        let mut previous=0;let mut source_selected=0;
        for range in &source.ranges {
            ensure!(range.start>=previous&&range.start<range.end&&range.end<=bytes.len()&&range.end-range.start<=8192,"summary.range.bounds_or_overlap");
            let value=text.get(range.start..range.end).context("summary.range.utf8_boundary")?;local::text(value,8192)?;
            selected+=value.len();source_selected+=value.len();ensure!(selected<=32768&&slices.len()<64,"summary.input.bounds");
            slices.push(SourceSlice {evidence_id:evidence_id(&source.id,&source.sha256,range.start,range.end),source_id:source.id.clone(),
                artifact_id:artifact.id().to_string(),source_sha256:source.sha256.clone(),start:range.start,end:range.end,
                text_sha256:local::sha256(value.as_bytes()),text:value.into()});previous=range.end;
        }
        sources.push(SourceEvidence {source_id:source.id.clone(),artifact_id:artifact.id().to_string(),sha256:source.sha256.clone(),size_bytes:bytes.len(),
            selected_bytes:source_selected,unselected_bytes:bytes.len()-source_selected,approved_for_ai:source.approved_for_ai,
            privacy_approved_for_remote:source.privacy_approved_for_remote,approval_reference:source.approval_reference.clone()});artifacts.push(artifact);
    }
    let input=json!({"evidence":slices,"coverage":COVERAGE});
    ensure!(serde_json::to_vec(&input)?.len()<=131072,"summary.input.serialized_limit");cancelled(&token)?;
    Ok(PreparedSummary {recipe_sha256,sources,slices,artifacts,input})
}

impl PreparedSummary {
    pub fn plan(&self,profile:&RuntimeProfile,catalog:&AiModelCatalog)->Result<SummaryPlan> {
        Ok(SummaryPlan {schema_version:"renderflow.summary-plan/v1".into(),recipe_sha256:self.recipe_sha256.clone(),sources:self.sources.clone(),
            slice_ids:self.slices.iter().map(|slice|slice.evidence_id.clone()).collect(),selected_input_bytes:serde_json::to_vec(&self.input)?.len(),
            profile:profile.inspect(catalog,&skill()?)?,coverage_note:COVERAGE.into()})
    }
}

pub fn validate_body(body:&SummaryBody,slices:&[SourceSlice])->Result<()> {
    crate::ai::validate_json_instance(&skill()?.output_schema,&serde_json::to_value(body)?)?;
    ensure!(body.review_required&&!body.synopsis.is_empty()&&body.synopsis.len()<=8&&body.uncertainty.len()<=8&&body.omissions.len()<=8,"summary.body.bounds_review");
    let known=slices.iter().map(|slice|slice.evidence_id.as_str()).collect::<BTreeSet<_>>();
    for statement in &body.synopsis {
        local::text(&statement.text,2048)?;ensure!(!statement.evidence_ids.is_empty()&&statement.evidence_ids.len()<=8,"summary.citation.count");
        let mut seen=BTreeSet::new();for id in &statement.evidence_ids {ensure!(seen.insert(id)&&known.contains(id.as_str()),"summary.citation.unknown_or_repeated");}
    }
    for text in body.uncertainty.iter().chain(&body.omissions) {local::text(text,2048)?;}
    Ok(())
}
pub fn candidate_id(candidate:&SummaryCandidate)->Result<String> {
    let bytes=serde_json::to_vec(&(&candidate.recipe_sha256,&candidate.body,&candidate.sources,&candidate.slices,&candidate.processing,&candidate.grounding,&candidate.coverage_note,&candidate.reference_validator))?;
    Ok(format!("summary-{}",local::sha256(&bytes)))
}
pub fn validate_candidate(candidate:&SummaryCandidate)->Result<()> {
    ensure!(candidate.schema_version==CANDIDATE_SCHEMA&&candidate.state==AiCandidateState::Candidate&&candidate.review_required
        &&candidate.grounding=="references_checked_semantics_unverified"&&candidate.coverage_note==COVERAGE
        &&candidate.reference_validator=="validator.summary.references/v1","summary.candidate.contract");
    ensure!(!candidate.sources.is_empty()&&candidate.sources.len()<=8&&!candidate.slices.is_empty()&&candidate.slices.len()<=64,"summary.candidate.sources");
    let sources=candidate.sources.iter().map(|source|(source.source_id.as_str(),source)).collect::<BTreeMap<_,_>>();
    ensure!(sources.len()==candidate.sources.len()&&candidate.sources.iter().all(|source|source.approved_for_ai),"summary.candidate.source_permission");
    for source in &candidate.sources {local::identifier(&source.source_id)?;local::text(&source.approval_reference,1024)?;
        ensure!(source.size_bytes<=1024*1024&&source.sha256.len()==64&&source.sha256.bytes().all(|byte|byte.is_ascii_digit()||(b'a'..=b'f').contains(&byte)),"summary.candidate.source_identity");}
    let mut seen=BTreeSet::new();let mut lengths=BTreeMap::<&str,usize>::new();let mut ends=BTreeMap::<&str,usize>::new();let mut total=0;
    for slice in &candidate.slices {
        let source=sources.get(slice.source_id.as_str()).context("summary.candidate.source_missing")?;
        ensure!(seen.insert(&slice.evidence_id)&&source.artifact_id==slice.artifact_id&&source.sha256==slice.source_sha256
            &&slice.start>=*ends.get(slice.source_id.as_str()).unwrap_or(&0)&&slice.start<slice.end&&slice.end<=source.size_bytes
            &&slice.end-slice.start==slice.text.len()&&slice.text.len()<=8192&&local::sha256(slice.text.as_bytes())==slice.text_sha256
            &&slice.evidence_id==evidence_id(&slice.source_id,&slice.source_sha256,slice.start,slice.end),"summary.candidate.slice_binding");
        local::text(&slice.text,8192)?;*lengths.entry(slice.source_id.as_str()).or_default()+=slice.text.len();ends.insert(&slice.source_id,slice.end);total+=slice.text.len();
    }
    ensure!(total<=32768&&candidate.sources.iter().all(|source|source.selected_bytes==*lengths.get(source.source_id.as_str()).unwrap_or(&0)
        &&source.selected_bytes.checked_add(source.unselected_bytes)==Some(source.size_bytes)),"summary.candidate.coverage");
    validate_body(&candidate.body,&candidate.slices)?;
    ensure!(candidate.candidate_id==candidate_id(candidate)?,"summary.candidate.identity");Ok(())
}

/// Optional bounded local observer. It receives no prompts, text or paths. An
/// exporter failure is reported separately and cannot change candidate identity.
pub trait SummaryObserver {fn report(&self,event:&Value)->Result<()>;}

pub fn generate(prepared:&PreparedSummary,profile:&RuntimeProfile,catalog:&AiModelCatalog,store:&ArtifactStore,
    token:ProcessCancellationToken,observer:Option<&dyn SummaryObserver>)->Result<SummaryOutcome> {
    generate_inner(prepared,profile,catalog,store,token,observer,None)
}
/// Embedding/test port for an explicitly supplied implementation of the existing
/// AiProvider trait; the selected profile/identity ceilings still apply.
pub fn generate_with_provider(prepared:&PreparedSummary,profile:&RuntimeProfile,catalog:&AiModelCatalog,store:&ArtifactStore,
    token:ProcessCancellationToken,observer:Option<&dyn SummaryObserver>,provider:&dyn AiProvider)->Result<SummaryOutcome> {
    generate_inner(prepared,profile,catalog,store,token,observer,Some(provider))
}
fn generate_inner(prepared:&PreparedSummary,profile:&RuntimeProfile,catalog:&AiModelCatalog,store:&ArtifactStore,
    token:ProcessCancellationToken,observer:Option<&dyn SummaryObserver>,provided:Option<&dyn AiProvider>)->Result<SummaryOutcome> {
    let started=Instant::now();let plan=prepared.plan(profile,catalog)?;
    let nonce=(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(),INVOCATIONS.fetch_add(1,Ordering::Relaxed));
    let mut result=SummaryOutcome {schema_version:"renderflow.summary-outcome/v1".into(),status:SummaryStatus::Unavailable,diagnostic_code:"summary.unavailable".into(),plan,
        candidate:None,artifact:None,execution:None,operation:OperationalReport {invocation_id:format!("invocation-{}",local::sha256(&serde_json::to_vec(&nonce)?)),
            duration_ms:0,attempt_ceiling:1,observed_attempts:Some(0),input_tokens:None,output_tokens:None,cost_microunits:None,cost_status:"unknown".into(),telemetry_status:"not_configured".into()}};
    if profile.mode==RuntimeMode::Disabled {result.status=SummaryStatus::Disabled;result.diagnostic_code="summary.disabled".into();}
    else if token.is_cancelled() {result.status=SummaryStatus::Cancelled;result.diagnostic_code="summary.cancelled".into();}
    else if result.plan.profile.status!=ProfileStatus::DeclaredReady {
        result.status=if matches!(result.plan.profile.status,ProfileStatus::Refused|ProfileStatus::Incompatible){SummaryStatus::Refused}else{SummaryStatus::Unavailable};
        result.diagnostic_code=result.plan.profile.diagnostic_code.clone();
    }
    else if prepared.sources.iter().any(|source|!source.approved_for_ai)||(profile.mode==RuntimeMode::Remote&&prepared.sources.iter().any(|source|!source.privacy_approved_for_remote)) {
        result.status=SummaryStatus::Refused;result.diagnostic_code="summary.source_or_remote_permission_required".into();
    } else if serde_json::to_vec(&prepared.input)?.len()>profile.budgets.max_input_bytes {
        result.status=SummaryStatus::Refused;result.diagnostic_code="summary.input_budget".into();
    } else {
        for artifact in &prepared.artifacts {store.verify(artifact)?;}
        let binding=profile.bind(catalog,&skill()?)?.context("summary.binding_changed")?;
        let owned=if provided.is_none(){Some(binding.provider()?)}else{None};
        let provider:&dyn AiProvider=match provided {Some(provider)=>provider,None=>owned.as_deref().context("summary.provider_missing")?};
        ensure!(provider.name()==binding.catalog().providers[0].adapter,"summary.adapter_identity");
        let mut registry=AiSkillRegistry::default();registry.insert(binding.skill().clone())?;
        let runtime=AiSkillRuntime::new(binding.catalog(),&registry,vec![provider]).with_execution_context(binding.context(token.clone()));
        let request=AiSkillExecutionRequest {skill_id:SKILL_ID.into(),skill_version:Some(SKILL_VERSION.into()),input:prepared.input.clone(),
            input_artifacts:prepared.artifacts.iter().map(|artifact|AiInputArtifactEvidence {artifact_id:artifact.id().to_string(),digest:artifact.digest().value().into(),media_type:artifact.media_type().to_string(),approved_for_ai:true}).collect(),
            preference:if profile.mode==RuntimeMode::Remote {AiExecutionPreferenceV1::RemoteOnly}else{AiExecutionPreferenceV1::LocalOnly},allow_remote:profile.mode==RuntimeMode::Remote,
            allow_unverified:false,source_approved:true,privacy_approved_for_remote:profile.mode==RuntimeMode::Remote,additional_protected_references:Vec::new()};
        result.operation.observed_attempts=None;
        match runtime.execute(&request,store) {
            Err(_)=> {result.status=if token.is_cancelled(){SummaryStatus::Cancelled}else{SummaryStatus::Failed};result.diagnostic_code="summary.runtime_failed_or_cancelled".into();},
            Ok(outcome)=> {
                result.operation.observed_attempts=Some(1);result.operation.input_tokens=outcome.evidence.usage.input_tokens;result.operation.output_tokens=outcome.evidence.usage.output_tokens;
                let checked=(||->Result<(SummaryCandidate,Artifact)> {
                    cancelled(&token)?;store.verify(&outcome.artifact)?;let bytes=store.read_bytes(&outcome.artifact)?;
                    ensure!(outcome.evidence.output_artifact_id==outcome.artifact.id().to_string()&&outcome.evidence.output_digest.value==local::sha256(&bytes)
                        &&outcome.evidence.input_digest.value==local::sha256(&serde_json::to_vec(&request.input)?)
                        &&outcome.evidence.skill_id==SKILL_ID&&outcome.evidence.skill_version==SKILL_VERSION
                        &&outcome.evidence.skill_digest.value==local::sha256(&serde_json::to_vec(binding.skill())?)
                        &&outcome.evidence.output_state==AiCandidateState::Candidate&&outcome.evidence.approval_required,"summary.native_identity");
                    let body:SummaryBody=crate::evidence::json::object(&bytes,profile.budgets.max_output_bytes)?;validate_body(&body,&prepared.slices)?;
                    let native=&outcome.evidence;
                    let mut candidate=SummaryCandidate {schema_version:CANDIDATE_SCHEMA.into(),candidate_id:String::new(),recipe_sha256:prepared.recipe_sha256.clone(),body,
                        sources:prepared.sources.clone(),slices:prepared.slices.clone(),processing:ProcessingIdentity {profile_sha256:profile.digest()?,skill_id:SKILL_ID.into(),skill_version:SKILL_VERSION.into(),
                            skill_sha256:native.skill_digest.value.clone(),input_sha256:native.input_digest.value.clone(),output_schema_sha256:native.output_schema_digest.value.clone(),
                            settings_sha256:native.settings_digest.value.clone(),hygiene_sha256:native.hygiene_policy_digest.value.clone(),cache_identity:native.cache_identity.value.clone(),model:native.identity.clone()},
                        state:AiCandidateState::Candidate,review_required:true,grounding:"references_checked_semantics_unverified".into(),coverage_note:COVERAGE.into(),
                        reference_validator:"validator.summary.references/v1".into()};
                    candidate.candidate_id=candidate_id(&candidate)?;validate_candidate(&candidate)?;
                    let bytes=serde_json::to_vec_pretty(&candidate)?;ensure!(bytes.len()<=131072,"summary.candidate.byte_limit");cancelled(&token)?;
                    let artifact=store.put_bytes(&bytes,ArtifactDescriptor::for_format(Format::Json,ArtifactStorageClass::Intermediate)
                        .with_sources(prepared.artifacts.iter().map(|artifact|artifact.id().clone()).chain(std::iter::once(outcome.artifact.id().clone())))
                        .with_metadata("renderflow.ai.output_state","candidate").with_metadata("renderflow.ai.review_required",true)
                        .with_metadata("renderflow.summary.candidate_id",candidate.candidate_id.clone()))?;store.verify(&artifact)?;Ok((candidate,artifact))
                })();
                result.execution=Some(outcome.evidence);
                match checked {Ok((candidate,artifact))=>{result.status=SummaryStatus::AwaitingReview;result.diagnostic_code="summary.review_required".into();result.candidate=Some(candidate);result.artifact=Some(artifact);},
                    Err(_)=>{result.status=if token.is_cancelled(){SummaryStatus::Cancelled}else{SummaryStatus::Invalid};result.diagnostic_code="summary.reference_or_contract_invalid".into();}}
            },
        }
    }
    result.operation.duration_ms=started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    if let Some(observer)=observer {
        let event=json!({"event":"publication_summary","invocation_id":result.operation.invocation_id,"profile_sha256":result.plan.profile.profile_sha256,
            "status":result.status,"duration_ms":result.operation.duration_ms,"observed_attempts":result.operation.observed_attempts,
            "input_tokens":result.operation.input_tokens,"output_tokens":result.operation.output_tokens,"cost_status":"unknown"});
        result.operation.telemetry_status=if observer.report(&event).is_ok(){"delivered_to_observer"}else{"observer_failed"}.into();
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
