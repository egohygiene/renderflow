//! Opt-in discovery of verified historical speech results in an existing CAS.
//!
//! This small index selects retained output bytes; it does not establish native
//! reproducibility or attest an installed provider. A missing index permits a
//! native run. A present invalid index or incomplete evidence is an error, never
//! a silent miss. Generic DAG cache and checkpoint reuse remain disabled.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::artifact::{Artifact, ArtifactStore};
use crate::graph::Format;
use crate::process::ProcessCancellationToken;

use super::policy::SpeechPolicy;
use super::provider::{ensure_not_cancelled, expected_digest, read_bounded, sha256};
use super::recovery::{bind_saved_result, configuration_digest, inspect_saved_store, parse_exact, receipt_bytes, require_directory, stable_store_root, SavedSpeechResult, MAX_RECEIPT_BYTES};

pub const SAVED_RESULTS_INDEX_SCHEMA: &str = "renderflow.speech-saved-results-index/v1";
pub const SAVED_RESULTS_INDEX_PROFILE: &str = "historical-speech-discovery/v1";
pub const MAX_INDEX_BYTES: u64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedResultIndex {
    schema_version: String,
    profile: String,
    lookup_sha256: String,
    configuration_sha256: String,
    source_format: String,
    source_sha256: String,
    receipt_sha256: String,
}

/// Deterministic path only: no files or directories are inspected or created.
/// Selection controls are excluded from the existing recognition identity.
pub fn index_path(root: &Path, policy: &SpeechPolicy, source_format: Format) -> Result<PathBuf> {
    Ok(root.join("speech-results").join("v1").join(format!("{}.json", lookup_digest(policy, source_format)?)))
}

/// Read-only historical assessment; never constructs an ArtifactStore.
/// Only absence of the index (including absent parent directories) is a miss.
/// Any present invalid pointer or missing/corrupt evidence fails closed.
pub fn lookup_saved_result(
    root: &Path, policy: &SpeechPolicy, source_format: Format,
    source: Option<&Artifact>, cancellation: &ProcessCancellationToken,
) -> Result<Option<SavedSpeechResult>> {
    ensure_not_cancelled(cancellation)?;
    policy.validate_source(source_format, false)?;
    if !policy.saved_results.is_enabled() { return Ok(None); }
    if !existing_directory(root)? { return Ok(None); }
    let root = stable_store_root(root)?;
    let directory = root.join("speech-results");
    if !existing_directory(&directory)? { return Ok(None); }
    let directory = directory.join("v1");
    if !existing_directory(&directory)? { return Ok(None); }
    let path = index_path(&root, policy, source_format)?;
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("speech saved-result index is unavailable"),
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            bail!("speech saved-result index must be a non-symlink regular file");
        }
        Ok(_) => {},
    }
    let bytes = read_bounded(&path, MAX_INDEX_BYTES, cancellation)?;
    let index: SavedResultIndex = parse_exact(&bytes, "speech saved-result index")?;
    validate_index(&index, policy, source_format)?;
    let saved = inspect_saved_store(&root, &index.receipt_sha256, cancellation)?;
    bind_saved_result(&saved, policy, source_format, source, cancellation)?;
    // Recheck every index directory and exact pointer after the potentially
    // longer bounded receipt/payload verification. A changed pointer is not a hit.
    require_directory(&root)?;
    require_directory(&root.join("speech-results"))?;
    require_directory(&directory)?;
    if read_bounded(&path, MAX_INDEX_BYTES, cancellation)? != bytes {
        bail!("speech saved-result index changed during verification");
    }
    ensure_not_cancelled(cancellation)?;
    Ok(Some(saved))
}

