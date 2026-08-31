use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::graph::Format;

/// Stable identifier for an artifact record.
///
/// Payload bytes are content-addressed independently by [`ArtifactDigest`]. The
/// record identifier additionally commits to canonical format and ordered source
/// lineage, so byte-preserving conversions remain distinct provenance records.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactId(String);

impl ArtifactId {
    pub(crate) fn from_record(
        digest: &ArtifactDigest,
        format: &CanonicalFormat,
        sources: &[ArtifactId],
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(digest.algorithm().to_string().as_bytes());
        hasher.update(b"\x00digest\x00");
        hasher.update(digest.value().as_bytes());
        hasher.update(b"\x00format\x00");
        hasher.update(format.as_str().as_bytes());
        hasher.update(b"\x00sources\x00");
        for source in sources {
            hasher.update(source.as_str().as_bytes());
            hasher.update(b"\x00");
        }
        Self(format!("artifact:sha256:{:x}", hasher.finalize()))
    }

    /// Return the stable identifier as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Digest algorithm used for content identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DigestAlgorithm {
    /// SHA-256, the canonical Renderflow artifact digest algorithm.
    Sha256,
}

impl fmt::Display for DigestAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sha256 => f.write_str("sha256"),
        }
    }
}

/// Content digest for an artifact payload.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactDigest {
    algorithm: DigestAlgorithm,
    value: String,
}

impl ArtifactDigest {
    pub(crate) fn sha256(value: String) -> Self {
        Self {
            algorithm: DigestAlgorithm::Sha256,
            value,
        }
    }

    /// Digest algorithm.
    pub fn algorithm(&self) -> DigestAlgorithm {
        self.algorithm
    }

    /// Lowercase hexadecimal digest value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ArtifactDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm, self.value)
    }
}

/// Stable, tool-neutral format identifier carried by an artifact.
///
/// The artifact kernel intentionally stores the canonical string form rather
/// than duplicating the graph's format enum in serialized cache/evidence data.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CanonicalFormat(String);

impl CanonicalFormat {
    /// Construct a canonical format from a non-empty identifier.
    pub fn new(value: impl Into<String>) -> anyhow::Result<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            anyhow::bail!("artifact format must not be empty");
        }
        Ok(Self(value))
    }

    /// Return the canonical identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<Format> for CanonicalFormat {
    fn from(value: Format) -> Self {
        Self(value.to_string())
    }
}

impl fmt::Display for CanonicalFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// MIME/media type carried with an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MediaType(String);

