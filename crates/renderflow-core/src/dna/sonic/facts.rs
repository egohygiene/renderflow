use std::collections::BTreeMap;
use anyhow::{ensure,Result};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use crate::dna::{ArtifactDna,DnaDeterminism,DnaEvidenceOrigin};
use crate::publication::local;

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Confidence {
    Calibrated {score:f64},
    Uncalibrated {score:f64},
    Unavailable {reason:String},
    NotApplicable,
}

impl Confidence {
    pub(super) fn validate(&self,class:DnaDeterminism)->Result<()> {
        match self {
            Self::Calibrated {score}|Self::Uncalibrated {score}=>ensure!(score.is_finite() && (0.0..=1.0).contains(score),"sonic.confidence.score"),
            Self::Unavailable {reason}=>local::text(reason,4096)?,
            Self::NotApplicable=>ensure!(class==DnaDeterminism::Deterministic,"sonic.confidence.class"),
        }
        Ok(())
    }
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SonicFact {
    pub id:String,
    pub dimension:String,
    pub value:Value,
    pub origin:DnaEvidenceOrigin,
    pub class:DnaDeterminism,
    pub confidence:Confidence,
    /// Digest/JSON-pointer references, never paths to follow or implicit authority.
    pub evidence:Value,
    pub correction_of:Option<String>,
}

pub(super) fn collect(dna:&ArtifactDna)->Result<Vec<SonicFact>> {
    let mut facts=Vec::new();
    for observation in &dna.observations {
        if !observation.dimension.starts_with("sonic.") {continue;}
        facts.push(SonicFact {id:observation.id.clone(),dimension:observation.dimension.clone(),value:observation.value.clone(),
            origin:observation.evidence.origin,class:observation.evidence.determinism,confidence:Confidence::NotApplicable,
            evidence:json!({"source_sha256":dna.source.digest.value,"kind":"structural_pcm_observation"}),correction_of:None});
    }
    if let Some(extension)=dna.provider_extensions.get("renderflow.sonic") {
        if let Some(rows)=extension.get("facts") {
            let extra:Vec<SonicFact>=serde_json::from_value(rows.clone())?;
            facts.extend(extra);
        }
    }
    ensure!(facts.len()<=512,"sonic.facts.count");
    let mut ids=BTreeMap::new();
    for fact in &facts {ensure!(ids.insert(fact.id.as_str(),()).is_none(),"sonic.fact.duplicate");}
    Ok(facts)
}
