use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use super::{Artifact, ArtifactDescriptor, ArtifactDigest, ArtifactId, ArtifactPayload};

const COPY_BUFFER_BYTES: usize = 64 * 1024;

/// File-backed content-addressed artifact store.
///
/// Payloads are streamed into a temporary file while a SHA-256 digest is
/// computed, then atomically promoted to a deterministic object path. Canonical
/// source files are therefore never mutated in place.
#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    /// Open or create a content-addressed store at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("objects/sha256"))
            .with_context(|| format!("Failed to create artifact store at '{}'", root.display()))?;
        fs::create_dir_all(root.join(".tmp")).with_context(|| {
            format!(
                "Failed to create artifact store temporary directory at '{}'",
                root.display()
            )
        })?;
        Ok(Self { root })
    }

    /// Store root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Import an immutable source/intermediate file without assuming UTF-8.
    pub fn import_path(
        &self,
        path: impl AsRef<Path>,
        descriptor: ArtifactDescriptor,
    ) -> Result<Artifact> {
        let path = path.as_ref();
        let file = File::open(path)
            .with_context(|| format!("Failed to open artifact source '{}'", path.display()))?;
        self.put_reader(file, descriptor)
            .with_context(|| format!("Failed to import artifact source '{}'", path.display()))
    }

    /// Store an in-memory byte slice.
    pub fn put_bytes(&self, bytes: &[u8], descriptor: ArtifactDescriptor) -> Result<Artifact> {
        self.put_reader(Cursor::new(bytes), descriptor)
    }

    /// Stream arbitrary bytes into the content-addressed store.
    ///
    /// The entire payload is never required to reside in memory. The temporary
    /// object is promoted only after the digest and size are known and all bytes
    /// have been written successfully.
    pub fn put_reader<R: Read>(
        &self,
        mut reader: R,
        descriptor: ArtifactDescriptor,
    ) -> Result<Artifact> {
        let mut temporary = tempfile::NamedTempFile::new_in(self.temporary_directory())
            .context("Failed to create artifact-store temporary file")?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];

        loop {
            let read = reader
                .read(&mut buffer)
                .context("Failed while reading artifact payload")?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            temporary
                .write_all(&buffer[..read])
                .context("Failed while writing artifact-store temporary file")?;
            size_bytes = size_bytes
                .checked_add(read as u64)
                .context("Artifact size overflowed u64")?;
        }

        temporary
            .flush()
            .context("Failed to flush artifact-store temporary file")?;
        temporary
            .as_file()
            .sync_all()
            .context("Failed to sync artifact-store temporary file")?;

        let digest_hex = format!("{:x}", hasher.finalize());
        let digest = ArtifactDigest::sha256(digest_hex.clone());
        let relative_path = Self::object_relative_path(&digest_hex);
        let target = self.root.join(&relative_path);

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create artifact object directory '{}'",
                    parent.display()
                )
            })?;
        }

        if !target.exists() {
            match temporary.persist(&target) {
                Ok(file) => {
                    file.sync_all().with_context(|| {
                        format!("Failed to sync artifact object '{}'", target.display())
                    })?;
                }
                Err(error) if target.exists() => {
                    // A concurrent writer won the race for the same digest.
                    drop(error.file);
                }
                Err(error) => {
                    return Err(error.error).with_context(|| {
                        format!("Failed to persist artifact object '{}'", target.display())
                    })
                }
            }
        }

        let id = ArtifactId::from_record(&digest, &descriptor.format, &descriptor.sources);
        Ok(Artifact::new(
            id,
            descriptor.format,
            descriptor.media_type,
            digest,
            size_bytes,
            ArtifactPayload::new(relative_path),
            descriptor.metadata,
            descriptor.sources,
            descriptor.storage_class,
        ))
    }

    /// Resolve an artifact's store-relative payload path safely.
    pub fn payload_path(&self, artifact: &Artifact) -> Result<PathBuf> {
        let relative = artifact.payload().relative_path();
        Self::validate_relative_payload_path(relative)?;
        Ok(self.root.join(relative))
    }

    /// Return whether the payload referenced by an artifact is present.
    pub fn contains(&self, artifact: &Artifact) -> bool {
        self.payload_path(artifact)
            .map(|path| path.is_file())
            .unwrap_or(false)
    }

    /// Open the artifact payload for streaming reads.
    pub fn open(&self, artifact: &Artifact) -> Result<File> {
        let path = self.payload_path(artifact)?;
        File::open(&path)
            .with_context(|| format!("Artifact payload '{}' is unavailable", path.display()))
    }

    /// Read the complete payload as bytes.
    ///
    /// Prefer [`open`](Self::open) for large artifacts.
    pub fn read_bytes(&self, artifact: &Artifact) -> Result<Vec<u8>> {
        let path = self.payload_path(artifact)?;
        fs::read(&path)
            .with_context(|| format!("Failed to read artifact payload '{}'", path.display()))
    }

    /// Read an artifact through the legacy UTF-8 text compatibility boundary.
    pub fn read_text(&self, artifact: &Artifact) -> Result<String> {
        let bytes = self.read_bytes(artifact)?;
        String::from_utf8(bytes).with_context(|| {
            format!(
                "Artifact '{}' ({}) is not valid UTF-8; use an artifact-native transform",
                artifact.id(),
                artifact.media_type()
            )
        })
    }

    /// Atomically materialize a stored artifact at its final destination.
    ///
    /// The destination is replaced only after the complete payload has been
    /// copied and synced to a temporary file in the destination directory.
    pub fn materialize(&self, artifact: &Artifact, destination: impl AsRef<Path>) -> Result<()> {
        let destination = destination.as_ref();
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create final artifact directory '{}'",
                parent.display()
            )
        })?;

        let mut input = self.open(artifact)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
            format!(
                "Failed to create atomic output temporary file in '{}'",
                parent.display()
            )
        })?;
        io::copy(&mut input, &mut temporary).with_context(|| {
            format!(
                "Failed to copy artifact '{}' to temporary output",
                artifact.id()
            )
        })?;
        temporary.flush().context("Failed to flush final artifact")?;
        temporary
            .as_file()
            .sync_all()
            .context("Failed to sync final artifact")?;
        temporary
            .persist(destination)
            .map_err(|error| error.error)
            .with_context(|| {
                format!(
                    "Failed to atomically materialize artifact '{}' at '{}'",
                    artifact.id(),
                    destination.display()
                )
            })?;
        Ok(())
    }

    pub(crate) fn temporary_directory(&self) -> PathBuf {
        self.root.join(".tmp")
    }

    fn object_relative_path(digest_hex: &str) -> PathBuf {
        PathBuf::from("objects")
            .join("sha256")
            .join(&digest_hex[..2])
            .join(digest_hex)
    }

    fn validate_relative_payload_path(path: &Path) -> Result<()> {
        if path.is_absolute() {
            anyhow::bail!("artifact payload path must be store-relative");
        }
        for component in path.components() {
            match component {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    anyhow::bail!("artifact payload path escapes the artifact store")
                }
                Component::CurDir | Component::Normal(_) => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactStorageClass;
    use crate::graph::Format;

    fn fixture(format: Format, bytes: &[u8]) {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let artifact = store
            .put_bytes(
                bytes,
                ArtifactDescriptor::for_format(format, ArtifactStorageClass::Source),
            )
            .unwrap();
        assert_eq!(artifact.size_bytes(), bytes.len() as u64);
        assert_eq!(artifact.digest().value().len(), 64);
        assert_eq!(store.read_bytes(&artifact).unwrap(), bytes);
    }

    #[test]
    fn representative_artifact_families_are_binary_safe() {
        fixture(Format::Pdf, b"%PDF-1.7\n%\x80\x81\x82\n");
        fixture(Format::Png, b"\x89PNG\r\n\x1a\n\x00\xff\x00");
        fixture(Format::Wav, b"RIFF\x00\x00\x00\x00WAVEfmt \x00\xff");
        fixture(Format::Markdown, b"# artifact kernel\n");
    }

    #[test]
    fn identical_payloads_share_content_storage() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let descriptor = || {
            ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Intermediate)
        };
        let first = store.put_bytes(b"\x89PNG\x00", descriptor()).unwrap();
        let second = store.put_bytes(b"\x89PNG\x00", descriptor()).unwrap();
        assert_eq!(first.id(), second.id());
        assert_eq!(first.digest(), second.digest());
        assert_eq!(first.payload(), second.payload());
    }

    #[test]
    fn importing_a_source_never_mutates_the_source_file() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.bin");
        fs::write(&source, [0_u8, 255, 1, 2]).unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let artifact = store
            .import_path(
                &source,
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Source),
            )
            .unwrap();
        assert_eq!(fs::read(&source).unwrap(), vec![0, 255, 1, 2]);
        assert_ne!(store.payload_path(&artifact).unwrap(), source);
    }

    #[test]
    fn materialize_replaces_final_output_after_complete_copy() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let artifact = store
            .put_bytes(
                b"new bytes",
                ArtifactDescriptor::for_format(Format::Pdf, ArtifactStorageClass::Terminal),
            )
            .unwrap();
        let destination = directory.path().join("release.pdf");
        fs::write(&destination, b"old bytes").unwrap();
        store.materialize(&artifact, &destination).unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"new bytes");
    }
}
