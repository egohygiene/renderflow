//! Authored/unrun profile and shared-runtime regressions; no live endpoint.
use super::*;
use crate::ai::{AiCapabilities,AiModel,AiRequest,AiResponse,AiSkillExecutionRequest,AiSkillRegistry,AiSkillRuntime};
use std::sync::atomic::{AtomicUsize,Ordering};
use serde_json::json;

pub(super) fn profile()->RuntimeProfile {RuntimeProfile {
    schema_version:PROFILE_SCHEMA.into(),id:"synthetic.local".into(),mode:RuntimeMode::LocalOnly,
    provider_id:"provider.ollama".into(),model_id:"llama3.2".into(),endpoint:"http://127.0.0.1:11434".into(),credential_env:None,
    local_service_egress_disabled:true,preparation_reference:Some("synthetic-preparation-only".into()),allowed_reported_models:Vec::new(),
    generation:generation(),budgets:ProfileBudgets::default(),
}}
pub(super) fn catalog()->AiModelCatalog {
    let mut catalog=AiModelCatalog::bundled().unwrap();let provider=&mut catalog.providers[0];
    provider.runtime.revision=Some("synthetic-runtime".into());let model=&mut provider.models[0];
    model.availability=AiAvailability::Available;model.weights_digest=Some("a".repeat(64));
    model.limits.max_input_bytes=Some(131072);model.limits.output_tokens=Some(4096);catalog
}
fn skill()->AiSkillSpec {AiSkillRegistry::bundled().unwrap().get("skill.metadata.extract",None).unwrap().clone()}

#[test]
fn disabled_and_unprepared_profiles_never_produce_an_executable_binding() {
    let mut p=profile();p.mode=RuntimeMode::Disabled;
    assert_eq!(p.inspect(&catalog(),&skill()).unwrap().status,ProfileStatus::Disabled);assert!(p.bind(&catalog(),&skill()).unwrap().is_none());
    p.mode=RuntimeMode::LocalOnly;let original=AiModelCatalog::bundled().unwrap();
    assert_eq!(p.inspect(&original,&skill()).unwrap().status,ProfileStatus::Unavailable);assert!(p.bind(&original,&skill()).unwrap().is_none());
}
#[test]
fn literal_loopback_and_egress_assertion_are_required_without_url_or_secret_exposure() {
    for endpoint in ["http://example.invalid:11434","http://127.0.0.1:11434/?key=secret","http://user:secret@127.0.0.1:11434","http://0.0.0.0:11434"] {
        let mut p=profile();p.endpoint=endpoint.into();assert!(p.validate().is_err());
    }
    let mut p=profile();p.local_service_egress_disabled=false;assert!(p.validate().is_err());
    p=profile();p.credential_env=Some("MODEL_TOKEN".into());let report=p.inspect(&catalog(),&skill()).unwrap();
    assert_eq!(report.status,ProfileStatus::Incompatible);assert!(!serde_json::to_string(&report).unwrap().contains("127.0.0.1"));
}
#[test]
fn missing_selected_model_never_falls_back_to_another_model_or_remote_provider() {
    let mut p=profile();p.model_id="not.prepared".into();let report=p.inspect(&catalog(),&skill()).unwrap();
    assert_eq!(report.status,ProfileStatus::Unavailable);assert!(report.resolution.is_none());
    p=profile();let binding=p.bind(&catalog(),&skill()).unwrap().unwrap();
    assert_eq!(binding.catalog().providers.len(),1);assert_eq!(binding.catalog().providers[0].models.len(),1);
    assert_eq!(binding.skill().budgets.max_retries,0);
}
#[test]
fn capability_limits_settings_and_unsupported_cost_policies_are_prelaunch_refusals() {
    let p=profile();let mut c=catalog();c.providers[0].models[0].features.json_schema=false;
    assert_eq!(p.inspect(&c,&skill()).unwrap().status,ProfileStatus::Incompatible);
    c=catalog();c.providers[0].models[0].limits.output_tokens=Some(1);assert_eq!(p.inspect(&c,&skill()).unwrap().status,ProfileStatus::Incompatible);
    let mut p=profile();p.budgets.max_cost_microunits=Some(1);assert_eq!(p.inspect(&catalog(),&skill()).unwrap().status,ProfileStatus::Refused);
}
#[test]
fn endpoint_settings_and_profile_changes_bind_processing_identity() {
    let p=profile();let original=p.digest().unwrap();let mut changed=p.clone();changed.endpoint="http://127.0.0.1:11435".into();
    assert_ne!(original,changed.digest().unwrap());changed=p.clone();changed.generation.max_tokens=Some(512);assert_ne!(original,changed.digest().unwrap());
    let report=p.inspect(&catalog(),&skill()).unwrap();assert_eq!(report.status,ProfileStatus::DeclaredReady);assert!(!report.runtime_qualified);assert_eq!(report.cost_status,"unknown");
}

