//! One explicit deployment binding over the existing catalog and skill runtime.
//! This Renderflow-owned contract is a proposed Flow #80 mapping, not a new
//! provider router or a ratified suite interoperability specification.

use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use anyhow::{ensure,Context,Result};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use super::{AiAvailability,AiExecutionPreferenceV1,AiLocality,AiModelCatalog,AiProvider,AiResolutionReport,AiSkillSpec,OllamaProvider,OpenAiProvider};
use super::skill::AiSkillGeneration;
use super::runtime::AiExecutionContext;
use crate::process::ProcessCancellationToken;

pub const PROFILE_SCHEMA:&str="renderflow.ai-runtime-profile/v1";
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum RuntimeMode {#[default] Disabled,LocalOnly,Remote}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct ProfileBudgets {
    pub max_input_bytes:usize,pub max_output_bytes:usize,pub max_response_bytes:usize,
    pub max_output_tokens:u32,pub max_duration_ms:u64,
    pub max_cost_microunits:Option<u64>,pub allow_unknown_cost:bool,
}
impl Default for ProfileBudgets {
    fn default()->Self {Self {max_input_bytes:65536,max_output_bytes:16384,max_response_bytes:65536,
        max_output_tokens:1024,max_duration_ms:60000,max_cost_microunits:None,allow_unknown_cost:false}}
}
fn generation()->AiSkillGeneration {AiSkillGeneration {temperature:None,max_tokens:Some(1024),seed:None,top_p:None,stop:Vec::new(),provider_extensions:Default::default()}}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProfile {
    pub schema_version:String,pub id:String,
    #[serde(default)] pub mode:RuntimeMode,
    #[serde(default)] pub provider_id:String,
    #[serde(default)] pub model_id:String,
    #[serde(default)] pub endpoint:String,
    #[serde(default)] pub credential_env:Option<String>,
    /// Operator assertion about the local runtime's own remote/cloud features.
    /// It is not observed egress or weight attestation.
    #[serde(default)] pub local_service_egress_disabled:bool,
    #[serde(default)] pub preparation_reference:Option<String>,
    #[serde(default)] pub allowed_reported_models:Vec<String>,
    #[serde(default="generation")] pub generation:AiSkillGeneration,
    #[serde(default)] pub budgets:ProfileBudgets,
}

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ProfileStatus {Disabled,Unavailable,Incompatible,Refused,DeclaredReady}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePlan {
    pub schema_version:String,pub profile_id:String,pub profile_sha256:String,pub mode:RuntimeMode,
    pub status:ProfileStatus,pub diagnostic_code:String,pub resolution:Option<AiResolutionReport>,
    pub endpoint_identity:String,pub cost_status:String,pub runtime_qualified:bool,
}

pub struct RuntimeBinding {profile:RuntimeProfile,catalog:AiModelCatalog,skill:AiSkillSpec,plan:ProfilePlan}
impl RuntimeBinding {
    pub fn catalog(&self)->&AiModelCatalog {&self.catalog}
    pub fn skill(&self)->&AiSkillSpec {&self.skill}
    pub fn plan(&self)->&ProfilePlan {&self.plan}
    pub fn context(&self,token:ProcessCancellationToken)->AiExecutionContext {
        let mut models=self.profile.allowed_reported_models.clone();models.push(self.profile.model_id.clone());models.sort();models.dedup();
        AiExecutionContext {profile_sha256:self.plan.profile_sha256.clone(),endpoint_identity:self.plan.endpoint_identity.clone(),
            max_response_bytes:self.profile.budgets.max_response_bytes,allowed_reported_models:models,cancellation:token}
    }
    /// Construct the selected existing adapter only. No probe, model pull or
    /// inference occurs here; the caller must still explicitly execute a skill.
    pub fn provider(&self)->Result<Box<dyn AiProvider>> {
        let adapter=&self.catalog.providers[0].adapter;
        match adapter.as_str() {
            "ollama"=>Ok(Box::new(OllamaProvider::new(&self.profile.endpoint).with_models(vec![self.profile.model_id.clone()]))),
            "openai"=> {
                let mut provider=OpenAiProvider::new().with_endpoint(&self.profile.endpoint).with_models(vec![self.profile.model_id.clone()]);
                if let Some(handle)=&self.profile.credential_env {provider=provider.with_api_key_env(handle);}
                Ok(Box::new(provider))
            },
            _=>anyhow::bail!("ai.profile.adapter_not_supported"),
        }
    }
}

