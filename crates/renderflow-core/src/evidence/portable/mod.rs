//! A pinned native receipt plus content-addressed copies of retained inventory.
//! Original native locators/metadata are immutable; this index supplies the
//! portable payload mapping. It is an internal handoff, not a sanitized upload.

mod dependencies;

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::files::{self, BundleRoot, FileObservation};
use super::verification::{PinnedRunReceipt, VerificationError, VerificationLimits};
use super::{ArtifactRole, PublishedTarget, RunState};
use crate::process::ProcessCancellationToken;

pub const PORTABLE_SCHEMA: &str = "renderflow.portable-forest/v1";
const RECEIPT_PATH: &str = "native/run.json";
const INDEX_PATH: &str = "bundle.json";
pub type Result<T> = std::result::Result<T, PortableError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PortableError {
    #[error(transparent)]
    Verification(#[from] VerificationError),
    #[error("invalid portable forest bundle: {reason}")]
    Invalid { reason: &'static str },
    #[error("portable bundle JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("portable bundle I/O: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub include_sources: bool,
    pub max_records: usize,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_sources: false,
            max_records: 16384,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptSubject {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryBinding {
    pub native_index: usize,
    /// None only for a deliberately external source record.
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableIndex {
    pub schema_version: String,
    pub run_id: String,
    pub run_state: RunState,
    pub native_receipt: ReceiptSubject,
    pub include_sources: bool,
    pub records: Vec<InventoryBinding>,
}

/// Constructed only after all included payloads and the original receipt pass.
/// Public getters preserve original state/metadata; no later-use authority.
pub struct VerifiedPortableBundle {
    index: PortableIndex,
    receipt: PinnedRunReceipt,
    observations: Vec<Option<FileObservation>>,
    index_sha256: String,
}

pub struct PortablePublication<'a> {
    pub native: PublishedTarget<'a>,
    pub relative_payload: &'a str,
    pub observation: &'a FileObservation,
}

impl VerifiedPortableBundle {
    pub fn index(&self) -> &PortableIndex {
        &self.index
    }
    pub fn receipt(&self) -> &PinnedRunReceipt {
        &self.receipt
    }
    pub fn index_sha256(&self) -> &str {
        &self.index_sha256
    }
    pub fn publications(&self) -> Result<Vec<PortablePublication<'_>>> {
        self.receipt
            .manifest()
            .published_targets()
            .map_err(VerificationError::from)?
            .into_iter()
            .map(|native| {
                let index =
                    native
                        .execution
                        .artifact_evidence_index
                        .ok_or(PortableError::Invalid {
                            reason: "publication index missing",
                        })?;
                Ok(PortablePublication {
                    native,
                    relative_payload: self.index.records[index].path.as_deref().ok_or(
                        PortableError::Invalid {
                            reason: "published target cannot be an external source binding",
                        },
                    )?,
                    observation: self.observations[index].as_ref().ok_or(
                        PortableError::Invalid {
                            reason: "published target was not observed",
                        },
                    )?,
                })
            })
            .collect()
    }
    pub fn report(&self) -> Result<Value> {
        let publications = self.publications()?;
        Ok(
            json!({"schema_version":"renderflow.portable-forest-verification/v1",
            "index_sha256":self.index_sha256,"native_receipt_sha256":self.receipt.sha256(),
            "run_id":self.index.run_id,"run_state":self.index.run_state,"include_sources":self.index.include_sources,
            "observed_records":self.observations.iter().filter(|subject|subject.is_some()).count(),
            "external_source_records":self.observations.iter().filter(|subject|subject.is_none()).count(),
            "publications":publications.iter().map(|subject|json!({"target_id":subject.native.branch.target_id,
                "artifact_id":subject.native.artifact.artifact_id,"original_locator":subject.native.artifact.locator,
                "relative_payload":subject.relative_payload,"sha256":subject.observation.sha256,
                "size_bytes":subject.observation.size_bytes,"validation":subject.native.artifact.validation,
                "fidelity":subject.native.artifact.fidelity,"source_identity":subject.native.execution.source_identity})).collect::<Vec<_>>(),
            "dependency_scope":"Inventory lineage and known v1 body/recovery/subtitle/speech/lyrics references; unknown extension metadata remains opaque.",
            "runtime_qualified":false,"publisher_authenticated":false,"creative_approval_inferred":false}),
        )
    }
}

pub fn export(
    receipt: &PinnedRunReceipt,
    bundle_root: &Path,
    artifact_store_root: Option<&Path>,
    output: &Path,
    options: &ExportOptions,
    limits: &VerificationLimits,
    cancellation: ProcessCancellationToken,
) -> Result<Value> {
    limits.validate()?;
    let started = Instant::now();
    files::check_budget(started, limits, &cancellation)?;
    if receipt.bytes().len() as u64 > limits.max_manifest_bytes {
        return Err(invalid("native receipt exceeds configured byte bound"));
    }
    if options.max_records == 0 || options.max_records > 100_000 {
        return Err(invalid("invalid inventory count bound"));
    }
    let manifest = receipt.manifest();
    let publications = manifest
        .published_targets()
        .map_err(VerificationError::from)?;
    if publications.is_empty() {
        return Err(invalid(
            "export needs at least one observed native publication",
        ));
    }
    if publications.len() > limits.max_publications
        || manifest.artifact_manifest.artifacts.len() > options.max_records
    {
        return Err(invalid("publication or inventory count bound exceeded"));
    }
    let required = dependencies::check(manifest)?;
    if fs::symlink_metadata(output).is_ok() {
        return Err(invalid("output must not exist"));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.is_dir() {
        return Err(invalid("output parent must already exist"));
    }
    let bundle = BundleRoot::new(bundle_root)?;
    let need_store = manifest.artifact_manifest.artifacts.iter().any(|record| {
        (options.include_sources
            || record.lifecycle != ArtifactRole::Source
            || required.contains(&record.artifact_id))
            && record.locator.starts_with("artifact-store:")
    });
    let store = if need_store {
        Some(BundleRoot::new(artifact_store_root.ok_or(invalid(
            "retained payloads require an explicit artifact store root",
        ))?)?)
    } else {
        None
    };
    let temporary = tempfile::Builder::new()
        .prefix("renderflow-portable-")
        .tempdir_in(parent)?;
    let stage = temporary.path();
    let mut total = 0_u64;
    let mut copied = BTreeSet::new();
    let mut records = Vec::new();
    for (native_index, record) in manifest.artifact_manifest.artifacts.iter().enumerate() {
        files::check_budget(started, limits, &cancellation)?;
        if record.lifecycle == ArtifactRole::Source
            && !options.include_sources
            && !required.contains(&record.artifact_id)
        {
            records.push(InventoryBinding {
                native_index,
                path: None,
            });
            continue;
        }
        if record.digest.algorithm != "sha256" {
            return Err(VerificationError::UnsupportedDigest.into());
        }
        let path = object_path(&record.digest.value)?;
        let (root, relative) = if let Some(relative) = record.locator.strip_prefix("bundle:") {
            (&bundle, relative)
        } else if let Some(relative) = record.locator.strip_prefix("artifact-store:") {
            if relative != path {
                return Err(invalid(
                    "artifact-store locator differs from its digest-derived object path",
                ));
            }
            (
                store
                    .as_ref()
                    .ok_or(invalid("artifact store root missing"))?,
                relative,
            )
        } else {
            return Err(invalid(
                "included inventory locator is not bundle or artifact-store",
            ));
        };
        total = total
            .checked_add(record.size_bytes)
            .ok_or(invalid("source read-work overflow"))?;
        if total > limits.max_total_bytes {
            return Err(invalid("source read-work bound exceeded"));
        }
        if copied.insert(path.clone()) {
            let target = stage.join(&path);
            fs::create_dir_all(target.parent().ok_or(invalid("object parent missing"))?)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)?;
            files::observe(
                root,
                relative,
                files::ExpectedFile {
                    size_bytes: record.size_bytes,
                    sha256: &record.digest.value,
                },
                limits,
                started,
                &cancellation,
                &mut file,
            )?;
            file.sync_all()?;
        } else {
            // Every declared origin is checked even when its destination deduplicates.
            files::observe(
                root,
                relative,
                files::ExpectedFile {
                    size_bytes: record.size_bytes,
                    sha256: &record.digest.value,
                },
                limits,
                started,
                &cancellation,
                &mut std::io::sink(),
            )?;
        }
        records.push(InventoryBinding {
            native_index,
            path: Some(path),
        });
    }
    files::check_budget(started, limits, &cancellation)?;
    let index = PortableIndex {
        schema_version: PORTABLE_SCHEMA.into(),
        run_id: manifest.run_id.clone(),
        run_state: manifest.state,
        native_receipt: ReceiptSubject {
            path: RECEIPT_PATH.into(),
            sha256: receipt.sha256().into(),
            size_bytes: receipt.bytes().len() as u64,
        },
        include_sources: options.include_sources,
        records,
    };
    write_new(&stage.join(RECEIPT_PATH), receipt.bytes())?;
    let index_bytes = serde_json::to_vec_pretty(&index)?;
    if index_bytes.len() as u64 > limits.max_manifest_bytes {
        return Err(invalid("portable index exceeds receipt byte bound"));
    }
    let index_sha256 = sha256(&index_bytes);
    write_new(&stage.join(INDEX_PATH), &index_bytes)?;
    // Stage verification is a separate bounded phase, with no original store fallback.
    verify(
        stage,
        &index_sha256,
        options.max_records,
        limits,
        cancellation.clone(),
    )?;
    let publication_started = Instant::now();
    files::check_budget(publication_started, limits, &cancellation)?;
    // Atomic no-overwrite reservation. Only copies owned by this staging
    // directory are hard-linked; never link original input/store files.
    fs::create_dir(output)?;
    for path in copied
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(RECEIPT_PATH))
    {
        files::check_budget(publication_started, limits, &cancellation)?;
        let destination = output.join(path);
        fs::create_dir_all(
            destination
                .parent()
                .ok_or(invalid("output parent missing"))?,
        )?;
        fs::hard_link(stage.join(path), destination)?;
    }
    if cancellation.is_cancelled() {
        return Err(VerificationError::Cancelled.into());
    }
    // Destination re-observation precedes the completion marker. This is a
    // separate bounded verification phase, not an end-to-end hard I/O deadline.
    let destination = BundleRoot::new(output)?;
    read_pinned(
        &destination,
        RECEIPT_PATH,
        receipt.sha256(),
        Some(receipt.bytes().len() as u64),
        limits,
        Instant::now(),
        &cancellation,
    )?;
    verify_records(
        &destination,
        &index,
        receipt,
        options.max_records,
        limits,
        &cancellation,
    )?;
    fs::hard_link(stage.join(INDEX_PATH), output.join(INDEX_PATH))?;
    Ok(
        json!({"schema_version":"renderflow.portable-forest-export/v1","index":"bundle.json",
        "index_sha256":index_sha256,"native_receipt_sha256":receipt.sha256(),"run_id":manifest.run_id,
        "run_state":manifest.state,"include_sources":options.include_sources,"record_count":index.records.len(),
        "object_count":copied.len(),"runtime_qualified":false,"creative_approval_inferred":false}),
    )
}

pub fn verify(
    root: &Path,
    expected_index_sha256: &str,
    max_records: usize,
    limits: &VerificationLimits,
    cancellation: ProcessCancellationToken,
) -> Result<VerifiedPortableBundle> {
    limits.validate()?;
    files::digest(expected_index_sha256)?;
    let started = Instant::now();
    files::check_budget(started, limits, &cancellation)?;
    if max_records == 0 || max_records > 100_000 {
        return Err(invalid("invalid inventory count bound"));
    }
    let root = BundleRoot::new(root)?;
    let index_bytes = read_pinned(
        &root,
        INDEX_PATH,
        expected_index_sha256,
        None,
        limits,
        started,
        &cancellation,
    )?;
    let shape: Value = serde_json::from_slice(&index_bytes)?;
    if !shape.is_object()
        || !shape.get("native_receipt").is_some_and(Value::is_object)
        || !shape
            .get("records")
            .and_then(Value::as_array)
            .is_some_and(|rows| rows.iter().all(Value::is_object))
    {
        return Err(invalid(
            "portable index and subject bindings must use object shapes",
        ));
    }
    let index: PortableIndex = serde_json::from_slice(&index_bytes)?;
    if index.schema_version != PORTABLE_SCHEMA || index.native_receipt.path != RECEIPT_PATH {
        return Err(invalid(
            "unsupported portable index schema or native receipt path",
        ));
    }
    let bytes = read_pinned(
        &root,
        RECEIPT_PATH,
        &index.native_receipt.sha256,
        Some(index.native_receipt.size_bytes),
        limits,
        started,
        &cancellation,
    )?;
    let receipt = PinnedRunReceipt::from_bytes(bytes, &index.native_receipt.sha256, limits)?;
    files::check_budget(started, limits, &cancellation)?;
    let observations = verify_records(&root, &index, &receipt, max_records, limits, &cancellation)?;
    Ok(VerifiedPortableBundle {
        index,
        receipt,
        observations,
        index_sha256: expected_index_sha256.into(),
    })
}

fn verify_records(
    root: &BundleRoot,
    index: &PortableIndex,
    receipt: &PinnedRunReceipt,
    max_records: usize,
    limits: &VerificationLimits,
    cancellation: &ProcessCancellationToken,
) -> Result<Vec<Option<FileObservation>>> {
    let started = Instant::now();
    let manifest = receipt.manifest();
    let publications = manifest
        .published_targets()
        .map_err(VerificationError::from)?;
    if index.schema_version != PORTABLE_SCHEMA
        || index.run_id != manifest.run_id
        || index.run_state != manifest.state
        || index.native_receipt.sha256 != receipt.sha256()
        || index.native_receipt.size_bytes != receipt.bytes().len() as u64
        || index.records.len() != manifest.artifact_manifest.artifacts.len()
        || index.records.len() > max_records
        || publications.is_empty()
        || publications.len() > limits.max_publications
    {
        return Err(invalid(
            "index and pinned native receipt disagree or exceed bounds",
        ));
    }
    let required = dependencies::check(manifest)?;
    let mut total = 0_u64;
    let mut observations = Vec::new();
    for (position, (binding, record)) in index
        .records
        .iter()
        .zip(&manifest.artifact_manifest.artifacts)
        .enumerate()
    {
        files::check_budget(started, limits, cancellation)?;
        if binding.native_index != position {
            return Err(invalid(
                "inventory bindings must be complete, unique and in native order",
            ));
        }
        if record.lifecycle == ArtifactRole::Source
            && !index.include_sources
            && !required.contains(&record.artifact_id)
        {
            if binding.path.is_some() {
                return Err(invalid("external source cannot carry an included binding"));
            }
            observations.push(None);
            continue;
        }
        if record.digest.algorithm != "sha256" {
            return Err(VerificationError::UnsupportedDigest.into());
        }
        let expected = object_path(&record.digest.value)?;
        if binding.path.as_deref() != Some(expected.as_str()) {
            return Err(invalid(
                "included inventory path differs from its digest identity",
            ));
        }
        total = total
            .checked_add(record.size_bytes)
            .ok_or(invalid("verification work overflow"))?;
        if total > limits.max_total_bytes {
            return Err(invalid("verification read-work bound exceeded"));
        }
        observations.push(Some(files::observe(
            root,
            &expected,
            files::ExpectedFile {
                size_bytes: record.size_bytes,
                sha256: &record.digest.value,
            },
            limits,
            started,
            cancellation,
            &mut std::io::sink(),
        )?));
    }
    root.recheck()?;
    files::check_budget(started, limits, cancellation)?;
    Ok(observations)
}

fn read_pinned(
    root: &BundleRoot,
    relative: &str,
    expected: &str,
    size: Option<u64>,
    limits: &VerificationLimits,
    started: Instant,
    cancellation: &ProcessCancellationToken,
) -> Result<Vec<u8>> {
    let path = root.resolve(relative, limits)?;
    let observed_size = fs::symlink_metadata(path)?.len();
    if observed_size > limits.max_manifest_bytes || size.is_some_and(|size| size != observed_size) {
        return Err(invalid(
            "index/native receipt size mismatch or byte bound exceeded",
        ));
    }
    let mut bytes = Vec::new();
    let metadata_limits = VerificationLimits {
        max_file_bytes: limits.max_manifest_bytes,
        ..limits.clone()
    };
    files::observe(
        root,
        relative,
        files::ExpectedFile {
            size_bytes: observed_size,
            sha256: expected,
        },
        &metadata_limits,
        started,
        cancellation,
        &mut bytes,
    )?;
    Ok(bytes)
}

pub(super) fn object_path(digest: &str) -> Result<String> {
    files::digest(digest)?;
    Ok(format!("objects/sha256/{}/{digest}", &digest[..2]))
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().ok_or(invalid("staging parent missing"))?)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn invalid(reason: &'static str) -> PortableError {
    PortableError::Invalid { reason }
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
