//! Opt-in direct rendering of an original Fountain source through afterwriting.
//!
//! The transform and terminal publication both inspect the actual PDF bytes.
//! Provider claims, analysis, source lineage and output identity remain distinct
//! evidence; no generic cache or checkpoint is qualified for this route.

use std::fs;
use std::io::Read;
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactId, ArtifactStorageClass, ArtifactStore, ArtifactTransform};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::process::ProcessCancellationToken;

use super::afterwriting::{self, AfterwritingConfig, AfterwritingEvidence, AfterwritingRequest};
use super::analysis::{analyze_fountain, ScreenplayAnalysis};
use super::pdf::{inspect_pdf, ExpectedScreenplayPdf, MAX_SCREENPLAY_PDF_BYTES};
use super::MAX_INPUT_BYTES;

pub const SCREENPLAY_PDF_CAPABILITY: &str = "screenplay.render.fountain.pdf";
pub const SCREENPLAY_PDF_ARTIFACT_METADATA_KEY: &str = "renderflow.screenplay_pdf";
pub const SCREENPLAY_PDF_ARTIFACT_SCHEMA: &str = "renderflow.screenplay-pdf-artifact/v1";
const MAX_EVIDENCE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayPdfArtifactEvidence {
    pub schema_version: String,
    pub input_artifact_id: ArtifactId,
    pub input_sha256: String,
    pub output_sha256: String,
    pub policy_sha256: String,
    pub provider_evidence_sha256: String,
    pub analysis_sha256: String,
    pub provider_evidence: AfterwritingEvidence,
    pub analysis: ScreenplayAnalysis,
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn digest_json(value: &impl Serialize) -> Result<String> { Ok(digest(&serde_json::to_vec(value)?)) }

fn bounded_payload(artifact: &Artifact, store: &ArtifactStore, maximum: usize) -> Result<Vec<u8>> {
    let path = store.payload_path(artifact)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink()
        || metadata.len() != artifact.size_bytes() || metadata.len() == 0 || metadata.len() > maximum as u64
    {
        bail!("screenplay_pdf.validation.payload: payload differs from its bounded artifact record");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != artifact.size_bytes() || digest(&bytes) != artifact.digest().value() {
        bail!("screenplay_pdf.validation.payload: payload bytes changed after intake or rendering");
    }
    Ok(bytes)
}

fn validate_provider_claims(evidence: &AfterwritingEvidence, analysis: &ScreenplayAnalysis, policy: &AfterwritingConfig) -> Result<()> {
    let expected_argv = ["--no-addons", "--no-global-search-paths", "package/awc.js", "--source",
        "source.fountain", "--pdf", "output.pdf", "--config", "profile.json"].map(str::to_owned);
    if evidence.schema_version != afterwriting::AFTERWRITING_EVIDENCE_SCHEMA
        || evidence.provider_id != afterwriting::AFTERWRITING_PROVIDER_ID
        || evidence.node_provider_id != afterwriting::NODE_PROVIDER_ID
        || evidence.provider_version != policy.expected_provider_version
        || evidence.node_version != policy.expected_node_version
        || evidence.node_sha256 != policy.node_sha256
        || evidence.package_tree_sha256 != policy.package_tree_sha256
        || evidence.package_tree_algorithm != afterwriting::PACKAGE_TREE_ALGORITHM
        || evidence.adapter_reference_commit != afterwriting::AFTERWRITING_SOURCE_REF
        || evidence.package_file_count == 0 || evidence.package_total_bytes == 0
        || evidence.profile != policy.profile || evidence.settings != policy.profile.settings()
        || evidence.config_sha256 != digest_json(&policy.profile.settings())?
        || evidence.argv != expected_argv || evidence.argv_sha256 != digest_json(&evidence.argv)?
        || evidence.source_format != "fountain" || evidence.target_format != "pdf"
        || evidence.source_sha256 != analysis.source.sha256
        || evidence.source_element_count != analysis.total_elements
        || !evidence.input_structure_validated || !evidence.output_structure_validated
        || evidence.qualification != "authored_unqualified" || evidence.fidelity != "layout_derivative"
        || evidence.executable_source_attested || evidence.dependency_closure_attested
        || evidence.inspection.byte_reproducibility_verified
        || evidence.inspection.source_text_equivalence_verified || evidence.inspection.visual_fidelity_verified
    {
        bail!("screenplay_pdf.validation.provider: provider claims differ from the frozen configuration or adopted evidence contract");
    }
    Ok(())
}

fn expected_pdf(policy: &AfterwritingConfig) -> ExpectedScreenplayPdf {
    let (paper_width_pt, paper_height_pt) = policy.profile.dimensions();
    ExpectedScreenplayPdf { paper_width_pt, paper_height_pt }
}

pub struct AfterwritingArtifactTransform {
    policy: AfterwritingConfig,
    cancellation: ProcessCancellationToken,
}

impl AfterwritingArtifactTransform {
    pub fn new(policy: AfterwritingConfig, cancellation: Option<Arc<AtomicBool>>) -> Result<Self> {
        policy.validate()?;
        Ok(Self {
            policy,
            cancellation: cancellation.map(ProcessCancellationToken::from_shared).unwrap_or_default(),
        })
    }

    fn ensure_not_cancelled(&self) -> Result<()> {
        if self.cancellation.is_cancelled() { bail!("screenplay_pdf.cancelled: rendering cancelled"); }
        Ok(())
    }
}

impl ArtifactTransform for AfterwritingArtifactTransform {
    fn name(&self) -> &str { "renderflow.screenplay.afterwriting" }
    fn version(&self) -> &str { "1" }
    fn cache_identity(&self) -> String {
        format!("{}:v1:{}", self.name(), digest_json(&self.policy).expect("policy serialization is infallible"))
    }
    fn cacheable(&self) -> bool { false }
    fn fidelity(&self) -> Option<FidelityDeclaration> { Some(FidelityDeclaration::Partial) }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        self.ensure_not_cancelled()?;
        if input.format().as_str() != "fountain" || output_format != Format::Pdf {
            bail!("screenplay_pdf.route: only original Fountain to PDF is supported");
        }
        if input.id() != &ArtifactId::from_record(input.digest(), input.format(), input.sources()) || !input.sources().is_empty() {
            bail!("screenplay_pdf.source.identity: an intact original source artifact is required; derived Fountain cannot silently replace native source ancestry");
        }
        if input.size_bytes() == 0 || input.size_bytes() > MAX_INPUT_BYTES as u64 {
            bail!("screenplay_pdf.source.bounds: Fountain input exceeds the nonempty bounded profile");
        }
        let path = store.payload_path(input)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != input.size_bytes() {
            bail!("screenplay_pdf.source.identity: source payload no longer matches its immutable artifact record");
        }
        let run = afterwriting::render(&AfterwritingRequest {
            input: path,
            config: self.policy.clone(),
            expected_source_sha256: Some(input.digest().value().to_owned()),
        }, self.cancellation.clone())?;
        self.ensure_not_cancelled()?;
        let output_sha256 = digest(&run.output);
        let analysis_sha256 = digest_json(&run.analysis)?;
        if run.evidence.source_sha256 != input.digest().value()
            || run.evidence.output_sha256 != output_sha256
            || run.evidence.analysis_sha256 != analysis_sha256
        {
            bail!("screenplay_pdf.evidence.identity: provider evidence does not bind the selected source, PDF, and independent analysis");
        }
        let source_analysis = analyze_fountain(&bounded_payload(input, store, MAX_INPUT_BYTES)?)?;
        if run.analysis != source_analysis {
            bail!("screenplay_pdf.evidence.analysis: analysis differs from the original source bytes");
        }
        validate_provider_claims(&run.evidence, &run.analysis, &self.policy)?;
        let inspection = inspect_pdf(&run.output, expected_pdf(&self.policy))?;
        if inspection != run.evidence.inspection {
            bail!("screenplay_pdf.evidence.inspection: actual PDF inspection differs from the provider evidence");
        }
        let evidence = ScreenplayPdfArtifactEvidence {
            schema_version: SCREENPLAY_PDF_ARTIFACT_SCHEMA.to_owned(),
            input_artifact_id: input.id().clone(),
            input_sha256: input.digest().value().to_owned(),
            output_sha256,
            policy_sha256: digest_json(&self.policy)?,
            provider_evidence_sha256: digest_json(&run.evidence)?,
            analysis_sha256,
            provider_evidence: run.evidence,
            analysis: run.analysis,
        };
        let evidence_bytes = serde_json::to_vec(&evidence)?;
        if evidence_bytes.len() > MAX_EVIDENCE_BYTES {
            bail!("screenplay_pdf.evidence.bounds: artifact evidence exceeds its bound");
        }
        self.ensure_not_cancelled()?;
        store.put_bytes(&run.output, ArtifactDescriptor::for_format(Format::Pdf, ArtifactStorageClass::Intermediate)
            .with_source(input.id().clone())
            .with_metadata(SCREENPLAY_PDF_ARTIFACT_METADATA_KEY, serde_json::to_value(evidence)?))
    }
}

/// Mandatory terminal check independent of generic PDF envelope validation and
/// execution.validation.required. Rejects changed bytes or detached evidence
/// before an output locator is published.
pub fn validate_published_artifact(
    artifact: &Artifact,
    source: &Artifact,
    store: &ArtifactStore,
    policy: &AfterwritingConfig,
) -> Result<()> {
    policy.validate()?;
    if artifact.format().as_str() != "pdf" || source.format().as_str() != "fountain"
        || !source.sources().is_empty() || artifact.sources() != [source.id().clone()]
        || artifact.id() != &ArtifactId::from_record(artifact.digest(), artifact.format(), artifact.sources())
        || source.id() != &ArtifactId::from_record(source.digest(), source.format(), source.sources())
    {
        bail!("screenplay_pdf.validation.lineage: original Fountain source and direct PDF identity are required");
    }
    let value = artifact.metadata().get(SCREENPLAY_PDF_ARTIFACT_METADATA_KEY)
        .context("screenplay_pdf.validation.evidence: screenplay PDF artifact evidence is missing")?;
    if serde_json::to_vec(value)?.len() > MAX_EVIDENCE_BYTES {
        bail!("screenplay_pdf.validation.evidence: metadata exceeds its bound");
    }
    let evidence: ScreenplayPdfArtifactEvidence = serde_json::from_value(value.clone())?;
    if evidence.schema_version != SCREENPLAY_PDF_ARTIFACT_SCHEMA
        || evidence.input_artifact_id != *source.id()
        || evidence.input_sha256 != source.digest().value()
        || evidence.output_sha256 != artifact.digest().value()
        || evidence.policy_sha256 != digest_json(policy)?
        || evidence.provider_evidence_sha256 != digest_json(&evidence.provider_evidence)?
        || evidence.analysis_sha256 != digest_json(&evidence.analysis)?
        || evidence.provider_evidence.source_sha256 != evidence.input_sha256
        || evidence.provider_evidence.output_sha256 != evidence.output_sha256
        || evidence.provider_evidence.analysis_sha256 != evidence.analysis_sha256
    {
        bail!("screenplay_pdf.validation.evidence: source, output, policy, analysis, or provider evidence digest differs");
    }
    let source_analysis = analyze_fountain(&bounded_payload(source, store, MAX_INPUT_BYTES)?)?;
    if evidence.analysis != source_analysis {
        bail!("screenplay_pdf.validation.analysis: recorded counts differ from independently reanalyzed source bytes");
    }
    validate_provider_claims(&evidence.provider_evidence, &evidence.analysis, policy)?;
    let bytes = bounded_payload(artifact, store, MAX_SCREENPLAY_PDF_BYTES)?;
    if inspect_pdf(&bytes, expected_pdf(policy))? != evidence.provider_evidence.inspection {
        bail!("screenplay_pdf.validation.inspection: terminal PDF inspection differs from recorded evidence");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::afterwriting::ScreenplayPdfProfile;

    fn policy() -> AfterwritingConfig {
        AfterwritingConfig {
            node_executable: std::env::temp_dir().join("node"),
            node_sha256: "a".repeat(64),
            expected_node_version: "v24.0.0".to_owned(),
            package_root: std::env::temp_dir().join("afterwriting"),
            package_tree_sha256: "b".repeat(64),
            expected_provider_version: "1.17.3".to_owned(),
            profile: ScreenplayPdfProfile::A4,
            timeout_seconds: 30,
            allow_unqualified: true,
        }
    }

    #[test]
    fn cancellation_prevents_all_rendering_and_output_storage() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temporary.path()).unwrap();
        let source = store.put_bytes(b"INT. LAB - DAY\n\nA light blinks.\n", ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Source)).unwrap();
        let transform = AfterwritingArtifactTransform::new(policy(), Some(Arc::new(AtomicBool::new(true)))).unwrap();
        let error = transform.apply(&source, Format::Pdf, &store).unwrap_err();
        assert!(error.to_string().contains("screenplay_pdf.cancelled"));
    }

    #[test]
    fn derived_fountain_cannot_disguise_native_source_ancestry() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temporary.path()).unwrap();
        let original = store.put_bytes(b"native", ArtifactDescriptor::for_format(Format::Fdx, ArtifactStorageClass::Source)).unwrap();
        let derived = store.put_bytes(b"INT. LAB - DAY", ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Intermediate).with_source(original.id().clone())).unwrap();
        let transform = AfterwritingArtifactTransform::new(policy(), None).unwrap();
        assert!(transform.apply(&derived, Format::Pdf, &store).unwrap_err().to_string().contains("source.identity"));
    }

    #[test]
    fn same_format_and_other_target_routes_fail_before_provider_io() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temporary.path()).unwrap();
        let source = store.put_bytes(b"INT. LAB - DAY", ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Source)).unwrap();
        let transform = AfterwritingArtifactTransform::new(policy(), None).unwrap();
        for target in [Format::Fountain, Format::Json, Format::ScreenplayJson, Format::Fdx, Format::Markdown] {
            assert!(transform.apply(&source, target, &store).unwrap_err().to_string().contains("screenplay_pdf.route"));
        }
    }

    #[test]
    fn pins_and_profiles_change_identity_without_enabling_reuse() {
        let first = AfterwritingArtifactTransform::new(policy(), None).unwrap();
        let mut changed = policy();
        changed.package_tree_sha256 = "c".repeat(64);
        let second = AfterwritingArtifactTransform::new(changed, None).unwrap();
        let mut letter = policy();
        letter.profile = ScreenplayPdfProfile::UsLetter;
        let third = AfterwritingArtifactTransform::new(letter, None).unwrap();
        assert_ne!(first.cache_identity(), second.cache_identity());
        assert_ne!(first.cache_identity(), third.cache_identity());
        assert!(!first.cacheable());
        assert!(!second.cacheable());
        assert!(!third.cacheable());
        assert_eq!(first.fidelity(), Some(FidelityDeclaration::Partial));
    }

    #[test]
    fn terminal_validation_rejects_missing_evidence_even_for_pdf_envelopes() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temporary.path()).unwrap();
        let source = store.put_bytes(b"INT. LAB - DAY", ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Source)).unwrap();
        let pdf = store.put_bytes(b"%PDF-1.3\n%%EOF\n", ArtifactDescriptor::for_format(Format::Pdf, ArtifactStorageClass::Intermediate).with_source(source.id().clone())).unwrap();
        assert!(validate_published_artifact(&pdf, &source, &store, &policy()).unwrap_err().to_string().contains("evidence is missing"));
    }

    #[test]
    fn terminal_validation_rejects_detached_source_lineage() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temporary.path()).unwrap();
        let source = store.put_bytes(b"INT. LAB - DAY", ArtifactDescriptor::for_format(Format::Fountain, ArtifactStorageClass::Source)).unwrap();
        let pdf = store.put_bytes(b"%PDF-1.3\n%%EOF\n", ArtifactDescriptor::for_format(Format::Pdf, ArtifactStorageClass::Intermediate)).unwrap();
        assert!(validate_published_artifact(&pdf, &source, &store, &policy()).unwrap_err().to_string().contains("validation.lineage"));
    }
}