fn halt(mut plan:ProfilePlan,status:ProfileStatus,code:&str)->ProfilePlan {plan.status=status;plan.diagnostic_code=code.into();plan}
fn sha(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}
fn identifier(value:&str)->bool {!value.is_empty()&&value.len()<=128&&value.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'.'|b'-'|b'_'))}
fn weight_digest(value:&str)->bool {let value=value.strip_prefix("sha256:").unwrap_or(value);value.len()==64&&value.bytes().all(|byte|byte.is_ascii_digit()||(b'a'..=b'f').contains(&byte))}

pub fn load_catalog(path:Option<&Path>)->Result<AiModelCatalog> {
    let Some(path)=path else {return AiModelCatalog::bundled();};
    let metadata=std::fs::symlink_metadata(path).context("ai.profile.catalog_read")?;
    ensure!(metadata.is_file()&&!metadata.file_type().is_symlink()&&metadata.len()<=2*1024*1024,"ai.profile.catalog_limit");
    let mut bytes=Vec::new();std::fs::File::open(path)?.take(2*1024*1024+1).read_to_end(&mut bytes)?;
    let catalog:AiModelCatalog=crate::evidence::json::object(&bytes,2*1024*1024).context("ai.profile.catalog_contract")?;
    catalog.validate()?;Ok(catalog)
}