/// Remember a fully published, semantically verified historical result.
///
/// The caller must invoke this only after run validation, budgets, and output
/// publication succeed. The receipt is added to CAS; supporting media/payloads
/// are already stored there. The atomic index never replaces an existing entry.
/// A concurrent first valid result wins, even when its output differs from the
/// current run; callers must not label that historical winner as current output.
/// Interruption after atomic index promotion may leave the already committed,
/// verified result available even when the final recheck reports an error.
pub fn remember_saved_result(
    store: &ArtifactStore, policy: &SpeechPolicy, input: &Artifact,
    canonical: &Artifact, cancellation: &ProcessCancellationToken,
) -> Result<SavedSpeechResult> {
    ensure_not_cancelled(cancellation)?;
    let source_format: Format = input.format().as_str().parse()?;
    policy.validate_source(source_format, false)?;
    if !policy.saved_results.is_enabled() { bail!("speech saved-result discovery is disabled"); }
    let bytes = receipt_bytes(input, policy, canonical, store, cancellation)?;
    // Validate an existing first result before any index or receipt mutation.
    if let Some(saved) = lookup_saved_result(store.root(), policy, source_format, Some(input), cancellation)? {
        return Ok(saved);
    }
    let root = stable_store_root(store.root())?;
    let pin = sha256(&bytes);
    require_directory(&root.join("objects"))?;
    let algorithm = root.join("objects").join("sha256");
    require_directory(&algorithm)?;
    let prefix = algorithm.join(&pin[..2]);
    create_directory(&prefix)?;
    let receipt_path = prefix.join(&pin);
    publish_once(&receipt_path, &bytes, cancellation)?;
    if sha256(&read_bounded(&receipt_path, MAX_RECEIPT_BYTES, cancellation)?) != pin {
        bail!("speech saved-result CAS receipt differs from its digest");
    }
    // Re-read from its future lookup origin; no index becomes visible until the
    // receipt, canonical, and complete supporting evidence all pass validation.
    let saved = inspect_saved_store(&root, &pin, cancellation)?;
    bind_saved_result(&saved, policy, source_format, Some(input), cancellation)?;
    let index = SavedResultIndex {
        schema_version: SAVED_RESULTS_INDEX_SCHEMA.to_owned(),
        profile: SAVED_RESULTS_INDEX_PROFILE.to_owned(),
        lookup_sha256: lookup_digest(policy, source_format)?,
        configuration_sha256: configuration_digest(policy)?,
        source_format: source_format.to_string(), source_sha256: policy.source_sha256.clone(),
        receipt_sha256: pin,
    };
    let bytes = serde_json::to_vec(&index)?;
    if bytes.len() as u64 > MAX_INDEX_BYTES { bail!("speech saved-result index exceeds its byte limit"); }
    let directory = root.join("speech-results");
    create_directory(&directory)?;
    let version = directory.join("v1");
    create_directory(&version)?;
    let path = index_path(&root, policy, source_format)?;
    ensure_not_cancelled(cancellation)?;
    require_directory(&root)?;
    require_directory(&directory)?;
    require_directory(&version)?;
    publish_once(&path, &bytes, cancellation)?;
    // This also checks the winning pointer on a race. Invalid existing entries
    // stay untouched and fail; a later run never silently repairs historical data.
    lookup_saved_result(&root, policy, source_format, Some(input), cancellation)?
        .context("speech saved-result index disappeared after publication")
}

fn lookup_digest(policy: &SpeechPolicy, source_format: Format) -> Result<String> {
    policy.validate_source(source_format, false)?;
    Ok(sha256(&serde_json::to_vec(&(SAVED_RESULTS_INDEX_PROFILE, source_format.to_string(), configuration_digest(policy)?))?))
}

fn validate_index(index: &SavedResultIndex, policy: &SpeechPolicy, source_format: Format) -> Result<()> {
    expected_digest(&index.receipt_sha256, "speech saved-result receipt digest")?;
    if index.schema_version != SAVED_RESULTS_INDEX_SCHEMA
        || index.profile != SAVED_RESULTS_INDEX_PROFILE
        || index.lookup_sha256 != lookup_digest(policy, source_format)?
        || index.configuration_sha256 != configuration_digest(policy)?
        || index.source_format != source_format.to_string()
        || index.source_sha256 != policy.source_sha256
    { bail!("speech saved-result index differs from its source and recognition intent"); }
    Ok(())
}

fn existing_directory(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("speech saved-result directory is unavailable"),
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            bail!("speech saved-result requires non-symlink directories");
        }
        Ok(_) => Ok(true),
    }
}

fn create_directory(path: &Path) -> Result<()> {
    match std::fs::create_dir(path) {
        Ok(()) => {},
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
        Err(error) => return Err(error).context("failed to create speech saved-result directory"),
    }
    require_directory(path)
}

fn publish_once(path: &Path, bytes: &[u8], cancellation: &ProcessCancellationToken) -> Result<()> {
    ensure_not_cancelled(cancellation)?;
    let directory = path.parent().context("speech saved-result destination has no parent")?;
    require_directory(directory)?;
    // Avoid even temporary writes when a first writer already published. The
    // caller must fully validate that winner after this function returns.
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => return Ok(()),
        Ok(_) => bail!("speech saved-result destination must be a non-symlink regular file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error).context("speech saved-result destination is unavailable"),
    }
    let mut temporary = tempfile::NamedTempFile::new_in(directory)
        .context("failed to create speech saved-result temporary file")?;
    temporary.write_all(bytes).context("failed to write speech saved-result temporary file")?;
    temporary.flush().context("failed to flush speech saved-result temporary file")?;
    temporary.as_file().sync_all().context("failed to sync speech saved-result temporary file")?;
    ensure_not_cancelled(cancellation)?;
    require_directory(directory)?;
    match temporary.persist_noclobber(path) {
        Ok(file) => file.sync_all().context("failed to sync speech saved-result file")?,
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {},
        Err(error) => return Err(error.error).context("failed to publish speech saved-result without overwriting"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_atomic_writer_wins_without_replacing_existing_bytes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("index.json");
        let token = ProcessCancellationToken::default();
        publish_once(&path, b"first historical pointer", &token).unwrap();
        publish_once(&path, b"different later pointer", &token).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"first historical pointer");
    }
}
