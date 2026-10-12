//! Explicit scripttool conversions over immutable graph inputs.
//!
//! Native input and output are independently checked by the bounded screenplay
//! adapter. Its semantic comparison and loss disclosure travel in the artifact
//! manifest; a successful process exit alone cannot publish a conversion.

use std::fs;
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactId, ArtifactStorageClass, ArtifactStore, ArtifactTransform};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::process::ProcessCancellationToken;

use super::native::NativeScreenplayFormat;
use super::provider::{self, ScripttoolConfig, ScripttoolEvidence, ScripttoolRequest};
use super::MAX_INPUT_BYTES;

pub const SCREENPLAY_ARTIFACT_METADATA_KEY: &str = "renderflow.screenplay";
pub const SCREENPLAY_ARTIFACT_SCHEMA: &str = "renderflow.screenplay-artifact/v1";
const MAX_EVIDENCE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayArtifactEvidence {
    pub schema_version: String,
    pub input_artifact_id: ArtifactId,
    pub input_sha256: String,
    pub output_sha256: String,
    /// Digest of the serialized provider evidence embedded immediately below.
    pub provider_evidence_sha256: String,
    pub provider_evidence: ScripttoolEvidence,
}

pub fn native_format(format: Format) -> Option<NativeScreenplayFormat> {
    match format {
        Format::Fountain => Some(NativeScreenplayFormat::Fountain),
        Format::Fdx => Some(NativeScreenplayFormat::Fdx),
        Format::FadeIn => Some(NativeScreenplayFormat::FadeIn),
        Format::Osf => Some(NativeScreenplayFormat::Osf),
        _ => None,
    }
}

pub struct ScreenplayArtifactTransform {
    input_format: Format,
    output_format: Format,
    policy: ScripttoolConfig,
    cancellation: ProcessCancellationToken,
}

impl ScreenplayArtifactTransform {
    pub fn new(input: Format, output: Format, policy: ScripttoolConfig, cancellation: Option<Arc<AtomicBool>>) -> Result<Self> {
        policy.validate()?;
        let from = native_format(input).context("screenplay graph input is not a native screenplay format")?;
        let to = native_format(output).context("screenplay graph output is not a native screenplay format")?;
        if !provider::supported_route(from, to) {
            bail!("unsupported scripttool graph edge '{input}' -> '{output}'; only the adopted direct imports to Fountain may execute");
        }
        Ok(Self {
            input_format: input,
            output_format: output,
            policy,
            cancellation: cancellation.map(ProcessCancellationToken::from_shared).unwrap_or_default(),
        })
    }

    fn ensure_not_cancelled(&self) -> Result<()> {
        if self.cancellation.is_cancelled() { bail!("screenplay.cancelled: conversion cancelled"); }
        Ok(())
    }
}

impl ArtifactTransform for ScreenplayArtifactTransform {
    fn name(&self) -> &str { "renderflow.screenplay.scripttool" }

    fn version(&self) -> &str { "1" }

    fn cache_identity(&self) -> String {
        let policy = serde_json::to_vec(&self.policy).expect("ScripttoolConfig serialization is infallible");
        format!("{}:v1:{}:{}:{:x}", self.name(), self.input_format, self.output_format, Sha256::digest(&policy))
    }

    /// No generic native cache identity is qualified for scripttool yet.
    fn cacheable(&self) -> bool { false }

