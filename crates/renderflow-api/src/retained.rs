//! Private published-file copies. Native receipts stay unchanged; rendering is
//! never replayed. Core owns publication joins and every source/delivery copy.
use super::*;
use renderflow::evidence::{verification::VerificationLimits, RunState};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
const SCHEMA: &str = "renderflow.api-completed-artifacts/v1";
const MAX_FILES: usize = 4096;
#[cfg(all(test, unix))]
#[path = "retained_tests.rs"]
mod tests;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Member {
    target: String,
    path: String,
    sha256: String,
    size: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Retained {
    schema: String,
    pub job_id: String,
    result_sha256: String,
    files: Vec<Member>,
}
fn invalid() -> ApiError {
    ApiError::new("artifact_cache_invalid","Retained published files or native bindings are changed, incomplete, unsupported or over budget; preserve them for reconciliation",422)
}
fn io() -> ApiError {
    ApiError::new(
        "artifact_cache_storage",
        "Published-file cache could not be durably committed; no rendering is replayed",
        503,
    )
}
fn job_id(id: &str) -> bool {
    id.strip_prefix("job-").is_some_and(crate::uploads::sha256)
}
fn native(bytes: &[u8], limits: &Limits) -> ApiResult<renderflow::ExecutionResult> {
    if bytes.len() > limits.result_bytes {
        return Err(invalid());
    }
    let output: JobOutput =
        renderflow::evidence::json::object(bytes, limits.result_bytes).map_err(|_| invalid())?;
    let JobOutput::Execute(result) = output else {
        return Err(invalid());
    };
    if result.run_manifest.state != RunState::Complete {
        return Err(invalid());
    }
    Ok(result)
}
fn result_digest(bytes: &[u8]) -> ApiResult<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    Ok(package::digest(
        &serde_json::to_vec(&value).map_err(|_| invalid())?,
    ))
}
fn descriptor(id: &str, bytes: &[u8], limits: &Limits) -> ApiResult<Retained> {
    if !job_id(id) {
        return Err(invalid());
    }
    let result = native(bytes, limits)?;
    let publications = result
        .run_manifest
        .published_targets()
        .map_err(|_| invalid())?;
    if publications.len() > MAX_FILES {
        return Err(invalid());
    }
    let mut members = Vec::new();
    let mut paths = BTreeMap::new();
    let mut folded = BTreeSet::new();
    for publication in publications {
        let artifact = publication.artifact;
        let relative = artifact
            .locator
            .strip_prefix("bundle:")
            .ok_or_else(invalid)?;
        package::relative(relative)?;
        if artifact.digest.algorithm != "sha256"
            || !crate::uploads::sha256(&artifact.digest.value)
            || artifact.size_bytes > limits.job_storage_bytes
        {
            return Err(invalid());
        }
        let identity = (artifact.digest.value.clone(), artifact.size_bytes);
        if let Some(prior) = paths.insert(relative.to_owned(), identity.clone()) {
            if prior != identity {
                return Err(invalid());
            }
        } else if !folded.insert(relative.to_ascii_lowercase()) {
            return Err(invalid());
        }
        members.push(Member {
            target: publication.branch.target_id.clone(),
            path: relative.into(),
            sha256: artifact.digest.value.clone(),
            size: artifact.size_bytes,
        });
    }
    members.sort_by(|a, b| (&a.path, &a.target).cmp(&(&b.path, &b.target)));
    let value = Retained {
        schema: SCHEMA.into(),
        job_id: id.into(),
        result_sha256: result_digest(bytes)?,
        files: members,
    };
    value.validate(limits)?;
    Ok(value)
}
impl Retained {
    pub(super) fn total_bytes(&self) -> ApiResult<u64> {
        let mut unique = BTreeMap::new();
        for member in &self.files {
            unique.insert(&member.path, member.size);
        }
        unique
            .into_values()
            .try_fold(0u64, |sum, n| sum.checked_add(n).ok_or_else(invalid))
    }
    fn validate(&self, limits: &Limits) -> ApiResult<()> {
        if self.schema != SCHEMA
            || !job_id(&self.job_id)
            || !crate::uploads::sha256(&self.result_sha256)
            || self.files.len() > MAX_FILES
            || self.total_bytes()? > limits.job_storage_bytes
        {
            return Err(invalid());
        }
        let mut seen = BTreeSet::new();
        let mut paths = BTreeMap::new();
        let mut folded = BTreeSet::new();
        for member in &self.files {
            package::relative(&member.path)?;
            if member.path.split('/').count() > 64 {
                return Err(invalid());
            }
            if member.target.is_empty()
                || member.target.len() > 4096
                || !seen.insert(&member.target)
                || !crate::uploads::sha256(&member.sha256)
            {
                return Err(invalid());
            }
            let identity = (&member.sha256, member.size);
            if let Some(old) = paths.insert(&member.path, identity) {
                if old != identity {
                    return Err(invalid());
                }
            } else if !folded.insert(member.path.to_ascii_lowercase()) {
                return Err(invalid());
            }
        }
        let mut directories = BTreeSet::new();
        for member in &self.files {
            let mut parent = Path::new(&member.path).parent();
            while let Some(p) = parent {
                directories.insert(p.to_owned());
                parent = p.parent();
            }
        }
        if directories
            .len()
            .saturating_add(paths.len())
            .saturating_add(3)
            > 65536
        {
            return Err(invalid());
        }
        if serde_json::to_vec(self).map_err(|_| invalid())?.len() > limits.result_bytes {
            return Err(invalid());
        }
        Ok(())
    }
}
fn private_directory(path: &Path) -> ApiResult<()> {
    package::no_links(path)?;
    let m = fs::symlink_metadata(path).map_err(|_| io())?;
    if !m.is_dir() {
        return Err(invalid());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if m.permissions().mode() & 0o077 != 0 {
            return Err(invalid());
        }
    }
    Ok(())
}
fn mkdir(path: &Path) -> ApiResult<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| io())?;
    private_directory(path)
}
pub(super) fn prepare_root(root: &Path) -> ApiResult<()> {
    let path = root.join("artifacts");
    match fs::symlink_metadata(&path) {
        Ok(_) => private_directory(&path)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => mkdir(&path)?,
        Err(_) => return Err(io()),
    }
    sync(root)
}
pub(super) fn inventory_root(root: &Path, limits: &Limits) -> ApiResult<()> {
    private_directory(root)?;
    let mut count = 0;
    for item in fs::read_dir(root).map_err(|_| io())? {
        count += 1;
        if count > limits.retained_jobs * 2 + 1 {
            return Err(invalid());
        }
        let item = item.map_err(|_| io())?;
        if !item.file_name().to_str().is_some_and(job_id) {
            return Err(invalid());
        }
        private_directory(&item.path())?;
        inventory(
            &item.path(),
            65536,
            limits
                .job_storage_bytes
                .saturating_add(2 * limits.result_bytes as u64),
        )?;
    }
    Ok(())
}
fn inventory(root: &Path, maximum: usize, maximum_bytes: u64) -> ApiResult<BTreeSet<String>> {
    fn walk(
        root: &Path,
        path: &Path,
        depth: usize,
        count: &mut usize,
        total: &mut u64,
        maximum: usize,
        maximum_bytes: u64,
        files: &mut BTreeSet<String>,
    ) -> ApiResult<()> {
        if depth > 64 {
            return Err(invalid());
        }
        for item in fs::read_dir(path).map_err(|_| io())? {
            *count += 1;
            if *count > maximum {
                return Err(invalid());
            }
            let path = item.map_err(|_| io())?.path();
            let m = fs::symlink_metadata(&path).map_err(|_| io())?;
            if m.file_type().is_symlink() || (!m.is_file() && !m.is_dir()) {
                return Err(invalid());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if m.mode() & 0o077 != 0 || (m.is_file() && m.nlink() != 1) {
                    return Err(invalid());
                }
            }
            if m.is_dir() {
                walk(
                    root,
                    &path,
                    depth + 1,
                    count,
                    total,
                    maximum,
                    maximum_bytes,
                    files,
                )?;
            } else {
                *total = total.checked_add(m.len()).ok_or_else(invalid)?;
                if *total > maximum_bytes {
                    return Err(invalid());
                }
                files.insert(
                    path.strip_prefix(root)
                        .map_err(|_| invalid())?
                        .to_str()
                        .ok_or_else(invalid)?
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                );
            }
        }
        Ok(())
    }
    let mut files = BTreeSet::new();
    walk(
        root,
        root,
        0,
        &mut 0,
        &mut 0,
        maximum,
        maximum_bytes,
        &mut files,
    )?;
    Ok(files)
}
fn sync(path: &Path) -> ApiResult<()> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| io())
}
fn sync_tree(path: &Path, depth: usize) -> ApiResult<()> {
    if depth > 64 {
        return Err(invalid());
    }
    for item in fs::read_dir(path).map_err(|_| io())? {
        let path = item.map_err(|_| io())?.path();
        if fs::symlink_metadata(&path).map_err(|_| io())?.is_dir() {
            sync_tree(&path, depth + 1)?;
        }
    }
    sync(path)
}
fn new_file(path: &Path) -> ApiResult<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|_| io())
}
fn read_marker(path: &Path, limits: &Limits) -> ApiResult<Retained> {
    let bytes = super::durable::read(path, limits.result_bytes as u64)?;
    let value: Retained =
        renderflow::evidence::json::object(&bytes, limits.result_bytes).map_err(|_| invalid())?;
    value.validate(limits)?;
    Ok(value)
}
fn checked_root(root: &Path, cache: &Retained, limits: &Limits) -> ApiResult<PathBuf> {
    cache.validate(limits)?;
    let path = root.join("artifacts").join(&cache.job_id);
    private_directory(&path)?;
    if read_marker(&path.join("cache.json"), limits)? != *cache {
        return Err(invalid());
    }
    let mut expected = cache
        .files
        .iter()
        .map(|m| format!("bundle/{}", m.path))
        .collect::<BTreeSet<_>>();
    expected.insert("cache.json".into());
    expected.insert("result.json".into());
    if inventory(
        &path,
        65536,
        limits
            .job_storage_bytes
            .saturating_add(2 * limits.result_bytes as u64),
    )? != expected
    {
        return Err(invalid());
    }
    let bytes = super::durable::read(&path.join("result.json"), limits.result_bytes as u64)?;
    if descriptor(&cache.job_id, &bytes, limits)? != *cache {
        return Err(invalid());
    }
    Ok(path)
}
fn verification(limits: &Limits) -> VerificationLimits {
    VerificationLimits {
        max_file_bytes: limits.job_storage_bytes,
        max_total_bytes: limits.job_storage_bytes,
        max_publications: MAX_FILES,
        timeout_seconds: limits.job_timeout_seconds.min(3600),
        ..Default::default()
    }
}
fn copy_files(
    result: &renderflow::ExecutionResult,
    source: &Path,
    destination: &Path,
    cache: &Retained,
    limits: &Limits,
) -> ApiResult<()> {
    mkdir(destination)?;
    let mut copied = BTreeSet::new();
    let started = Instant::now();
    for member in &cache.files {
        if !copied.insert(&member.path) {
            continue;
        }
        let output = destination.join(package::relative(&member.path)?);
        let parent = output.parent().ok_or_else(invalid)?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent).map_err(|_| io())?;
        private_directory(parent)?;
        let mut file = new_file(&output)?;
        let mut budget = verification(limits);
        budget.timeout_seconds = budget
            .timeout_seconds
            .checked_sub(started.elapsed().as_secs())
            .filter(|n| *n > 0)
            .ok_or_else(invalid)?;
        renderflow::evidence::verification::copy_publication(
            &result.run_manifest,
            &member.target,
            source,
            &budget,
            renderflow::process::ProcessCancellationToken::new(),
            &mut file,
        )
        .map_err(|_| invalid())?;
        file.flush()
            .and_then(|_| file.sync_all())
            .map_err(|_| io())?;
    }
    sync_tree(destination, 0)
}
pub(super) fn estimate(bytes: &[u8], limits: &Limits) -> ApiResult<u64> {
    let d = descriptor(&format!("job-{}", "0".repeat(64)), bytes, limits)?;
    if d.files.is_empty() {
        return Ok(0);
    }
    let mut directories = BTreeSet::new();
    for member in &d.files {
        let mut path = Path::new(&member.path).parent();
        while let Some(p) = path {
            directories.insert(p.to_owned());
            path = p.parent();
        }
    }
    d.total_bytes()?
        .checked_add(2 * limits.result_bytes as u64)
        .and_then(|n| n.checked_add((directories.len() as u64 + 3) * 4096))
        .ok_or_else(invalid)
}
pub(super) fn capture(
    root: &Path,
    id: &str,
    bytes: &[u8],
    workspace: &Workspace,
    limits: &Limits,
) -> ApiResult<Option<Retained>> {
    let cache = descriptor(id, bytes, limits)?;
    if cache.files.is_empty() {
        return Ok(None);
    }
    let path = root.join("artifacts").join(id);
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            checked_root(root, &cache, limits)?;
            return Ok(Some(cache));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(io()),
    }
    // Exclusive new directory; marker and journal are the only commit points.
    // Partial or orphan directories are preserved, counted and never adopted.
    mkdir(&path)?;
    let result = native(bytes, limits)?;
    copy_files(
        &result,
        &workspace.bundle(),
        &path.join("bundle"),
        &cache,
        limits,
    )?;
    let mut result_file = new_file(&path.join("result.json"))?;
    result_file
        .write_all(bytes)
        .and_then(|_| result_file.sync_all())
        .map_err(|_| io())?;
    let marker = serde_json::to_vec(&cache).map_err(|_| invalid())?;
    if marker.len() > limits.result_bytes {
        return Err(invalid());
    }
    let mut file = new_file(&path.join("cache.json"))?;
    file.write_all(&marker)
        .and_then(|_| file.sync_all())
        .map_err(|_| io())?;
    sync(&path)?;
    sync(&root.join("artifacts"))?;
    sync(root)?;
    checked_root(root, &cache, limits)?;
    Ok(Some(cache))
}
pub(super) fn hydrate(
    root: &Path,
    cache: &Retained,
    bytes: &[u8],
    configuration: &str,
    parent: &Path,
    limits: &Limits,
) -> ApiResult<Workspace> {
    if !crate::uploads::sha256(configuration) || descriptor(&cache.job_id, bytes, limits)? != *cache
    {
        return Err(invalid());
    }
    let source = checked_root(root, cache, limits)?;
    let directory = tempfile::Builder::new()
        .prefix("recovered-artifacts-")
        .tempdir_in(parent)
        .map_err(|_| io())?;
    let result = native(bytes, limits)?;
    copy_files(
        &result,
        &source.join("bundle"),
        &directory.path().join("bundle"),
        cache,
        limits,
    )?;
    checked_root(root, cache, limits)?;
    Ok(Workspace {
        directory,
        effective_config_sha256: configuration.into(),
        artifacts_only: true,
        cache_ready: AtomicBool::new(true),
    })
}
pub(super) fn remove(root: &Path, cache: &Retained, limits: &Limits) -> ApiResult<()> {
    let path = root.join("artifacts").join(&cache.job_id);
    cache.validate(limits)?;
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(io()),
        Ok(_) => {}
    }
    let path = checked_root(root, cache, limits)?;
    let result = native(
        &super::durable::read(&path.join("result.json"), limits.result_bytes as u64)?,
        limits,
    )?;
    let started = Instant::now();
    let mut seen = BTreeSet::new();
    for member in &cache.files {
        if seen.insert(&member.path) {
            let mut budget = verification(limits);
            budget.timeout_seconds = budget
                .timeout_seconds
                .checked_sub(started.elapsed().as_secs())
                .filter(|n| *n > 0)
                .ok_or_else(invalid)?;
            renderflow::evidence::verification::copy_publication(
                &result.run_manifest,
                &member.target,
                &path.join("bundle"),
                &budget,
                renderflow::process::ProcessCancellationToken::new(),
                &mut std::io::sink(),
            )
            .map_err(|_| invalid())?;
        }
    }
    checked_root(root, cache, limits)?;
    fs::remove_dir_all(&path).map_err(|_| io())?;
    sync(&root.join("artifacts"))
}
