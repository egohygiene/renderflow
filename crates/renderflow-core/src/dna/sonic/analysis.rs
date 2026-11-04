//! A bounded projection of a pinned public Aniflow contract, not its complete
//! native semantic validator. No executable or arbitrary metadata path is used.

use std::collections::{BTreeMap,BTreeSet};
use anyhow::{ensure,Context,Result};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use crate::dna::{DnaDeterminism,DnaEvidenceOrigin};
use crate::evidence::files;
use crate::publication::local;
use crate::speech::provider::PcmWavInspection;
use super::facts::{Confidence,SonicFact};

pub const CONTRACT_PIN:&str="8008e0176f45b34d2a9e74d8a26d9b3fd374b81f";

#[derive(Debug,Clone,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArtifactRef {pub id:String,pub sha256:String,pub byte_size:u64}
#[derive(Debug,Clone,Deserialize)]
#[serde(deny_unknown_fields)]
struct Origin {numerator:u64,denominator:u64}
#[derive(Debug,Clone,Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {artifact:ArtifactRef,stream_index:u32,sample_rate_hz:u32,channels:u32,frame_count:u64,origin:Origin,stem:Option<Value>}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderRef {id:String,version:String}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Provider {id:String,provider:ProviderRef,implementation_sha256:String,configuration_sha256:String,tools:Vec<Value>,models:Value,license:Value}
#[derive(Debug,Clone,Deserialize)]
#[serde(deny_unknown_fields)]
struct Capability {capability:ProviderRef,status:String,provider_evidence_ids:Vec<String>,evidence_artifact_ids:Vec<String>,diagnostic_ids:Vec<String>}
#[derive(Debug,Clone,Deserialize)]
#[serde(deny_unknown_fields)]
struct Scope {channels:Vec<u32>,stem_id:Option<String>}
#[derive(Debug,Clone,Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {class:DnaDeterminism,confidence:Confidence,provider_evidence_id:String,evidence_artifact_ids:Vec<String>}
#[derive(Debug,Clone,Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {id:String,capability_id:String,kind:String,value:Value,scope:Scope,provenance:Provenance}
#[derive(Debug,Clone,Deserialize)]
#[serde(deny_unknown_fields)]
struct Analysis {schema:String,status:String,source:Source,artifacts:Vec<ArtifactRef>,providers:Vec<Provider>,
    capabilities:Vec<Capability>,observations:Vec<Observation>,timelines:Vec<Value>,semantic_artifacts:Vec<Value>,
    excerpts:Vec<Value>,diagnostics:Vec<Value>,extensions:BTreeMap<String,Value>}

pub(super) struct Projection {
    pub status:String,
    pub facts:Vec<SonicFact>,
    pub unprojected:Value,
}

fn status(value:&str)->Result<()> {
    ensure!(matches!(value,"complete"|"partial"|"unavailable"|"unsupported"|"cancelled"|"failed"),"sonic.aniflow.status");
    Ok(())
}