impl MediaType {
    /// Construct a media type from a non-empty value.
    pub fn new(value: impl Into<String>) -> anyhow::Result<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            anyhow::bail!("artifact media type must not be empty");
        }
        Ok(Self(value))
    }

    /// Return the media type string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return Renderflow's default media type for a graph [`Format`].
    pub fn for_format(format: Format) -> Self {
        let value = match format {
            Format::Markdown => "text/markdown",
            Format::Html => "text/html",
            Format::Pdf => "application/pdf",
            Format::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            Format::Epub => "application/epub+zip",
            Format::Rst => "text/x-rst",
            Format::Latex => "application/x-latex",
            Format::Fountain => "text/plain",
            Format::Jpeg => "image/jpeg",
            Format::Png => "image/png",
            Format::Tiff => "image/tiff",
            Format::Webp => "image/webp",
            Format::Gif => "image/gif",
            Format::Bmp => "image/bmp",
            Format::Avif => "image/avif",
            Format::Svg => "image/svg+xml",
            Format::Cbz => "application/vnd.comicbook+zip",
            Format::Wav | Format::Bwf => "audio/wav",
            Format::Aiff => "audio/aiff",
            Format::Pcm => "application/octet-stream",
            Format::Flac => "audio/flac",
            Format::M4aAlac | Format::M4aAac => "audio/mp4",
            Format::Wv => "audio/x-wavpack",
            Format::Ape => "audio/ape",
            Format::Tta => "audio/x-tta",
            Format::Dsf | Format::Dff => "audio/dsd",
            Format::Shn => "audio/x-shorten",
            Format::Mp3 | Format::Mp2 => "audio/mpeg",
            Format::Aac => "audio/aac",
            Format::Ogg => "audio/ogg",
            Format::Opus => "audio/opus",
            Format::Wma => "audio/x-ms-wma",
            Format::Amr => "audio/amr",
            Format::Ra => "audio/vnd.rn-realaudio",
            Format::Oma => "audio/atrac",
            Format::Ac3 => "audio/ac3",
            Format::Ec3 => "audio/eac3",
            Format::Thd => "audio/vnd.dolby.mlp",
            Format::Dts => "audio/vnd.dts",
            Format::DtsHd => "audio/vnd.dts.hd",
            Format::Midi => "audio/midi",
            Format::Mod => "audio/mod",
            Format::Mp4 => "video/mp4",
            Format::Mov => "video/quicktime",
            Format::Mkv => "video/x-matroska",
            Format::WebM => "video/webm",
            Format::Avi => "video/x-msvideo",
            Format::Json => "application/json",
            Format::Yaml => "application/yaml",
            Format::Toml => "application/toml",
            Format::Csv => "text/csv",
            Format::Tsv => "text/tab-separated-values",
            Format::Xml => "application/xml",
            Format::Zip => "application/zip",
            Format::TarGz => "application/gzip",
            Format::TarXz => "application/x-xz",
            Format::Srt => "application/x-subrip",
            Format::WebVtt => "text/vtt",
        };
        Self(value.to_string())
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lifecycle/storage role of an artifact inside an execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStorageClass {
    /// Immutable source copied into the content-addressed store before execution.
    Source,
    /// Durable work product used by downstream graph edges.
    Intermediate,
    /// Artifact selected for final materialization/publication.
    Terminal,
    /// Artifact reused from a previous execution cache entry.
    Cached,
    /// Short-lived artifact that may be garbage-collected after the run.
    Ephemeral,
}

/// File-backed payload handle relative to an [`ArtifactStore`](super::ArtifactStore).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPayload {
    relative_path: PathBuf,
}

impl ArtifactPayload {
    pub(crate) fn new(relative_path: PathBuf) -> Self {
        Self { relative_path }
    }

    /// Store-relative payload path. Resolve it through [`ArtifactStore`](super::ArtifactStore)
    /// rather than joining it manually.
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }
}

/// First-class binary-safe artifact record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    id: ArtifactId,
    format: CanonicalFormat,
    media_type: MediaType,
    digest: ArtifactDigest,
    size_bytes: u64,
    payload: ArtifactPayload,
    #[serde(default)]
    metadata: BTreeMap<String, Value>,
    #[serde(default)]
    sources: Vec<ArtifactId>,
    storage_class: ArtifactStorageClass,
}

impl Artifact {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: ArtifactId,
        format: CanonicalFormat,
        media_type: MediaType,
        digest: ArtifactDigest,
        size_bytes: u64,
        payload: ArtifactPayload,
        metadata: BTreeMap<String, Value>,
        sources: Vec<ArtifactId>,
        storage_class: ArtifactStorageClass,
    ) -> Self {
        Self {
            id,
            format,
            media_type,
            digest,
            size_bytes,
            payload,
            metadata,
            sources,
            storage_class,
        }
    }

    /// Stable artifact identifier.
    pub fn id(&self) -> &ArtifactId {
        &self.id
    }

    /// Canonical Renderflow format identifier.
    pub fn format(&self) -> &CanonicalFormat {
        &self.format
    }

    /// MIME/media type.
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    /// SHA-256 content digest.
    pub fn digest(&self) -> &ArtifactDigest {
        &self.digest
    }

    /// Payload size in bytes.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// File-backed payload handle.
    pub fn payload(&self) -> &ArtifactPayload {
        &self.payload
    }

    /// Structured artifact metadata.
    pub fn metadata(&self) -> &BTreeMap<String, Value> {
        &self.metadata
    }

    /// Ordered parent/source artifact identifiers.
    pub fn sources(&self) -> &[ArtifactId] {
        &self.sources
    }

    /// Lifecycle/storage role.
    pub fn storage_class(&self) -> ArtifactStorageClass {
        self.storage_class
    }

    pub(crate) fn with_storage_class(mut self, storage_class: ArtifactStorageClass) -> Self {
        self.storage_class = storage_class;
        self
    }
}

