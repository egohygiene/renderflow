//! Artifact adapters over the existing timed-text codecs and canonical store.

use std::io::Read;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactDigest, ArtifactId,
    ArtifactStorageClass, ArtifactStore, ArtifactTransform, MediaType};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::process::ProcessCancellationToken;
use super::{normalize, parse_document, render, sha256, LyricsDocument, LyricsPolicy, MAX_DOCUMENT_BYTES};

pub const METADATA_KEY: &str = "renderflow.lyrics";

pub struct LyricsTransform {
    from: Format,
    to: Format,
    policy: LyricsPolicy,
    cancellation: ProcessCancellationToken,
}

impl LyricsTransform {
    pub fn new(from: Format, to: Format, policy: LyricsPolicy, cancellation: Option<Arc<AtomicBool>>) -> Result<Self> {
        policy.validate()?;
        ensure!((super::input_format(from).is_ok() && to == Format::LyricsJson)
            || (from == Format::LyricsJson && super::OUTPUTS.contains(&to) && to != Format::LyricsJson),
            "lyrics.edge: unsupported format pair");
        Ok(Self {from,to,policy,cancellation:cancellation.map(ProcessCancellationToken::from_shared).unwrap_or_default()})
    }
}

impl ArtifactTransform for LyricsTransform {
    fn name(&self) -> &str { "renderflow.lyrics.convert" }
    fn version(&self) -> &str { "1" }
    fn cache_identity(&self) -> String {
        format!("{}:v1:{}:{}:{}",self.name(),self.from,self.to,
            sha256(&serde_json::to_vec(&self.policy).expect("closed lyrics policy is JSON serializable")))
    }
    fn cacheable(&self) -> bool { false }
    fn fidelity(&self) -> Option<FidelityDeclaration> { Some(FidelityDeclaration::Partial) }

    fn apply(&self,input:&Artifact,output_format:Format,store:&ArtifactStore) -> Result<Artifact> {
        ensure!(!self.cancellation.is_cancelled(),"lyrics.cancelled");
        ensure!(input.format().as_str() == self.from.to_string() && output_format == self.to,"lyrics.edge.mismatch");
        let bytes = read(input,store)?;
        let document = if self.from == Format::LyricsJson {
            let document = parse_document(&bytes)?;
            ensure!(document.policy == self.policy,"lyrics.policy.changed: canonical source declarations differ");
            document
        } else {
            normalize(&bytes,self.from,&self.policy)?
        };
        let output = render(&document,self.to)?;
        let canonical_sha256 = sha256(&render(&document,Format::LyricsJson)?);
        let descriptor = ArtifactDescriptor::for_format(self.to,ArtifactStorageClass::Intermediate)
            .with_source(input.id().clone()).with_metadata(METADATA_KEY,metadata(&document,self.to,&canonical_sha256));
        ensure!(!self.cancellation.is_cancelled(),"lyrics.cancelled");
        store.put_bytes(&output,descriptor)
    }
}

fn metadata(document:&LyricsDocument,format:Format,canonical_sha256:&str) -> Value {
    json!({"schema_version":"renderflow.lyrics-artifact/v1","profile":super::PROFILE,
        "source_sha256":document.policy.source_sha256,"source_format":document.source_format,
        "policy_sha256":sha256(&serde_json::to_vec(&document.policy).expect("closed policy serialization")),
        "canonical_sha256":canonical_sha256,"review_state":document.policy.review_state,
        "review_is_declared":true,"losses":super::losses(document,format),
        "eligibility":super::eligibility(document),"runtime_qualified":false})
}

fn read(artifact:&Artifact,store:&ArtifactStore) -> Result<Vec<u8>> {
    ensure!(artifact.size_bytes() <= MAX_DOCUMENT_BYTES as u64,"lyrics.artifact.limit");
    ensure!(artifact.id() == &ArtifactId::from_record(artifact.digest(),artifact.format(),artifact.sources()),"lyrics.artifact.identity");
    let mut bytes = Vec::new();
    store.open(artifact)?.take(MAX_DOCUMENT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_DOCUMENT_BYTES && bytes.len() as u64 == artifact.size_bytes()
        && sha256(&bytes) == artifact.digest().value(),"lyrics.artifact.changed");
    Ok(bytes)
}