pub(super) fn project(bytes:&[u8],source_sha256:&str,source_size:u64,
    pcm:PcmWavInspection,retained:&BTreeMap<String,(String,u64)>)->Result<Projection> {
    let value:Analysis=local::parse(bytes)?;
    ensure!(value.schema=="aniflow.audio-analysis/v1","sonic.aniflow.schema");
    status(&value.status)?;
    ensure!(value.source.artifact.sha256==source_sha256 && value.source.artifact.byte_size==source_size,
        "sonic.aniflow.source.changed");
    ensure!(value.source.stream_index==0 && value.source.sample_rate_hz==16000 && value.source.channels==1
        && value.source.frame_count==pcm.frame_count && value.source.origin.numerator==0
        && value.source.origin.denominator==1 && value.source.stem.is_none(),"sonic.aniflow.source.clock: only the selected original zero-origin PCM stream is admitted");
    for count in [value.artifacts.len(),value.providers.len(),value.capabilities.len(),value.observations.len(),
        value.timelines.len(),value.semantic_artifacts.len(),value.excerpts.len(),value.diagnostics.len(),value.extensions.len()] {
        ensure!(count<=256,"sonic.aniflow.count");
    }
    ensure!(!value.capabilities.is_empty(),"sonic.aniflow.capabilities.empty");
    let mut artifacts=BTreeMap::new();
    local::identifier(&value.source.artifact.id)?;
    artifacts.insert(value.source.artifact.id.as_str(),(source_sha256,source_size));
    ensure!(value.artifacts.len()==retained.len(),"sonic.aniflow.evidence.membership");
    for artifact in &value.artifacts {
        local::identifier(&artifact.id)?; files::digest(&artifact.sha256)?;
        ensure!(artifacts.insert(&artifact.id,(&artifact.sha256,artifact.byte_size)).is_none(),"sonic.aniflow.artifact.duplicate");
        ensure!(retained.get(&artifact.id)==Some(&(artifact.sha256.clone(),artifact.byte_size)),"sonic.aniflow.evidence.changed");
    }
    let mut providers=BTreeMap::new();
    for (index,provider) in value.providers.iter().enumerate() {
        local::identifier(&provider.id)?; local::identifier(&provider.provider.id)?; local::text(&provider.provider.version,128)?;
        files::digest(&provider.implementation_sha256)?; files::digest(&provider.configuration_sha256)?;
        ensure!(provider.tools.len()<=32 && provider.models.is_object() && provider.license.is_object(),"sonic.aniflow.provider.shape");
        // Pin large tool/model/license declarations once, rather than cloning
        // unbounded opaque metadata into every observation.
        let summary=json!({"id":provider.provider.id,"version":provider.provider.version,
            "implementation_sha256":provider.implementation_sha256,"configuration_sha256":provider.configuration_sha256,
            "tools_sha256":local::sha256(&serde_json::to_vec(&provider.tools)?),
            "models_sha256":local::sha256(&serde_json::to_vec(&provider.models)?),
            "license_sha256":local::sha256(&serde_json::to_vec(&provider.license)?),
            "manifest_pointer":format!("/providers/{index}"),"authority":"source_reported"});
        ensure!(providers.insert(provider.id.as_str(),summary).is_none(),"sonic.aniflow.provider.duplicate");
    }
    let mut diagnostics=BTreeSet::new();
    for diagnostic in &value.diagnostics {
        let id=diagnostic.get("id").and_then(Value::as_str).context("sonic.aniflow.diagnostic.id")?;
        local::identifier(id)?; ensure!(diagnostics.insert(id),"sonic.aniflow.diagnostic.duplicate");
    }
    let mut capabilities=BTreeMap::new();
    for capability in &value.capabilities {
        status(&capability.status)?;
        local::text(&capability.capability.id,128)?; local::text(&capability.capability.version,128)?;
        refs(&capability.provider_evidence_ids,|id|providers.contains_key(id))?;
        refs(&capability.evidence_artifact_ids,|id|artifacts.contains_key(id))?;
        refs(&capability.diagnostic_ids,|id|diagnostics.contains(id))?;
        ensure!(capabilities.insert(capability.capability.id.as_str(),capability).is_none(),"sonic.aniflow.capability.duplicate");
    }
    let mut seen=BTreeSet::new();
    let mut facts=Vec::new();
    let manifest_sha256=local::sha256(bytes);
    for (index,observation) in value.observations.iter().enumerate() {
        local::identifier(&observation.id)?;
        ensure!(seen.insert(&observation.id),"sonic.aniflow.observation.duplicate");
        let capability=capabilities.get(observation.capability_id.as_str()).context("sonic.aniflow.observation.capability")?;
        ensure!(matches!(capability.status.as_str(),"complete"|"partial"),"sonic.aniflow.unobserved.capability");
        let provider=providers.get(observation.provenance.provider_evidence_id.as_str()).context("sonic.aniflow.observation.provider")?;
        ensure!(capability.provider_evidence_ids.contains(&observation.provenance.provider_evidence_id),"sonic.aniflow.provider.membership");
        ensure!(!observation.provenance.evidence_artifact_ids.is_empty(),"sonic.aniflow.evidence.empty");
        refs(&observation.provenance.evidence_artifact_ids,|id|
            artifacts.contains_key(id) && capability.evidence_artifact_ids.iter().any(|candidate|candidate==id))?;
        ensure!(observation.scope.channels==[0] && observation.scope.stem_id.is_none(),"sonic.aniflow.observation.scope");
        observation.provenance.confidence.validate(observation.provenance.class)?;
        let (dimension,expected_capability)=match observation.kind.as_str() {
            "sample_rate"=>("sonic.technical.sample_rate_hz","aniflow/audio-technical-inspection"),
            "channel_count"=>("sonic.technical.channels","aniflow/audio-technical-inspection"),
            "tempo"=>("sonic.rhythm.tempo_bpm","aniflow/audio-musical-structure"),
            "key"=>("sonic.harmony.key","aniflow/audio-musical-structure"),
            _=>anyhow::bail!("sonic.aniflow.observation.kind"),
        };
        ensure!(observation.capability_id==expected_capability,"sonic.aniflow.observation.kind_capability");
        let scalar=observation.value.get("value").context("sonic.aniflow.observation.value")?.clone();
        validate_value(&observation.kind,&observation.value,observation.provenance.class)?;
        facts.push(SonicFact {id:format!("analysis.{manifest_sha256}.observation.{index}"),dimension:dimension.into(),value:scalar,
            origin:DnaEvidenceOrigin::SourceReported,class:observation.provenance.class,
            confidence:observation.provenance.confidence.clone(),
            evidence:json!({"manifest_sha256":manifest_sha256,"pointer":format!("/observations/{index}"),
                "native_observation_id":observation.id,"provider_declaration":provider,
                "provider_authentication":"not_established","native_validation":"bounded_projection_only",
                "evidence_artifacts":observation.provenance.evidence_artifact_ids.iter().map(|id|{
                    let (sha256,size)=artifacts[id.as_str()];json!({"sha256":sha256,"size_bytes":size})}).collect::<Vec<_>>()}),
            correction_of:None});
    }
    Ok(Projection {status:value.status,facts,unprojected:json!({"timelines":value.timelines.len(),
        "semantic_artifacts":value.semantic_artifacts.len(),"excerpts":value.excerpts.len(),
        "diagnostics":value.diagnostics.len(),"extensions":value.extensions.len(),
        "scope":"Counts only; native temporal/semantic payloads remain outside this projection."})})
}