impl RuntimeProfile {
    pub fn load(path:impl AsRef<Path>)->Result<Self> {
        let path=path.as_ref();let metadata=std::fs::symlink_metadata(path).context("ai.profile.read")?;
        ensure!(metadata.is_file()&&!metadata.file_type().is_symlink()&&metadata.len()<=65536,"ai.profile.file_limit");
        let mut bytes=Vec::new();std::fs::File::open(path)?.take(65537).read_to_end(&mut bytes)?;
        let profile:Self=crate::evidence::json::object(&bytes,65536).context("ai.profile.json")?;profile.validate()?;Ok(profile)
    }
    pub fn digest(&self)->Result<String> {Ok(sha(&serde_json::to_vec(self)?))}
    pub fn validate(&self)->Result<()> {
        ensure!(self.schema_version==PROFILE_SCHEMA&&identifier(&self.id),"ai.profile.contract");
        let b=&self.budgets;
        ensure!((1..=131072).contains(&b.max_input_bytes)&&(1..=65536).contains(&b.max_output_bytes)
            && b.max_response_bytes>=b.max_output_bytes&&b.max_response_bytes<=1024*1024
            && (1..=4096).contains(&b.max_output_tokens)&&(1..=120000).contains(&b.max_duration_ms),"ai.profile.bounds");
        ensure!(self.generation.max_tokens.is_some_and(|tokens|tokens>0&&tokens<=b.max_output_tokens)
            && self.generation.provider_extensions.is_empty()&&self.generation.stop.len()<=16
            && self.generation.stop.iter().all(|value|!value.is_empty()&&value.len()<=256)
            && self.generation.temperature.is_none_or(|value|value.is_finite()&&(0.0..=2.0).contains(&value))
            && self.generation.top_p.is_none_or(|value|value.is_finite()&&(0.0..=1.0).contains(&value)),"ai.profile.settings");
        if self.mode==RuntimeMode::Disabled {return Ok(());}
        ensure!(identifier(&self.provider_id)&&identifier(&self.model_id),"ai.profile.selected_identity");
        ensure!(self.allowed_reported_models.len()<=8&&self.allowed_reported_models.iter().all(|value|!value.is_empty()&&value.len()<=128
            && value.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'.'|b'-'|b'_'|b':'|b'/'))),"ai.profile.reported_identity");
        if let Some(handle)=&self.credential_env {
            ensure!(!handle.is_empty()&&handle.len()<=128&&handle.bytes().all(|byte|byte.is_ascii_alphanumeric()||byte==b'_')
                && !handle.as_bytes()[0].is_ascii_digit(),"ai.profile.credential_handle");
        }
        let endpoint=self.endpoint.trim_end_matches('/');
        let (scheme,authority)=endpoint.split_once("://").context("ai.profile.endpoint")?;
        ensure!(!authority.is_empty()&&authority.len()<=255&&!authority.bytes().any(|byte|byte.is_ascii_whitespace())
            && !authority.contains(['@','/','?','#','\\','%']),"ai.profile.endpoint_authority");
        match self.mode {
            RuntimeMode::LocalOnly=> {
                let socket:SocketAddr=authority.parse().context("ai.profile.local_endpoint_requires_literal_loopback_and_port")?;
                ensure!(matches!(scheme,"http"|"https")&&socket.ip().is_loopback()&&socket.port()!=0
                    &&self.local_service_egress_disabled,"ai.profile.local_egress_policy");
            },
            RuntimeMode::Remote=> {
                ensure!(scheme=="https"&&authority.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'.'|b'-'|b':')),
                    "ai.profile.remote_https_endpoint");
                let host=if let Some((host,port))=authority.rsplit_once(':') {
                    ensure!(port.parse::<u16>().is_ok_and(|port|port!=0),"ai.profile.remote_port");host
                } else {authority};
                ensure!(!host.is_empty()&&host.split('.').all(|label|!label.is_empty()&&label.len()<=63
                    &&label.bytes().all(|byte|byte.is_ascii_alphanumeric()||byte==b'-')
                    &&label.as_bytes()[0].is_ascii_alphanumeric()&&label.as_bytes()[label.len()-1].is_ascii_alphanumeric()),"ai.profile.remote_host");
            },
            RuntimeMode::Disabled=>{},
        }
        Ok(())
    }

    fn scoped(&self,catalog:&AiModelCatalog)->Result<Option<AiModelCatalog>> {
        let Some(provider)=catalog.providers.iter().find(|provider|provider.id==self.provider_id) else{return Ok(None);};
        let Some(model)=provider.models.iter().find(|model|model.id==self.model_id) else{return Ok(None);};
        let mut provider=provider.clone();provider.models=vec![model.clone()];
        Ok(Some(AiModelCatalog {schema_version:catalog.schema_version.clone(),revision:catalog.revision.clone(),providers:vec![provider]}))
    }
    fn configured_skill(&self,skill:&AiSkillSpec)->Result<AiSkillSpec> {
        let mut skill=skill.clone();let b=&self.budgets;
        skill.generation=self.generation.clone();skill.budgets.max_input_bytes=b.max_input_bytes;
        skill.budgets.max_output_bytes=b.max_output_bytes;skill.budgets.max_tokens=b.max_output_tokens;
        skill.budgets.max_duration_ms=b.max_duration_ms;skill.budgets.max_retries=0;
        skill.budgets.network=self.mode==RuntimeMode::Remote;skill.budgets.remote_execution=self.mode==RuntimeMode::Remote;
        skill.budgets.max_cost_microunits=None;skill.validate()?;Ok(skill)
    }
    pub fn inspect(&self,catalog:&AiModelCatalog,skill:&AiSkillSpec)->Result<ProfilePlan> {
        self.validate()?;
        let mut plan=ProfilePlan {schema_version:"renderflow.ai-profile-plan/v1".into(),profile_id:self.id.clone(),profile_sha256:self.digest()?,mode:self.mode,
            status:ProfileStatus::Disabled,diagnostic_code:"ai.profile.disabled".into(),resolution:None,
            endpoint_identity:format!("endpoint:sha256:{}",sha(self.endpoint.trim_end_matches('/').as_bytes())),cost_status:"unknown".into(),runtime_qualified:false};
        if self.mode==RuntimeMode::Disabled {return Ok(plan);}
        catalog.validate()?;
        if self.budgets.max_cost_microunits.is_some() || (self.mode==RuntimeMode::Remote&&!self.budgets.allow_unknown_cost) {
            return Ok(halt(plan,ProfileStatus::Refused,"ai.profile.cost_policy_unsupported_or_unknown_not_allowed"));
        }
        let Some(scoped)=self.scoped(catalog)? else {return Ok(halt(plan,ProfileStatus::Unavailable,"ai.profile.identity_absent"));};
        let provider=&scoped.providers[0];let model=&provider.models[0];
        if !matches!(provider.adapter.as_str(),"ollama"|"openai") || (provider.adapter=="ollama"&&self.credential_env.is_some())
            || (self.mode==RuntimeMode::LocalOnly&&provider.locality!=AiLocality::Local)
            || (self.mode==RuntimeMode::Remote&&provider.locality!=AiLocality::Remote) {
            return Ok(halt(plan,ProfileStatus::Incompatible,"ai.profile.adapter_locality_or_credential_mismatch"));
        }
        if model.availability!=AiAvailability::Available {
            return Ok(halt(plan,ProfileStatus::Unavailable,"ai.profile.model_missing_or_unverified"));
        }
        if self.mode==RuntimeMode::LocalOnly&&(model.weights_digest.as_ref().is_none_or(|digest|!weight_digest(digest))
            || provider.runtime.revision.as_ref().is_none_or(|value|value.trim().is_empty())
            || self.preparation_reference.as_ref().is_none_or(|value|value.trim().is_empty()||value.len()>1024)) {
            return Ok(halt(plan,ProfileStatus::Unavailable,"ai.profile.local_preparation_evidence_missing"));
        }
        if !model.features.json_schema || self.generation.seed.is_some()&&!model.features.seed
            || (self.generation.temperature.is_some()||self.generation.top_p.is_some()||!self.generation.stop.is_empty())&&!model.features.sampler_controls
            || model.limits.max_input_bytes.is_none_or(|maximum|self.budgets.max_input_bytes as u64>maximum)
            || model.limits.output_tokens.is_none_or(|maximum|u64::from(self.budgets.max_output_tokens)>maximum) {
            return Ok(halt(plan,ProfileStatus::Incompatible,"ai.profile.capability_or_limits_unsupported"));
        }
        let skill=self.configured_skill(skill)?;
        let request=skill.resolution_request(if self.mode==RuntimeMode::LocalOnly {AiExecutionPreferenceV1::LocalOnly}else{AiExecutionPreferenceV1::RemoteOnly},
            self.mode==RuntimeMode::Remote,false);
        let resolution=scoped.resolve(&request);
        plan.status=if resolution.selected.as_ref().is_some_and(|selection|selection.execution_ready){ProfileStatus::DeclaredReady}else{ProfileStatus::Incompatible};
        plan.diagnostic_code=if plan.status==ProfileStatus::DeclaredReady {"ai.profile.catalog_declared_ready_not_attested"}else{"ai.profile.skill_incompatible"}.into();
        plan.resolution=Some(resolution);Ok(plan)
    }
    pub fn bind(&self,catalog:&AiModelCatalog,skill:&AiSkillSpec)->Result<Option<RuntimeBinding>> {
        let plan=self.inspect(catalog,skill)?;if plan.status!=ProfileStatus::DeclaredReady{return Ok(None);}
        Ok(Some(RuntimeBinding {profile:self.clone(),catalog:self.scoped(catalog)?.context("ai.profile.binding")?,skill:self.configured_skill(skill)?,plan}))
    }
}

#[cfg(test)]
mod tests;
