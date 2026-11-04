//! Bounded observations of declared bundle files. No provider or mutation occurs.
//!
//! Path and file identity checks detect observed links and persistent changes.
//! They are not a descriptor-relative filesystem sandbox against a hostile
//! concurrent writer. Consumers must re-observe bytes when subsequently used.

use std::fs::{self,File,Metadata};
use std::io::{Read,Write};
use std::path::{Path,PathBuf};
use std::time::{Duration,Instant};

use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};

use crate::process::ProcessCancellationToken;
use super::verification::{VerificationError,VerificationLimits};

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileObservation {
    pub sha256:String,
    pub size_bytes:u64,
}

pub(crate) struct BundleRoot {
    requested:PathBuf,
    pub(crate) path:PathBuf,
    metadata:Metadata,
}

impl BundleRoot {
    pub(crate) fn new(root:&Path) -> Result<Self,VerificationError> {
        let metadata=fs::symlink_metadata(root).map_err(io("inspect bundle root"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(VerificationError::Path {reason:"bundle root must be a real directory"});
        }
        let path=fs::canonicalize(root).map_err(io("resolve bundle root"))?;
        let value=Self {requested:root.to_path_buf(),path,metadata};
        value.recheck()?;
        Ok(value)
    }

    pub(crate) fn recheck(&self) -> Result<(),VerificationError> {
        let now=fs::symlink_metadata(&self.requested).map_err(io("reinspect bundle root"))?;
        if now.file_type().is_symlink() || !now.is_dir()
            || fs::canonicalize(&self.requested).map_err(io("resolve bundle root again"))? != self.path
            || !same_identity(&self.metadata,&now,false) {
            return Err(VerificationError::Changed {subject:"bundle root"});
        }
        Ok(())
    }

    pub(crate) fn resolve(&self,relative:&str,limits:&VerificationLimits) -> Result<PathBuf,VerificationError> {
        components(relative,limits.max_path_components)?;
        self.recheck()?;
        let mut current=self.path.clone();
        let parts=relative.split('/').collect::<Vec<_>>();
        for (index,part) in parts.iter().enumerate() {
            current.push(part);
            let metadata=fs::symlink_metadata(&current).map_err(io("inspect bundle path"))?;
            if metadata.file_type().is_symlink()
                || (index+1<parts.len() && !metadata.is_dir())
                || (index+1==parts.len() && !metadata.is_file()) {
                return Err(VerificationError::Path {reason:"bundle components must be real directories followed by a regular file"});
            }
        }
        if !fs::canonicalize(&current).map_err(io("resolve bundle file"))?.starts_with(&self.path) {
            return Err(VerificationError::Path {reason:"resolved file is outside the selected bundle"});
        }
        Ok(current)
    }
}

/// A locator is a logical POSIX path, never a URL to decode or an OS absolute path.
pub(crate) fn components(relative:&str,maximum:usize) -> Result<(),VerificationError> {
    if relative.is_empty() || relative.len()>4096 || relative.contains(['\\',':','%','?','#'])
        || relative.chars().any(char::is_control) {
        return Err(VerificationError::Path {reason:"expected a bounded relative POSIX path without encoded or alternate separators"});
    }
    let parts=relative.split('/').collect::<Vec<_>>();
    if parts.len()>maximum || parts.iter().any(|part| part.is_empty() || *part=="." || *part=="..") {
        return Err(VerificationError::Path {reason:"empty, dot, parent or over-deep bundle path"});
    }
    Ok(())
}

pub(crate) fn io(operation:&'static str) -> impl FnOnce(std::io::Error)->VerificationError {
    move |source| VerificationError::Io {operation,source}
}

pub(crate) fn check_budget(started:Instant,limits:&VerificationLimits,cancellation:&ProcessCancellationToken) -> Result<(),VerificationError> {
    if cancellation.is_cancelled() {return Err(VerificationError::Cancelled);}
    if started.elapsed()>Duration::from_secs(limits.timeout_seconds) {return Err(VerificationError::Deadline);}
    Ok(())
}

/// Observe while optionally copying into a caller-owned new staging file. Never
/// publish a completion receipt unless the observation and destination both pass.
pub(crate) fn observe(
    root:&BundleRoot,relative:&str,expected_size:u64,expected_sha256:&str,
    limits:&VerificationLimits,started:Instant,cancellation:&ProcessCancellationToken,
    sink:&mut impl Write,
) -> Result<FileObservation,VerificationError> {
    check_budget(started,limits,cancellation)?;
    digest(expected_sha256)?;
    if expected_size>limits.max_file_bytes {return Err(VerificationError::Limit {resource:"file bytes"});}
    let path=root.resolve(relative,limits)?;
    let before=fs::symlink_metadata(&path).map_err(io("inspect payload"))?;
    if before.len()!=expected_size {return Err(VerificationError::Integrity {field:"recorded byte size"});}
    let mut file=File::open(&path).map_err(io("open payload"))?;
    let opened=file.metadata().map_err(io("inspect open payload"))?;
    if !same_identity(&before,&opened,true) {return Err(VerificationError::Changed {subject:"open payload"});}
    let mut hasher=Sha256::new();
    let mut size=0_u64;
    let mut buffer=[0_u8;64*1024];
    loop {
        check_budget(started,limits,cancellation)?;
        let count=file.read(&mut buffer).map_err(io("read payload"))?;
        if count==0 {break;}
        size=size.checked_add(count as u64).ok_or(VerificationError::Limit {resource:"file bytes"})?;
        if size>expected_size || size>limits.max_file_bytes {return Err(VerificationError::Integrity {field:"payload grew beyond recorded size"});}
        hasher.update(&buffer[..count]);
        sink.write_all(&buffer[..count]).map_err(io("write staged payload"))?;
    }
    let observation=FileObservation {sha256:format!("{:x}",hasher.finalize()),size_bytes:size};
    if size!=expected_size || observation.sha256!=expected_sha256 {return Err(VerificationError::Integrity {field:"payload size or SHA-256"});}
    let after=file.metadata().map_err(io("reinspect open payload"))?;
    let named=fs::symlink_metadata(&path).map_err(io("reinspect named payload"))?;
    if !same_identity(&before,&after,true) || !same_identity(&before,&named,true)
        || !named.is_file() || named.file_type().is_symlink() || root.resolve(relative,limits)?!=path {
        return Err(VerificationError::Changed {subject:"payload after read"});
    }
    root.recheck()?;
    check_budget(started,limits,cancellation)?;
    Ok(observation)
}

pub(crate) fn digest(value:&str) -> Result<(),VerificationError> {
    if value.len()!=64 || !value.bytes().all(|ch|ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch)) {
        return Err(VerificationError::Integrity {field:"lowercase SHA-256 declaration"});
    }
    Ok(())
}

fn same_identity(left:&Metadata,right:&Metadata,contents:bool) -> bool {
    if left.is_file()!=right.is_file() || left.is_dir()!=right.is_dir() || right.file_type().is_symlink() {return false;}
    if contents && (left.len()!=right.len() || left.modified().ok()!=right.modified().ok()) {return false;}
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        if left.dev()!=right.dev() || left.ino()!=right.ino() {return false;}
    }
    true
}
