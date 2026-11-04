//! Shared local publication I/O. No provider, job scheduler or approval engine.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::{ensure, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::evidence::files::{self, BundleRoot, ExpectedFile};
use crate::evidence::verification::VerificationLimits;
use crate::process::ProcessCancellationToken;

pub(crate) const MAX_JSON: usize = 2 * 1024 * 1024;
pub(crate) const MAX_TOTAL: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSubject {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

pub(crate) fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

pub(crate) fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    ensure!(bytes.len() <= MAX_JSON, "publication.json.limit");
    ensure!(serde_json::from_slice::<Value>(bytes)?.is_object(), "publication.json.object");
    // Deserialize original bytes, retaining serde's duplicate struct-field refusal.
    serde_json::from_slice(bytes).context("publication.json.contract")
}

pub(crate) fn read_file(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path).context("publication.file.metadata")?;
    ensure!(before.is_file() && !before.file_type().is_symlink() && before.len() <= maximum as u64,
        "publication.file: expected a bounded regular file");
    let mut file = File::open(path)?;
    ensure!(files::same_identity(&before, &file.metadata()?, true), "publication.file.changed");
    let mut bytes = Vec::new();
    (&mut file).take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= maximum && bytes.len() as u64 == before.len()
        && files::same_identity(&before, &file.metadata()?, true)
        && files::same_identity(&before, &fs::symlink_metadata(path)?, true), "publication.file.changed");
    Ok(bytes)
}

pub(crate) struct Capture {
    root: BundleRoot,
    limits: VerificationLimits,
    started: Instant,
    token: ProcessCancellationToken,
    total: u64,
    count: usize,
}

impl Capture {
    pub(crate) fn new(root: &Path, token: ProcessCancellationToken) -> Result<Self> {
        let limits = limits();
        let started = Instant::now();
        files::check_budget(started, &limits, &token)?;
        Ok(Self {root: BundleRoot::new(root)?, limits, started, token, total: 0, count: 0})
    }

    pub(crate) fn read(&mut self, relative: &str, maximum: usize) -> Result<Vec<u8>> {
        self.check()?;
        self.count += 1;
        ensure!(self.count <= 256, "publication.inputs.count");
        let path = self.root.resolve(relative, &self.limits)?;
        let size = fs::symlink_metadata(&path)?.len();
        self.total = self.total.checked_add(size).context("publication.inputs.overflow")?;
        ensure!(self.total <= MAX_TOTAL, "publication.inputs.bytes");
        let bytes = read_file(&path, maximum)?;
        self.root.recheck()?;
        self.check()?;
        Ok(bytes)
    }

    pub(crate) fn check(&self) -> Result<()> {
        files::check_budget(self.started, &self.limits, &self.token)?;
        Ok(())
    }
}

fn limits() -> VerificationLimits {
    VerificationLimits {max_file_bytes: MAX_TOTAL, max_total_bytes: MAX_TOTAL,
        max_publications: 256, ..VerificationLimits::default()}
}

/// Writes only caller-owned bytes into a new directory. Re-observe payloads
/// before the last receipt; failures may leave an unaccepted partial directory.
pub(crate) fn publish(output: &Path, files: &BTreeMap<String, Vec<u8>>, receipt_name: &str,
    mut receipt: Value, token: ProcessCancellationToken) -> Result<Value> {
    let limits = limits();
    let started = Instant::now();
    files::check_budget(started, &limits, &token)?;
    ensure!(receipt.is_object() && !files.contains_key(receipt_name), "publication.receipt.shape");
    ensure!(files.len() <= 256, "publication.bundle.count");
    let mut total = 0_u64;
    for (path, bytes) in files {
        files::components(path, limits.max_path_components)?;
        total = total.checked_add(bytes.len() as u64).context("publication.bundle.overflow")?;
    }
    ensure!(total <= MAX_TOTAL, "publication.bundle.bytes");
    files::components(receipt_name, limits.max_path_components)?;
    let parent = output.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    ensure!(parent.is_dir(), "publication.output.parent: must exist");
    fs::create_dir(output).context("publication.output: must not exist")?;
    for (relative, bytes) in files {
        files::check_budget(started, &limits, &token)?;
        let path = output.join(relative);
        fs::create_dir_all(path.parent().context("publication.output.path")?)?;
        write_new(&path, bytes)?;
    }
    let root = BundleRoot::new(output)?;
    let mut observed = Vec::new();
    for (relative, bytes) in files {
        let digest = sha256(bytes);
        let subject = files::observe(&root, relative,
            ExpectedFile {size_bytes: bytes.len() as u64, sha256: &digest}, &limits,
            started, &token, &mut std::io::sink())?;
        observed.push(FileSubject {path: relative.clone(), sha256: subject.sha256, size_bytes: subject.size_bytes});
    }
    receipt["files"] = serde_json::to_value(observed)?;
    let bytes = serde_json::to_vec_pretty(&receipt)?;
    ensure!(bytes.len() <= MAX_JSON, "publication.receipt.limit");
    root.recheck()?;
    files::check_budget(started, &limits, &token)?;
    let path = output.join(receipt_name);
    fs::create_dir_all(path.parent().context("publication.receipt.parent")?)?;
    write_new(&path, &bytes)?;
    Ok(receipt)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

pub(crate) fn identifier(value: &str) -> Result<()> {
    ensure!(!value.is_empty() && value.len() <= 128 && value.bytes().all(|ch|
        ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b'.')), "publication.id");
    Ok(())
}

pub(crate) fn text(value: &str, maximum: usize) -> Result<()> {
    ensure!(!value.trim().is_empty() && value.len() <= maximum && !value.chars().any(|ch|
        (ch.is_control() && ch != '\n' && ch != '\t') || matches!(ch, '\u{fffe}' | '\u{ffff}')), "publication.text");
    Ok(())
}

pub(crate) fn language(value: &str) -> Result<()> {
    ensure!(value.len() <= 64 && value.split('-').next().is_some_and(|part|
        (2..=8).contains(&part.len()) && part.bytes().all(|ch| ch.is_ascii_alphabetic()))
        && value.split('-').all(|part| !part.is_empty() && part.bytes().all(|ch| ch.is_ascii_alphanumeric())),
        "publication.language");
    Ok(())
}

pub(crate) fn url(value: &str) -> Result<()> {
    text(value, 4096)?;
    ensure!((value.starts_with("https://") || value.starts_with("http://")) && value.len() <= 4096
        && !value.chars().any(|ch| ch.is_whitespace() || ch.is_control()
            || matches!(ch, '<' | '>' | '"' | '\'' | '\\' | '`' | '{' | '}')), "publication.url");
    let authority = value.split_once("://").map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or("")).unwrap_or("");
    ensure!(!authority.is_empty() && !authority.contains('@') && !authority.starts_with(':'), "publication.url.authority");
    Ok(())
}

pub(crate) fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&#39;")
}
