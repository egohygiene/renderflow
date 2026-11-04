//! File-backed publication verification, layered over the existing native join.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};

use crate::process::ProcessCancellationToken;
use super::{PublishedTarget,PublishedTargetsError,RunManifest};
use super::files::{self,BundleRoot};
pub use super::files::FileObservation;

pub const VERIFICATION_SCHEMA:&str="renderflow.publication-verification/v1";

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationLimits {
    pub max_manifest_bytes:u64,
    pub max_publications:usize,
    pub max_file_bytes:u64,
    pub max_total_bytes:u64,
    pub max_path_components:usize,
    pub timeout_seconds:u64,
}

impl Default for VerificationLimits {
    fn default()->Self {Self {max_manifest_bytes:16*1024*1024,max_publications:8192,
        max_file_bytes:2*1024*1024*1024,max_total_bytes:8*1024*1024*1024,
        max_path_components:64,timeout_seconds:120}}
}

impl VerificationLimits {
    pub fn validate(&self)->Result<(),VerificationError> {
        if self.max_manifest_bytes==0 || self.max_manifest_bytes>128*1024*1024
            || self.max_publications==0 || self.max_publications>100_000
            || self.max_file_bytes==0 || self.max_total_bytes==0
            || self.max_path_components==0 || self.max_path_components>128
            || self.timeout_seconds==0 || self.timeout_seconds>3600 {
            return Err(VerificationError::Limit {resource:"verification configuration"});
        }
        Ok(())
    }
}