#[derive(Debug)]
struct Fake {calls:AtomicUsize,reported:Option<String>}
impl AiProvider for Fake {
    fn name(&self)->&str {"ollama"} fn is_local(&self)->bool {true}
    fn capabilities(&self)->AiCapabilities {AiCapabilities::new()} fn models(&self)->Vec<AiModel> {Vec::new()}
    fn execute(&self,request:&AiRequest)->Result<AiResponse> {
        self.calls.fetch_add(1,Ordering::SeqCst);
        assert!(request.max_response_bytes.is_some()&&request.cancellation.is_some());
        let mut response=AiResponse::new(r#"{"title":"Synthetic","summary":"A small booklet.","keywords":[],"review_required":true}"#,&request.model,"ollama");
        response.reported_model=self.reported.clone();Ok(response)
    }
}
fn request()->AiSkillExecutionRequest {AiSkillExecutionRequest {
    skill_id:"skill.metadata.extract".into(),skill_version:Some("1.0.0".into()),input:json!({"source_text":"A small synthetic booklet."}),input_artifacts:Vec::new(),
    preference:AiExecutionPreferenceV1::LocalOnly,allow_remote:false,allow_unverified:false,source_approved:true,
    privacy_approved_for_remote:false,additional_protected_references:Vec::new(),
}}
#[test]
fn runtime_cancellation_and_unexpected_reported_identity_cannot_produce_a_candidate() {
    let p=profile();let binding=p.bind(&catalog(),&skill()).unwrap().unwrap();let mut registry=AiSkillRegistry::default();registry.insert(binding.skill().clone()).unwrap();
    let fake=Fake {calls:AtomicUsize::new(0),reported:Some("different-model".into())};
    let token=ProcessCancellationToken::new();token.cancel();
    let runtime=AiSkillRuntime::new(binding.catalog(),&registry,vec![&fake]).with_execution_context(binding.context(token));
    let directory=tempfile::tempdir().unwrap();let store=crate::artifact::ArtifactStore::new(directory.path()).unwrap();
    assert!(runtime.execute(&request(),&store).is_err());assert_eq!(fake.calls.load(Ordering::SeqCst),0);
    let runtime=AiSkillRuntime::new(binding.catalog(),&registry,vec![&fake]).with_execution_context(binding.context(ProcessCancellationToken::new()));
    assert!(runtime.execute(&request(),&store).is_err());assert_eq!(fake.calls.load(Ordering::SeqCst),1);
}
#[test]
fn runtime_cache_identity_includes_profile_endpoint_without_operational_timing() {
    let fake=Fake {calls:AtomicUsize::new(0),reported:Some("llama3.2".into())};let p=profile();let c=catalog();let base=skill();
    let first=p.bind(&c,&base).unwrap().unwrap();let mut registry=AiSkillRegistry::default();registry.insert(first.skill().clone()).unwrap();
    let directory=tempfile::tempdir().unwrap();let store=crate::artifact::ArtifactStore::new(directory.path()).unwrap();
    let runtime=AiSkillRuntime::new(first.catalog(),&registry,vec![&fake]).with_execution_context(first.context(ProcessCancellationToken::new()));
    let one=runtime.execute(&request(),&store).unwrap();let two=runtime.execute(&request(),&store).unwrap();assert_eq!(one.evidence.cache_identity,two.evidence.cache_identity);
    let mut changed=p;changed.endpoint="http://127.0.0.1:11435".into();let second=changed.bind(&c,&base).unwrap().unwrap();
    let runtime=AiSkillRuntime::new(second.catalog(),&registry,vec![&fake]).with_execution_context(second.context(ProcessCancellationToken::new()));
    let three=runtime.execute(&request(),&store).unwrap();assert_ne!(one.evidence.cache_identity,three.evidence.cache_identity);
    assert_eq!(three.evidence.identity.reported_model.as_deref(),Some("llama3.2"));assert_eq!(three.evidence.usage.cost_microunits,None);
}
