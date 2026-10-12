//! Bounded installed-package snapshots for the afterwriting adapter.
//!
//! The digest covers sorted regular-file records serialized as compact JSON:
//! `[{"path":"relative/posix/path","size_bytes":N,"sha256":"lowercase"}]`.
//! Empty directories are immaterial. Root `.git` and every `node_modules/.bin`
//! directory are excluded, including their contents. Nothing else is omitted.
//! This identifies supplied bytes; it does not attest dependency closure or builds.

use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::process::ProcessCancellationToken;
use super::afterwriting::AfterwritingError;

type Checked<T> = Result<T, AfterwritingError>;

pub const PACKAGE_TREE_ALGORITHM: &str = "renderflow.package-files-json-sha256/v1";
pub const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PACKAGE_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PACKAGE_ENTRIES: usize = 40_000;
pub const MAX_PACKAGE_DEPTH: usize = 32;
pub const FILE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageFingerprint {
    pub algorithm: String,
    pub tree_sha256: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct TreeEntry { path: String, size_bytes: u64, sha256: String }

#[derive(Debug)]
pub(super) struct TreeSnapshot {
    original_root: PathBuf,
    pub root: PathBuf,
    root_metadata: Metadata,
    files: Vec<FileSnapshot>,
    entries: Vec<TreeEntry>,
    pub fingerprint: PackageFingerprint,
}

pub fn package_fingerprint(root: &Path, cancellation: ProcessCancellationToken) -> Checked<PackageFingerprint> {
    Ok(TreeSnapshot::read(root, &cancellation)?.fingerprint)
}

impl TreeSnapshot {
    pub fn read(root: &Path, cancellation: &ProcessCancellationToken) -> Checked<Self> {
        check_cancelled(cancellation)?;
        absolute_text(root)?;
        let root_metadata = directory_metadata(root)?;
        let original_root = root.to_owned();
        let root = fs::canonicalize(root)?;
        if !same_identity(&root_metadata, &directory_metadata(&root)?) {
            return Err(changed("package root changed while resolving"));
        }
        let started = Instant::now();
        let mut paths = Vec::new();
        let mut entry_count = 0;
        walk(&root, &root, 0, &mut paths, &mut entry_count, started, cancellation)?;
        paths.sort_by(|left, right| left.0.cmp(&right.0));
        let mut files = Vec::new();
        let mut entries = Vec::new();
        let mut total_bytes = 0_u64;
        for (relative, path) in paths {
            check_budget(started, cancellation)?;
            let snapshot = FileSnapshot::read(&path, MAX_PACKAGE_FILE_BYTES, false, started, cancellation)?;
            total_bytes = total_bytes.checked_add(snapshot.metadata.len())
                .ok_or_else(|| invalid("package byte count overflow"))?;
            if total_bytes > MAX_PACKAGE_BYTES { return Err(invalid("installed package exceeds 512 MiB")); }
            entries.push(TreeEntry { path: relative, size_bytes: snapshot.metadata.len(), sha256: snapshot.sha256.clone() });
            files.push(snapshot);
        }
        if !same_identity(&root_metadata, &directory_metadata(&original_root)?) {
            return Err(changed("package root changed while snapshotting"));
        }
        let fingerprint = PackageFingerprint {
            algorithm: PACKAGE_TREE_ALGORITHM.to_owned(), tree_sha256: digest(&serde_json::to_vec(&entries)?),
            file_count: entries.len(), total_bytes,
        };
        Ok(Self { original_root, root, root_metadata, files, entries, fingerprint })
    }

    pub fn recheck(&self, cancellation: &ProcessCancellationToken) -> Checked<()> {
        if !same_identity(&self.root_metadata, &directory_metadata(&self.original_root)?) {
            return Err(changed("installed package root was replaced"));
        }
        let current = Self::read(&self.original_root, cancellation)?;
        if current.entries != self.entries || current.files.len() != self.files.len()
            || current.files.iter().zip(&self.files).any(|(left, right)| !same_metadata(&left.metadata, &right.metadata)) {
            return Err(changed("installed package files changed"));
        }
        Ok(())
    }

    pub fn stage(&self, destination: &Path, cancellation: &ProcessCancellationToken) -> Checked<Self> {
        check_cancelled(cancellation)?;
        fs::create_dir(destination)?;
        let started = Instant::now();
        for (entry, source) in self.entries.iter().zip(&self.files) {
            check_budget(started, cancellation)?;
            let bytes = source.bytes(started, cancellation)?;
            let target = destination.join(&entry.path);
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(target)?;
            std::io::Write::write_all(&mut file, &bytes)?;
        }
        self.recheck(cancellation)?;
        let staged = Self::read(destination, cancellation)?;
        if staged.fingerprint != self.fingerprint { return Err(changed("staged package differs from the pinned package")); }
        Ok(staged)
    }

    pub fn file_bytes(&self, relative: &str, cancellation: &ProcessCancellationToken) -> Checked<Vec<u8>> {
        let index = self.entries.binary_search_by(|entry| entry.path.as_str().cmp(relative))
            .map_err(|_| AfterwritingError::ProviderUnavailable(format!("installed package is missing {relative}")))?;
        self.files[index].bytes(Instant::now(), cancellation)
    }

    pub fn has_file(&self, relative: &str) -> bool {
        self.entries.binary_search_by(|entry| entry.path.as_str().cmp(relative)).is_ok()
    }
}

fn walk(root: &Path, directory: &Path, depth: usize, paths: &mut Vec<(String, PathBuf)>, count: &mut usize,
    started: Instant, cancellation: &ProcessCancellationToken) -> Checked<()> {
    if depth > MAX_PACKAGE_DEPTH { return Err(invalid("installed package exceeds depth 32")); }
    check_budget(started, cancellation)?;
    let before = directory_metadata(directory)?;
    for entry in fs::read_dir(directory)? {
        check_budget(started, cancellation)?;
        *count += 1;
        if *count > MAX_PACKAGE_ENTRIES { return Err(invalid("installed package exceeds 40000 entries")); }
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|_| invalid("package member escaped root"))?;
        let text = relative_text(relative)?;
        if text == ".git" || text == "node_modules/.bin" || text.ends_with("/node_modules/.bin") { continue; }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() { return Err(invalid("installed package contains a symlink")); }
        if metadata.is_dir() { walk(root, &path, depth + 1, paths, count, started, cancellation)?; }
        else if metadata.is_file() { paths.push((text, path)); }
        else { return Err(invalid("installed package contains a non-regular member")); }
    }
    if !same_metadata(&before, &directory_metadata(directory)?) { return Err(changed("package directory changed while listing")); }
    Ok(())
}