#[derive(Debug,thiserror::Error)]
#[non_exhaustive]
pub enum VerificationError {
    #[error(transparent)]
    Publications(#[from] PublishedTargetsError),
    #[error("verification limit exceeded or invalid: {resource}")]
    Limit {resource:&'static str},
    #[error("invalid bundle path: {reason}")]
    Path {reason:&'static str},
    #[error("publication integrity check failed: {field}")]
    Integrity {field:&'static str},
    #[error("subject changed while observed: {subject}")]
    Changed {subject:&'static str},
    #[error("unsupported payload digest algorithm")]
    UnsupportedDigest,
    #[error("verification cancelled")]
    Cancelled,
    #[error("verification cooperative deadline exceeded")]
    Deadline,
    #[error("cannot {operation}: {source}")]
    Io {operation:&'static str,#[source] source:std::io::Error},
    #[error("invalid native run receipt: {0}")]
    Manifest(#[from] serde_json::Error),
}

/// Constructed only by the verifier. This records an observation at one time;
/// it grants no mutation, signature, complete-run, fidelity or review authority.
#[derive(Debug)]
pub struct VerifiedPublication<'a> {
    publication:PublishedTarget<'a>,
    observation:FileObservation,
}

impl<'a> VerifiedPublication<'a> {
    pub fn publication(&self)->PublishedTarget<'a> {self.publication}
    pub fn observation(&self)->&FileObservation {&self.observation}
}

/// Own the exact selected receipt bytes alongside its parsed native model.
/// The caller's digest identifies bytes, not an authenticated publisher.
pub struct PinnedRunReceipt {
    pub(crate) bytes:Vec<u8>,
    pub(crate) manifest:RunManifest,
    sha256:String,
}

impl PinnedRunReceipt {
    pub fn read(path:&Path,expected_sha256:&str,limits:&VerificationLimits,
        cancellation:ProcessCancellationToken)->Result<Self,VerificationError> {
        limits.validate()?;
        files::digest(expected_sha256)?;
        let started=Instant::now();
        files::check_budget(started,limits,&cancellation)?;
        let metadata=fs::symlink_metadata(path).map_err(files::io("inspect run receipt"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len()>limits.max_manifest_bytes {
            return Err(VerificationError::Limit {resource:"regular receipt bytes"});
        }
        let mut bytes=Vec::new();
        fs::File::open(path).map_err(files::io("open run receipt"))?
            .take(limits.max_manifest_bytes+1).read_to_end(&mut bytes).map_err(files::io("read run receipt"))?;
        if bytes.len() as u64>limits.max_manifest_bytes {return Err(VerificationError::Limit {resource:"receipt bytes"});}
        let value=Self::from_bytes(bytes,expected_sha256,limits)?;
        files::check_budget(started,limits,&cancellation)?;
        Ok(value)
    }

    pub(crate) fn from_bytes(bytes:Vec<u8>,expected_sha256:&str,limits:&VerificationLimits)->Result<Self,VerificationError> {
        limits.validate()?;
        files::digest(expected_sha256)?;
        if bytes.len() as u64>limits.max_manifest_bytes {return Err(VerificationError::Limit {resource:"receipt bytes"});}
        let sha256=format!("{:x}",Sha256::digest(&bytes));
        if sha256!=expected_sha256 {return Err(VerificationError::Integrity {field:"selected receipt SHA-256"});}
        if !serde_json::from_slice::<Value>(&bytes)?.is_object() {
            return Err(VerificationError::Integrity {field:"native receipt must be a JSON object"});
        }
        let manifest=serde_json::from_slice(&bytes)?;
        Ok(Self {bytes,manifest,sha256})
    }

    pub fn manifest(&self)->&RunManifest {&self.manifest}
    pub fn sha256(&self)->&str {&self.sha256}
    pub fn bytes(&self)->&[u8] {&self.bytes}

    pub fn verify(&self,root:&Path,limits:&VerificationLimits,cancellation:ProcessCancellationToken)->Result<Value,VerificationError> {
        let subjects=verify(&self.manifest,root,limits,cancellation)?;
        Ok(report(&self.manifest,Some(&self.sha256),&subjects))
    }
}

pub fn verify<'a>(manifest:&'a RunManifest,root:&Path,limits:&VerificationLimits,
    cancellation:ProcessCancellationToken)->Result<Vec<VerifiedPublication<'a>>,VerificationError> {
    limits.validate()?;
    let started=Instant::now();
    files::check_budget(started,limits,&cancellation)?;
    let publications=manifest.published_targets()?;
    if publications.len()>limits.max_publications {return Err(VerificationError::Limit {resource:"publication count"});}
    let root=BundleRoot::new(root)?;
    let mut total=0_u64;
    let mut result=Vec::new();
    for publication in publications {
        let record=publication.artifact;
        if record.artifact_id.is_empty() {return Err(VerificationError::Integrity {field:"publication artifact identity"});}
        if record.digest.algorithm!="sha256" {return Err(VerificationError::UnsupportedDigest);}
        total=total.checked_add(record.size_bytes).ok_or(VerificationError::Limit {resource:"total bytes"})?;
        if total>limits.max_total_bytes {return Err(VerificationError::Limit {resource:"total bytes"});}
        let relative=record.locator.strip_prefix("bundle:").ok_or(VerificationError::Path {reason:"only bundle locators are verified"})?;
        let observation=files::observe(&root,relative,files::ExpectedFile {size_bytes:record.size_bytes,sha256:&record.digest.value},limits,started,
            &cancellation,&mut std::io::sink())?;
        result.push(VerifiedPublication {publication,observation});
    }
    root.recheck()?;
    files::check_budget(started,limits,&cancellation)?;
    Ok(result)
}

pub(crate) fn report(manifest:&RunManifest,receipt_sha256:Option<&str>,subjects:&[VerifiedPublication<'_>])->Value {
    json!({"schema_version":VERIFICATION_SCHEMA,"run_id":manifest.run_id,"run_state":manifest.state,
        "receipt_sha256":receipt_sha256,"verified_publication_count":subjects.len(),
        "has_publications":!subjects.is_empty(),"verified_publications":subjects.iter().map(|subject| {
            let publication=subject.publication();
            json!({"target_id":publication.branch.target_id,"artifact_id":publication.artifact.artifact_id,
                "locator":publication.artifact.locator,"sha256":subject.observation.sha256,
                "size_bytes":subject.observation.size_bytes,"validation":publication.artifact.validation,
                "fidelity":publication.artifact.fidelity,"source_identity":publication.execution.source_identity})
        }).collect::<Vec<_>>(),"scope":"Observed publication file bytes and receipt joins only; native metadata remains in the unchanged receipt.",
        "runtime_qualified":false,"publisher_authenticated":false,"creative_approval_inferred":false,
        "limits":"Cooperative I/O bounds and observed path/identity checks; not a sandbox against hostile concurrent filesystem changes or proof that later bytes remain unchanged."})
}
