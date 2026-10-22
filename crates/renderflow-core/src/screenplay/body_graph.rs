//! Built-in original-Fountain exports over the independently checked body profile.
//!
//! These are direct original-source edges, separate from lossy PDF/text recovery
//! and external scripttool imports. Retained canonical bytes and native exports
//! are reconstructed at publication; authored support never claims native-app
//! qualification or approval of the screenplay.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactDigest, ArtifactId, ArtifactStorageClass, ArtifactStore, ArtifactTransform, CanonicalFormat, MediaType};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::process::ProcessCancellationToken;

use super::native::{equivalent_semantics, export_body, parse_native, NativeScreenplayFormat};
use super::{parse_json, render_json, Screenplay, MAX_INPUT_BYTES};

pub const SCREENPLAY_BODY_CAPABILITY: &str = "screenplay.body.export";
pub const SCREENPLAY_BODY_ARTIFACT_METADATA_KEY: &str = "renderflow.screenplay_body";
pub const SCREENPLAY_BODY_ARTIFACT_SCHEMA: &str = "renderflow.screenplay-body-artifact/v1";
pub const SCREENPLAY_BODY_PROFILE: &str = "strict-six-kind-body-v1";
const MAX_EVIDENCE_BYTES: usize = 1024 * 1024;

/// Explicit permission to use authored built-in routes before application QA.
/// There is no provider executable, installation, download or mutable tool pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyExportPolicy {
    pub allow_unqualified: bool,
}

impl<'de> Deserialize<'de> for BodyExportPolicy {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PolicyVisitor;

        impl<'de> serde::de::Visitor<'de> for PolicyVisitor {
            type Value = BodyExportPolicy;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an original-Fountain body export policy object")
            }

            fn visit_map<M>(self, mut map: M) -> std::result::Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut allow_unqualified = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "allow_unqualified" => {
                            if allow_unqualified.is_some() {
                                return Err(serde::de::Error::duplicate_field("allow_unqualified"));
                            }
                            allow_unqualified = Some(map.next_value::<bool>()?);
                        }
                        _ => return Err(serde::de::Error::unknown_field(&key, &["allow_unqualified"])),
                    }
                }
                let allow_unqualified = allow_unqualified
                    .ok_or_else(|| serde::de::Error::missing_field("allow_unqualified"))?;
                Ok(BodyExportPolicy { allow_unqualified })
            }
        }

        // Derived struct readers also admit positional sequences. The public
        // policy is an object-only opt-in; false remains a validation diagnostic.
        deserializer.deserialize_map(PolicyVisitor)
    }
}