fn relative_text(path: &Path) -> Checked<String> {
    let mut output = Vec::new();
    for component in path.components() {
        let Component::Normal(name) = component else { return Err(invalid("package path contains an absolute or escaping component")); };
        let text = name.to_str().ok_or_else(|| invalid("package paths must be UTF-8"))?;
        if text.is_empty() || text.chars().any(char::is_control) || text.contains(['\\', ':']) {
            return Err(invalid("package member name contains controls or a platform-dependent separator"));
        }
        output.push(text);
    }
    if output.is_empty() { return Err(invalid("package member path is empty")); }
    Ok(output.join("/"))
}

pub(super) fn absolute_text(path: &Path) -> Checked<&str> {
    let text = path.to_str().ok_or_else(|| invalid("native paths must be UTF-8"))?;
    if !path.is_absolute() || text.chars().any(char::is_control) {
        return Err(invalid("configured paths must be absolute without control characters"));
    }
    Ok(text)
}

pub(super) fn expected_digest(value: &str, field: &str) -> Checked<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(invalid(&format!("{field} must be a lowercase SHA-256 digest")));
    }
    Ok(())
}

pub(super) fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
pub(super) fn check_cancelled(cancellation: &ProcessCancellationToken) -> Checked<()> {
    if cancellation.is_cancelled() { Err(AfterwritingError::Cancelled) } else { Ok(()) }
}
fn check_budget(started: Instant, cancellation: &ProcessCancellationToken) -> Checked<()> {
    check_cancelled(cancellation)?;
    if started.elapsed() >= FILE_TIMEOUT { return Err(AfterwritingError::TimedOut); }
    Ok(())
}
fn invalid(message: &str) -> AfterwritingError { AfterwritingError::InvalidRequest(message.to_owned()) }
fn changed(message: &str) -> AfterwritingError { AfterwritingError::SourceChanged(message.to_owned()) }
fn directory_metadata(path: &Path) -> Checked<Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() { return Err(invalid("package root/member must be a non-symlink directory")); }
    Ok(metadata)
}
fn regular_metadata(path: &Path) -> Checked<Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() { return Err(invalid("file must be regular with a non-symlink final component")); }
    Ok(metadata)
}
fn same_identity(left: &Metadata, right: &Metadata) -> bool {
    if left.is_file() != right.is_file() || left.is_dir() != right.is_dir() { return false; }
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev() || left.ino() != right.ino() { return false; }
    }
    true
}
fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    if !same_identity(left, right) || left.len() != right.len() || left.modified().ok() != right.modified().ok() { return false; }
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        if left.ctime() != right.ctime() || left.ctime_nsec() != right.ctime_nsec() { return false; }
    }
    true
}

