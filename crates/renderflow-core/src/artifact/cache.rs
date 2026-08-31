use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;

use super::Artifact;
use crate::graph::Format;

/// Artifact-native DAG cache mapping deterministic node keys to stored artifacts.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ArtifactCache {
    entries: HashMap<String, Artifact>,
}

impl ArtifactCache {
    pub(crate) fn get(&self, key: &str) -> Option<&Artifact> {
        self.entries.get(key)
    }

    pub(crate) fn insert(&mut self, key: String, artifact: Artifact) {
        self.entries.insert(key, artifact);
    }
}

/// Compute a DAG cache key from artifact identity and transform configuration.
///
/// Large payload bytes are deliberately not re-hashed here: the input artifact's
/// SHA-256 digest and byte size already establish content identity.
pub fn compute_artifact_node_hash(
    input: &Artifact,
    from: Format,
    to: Format,
    transform_identity: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.id().as_str().as_bytes());
    hasher.update(b"\x00artifact-id\x00");
    hasher.update(input.digest().algorithm().to_string().as_bytes());
    hasher.update(b"\x00digest\x00");
    hasher.update(input.digest().value().as_bytes());
    hasher.update(b"\x00size\x00");
    hasher.update(input.size_bytes().to_le_bytes());
    hasher.update(b"\x00from\x00");
    hasher.update(from.to_string().as_bytes());
    hasher.update(b"\x00to\x00");
    hasher.update(to.to_string().as_bytes());
    hasher.update(b"\x00transform\x00");
    hasher.update(transform_identity.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn load_artifact_cache(path: &Path) -> ArtifactCache {
    if !path.exists() {
        return ArtifactCache::default();
    }
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(cache) => cache,
            Err(error) => {
                warn!(
                    path = %path.display(),
                    error = %error,
                    "Artifact DAG cache is unreadable or uses a legacy schema; starting empty"
                );
                ArtifactCache::default()
            }
        },
        Err(error) => {
            warn!(
                path = %path.display(),
                error = %error,
                "Failed to read artifact DAG cache; starting empty"
            );
            ArtifactCache::default()
        }
    }
}

pub(crate) fn save_artifact_cache(cache: &ArtifactCache, path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "Failed to create artifact cache directory '{}'",
            parent.display()
        )
    })?;

    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .context("Failed to create artifact cache temporary file")?;
    serde_json::to_writer(&mut temporary, cache).context("Failed to serialize artifact cache")?;
    temporary
        .flush()
        .context("Failed to flush artifact cache")?;
    temporary
        .as_file()
        .sync_all()
        .context("Failed to sync artifact cache")?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "Failed to atomically save artifact cache '{}'",
                path.display()
            )
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass, ArtifactStore};

    #[test]
    fn cache_key_uses_artifact_identity_not_payload_buffer() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let artifact = store
            .put_bytes(
                &[0, 255, 1, 2],
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        let first = compute_artifact_node_hash(
            &artifact,
            Format::Png,
            Format::Webp,
            "adapter:v1:quality=90",
        );
        let second = compute_artifact_node_hash(
            &artifact,
            Format::Png,
            Format::Webp,
            "adapter:v1:quality=80",
        );
        assert_ne!(first, second);
        assert_eq!(first.len(), 64);
    }
}
