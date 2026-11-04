//! Check inventory membership for lineage and known v1 retained-artifact refs.
//! Unknown extension metadata stays opaque; no path strings are followed here.

use std::collections::{BTreeMap,BTreeSet};
use serde_json::Value;
use crate::artifact::Artifact;
use crate::evidence::{ArtifactEvidence,RunManifest};
use super::{PortableError,Result};

pub(super) fn check(manifest:&RunManifest)->Result<BTreeSet<String>> {
    let mut required=BTreeSet::new();
    let mut required_digests=BTreeSet::new();
    let mut ids=BTreeMap::<&str,&ArtifactEvidence>::new();
    let mut digests=BTreeMap::<(&str,&str),BTreeSet<&str>>::new();
    for record in &manifest.artifact_manifest.artifacts {
        if record.artifact_id.is_empty() {return Err(invalid("empty inventory identity"));}
        if let Some(previous)=ids.insert(&record.artifact_id,record) {
            if previous.digest!=record.digest || previous.size_bytes!=record.size_bytes
                || previous.format!=record.format || previous.media_type!=record.media_type
                || previous.sources!=record.sources {
                return Err(invalid("duplicate inventory identity has contradictory byte/lineage fields"));
            }
        }
        digests.entry((&record.format,&record.digest.value)).or_default().insert(&record.artifact_id);
    }
    for record in &manifest.artifact_manifest.artifacts {
        for parent in &record.sources {
            if !ids.contains_key(parent.as_str()) {return Err(invalid("inventory lineage references an absent record"));}
        }
        for (key,expected) in [
            ("renderflow.screenplay_body","renderflow.screenplay-body-artifact/v1"),
            ("renderflow.screenplay_recovery","renderflow.screenplay-recovery-artifact/v1"),
            ("renderflow.subtitle","renderflow.subtitle-artifact/v1"),
            ("renderflow.speech","renderflow.speech-artifact/v1"),
            ("renderflow.lyrics","renderflow.lyrics-artifact/v1"),
        ] {
            let Some(value)=record.metadata.get(key) else {continue};
            if value.get("schema_version").and_then(Value::as_str)!=Some(expected) {
                return Err(invalid("unsupported retained-reference metadata schema"));
            }
            match key {
                "renderflow.screenplay_body"=>{
                    full(field(value,"original_source")?,&ids,false,&mut required)?;
                    let canonical=full(field(value,"canonical_artifact")?,&ids,true,&mut required)?;
                    if canonical.format().as_str()!="screenplay_json" || canonical.digest().value()!=string(value,"canonical_sha256")? {
                        return Err(invalid("body canonical kind/digest contradicts its declared companion"));
                    }
                }
                "renderflow.screenplay_recovery"=>{
                    full(field(value,"original_source")?,&ids,false,&mut required)?;
                    let canonical=reference(string(value,"canonical_artifact_id")?,string(value,"canonical_sha256")?,&ids)?;
                    if canonical.format!="screenplay_json" {return Err(invalid("recovery canonical reference has the wrong kind"));}
                    required.insert(string(value,"canonical_artifact_id")?.to_string());
                }
                "renderflow.subtitle"|"renderflow.speech"=>{
                    let canonical=field(value,"canonical_transcript")?;
                    let record=reference(string(canonical,"artifact_id")?,string(canonical,"sha256")?,&ids)?;
                    if record.format!="timed_transcript_json" {return Err(invalid("transcript canonical reference has the wrong kind"));}
                    required.insert(string(canonical,"artifact_id")?.to_string());
                    if key=="renderflow.speech" {
                        for name in ["provider_output","transcription"] {full(field(value,name)?,&ids,true,&mut required)?;}
                        for name in ["audio_probe","prepared_audio"] {
                            if let Some(subject)=value.get(name).filter(|subject|!subject.is_null()) {full(subject,&ids,true,&mut required)?;}
                        }
                    }
                }
                "renderflow.lyrics"=>{
                    let digest=string(value,"canonical_sha256")?;
                    if !digests.contains_key(&("lyrics_json",digest)) {
                        return Err(invalid("lyrics canonical companion is absent from inventory"));
                    }
                    required_digests.insert(("lyrics_json",digest));
                }
                _=>unreachable!(),
            }
        }
    }
    for key in required_digests {
        if let Some(ids)=digests.get(&key) {required.extend(ids.iter().map(|id|id.to_string()));}
    }
    Ok(required)
}

fn full(value:&Value,ids:&BTreeMap<&str,&ArtifactEvidence>,retain:bool,required:&mut BTreeSet<String>)->Result<Artifact> {
    if !value.is_object() {return Err(invalid("embedded retained artifact must be an object"));}
    let subject:Artifact=serde_json::from_value(value.clone())?;
    let record=ids.get(subject.id().as_str()).ok_or_else(||invalid("embedded retained artifact is absent from inventory"))?;
    reference(subject.id().as_str(),subject.digest().value(),ids)?;
    if subject.size_bytes()!=record.size_bytes || subject.format().as_str()!=record.format
        || subject.media_type().to_string()!=record.media_type
        || !subject.sources().iter().map(|id|id.as_str()).eq(record.sources.iter().map(String::as_str))
        || subject.payload().relative_path()!=std::path::Path::new(&super::object_path(subject.digest().value())?) {
        return Err(invalid("embedded retained artifact disagrees with inventory/payload identity"));
    }
    if retain {required.insert(subject.id().as_str().to_string());}
    Ok(subject)
}

fn reference<'a>(id:&str,digest:&str,ids:&BTreeMap<&str,&'a ArtifactEvidence>)->Result<&'a ArtifactEvidence> {
    let record=*ids.get(id).ok_or_else(||invalid("retained canonical reference is absent from inventory"))?;
    if record.digest.algorithm!="sha256" || record.digest.value!=digest {
        return Err(invalid("retained reference and inventory digest disagree"));
    }
    Ok(record)
}

fn field<'a>(value:&'a Value,name:&str)->Result<&'a Value> {
    value.get(name).ok_or_else(||invalid("known retained-reference field is missing"))
}
fn string<'a>(value:&'a Value,name:&str)->Result<&'a str> {
    field(value,name)?.as_str().ok_or_else(||invalid("known retained reference is not text"))
}
fn invalid(reason:&'static str)->PortableError {PortableError::Invalid {reason}}