/// Mandatory fresh admission for each published target, including restored
/// records. Recompute from the exact root rather than trusting a success flag.
pub fn validate_published(artifact:&Artifact,root:&Artifact,store:&ArtifactStore,policy:&LyricsPolicy) -> Result<()> {
    let source_format: Format = root.format().as_str().parse()?;
    let source_bytes = read(root,store)?;
    let document = normalize(&source_bytes,source_format,policy)?;
    let bytes = read(artifact,store)?;
    if artifact.format() == root.format() {
        ensure!(artifact.id() == root.id() && bytes == source_bytes,"lyrics.source_equivalent.changed");
        return Ok(());
    }
    let format: Format = artifact.format().as_str().parse()?;
    ensure!(artifact.media_type() == &MediaType::for_format(format),"lyrics.output.media_type");
    let expected = render(&document,format)?;
    ensure!(bytes == expected,"lyrics.output.changed: derivative differs from exact manual source");
    let canonical_bytes = render(&document,Format::LyricsJson)?;
    let canonical_sha256 = sha256(&canonical_bytes);
    let canonical_id = ArtifactId::from_record(&ArtifactDigest::sha256(canonical_sha256.clone()),
        &Format::LyricsJson.into(),&[root.id().clone()]);
    let parent = if format == Format::LyricsJson {root.id().clone()} else {canonical_id};
    ensure!(artifact.sources() == [parent],"lyrics.lineage.changed");
    ensure!(artifact.metadata().get(METADATA_KEY).context("lyrics.evidence.missing")?
        == &metadata(&document,format,&canonical_sha256),"lyrics.evidence.changed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use super::super::ReviewState;

    #[test]
    fn publication_rebinds_source_bytes_policy_and_exact_derivative_content() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let bytes = b"1\n00:00:01,000 --> 00:00:02,000\nStill here\n\n";
        let policy = LyricsPolicy {source_sha256:sha256(bytes),review_state:ReviewState::Unreviewed,
            language:None,revision:None,sections:BTreeMap::new()};
        let root = store.put_bytes(bytes,ArtifactDescriptor::for_format(Format::Srt,ArtifactStorageClass::Source)).unwrap();
        let canonical = LyricsTransform::new(Format::Srt,Format::LyricsJson,policy.clone(),None).unwrap()
            .apply(&root,Format::LyricsJson,&store).unwrap();
        let output = LyricsTransform::new(Format::LyricsJson,Format::Lrc,policy.clone(),None).unwrap()
            .apply(&canonical,Format::Lrc,&store).unwrap();
        validate_published(&output,&root,&store,&policy).unwrap();
        let detached = store.put_bytes(&store.read_bytes(&output).unwrap(),
            ArtifactDescriptor::for_format(Format::Lrc,ArtifactStorageClass::Intermediate)).unwrap();
        assert!(validate_published(&detached,&root,&store,&policy).is_err());
        let mut changed = policy.clone();
        changed.review_state = ReviewState::Reviewed;
        assert!(validate_published(&output,&root,&store,&changed).is_err());
        validate_published(&root,&root,&store,&policy).unwrap();
    }

    #[test]
    fn cancellation_stops_before_creating_a_lyrics_artifact() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let bytes = b"1\n00:00:01,000 --> 00:00:02,000\nStill here\n\n";
        let policy = LyricsPolicy {source_sha256:sha256(bytes),review_state:ReviewState::Unreviewed,
            language:None,revision:None,sections:BTreeMap::new()};
        let root = store.put_bytes(bytes,ArtifactDescriptor::for_format(Format::Srt,ArtifactStorageClass::Source)).unwrap();
        let flag = Arc::new(AtomicBool::new(true));
        assert!(LyricsTransform::new(Format::Srt,Format::LyricsJson,policy,Some(flag)).unwrap()
            .apply(&root,Format::LyricsJson,&store).is_err());
    }
}
