//! Explicitly lossy recovery routes with retained extraction and review evidence.
//!
//! Every derivative carries the original source record and the complete recovery
//! candidate, including extracted page text. A native screenplay file alone is
//! not evidence of an approved reconstruction. Publication repeats the pure
//! recovery/export checks even when generic artifact validation is disabled.

use std::fs;
use std::io::Read;
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactDigest, ArtifactId, ArtifactStorageClass, ArtifactStore, ArtifactTransform, CanonicalFormat, MediaType};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::process::ProcessCancellationToken;
use crate::screenplay::{parse_json, render_json, MAX_INPUT_BYTES};
use crate::screenplay::native::NativeScreenplayFormat;

use super::export::{export, validate_export, RecoveryExportEvidence};
use super::model::{recover_pages, text_pages, validate_candidate, ExtractedPage, RecoveryCandidate, RecoveryInputFormat};
use super::provider::{self, ExtractionEvidence, ExtractionRequest, ExtractionSourceFormat, RecoveryExtractionConfig};

pub const SCREENPLAY_RECOVERY_CAPABILITY: &str = "screenplay.recover.structure";
pub const SCREENPLAY_RECOVERY_EXPORT_CAPABILITY: &str = "screenplay.recovery.export";
pub const SCREENPLAY_RECOVERY_ARTIFACT_METADATA_KEY: &str = "renderflow.screenplay_recovery";
pub const SCREENPLAY_RECOVERY_ARTIFACT_SCHEMA: &str = "renderflow.screenplay-recovery-artifact/v1";
pub const MAX_RECOVERY_ARTIFACT_EVIDENCE_BYTES: usize = 96 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryArtifactEvidence {
    pub schema_version: String,
    pub original_source: Artifact,
    pub input_artifact_id: ArtifactId,
    pub input_sha256: String,
    pub canonical_artifact_id: ArtifactId,
    pub canonical_sha256: String,
    pub output_format: String,
    pub output_sha256: String,
    pub policy_sha256: String,
    pub candidate_sha256: String,
    pub extraction_evidence_sha256: String,
    pub candidate: RecoveryCandidate,
    pub extraction_evidence: ExtractionEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_evidence: Option<RecoveryExportEvidence>,
    pub review_required: bool,
    pub lossy: bool,
    pub qualification: String,
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn digest_json(value: &impl Serialize) -> Result<String> { Ok(digest(&serde_json::to_vec(value)?)) }

fn extraction_error(error: provider::ExtractionError) -> anyhow::Error {
    let message = format!("{}: {error}", error.code());
    anyhow::Error::new(error).context(message)
}

fn bounded_evidence(value: &impl Serialize) -> Result<()> {
    crate::screenplay::codec::bounded_encoded_len(value, MAX_RECOVERY_ARTIFACT_EVIDENCE_BYTES)
        .context("screenplay_recovery.evidence.bounds: retained recovery evidence exceeds its bound")?;
    Ok(())
}

fn bounded_payload(artifact: &Artifact, store: &ArtifactStore, maximum: u64) -> Result<Vec<u8>> {
    let path = store.payload_path(artifact)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink()
        || metadata.len() != artifact.size_bytes() || metadata.len() == 0 || metadata.len() > maximum
    {
        bail!("screenplay_recovery.validation.payload: payload differs from its bounded artifact record");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(maximum.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != artifact.size_bytes() || digest(&bytes) != artifact.digest().value() {
        bail!("screenplay_recovery.validation.payload: source or derivative bytes changed after intake");
    }
    Ok(bytes)
}

fn source_format(source: &Artifact) -> Result<(ExtractionSourceFormat, RecoveryInputFormat)> {
    match source.format().as_str() {
        "pdf" => Ok((ExtractionSourceFormat::Pdf, RecoveryInputFormat::Pdf)),
        "text" => Ok((ExtractionSourceFormat::Text, RecoveryInputFormat::Text)),
        _ => bail!("screenplay_recovery.route: recovery requires an original PDF or unstructured text source"),
    }
}

fn validate_source_identity(source: &Artifact) -> Result<()> {
    let (format, _) = source_format(source)?;
    let expected_format = match format { ExtractionSourceFormat::Pdf => Format::Pdf, ExtractionSourceFormat::Text => Format::Text };
    if !source.sources().is_empty()
        || source.id() != &ArtifactId::from_record(source.digest(), source.format(), source.sources())
        || source.media_type() != &MediaType::for_format(expected_format)
    {
        bail!("screenplay_recovery.source.identity: an intact original source artifact is required");
    }
    Ok(())
}

fn canonical_identity(bytes: &[u8], source: &Artifact) -> ArtifactId {
    ArtifactId::from_record(&ArtifactDigest::sha256(digest(bytes)),
        &CanonicalFormat::from(Format::ScreenplayJson), &[source.id().clone()])
}

fn candidate_pages(candidate: &RecoveryCandidate) -> Vec<ExtractedPage> {
    candidate.pages.iter().map(|page| ExtractedPage {
        page_number: page.page_number, text: page.text.clone(), method: page.method,
    }).collect()
}

fn native_format(format: Format) -> Result<NativeScreenplayFormat> {
    match format {
        Format::Fountain => Ok(NativeScreenplayFormat::Fountain),
        Format::Fdx => Ok(NativeScreenplayFormat::Fdx),
        Format::FadeIn => Ok(NativeScreenplayFormat::FadeIn),
        Format::Osf => Ok(NativeScreenplayFormat::Osf),
        _ => bail!("screenplay_recovery.route: recovered exports support Fountain, FDX, Fade In, and OSF only"),
    }
}

fn read_evidence(artifact: &Artifact) -> Result<RecoveryArtifactEvidence> {
    let value = artifact.metadata().get(SCREENPLAY_RECOVERY_ARTIFACT_METADATA_KEY)
        .context("screenplay_recovery.validation.evidence: complete recovery evidence is required")?;
    bounded_evidence(value)?;
    serde_json::from_value(value.clone())
        .context("screenplay_recovery.validation.evidence: recovery evidence has an invalid shape")
}

/// Revalidates the retained extraction envelope against the original bytes and
/// configured pins without invoking any native provider a second time.
fn validate_recovery_evidence(evidence: &RecoveryArtifactEvidence, store: &ArtifactStore,
    policy: &RecoveryExtractionConfig) -> Result<Vec<u8>> {
    policy.validate().map_err(extraction_error)?;
    bounded_evidence(evidence)?;
    validate_source_identity(&evidence.original_source)?;
    let source = &evidence.original_source;
    let (extraction_format, recovery_format) = source_format(source)?;
    if evidence.schema_version != SCREENPLAY_RECOVERY_ARTIFACT_SCHEMA
        || !evidence.review_required || !evidence.lossy || evidence.qualification != "authored_unqualified"
        || evidence.policy_sha256 != digest_json(policy)?
        || evidence.candidate_sha256 != digest_json(&evidence.candidate)?
        || evidence.extraction_evidence_sha256 != digest_json(&evidence.extraction_evidence)?
        || evidence.candidate.source_sha256 != source.digest().value()
        || evidence.candidate.source_format != recovery_format
        || evidence.extraction_evidence.source_format != extraction_format
        || evidence.extraction_evidence.source_bytes != source.size_bytes()
    {
        bail!("screenplay_recovery.validation.evidence: source, configuration, review state, or recovery evidence identity differs");
    }
    let source_bytes = bounded_payload(source, store, policy.limits.max_input_bytes)?;
    validate_candidate(&evidence.candidate)?;
    let pages = candidate_pages(&evidence.candidate);
    provider::validate_extraction_evidence(&pages, &evidence.extraction_evidence, policy, source.digest().value())
        .map_err(extraction_error)?;
    // Text recovery can be reconstructed from the actual source without a
    // native extractor. PDF evidence remains a provider observation, not an
    // assertion that extracted words exactly match the visual source.
    if extraction_format == ExtractionSourceFormat::Text
        && recover_pages(&text_pages(&source_bytes)?, source.digest().value(), recovery_format)? != evidence.candidate
    {
        bail!("screenplay_recovery.validation.source: candidate text differs from the immutable original source");
    }
    let canonical = render_json(&evidence.candidate.screenplay)?;
    if evidence.canonical_sha256 != digest(&canonical)
        || evidence.canonical_artifact_id != canonical_identity(&canonical, source)
    {
        bail!("screenplay_recovery.validation.canonical: recovered canonical identity differs");
    }
    Ok(canonical)
}

pub struct RecoveryArtifactTransform {
    policy: RecoveryExtractionConfig,
    cancellation: ProcessCancellationToken,
}

impl RecoveryArtifactTransform {
    pub fn new(policy: RecoveryExtractionConfig, cancellation: Option<Arc<AtomicBool>>) -> Result<Self> {
        policy.validate().map_err(extraction_error)?;
        Ok(Self { policy, cancellation: cancellation.map(ProcessCancellationToken::from_shared).unwrap_or_default() })
    }

    fn ensure_not_cancelled(&self) -> Result<()> {
        if self.cancellation.is_cancelled() { return Err(extraction_error(provider::ExtractionError::Cancelled)); }
        Ok(())
    }
}

impl ArtifactTransform for RecoveryArtifactTransform {
    fn name(&self) -> &str { "renderflow.screenplay.recovery" }
    fn version(&self) -> &str { "1" }
    fn cache_identity(&self) -> String {
        format!("{}:v1:{}", self.name(), digest_json(&self.policy).expect("policy serialization is infallible"))
    }
    fn cacheable(&self) -> bool { false }
    fn fidelity(&self) -> Option<FidelityDeclaration> { Some(FidelityDeclaration::Lossy) }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        self.ensure_not_cancelled()?;
        if output_format != Format::ScreenplayJson {
            bail!("screenplay_recovery.route: recovery produces validated canonical screenplay JSON first");
        }
        validate_source_identity(input)?;
        let (source_format, recovery_format) = source_format(input)?;
        bounded_payload(input, store, self.policy.limits.max_input_bytes)?;
        let extracted = provider::extract(&ExtractionRequest {
            input: store.payload_path(input)?, source_format,
            expected_source_sha256: Some(input.digest().value().to_owned()), config: self.policy.clone(),
        }, self.cancellation.clone()).map_err(extraction_error)?;
        self.ensure_not_cancelled()?;
        if extracted.source_sha256 != input.digest().value() {
            bail!("screenplay_recovery.source.identity: extractor returned a different source identity");
        }
        let candidate = recover_pages(&extracted.pages, input.digest().value(), recovery_format)?;
        validate_candidate(&candidate)?;
        let bytes = render_json(&candidate.screenplay)?;
        let evidence = RecoveryArtifactEvidence {
            schema_version: SCREENPLAY_RECOVERY_ARTIFACT_SCHEMA.to_owned(), original_source: input.clone(),
            input_artifact_id: input.id().clone(), input_sha256: input.digest().value().to_owned(),
            canonical_artifact_id: canonical_identity(&bytes, input), canonical_sha256: digest(&bytes),
            output_format: Format::ScreenplayJson.to_string(), output_sha256: digest(&bytes),
            policy_sha256: digest_json(&self.policy)?, candidate_sha256: digest_json(&candidate)?,
            extraction_evidence_sha256: digest_json(&extracted.evidence)?, candidate,
            extraction_evidence: extracted.evidence, export_evidence: None,
            review_required: true, lossy: true, qualification: "authored_unqualified".to_owned(),
        };
        validate_recovery_evidence(&evidence, store, &self.policy)?;
        self.ensure_not_cancelled()?;
        store.put_bytes(&bytes, ArtifactDescriptor::for_format(Format::ScreenplayJson, ArtifactStorageClass::Intermediate)
            .with_source(input.id().clone())
            .with_metadata(SCREENPLAY_RECOVERY_ARTIFACT_METADATA_KEY, serde_json::to_value(evidence)?))
    }
}

pub struct RecoveryExportArtifactTransform {
    output: Format,
    policy: RecoveryExtractionConfig,
}

impl RecoveryExportArtifactTransform {
    pub fn new(output: Format, policy: RecoveryExtractionConfig) -> Result<Self> {
        policy.validate().map_err(extraction_error)?;
        native_format(output)?;
        Ok(Self { output, policy })
    }
}

impl ArtifactTransform for RecoveryExportArtifactTransform {
    fn name(&self) -> &str { "renderflow.screenplay.recovery.export" }
    fn version(&self) -> &str { "1" }
    fn cache_identity(&self) -> String {
        format!("{}:v1:{}:{}", self.name(), self.output, digest_json(&self.policy).expect("policy serialization is infallible"))
    }
    fn cacheable(&self) -> bool { false }
    fn fidelity(&self) -> Option<FidelityDeclaration> { Some(FidelityDeclaration::Lossy) }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        if input.format().as_str() != "screenplay_json" || output_format != self.output {
            bail!("screenplay_recovery.route: native export requires this route's recovered canonical artifact");
        }
        let mut evidence = read_evidence(input)?;
        validate_published_artifact(input, &evidence.original_source, store, &self.policy)?;
        let output = export(&evidence.candidate, native_format(self.output)?)?;
        validate_export(&evidence.candidate, native_format(self.output)?, &output.bytes, &output.evidence)?;
        evidence.input_artifact_id = input.id().clone();
        evidence.input_sha256 = input.digest().value().to_owned();
        evidence.output_format = self.output.to_string();
        evidence.output_sha256 = digest(&output.bytes);
        evidence.export_evidence = Some(output.evidence);
        bounded_evidence(&evidence)?;
        store.put_bytes(&output.bytes, ArtifactDescriptor::for_format(self.output, ArtifactStorageClass::Intermediate)
            .with_source(input.id().clone())
            .with_metadata(SCREENPLAY_RECOVERY_ARTIFACT_METADATA_KEY, serde_json::to_value(evidence)?))
    }
}

/// Mandatory publication gate for both canonical and native recovery outputs.
/// This verifies hashes and retained observations; it never approves recovery,
/// re-runs OCR, or claims visual fidelity to the PDF.
pub fn validate_published_artifact(artifact: &Artifact, original_source: &Artifact,
    store: &ArtifactStore, policy: &RecoveryExtractionConfig) -> Result<()> {
    let evidence = read_evidence(artifact)?;
    let format = artifact.format().as_str().parse::<Format>()?;
    if evidence.original_source != *original_source
        || artifact.id() != &ArtifactId::from_record(artifact.digest(), artifact.format(), artifact.sources())
        || artifact.sources() != [evidence.input_artifact_id.clone()]
        || evidence.output_format != artifact.format().as_str()
        || evidence.output_sha256 != artifact.digest().value()
        || artifact.media_type() != &MediaType::for_format(format)
    {
        bail!("screenplay_recovery.validation.lineage: original source, actual parent, or output identity differs");
    }
    let canonical = validate_recovery_evidence(&evidence, store, policy)?;
    let bytes = bounded_payload(artifact, store, MAX_INPUT_BYTES as u64)?;
    if format == Format::ScreenplayJson {
        if evidence.input_artifact_id != *original_source.id()
            || evidence.input_sha256 != original_source.digest().value()
            || evidence.canonical_artifact_id != *artifact.id()
            || evidence.export_evidence.is_some() || bytes != canonical
            || parse_json(&bytes)? != evidence.candidate.screenplay
        {
            bail!("screenplay_recovery.validation.canonical: canonical bytes or direct recovery lineage differs");
        }
    } else {
        if evidence.input_artifact_id != evidence.canonical_artifact_id
            || evidence.input_sha256 != evidence.canonical_sha256
        {
            bail!("screenplay_recovery.validation.lineage: native export must descend from the retained canonical recovery");
        }
        let export_evidence = evidence.export_evidence.as_ref()
            .context("screenplay_recovery.validation.export: native export evidence is missing")?;
        validate_export(&evidence.candidate, native_format(format)?, &bytes, export_evidence)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