impl BodyExportPolicy {
    pub fn validate(&self) -> Result<()> {
        if !self.allow_unqualified {
            bail!("screenplay_body.policy: allow_unqualified must explicitly be true; native-application qualification remains deferred");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyArtifactEvidence {
    pub schema_version: String,
    pub original_source: Artifact,
    pub input_artifact_id: ArtifactId,
    pub input_sha256: String,
    /// A real retained canonical payload, not an inferred graph hop or provider claim.
    pub canonical_artifact: Artifact,
    pub canonical_sha256: String,
    pub output_format: String,
    pub output_sha256: String,
    pub policy_sha256: String,
    pub profile: String,
    pub qualification: String,
    pub runtime_qualified: bool,
    pub review_required: bool,
    pub preservation: String,
    pub limitations: Vec<String>,
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn digest_json(value: &impl Serialize) -> Result<String> { Ok(digest(&serde_json::to_vec(value)?)) }

fn bounded_evidence(value: &impl Serialize) -> Result<()> {
    super::codec::bounded_encoded_len(value, MAX_EVIDENCE_BYTES)
        .context("screenplay_body.evidence.bounds: retained body evidence exceeds 1 MiB")?;
    Ok(())
}

fn bounded_payload(artifact: &Artifact, store: &ArtifactStore) -> Result<Vec<u8>> {
    let recorded_digest = artifact.digest().value();
    if recorded_digest.len() != 64 || !recorded_digest.bytes().all(|byte|
        byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("screenplay_body.validation.payload: an exact lowercase SHA-256 identity is required");
    }
    let expected_path = Path::new("objects").join("sha256").join(&recorded_digest[..2]).join(recorded_digest);
    if artifact.payload().relative_path() != expected_path {
        bail!("screenplay_body.validation.payload: artifact payload reference differs from its content-addressed identity");
    }
    let path = store.payload_path(artifact)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink()
        || metadata.len() != artifact.size_bytes() || metadata.len() == 0
        || metadata.len() > MAX_INPUT_BYTES as u64
    {
        bail!("screenplay_body.validation.payload: payload differs from its nonempty bounded artifact record");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(MAX_INPUT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != artifact.size_bytes() || digest(&bytes) != artifact.digest().value() {
        bail!("screenplay_body.validation.payload: immutable source or derivative bytes changed");
    }
    Ok(bytes)
}

fn validate_source(source: &Artifact) -> Result<()> {
    if source.format().as_str() != "fountain" || !source.sources().is_empty()
        || source.storage_class() != ArtifactStorageClass::Source
        || source.id() != &ArtifactId::from_record(source.digest(), source.format(), source.sources())
        || source.media_type() != &MediaType::for_format(Format::Fountain)
    {
        bail!("screenplay_body.source.identity: an intact original Fountain source artifact is required");
    }
    Ok(())
}

fn native_format(format: Format) -> Result<NativeScreenplayFormat> {
    match format {
        Format::Fdx => Ok(NativeScreenplayFormat::Fdx),
        Format::FadeIn => Ok(NativeScreenplayFormat::FadeIn),
        Format::Osf => Ok(NativeScreenplayFormat::Osf),
        Format::Trelby => Ok(NativeScreenplayFormat::Trelby),
        _ => bail!("screenplay_body.route: native body exports support FDX, Fade In, OSF and Trelby only"),
    }
}

fn supported_output(format: Format) -> Result<()> {
    if format != Format::ScreenplayJson { native_format(format)?; }
    Ok(())
}

fn preservation(format: Format) -> &'static str {
    if format == Format::ScreenplayJson { "canonical_model_from_original_fountain" }
    else { "six_kind_body_semantics_with_regenerated_ids" }
}

fn limitations(format: Format) -> Vec<String> {
    let mut values = vec![
        "Only the independently accepted original Fountain body profile is supported; unsupported metadata and constructs are rejected, not stripped.".to_owned(),
        "Native application opening, typography, pagination and production suitability remain unqualified and require review.".to_owned(),
    ];
    if format != Format::ScreenplayJson {
        values.push("Native files preserve accepted ordered body text, paragraph kinds and character ownership; canonical stable IDs and original source identity remain in retained evidence rather than native body bytes.".to_owned());
    }
    values
}

fn canonical_identity(bytes: &[u8], source: &Artifact) -> ArtifactId {
    ArtifactId::from_record(&ArtifactDigest::sha256(digest(bytes)),
        &CanonicalFormat::from(Format::ScreenplayJson), &[source.id().clone()])
}

fn canonical_from_source(source: &Artifact, store: &ArtifactStore) -> Result<(Screenplay, Vec<u8>)> {
    validate_source(source)?;
    let bytes = bounded_payload(source, store)?;
    let screenplay = parse_native(&bytes, NativeScreenplayFormat::Fountain)?;
    if screenplay.source.format != "fountain" || screenplay.source.sha256 != source.digest().value() {
        bail!("screenplay_body.validation.source: parsed Fountain identity differs from the original source");
    }
    let canonical = render_json(&screenplay)?;
    if parse_json(&canonical)? != screenplay {
        bail!("screenplay_body.validation.canonical: canonical serialization changed the accepted source model");
    }
    Ok((screenplay, canonical))
}

fn checked_native_bytes(screenplay: &Screenplay, format: Format) -> Result<Vec<u8>> {
    let target = native_format(format)?;
    let bytes = export_body(screenplay, target)?;
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        bail!("screenplay_body.validation.output: native body output exceeds its nonempty bounded profile");
    }
    // This graph gate independently reparses what is actually stored, even
    // though the serializer also checks its own strict-profile output.
    let reparsed = parse_native(&bytes, target)?;
    equivalent_semantics(screenplay, &reparsed)?;
    Ok(bytes)
}

fn read_evidence(artifact: &Artifact) -> Result<BodyArtifactEvidence> {
    let value = artifact.metadata().get(SCREENPLAY_BODY_ARTIFACT_METADATA_KEY)
        .context("screenplay_body.validation.evidence: complete original-source body evidence is required")?;
    bounded_evidence(value)?;
    let evidence: BodyArtifactEvidence = serde_json::from_value(value.clone())
        .context("screenplay_body.validation.evidence: body export evidence has an invalid shape")?;
    if serde_json::to_value(&evidence)? != *value {
        bail!("screenplay_body.validation.evidence: nested artifact references contain unsupported or omitted fields");
    }
    Ok(evidence)
}

/// Expose the validated auxiliary record for retained run inventory and budgets.
/// This does not select the canonical payload for terminal publication.
pub fn retained_canonical(artifact: &Artifact, original_source: &Artifact,
    store: &ArtifactStore, policy: &BodyExportPolicy) -> Result<Artifact> {
    validate_artifact(artifact, original_source, store, policy)?;
    Ok(read_evidence(artifact)?.canonical_artifact)
}

pub struct FountainBodyArtifactTransform {
    output: Format,
    policy: BodyExportPolicy,
    cancellation: ProcessCancellationToken,
}

impl FountainBodyArtifactTransform {
    pub fn new(output: Format, policy: BodyExportPolicy, cancellation: Option<Arc<AtomicBool>>) -> Result<Self> {
        policy.validate()?;
        supported_output(output)?;
        Ok(Self { output, policy, cancellation: cancellation.map(ProcessCancellationToken::from_shared).unwrap_or_default() })
    }

    fn ensure_not_cancelled(&self) -> Result<()> {
        if self.cancellation.is_cancelled() { bail!("screenplay_body.cancelled: conversion cancelled"); }
        Ok(())
    }
}

impl ArtifactTransform for FountainBodyArtifactTransform {
    fn name(&self) -> &str { "renderflow.screenplay.fountain.body" }
    fn version(&self) -> &str { "1" }
    fn cache_identity(&self) -> String {
        format!("{}:v1:{}:{}:{}", self.name(), self.output, SCREENPLAY_BODY_PROFILE,
            digest_json(&self.policy).expect("body policy serialization is infallible"))
    }
    /// No application qualification or reusable execution receipt is claimed.
    fn cacheable(&self) -> bool { false }
    fn fidelity(&self) -> Option<FidelityDeclaration> { Some(FidelityDeclaration::Partial) }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        self.ensure_not_cancelled()?;
        if output_format != self.output {
            bail!("screenplay_body.route: requested output differs from this frozen direct edge");
        }
        let (screenplay, canonical_bytes) = canonical_from_source(input, store)?;
        let output = if self.output == Format::ScreenplayJson { canonical_bytes.clone() }
            else { checked_native_bytes(&screenplay, self.output)? };
        self.ensure_not_cancelled()?;
        // Retain canonical bytes without turning the direct Fountain edge into
        // a canonical-input route. Its reference has a bounded, empty metadata bag.
        let canonical = store.put_bytes(&canonical_bytes,
            ArtifactDescriptor::for_format(Format::ScreenplayJson, ArtifactStorageClass::Intermediate)
                .with_source(input.id().clone()))?;
        let evidence = BodyArtifactEvidence {
            schema_version: SCREENPLAY_BODY_ARTIFACT_SCHEMA.to_owned(), original_source: input.clone(),
            input_artifact_id: input.id().clone(), input_sha256: input.digest().value().to_owned(),
            canonical_artifact: canonical, canonical_sha256: digest(&canonical_bytes),
            output_format: self.output.to_string(), output_sha256: digest(&output),
            policy_sha256: digest_json(&self.policy)?, profile: SCREENPLAY_BODY_PROFILE.to_owned(),
            qualification: "authored_unqualified".to_owned(), runtime_qualified: false,
            review_required: true, preservation: preservation(self.output).to_owned(),
            limitations: limitations(self.output),
        };
        bounded_evidence(&evidence)?;
        self.ensure_not_cancelled()?;
        let artifact = store.put_bytes(&output,
            ArtifactDescriptor::for_format(self.output, ArtifactStorageClass::Intermediate)
                .with_source(input.id().clone())
                .with_metadata(SCREENPLAY_BODY_ARTIFACT_METADATA_KEY, serde_json::to_value(evidence)?))?;
        validate_artifact(&artifact, input, store, &self.policy)?;
        self.ensure_not_cancelled()?;
        Ok(artifact)
    }
}

/// Mandatory publication gate, independent of generic validation settings.
/// Reobserve original bytes and exact canonical/native payloads without invoking
/// a provider; intact hashes alone cannot validate forged semantic evidence.
pub fn validate_artifact(artifact: &Artifact, original_source: &Artifact,
    store: &ArtifactStore, policy: &BodyExportPolicy) -> Result<()> {
    policy.validate()?;
    let evidence = read_evidence(artifact)?;
    let format = artifact.format().as_str().parse::<Format>()?;
    supported_output(format)?;
    if evidence.schema_version != SCREENPLAY_BODY_ARTIFACT_SCHEMA
        || artifact.metadata().len() != 1
        || evidence.original_source != *original_source
        || evidence.input_artifact_id != *original_source.id()
        || evidence.input_sha256 != original_source.digest().value()
        || artifact.id() != &ArtifactId::from_record(artifact.digest(), artifact.format(), artifact.sources())
        || artifact.sources() != [original_source.id().clone()]
        || artifact.media_type() != &MediaType::for_format(format)
        || !matches!(artifact.storage_class(), ArtifactStorageClass::Intermediate | ArtifactStorageClass::Terminal)
        || evidence.output_format != artifact.format().as_str()
        || evidence.output_sha256 != artifact.digest().value()
        || evidence.policy_sha256 != digest_json(policy)?
        || evidence.profile != SCREENPLAY_BODY_PROFILE
        || evidence.qualification != "authored_unqualified" || evidence.runtime_qualified
        || !evidence.review_required || evidence.preservation != preservation(format)
        || evidence.limitations != limitations(format)
    {
        bail!("screenplay_body.validation.evidence: original source, direct lineage, output identity, policy or qualification differs");
    }
    let (screenplay, canonical_bytes) = canonical_from_source(original_source, store)?;
    let canonical = &evidence.canonical_artifact;
    if canonical.format().as_str() != "screenplay_json"
        || canonical.id() != &canonical_identity(&canonical_bytes, original_source)
        || canonical.sources() != [original_source.id().clone()]
        || canonical.media_type() != &MediaType::for_format(Format::ScreenplayJson)
        || canonical.storage_class() != ArtifactStorageClass::Intermediate
        || !canonical.metadata().is_empty()
        || evidence.canonical_sha256 != digest(&canonical_bytes)
        || canonical.digest().value() != evidence.canonical_sha256
        || bounded_payload(canonical, store)? != canonical_bytes
    {
        bail!("screenplay_body.validation.canonical: retained canonical reference differs from original Fountain semantics");
    }
    let bytes = bounded_payload(artifact, store)?;
    if format == Format::ScreenplayJson {
        if artifact.id() != canonical.id() || bytes != canonical_bytes || parse_json(&bytes)? != screenplay {
            bail!("screenplay_body.validation.canonical: canonical output differs from its retained source model");
        }
    } else {
        let reparsed = parse_native(&bytes, native_format(format)?)?;
        equivalent_semantics(&screenplay, &reparsed)?;
        if bytes != checked_native_bytes(&screenplay, format)? {
            bail!("screenplay_body.validation.native: native output differs from the deterministic original-source export");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