/// Metadata used when importing or writing a payload into the artifact store.
#[derive(Debug, Clone)]
pub struct ArtifactDescriptor {
    pub(crate) format: CanonicalFormat,
    pub(crate) media_type: MediaType,
    pub(crate) storage_class: ArtifactStorageClass,
    pub(crate) metadata: BTreeMap<String, Value>,
    pub(crate) sources: Vec<ArtifactId>,
}

impl ArtifactDescriptor {
    /// Create a descriptor using Renderflow's canonical format/media-type mapping.
    pub fn for_format(format: Format, storage_class: ArtifactStorageClass) -> Self {
        Self {
            format: format.into(),
            media_type: MediaType::for_format(format),
            storage_class,
            metadata: BTreeMap::new(),
            sources: Vec::new(),
        }
    }

    /// Override the media type when a provider has more precise information.
    pub fn with_media_type(mut self, media_type: MediaType) -> Self {
        self.media_type = media_type;
        self
    }

    /// Add one ordered parent/source relationship.
    pub fn with_source(mut self, source: ArtifactId) -> Self {
        self.sources.push(source);
        self
    }

    /// Replace the ordered source relationship list.
    pub fn with_sources(mut self, sources: impl IntoIterator<Item = ArtifactId>) -> Self {
        self.sources = sources.into_iter().collect();
        self
    }

    /// Attach structured metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Ordered collection of artifacts used by aggregation transforms.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtifactCollection {
    artifacts: Vec<Artifact>,
}

impl ArtifactCollection {
    /// Create an ordered collection.
    pub fn new(artifacts: Vec<Artifact>) -> Self {
        Self { artifacts }
    }

    /// Create a one-element collection.
    pub fn one(artifact: Artifact) -> Self {
        Self::new(vec![artifact])
    }

    /// Number of artifacts in the collection.
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    /// Whether the collection contains no artifacts.
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    /// Iterate in the declared input order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Artifact> {
        self.artifacts.iter()
    }

    /// Borrow the ordered artifacts.
    pub fn as_slice(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// Consume the collection into its ordered artifacts.
    pub fn into_vec(self) -> Vec<Artifact> {
        self.artifacts
    }

    /// Return the only artifact, or an error when the collection is not singular.
    pub fn into_one(self) -> anyhow::Result<Artifact> {
        if self.artifacts.len() != 1 {
            anyhow::bail!(
                "expected exactly one artifact, found {}",
                self.artifacts.len()
            );
        }
        Ok(self
            .artifacts
            .into_iter()
            .next()
            .expect("length checked above"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_artifact(digest_value: char) -> Artifact {
        let digest = ArtifactDigest::sha256(digest_value.to_string().repeat(64));
        let format: CanonicalFormat = Format::Png.into();
        let id = ArtifactId::from_record(&digest, &format, &[]);
        Artifact::new(
            id,
            format,
            MediaType::for_format(Format::Png),
            digest,
            1,
            ArtifactPayload::new(format!("objects/{digest_value}").into()),
            BTreeMap::new(),
            Vec::new(),
            ArtifactStorageClass::Source,
        )
    }

    #[test]
    fn collection_preserves_input_order() {
        let artifact_a = test_artifact('a');
        let artifact_b = test_artifact('b');
        let collection = ArtifactCollection::new(vec![artifact_a.clone(), artifact_b.clone()]);
        let ids: Vec<_> = collection.iter().map(|artifact| artifact.id()).collect();
        assert_eq!(ids, vec![artifact_a.id(), artifact_b.id()]);
    }

    #[test]
    fn record_identity_includes_format_and_lineage() {
        let digest = ArtifactDigest::sha256("a".repeat(64));
        let png: CanonicalFormat = Format::Png.into();
        let webp: CanonicalFormat = Format::Webp.into();
        let source = ArtifactId::from_record(&digest, &png, &[]);
        let derived = ArtifactId::from_record(&digest, &webp, std::slice::from_ref(&source));
        assert_ne!(source, derived);
    }
}