fn refs(ids:&[String],exists:impl Fn(&str)->bool)->Result<()> {
    ensure!(ids.len()<=256,"sonic.aniflow.refs.count");
    let mut unique=BTreeSet::new();
    for id in ids {local::identifier(id)?;ensure!(unique.insert(id) && exists(id),"sonic.aniflow.reference");}
    Ok(())
}

fn validate_value(kind:&str,value:&Value,class:DnaDeterminism)->Result<()> {
    let object=value.as_object().context("sonic.aniflow.value.object")?;
    let expected=if kind=="key" {vec!["kind","value"]} else {vec!["kind","unit","value"]};
    ensure!(object.keys().map(String::as_str).collect::<BTreeSet<_>>()==expected.into_iter().collect(),"sonic.aniflow.value.fields");
    match kind {
        "sample_rate"=>ensure!(value["kind"]=="quantity" && value["unit"]=="hertz" && value["value"].as_f64()==Some(16000.0)
            && class==DnaDeterminism::Deterministic,"sonic.aniflow.sample_rate"),
        "channel_count"=>ensure!(value["kind"]=="count" && value["unit"]=="channels" && value["value"].as_u64()==Some(1)
            && class==DnaDeterminism::Deterministic,"sonic.aniflow.channel_count"),
        "tempo"=>ensure!(value["kind"]=="quantity" && value["unit"]=="beats_per_minute"
            && value["value"].as_f64().is_some_and(|number|number>0.0 && number<=1000.0)
            && class!=DnaDeterminism::Deterministic,"sonic.aniflow.tempo"),
        "key"=>{ensure!(value["kind"]=="label" && class!=DnaDeterminism::Deterministic,"sonic.aniflow.key");local::text(value["value"].as_str().context("sonic.aniflow.key.value")?,256)?;},
        _=>anyhow::bail!("sonic.aniflow.kind"),
    }
    Ok(())
}