#[derive(Debug)]
pub(super) struct FileSnapshot {
    original: PathBuf,
    pub path: PathBuf,
    maximum: u64,
    nonempty: bool,
    metadata: Metadata,
    pub sha256: String,
}
impl FileSnapshot {
    pub fn read(path: &Path, maximum: u64, nonempty: bool, started: Instant, cancellation: &ProcessCancellationToken) -> Checked<Self> {
        check_budget(started, cancellation)?;
        let metadata = regular_metadata(path)?;
        let original = path.to_owned();
        let path = fs::canonicalize(path)?;
        absolute_text(&path)?;
        let (observed, sha256, _) = read_file(&path, maximum, nonempty, false, started, cancellation)?;
        if !same_metadata(&metadata, &observed) { return Err(changed("file changed while resolving its path")); }
        Ok(Self { original, path, maximum, nonempty, metadata, sha256 })
    }
    pub fn bytes(&self, started: Instant, cancellation: &ProcessCancellationToken) -> Checked<Vec<u8>> {
        if !same_metadata(&self.metadata, &regular_metadata(&self.original)?) { return Err(changed("original file path was replaced")); }
        let (metadata, sha256, bytes) = read_file(&self.path, self.maximum, self.nonempty, true, started, cancellation)?;
        if sha256 != self.sha256 || !same_metadata(&metadata, &self.metadata) { return Err(changed("file content changed")); }
        Ok(bytes)
    }
    pub fn recheck(&self, cancellation: &ProcessCancellationToken) -> Checked<()> {
        if !same_metadata(&self.metadata, &regular_metadata(&self.original)?) { return Err(changed("original file path was replaced")); }
        let (metadata, sha256, _) = read_file(&self.path, self.maximum, self.nonempty, false, Instant::now(), cancellation)?;
        if sha256 != self.sha256 || !same_metadata(&metadata, &self.metadata) { return Err(changed("file content changed")); }
        Ok(())
    }
}
fn read_file(path: &Path, maximum: u64, nonempty: bool, retain: bool, started: Instant,
    cancellation: &ProcessCancellationToken) -> Checked<(Metadata, String, Vec<u8>)> {
    check_budget(started, cancellation)?;
    let before = regular_metadata(path)?;
    if before.len() > maximum || (nonempty && before.len() == 0) { return Err(invalid("file does not meet the bounded nonempty/size policy")); }
    let mut file = File::open(path)?;
    if !same_metadata(&before, &file.metadata()?) { return Err(changed("file changed while opening")); }
    let mut hash = Sha256::new();
    let mut bytes = Vec::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_budget(started, cancellation)?;
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        total += count as u64;
        if total > maximum { return Err(changed("file grew beyond the configured bound")); }
        hash.update(&buffer[..count]);
        if retain { bytes.extend_from_slice(&buffer[..count]); }
    }
    check_budget(started, cancellation)?;
    if total != before.len() || !same_metadata(&before, &file.metadata()?) || !same_metadata(&before, &regular_metadata(path)?) {
        return Err(changed("file changed while reading"));
    }
    Ok((before, format!("{:x}", hash.finalize()), bytes))
}