    fn fidelity(&self) -> Option<FidelityDeclaration> { Some(FidelityDeclaration::Partial) }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        self.ensure_not_cancelled()?;
        if input.format().as_str() != self.input_format.to_string() || output_format != self.output_format {
            bail!("screenplay transform received formats inconsistent with its frozen direct edge");
        }
        if input.id() != &ArtifactId::from_record(input.digest(), input.format(), input.sources())
            || !input.sources().is_empty()
        {
            bail!("screenplay transform requires the original source artifact with an intact record identity");
        }
        if input.size_bytes() == 0 || input.size_bytes() > MAX_INPUT_BYTES as u64 {
            bail!("screenplay input exceeds its nonempty bounded profile");
        }
        let path = store.payload_path(input)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != input.size_bytes() {
            bail!("screenplay immutable input no longer matches its artifact record");
        }
        let run = provider::convert(&ScripttoolRequest {
            input: path,
            source_format: native_format(self.input_format).context("native input format missing")?,
            target_format: native_format(output_format).context("native output format missing")?,
            config: self.policy.clone(),
            expected_source_sha256: Some(input.digest().value().to_owned()),
        }, self.cancellation.clone())?;
        self.ensure_not_cancelled()?;
        if run.output.is_empty() || run.output.len() > MAX_INPUT_BYTES {
            bail!("screenplay output exceeds its nonempty bounded profile");
        }
        let output_sha256 = format!("{:x}", Sha256::digest(&run.output));
        if run.evidence.source_sha256 != input.digest().value() || run.evidence.output_sha256 != output_sha256 {
            bail!("screenplay provider evidence does not bind the selected input and output payloads");
        }
        let evidence_bytes = serde_json::to_vec(&run.evidence)?;
        if evidence_bytes.len() > MAX_EVIDENCE_BYTES {
            bail!("screenplay provider evidence exceeds the artifact metadata bound");
        }
        let evidence = ScreenplayArtifactEvidence {
            schema_version: SCREENPLAY_ARTIFACT_SCHEMA.to_owned(),
            input_artifact_id: input.id().clone(),
            input_sha256: input.digest().value().to_owned(),
            output_sha256,
            provider_evidence_sha256: format!("{:x}", Sha256::digest(&evidence_bytes)),
            provider_evidence: run.evidence,
        };
        self.ensure_not_cancelled()?;
        store.put_bytes(&run.output, ArtifactDescriptor::for_format(output_format, ArtifactStorageClass::Intermediate)
            .with_source(input.id().clone())
            .with_metadata(SCREENPLAY_ARTIFACT_METADATA_KEY, serde_json::to_value(evidence)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> ScripttoolConfig {
        ScripttoolConfig {
            executable: std::env::temp_dir().join("scripttool"),
            executable_sha256: "a".repeat(64),
            expected_version: "0.0.10".to_owned(),
            timeout_seconds: 30,
            allow_unqualified: true,
        }
    }

    #[test]
    fn only_the_three_direct_imports_are_registered_by_the_transform() {
        for input in [Format::Fdx, Format::FadeIn, Format::Osf] {
            let transform = ScreenplayArtifactTransform::new(input, Format::Fountain, policy(), None).unwrap();
            assert!(!transform.cacheable());
            assert_eq!(transform.fidelity(), Some(FidelityDeclaration::Partial));
            assert!(ScreenplayArtifactTransform::new(Format::Fountain, input, policy(), None).is_err());
        }
        for output in [Format::Json, Format::ScreenplayJson, Format::Pdf, Format::Fountain] {
            assert!(ScreenplayArtifactTransform::new(Format::Fountain, output, policy(), None).is_err());
        }
    }

    #[test]
    fn cancellation_blocks_before_any_provider_or_output_write() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temporary.path()).unwrap();
        let source = store.put_bytes(b"input", ArtifactDescriptor::for_format(Format::Fdx, ArtifactStorageClass::Source)).unwrap();
        let cancelled = Arc::new(AtomicBool::new(true));
        let transform = ScreenplayArtifactTransform::new(Format::Fdx, Format::Fountain, policy(), Some(cancelled)).unwrap();
        let error = transform.apply(&source, Format::Fountain, &store).unwrap_err();
        assert!(error.to_string().contains("screenplay.cancelled"));
    }

    #[test]
    fn derived_inputs_cannot_bypass_the_direct_original_source_contract() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temporary.path()).unwrap();
        let original = store.put_bytes(b"original", ArtifactDescriptor::for_format(Format::Fdx, ArtifactStorageClass::Source)).unwrap();
        let derived = store.put_bytes(b"derived", ArtifactDescriptor::for_format(Format::Fdx, ArtifactStorageClass::Intermediate)
            .with_source(original.id().clone())).unwrap();
        let transform = ScreenplayArtifactTransform::new(Format::Fdx, Format::Fountain, policy(), None).unwrap();
        let error = transform.apply(&derived, Format::Fountain, &store).unwrap_err();
        assert!(error.to_string().contains("original source artifact"));
    }

    #[test]
    fn declared_policy_changes_step_identity_without_enabling_reuse() {
        let first = ScreenplayArtifactTransform::new(Format::Fdx, Format::Fountain, policy(), None).unwrap();
        let mut changed = policy();
        changed.executable_sha256 = "b".repeat(64);
        let second = ScreenplayArtifactTransform::new(Format::Fdx, Format::Fountain, changed, None).unwrap();
        assert_ne!(first.cache_identity(), second.cache_identity());
        assert!(!first.cacheable());
        assert!(!second.cacheable());
    }
}
