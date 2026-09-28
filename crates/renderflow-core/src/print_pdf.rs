//! Exact, local print-interior PDF transform. The provider's output is not
//! admitted to the artifact store until independent page inspection succeeds.

use std::path::Path;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::artifact::{
    Artifact, ArtifactCollection, ArtifactCollectionTransform, ArtifactDescriptor,
    ArtifactStorageClass, ArtifactStore,
};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::print_pdf_inspect::{inspect_print_pdf, ExpectedPrintPage};
use crate::process::{
    ProcessCancellationToken, ProcessExecutor, ProcessExpectedOutput, ProcessInput,
    ProcessNetworkPolicy, ProcessOutputMode, ProcessRequest, ProcessTermination,
    DEFAULT_CAPTURE_LIMIT_BYTES,
};
use crate::spec::PrintPdfInteriorPolicy;

pub const PRINT_PDF_CAPABILITY: &str = "publication.generate.pdf.interior";
pub const PRINT_PDF_PROVIDER: &str = "tool.img2pdf";

/// A provider failure that retains its machine-readable reason through the DAG.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct PrintPdfProviderError {
    pub code: &'static str,
    pub message: String,
    pub cancelled: bool,
}

fn provider_error(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    PrintPdfProviderError {
        code,
        message: message.into(),
        cancelled: code == "print_pdf.provider.cancelled",
    }
    .into()
}

pub struct PrintInteriorPdfTransform {
    policy: PrintPdfInteriorPolicy,
    pages: Vec<ExpectedPrintPage>,
    input_digests: Vec<String>,
    cache_identity: String,
    cancellation: Option<Arc<AtomicBool>>,
}

impl PrintInteriorPdfTransform {
    pub fn new(
        policy: PrintPdfInteriorPolicy,
        pages: Vec<ExpectedPrintPage>,
        input_digests: Vec<String>,
        cancellation: Option<Arc<AtomicBool>>,
    ) -> Result<Self> {
        policy.validate()?;
        if pages.is_empty() || pages.len() != input_digests.len() || pages.len() > policy.max_pages
        {
            anyhow::bail!("print_pdf.bounds: expected page and digest lists must be nonempty and within max_pages");
        }
        let cache_identity = serde_json::to_string(&(&policy, &pages, &input_digests))?;
        Ok(Self {
            policy,
            pages,
            input_digests,
            cache_identity,
            cancellation,
        })
    }
}

impl ArtifactCollectionTransform for PrintInteriorPdfTransform {
    fn name(&self) -> &str {
        PRINT_PDF_CAPABILITY
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn cache_identity(&self) -> String {
        self.cache_identity.clone()
    }

    fn fidelity(&self) -> Option<FidelityDeclaration> {
        Some(FidelityDeclaration::Lossless)
    }

    fn apply(
        &self,
        inputs: &ArtifactCollection,
        output_format: Format,
        store: &ArtifactStore,
    ) -> Result<Artifact> {
        if output_format != Format::Pdf || inputs.len() != self.pages.len() {
            anyhow::bail!(
                "print_pdf.input.count: expected exactly one PDF output and {} ordered pages",
                self.pages.len()
            );
        }
        let total = inputs.iter().try_fold(0_u64, |sum, artifact| {
            sum.checked_add(artifact.size_bytes())
                .context("print_pdf.bounds: input byte count overflow")
        })?;
        if total > self.policy.max_input_bytes {
            anyhow::bail!("print_pdf.bounds: source bytes exceed max_input_bytes");
        }
        let paths = inputs
            .iter()
            .zip(&self.input_digests)
            .map(|(artifact, digest)| {
                if artifact.digest().to_string() != *digest {
                    anyhow::bail!(
                        "print_pdf.input.changed: artifact differs from frozen source identity"
                    );
                }
                store.payload_path(artifact)
            })
            .collect::<Result<Vec<_>>>()?;
        let first = &self.pages[0];
        let mm_per_point = 25.4 / 72.0;
        let media_width_mm = first.media_width_pt * mm_per_point;
        let media_height_mm = first.media_height_pt * mm_per_point;
        let trim_inset_mm = first.trim_inset_pt * mm_per_point;
        let mut args = vec![
            "--nodate".to_string(),
            "--engine".to_string(),
            "internal".to_string(),
            "--rotation".to_string(),
            "none".to_string(),
            "--pagesize".to_string(),
            format!("{media_width_mm:.6}mmx{media_height_mm:.6}mm"),
            "--imgsize".to_string(),
            format!("{media_width_mm:.6}mmx{media_height_mm:.6}mm"),
            "--fit".to_string(),
            "into".to_string(),
            "--bleed-border".to_string(),
            "0mm".to_string(),
            "--trim-border".to_string(),
            format!("{trim_inset_mm:.6}mm"),
        ];
        let temporary = tempfile::Builder::new()
            .prefix("print-interior-")
            .suffix(".pdf")
            .tempfile_in(store.temporary_directory())
            .context("print_pdf.output.temporary: cannot create temporary PDF")?
            .into_temp_path();
        let output = temporary
            .to_str()
            .context("print_pdf.output.path: temporary PDF path is not UTF-8")?;
        args.extend(["--output".to_string(), output.to_string()]);
        for path in &paths {
            args.push(
                path.to_str()
                    .context("print_pdf.input.path: artifact-store path is not UTF-8")?
                    .to_string(),
            );
        }
        let mut request = ProcessRequest::direct(&self.policy.executable)
            .args(args)
            .stdin(ProcessInput::Null)
            .stdout(ProcessOutputMode::capture(DEFAULT_CAPTURE_LIMIT_BYTES))
            .stderr(ProcessOutputMode::capture(DEFAULT_CAPTURE_LIMIT_BYTES))
            .timeout(Duration::from_secs(self.policy.timeout_seconds))
            .network_policy(ProcessNetworkPolicy::Deny)
            .expect_output(
                ProcessExpectedOutput::file(Path::new(output))
                    .require_non_empty()
                    .require_change()
                    .max_bytes(self.policy.max_output_bytes),
            );
        if let Some(cancellation) = &self.cancellation {
            request =
                request.cancellation(ProcessCancellationToken::from_shared(cancellation.clone()));
        }
        let result = ProcessExecutor::new().execute(request).map_err(|error| {
            let code = if matches!(
                error,
                crate::process::ProcessError::MissingExecutable { .. }
            ) {
                "print_pdf.provider.unavailable"
            } else {
                "print_pdf.provider.launch"
            };
            provider_error(code, error.to_string())
        })?;
        let code = match result.termination() {
            ProcessTermination::Exited { code: 0 } if result.is_success() => None,
            ProcessTermination::Exited { code: 0 } => Some("print_pdf.provider.output"),
            ProcessTermination::Exited { .. } => Some("print_pdf.provider.nonzero"),
            ProcessTermination::Signaled => Some("print_pdf.provider.signaled"),
            ProcessTermination::TimedOut => Some("print_pdf.provider.timeout"),
            ProcessTermination::Cancelled => Some("print_pdf.provider.cancelled"),
            ProcessTermination::OutputLimitExceeded => Some("print_pdf.provider.output_limit"),
        };
        if let Some(code) = code {
            return Err(provider_error(code, result.failure_message()));
        }
        if self
            .cancellation
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst))
        {
            return Err(provider_error(
                "print_pdf.provider.cancelled",
                "execution was cancelled after provider completion",
            ));
        }
        let metadata = std::fs::symlink_metadata(&temporary)
            .context("print_pdf.output.missing: provider output vanished")?;
        if !metadata.file_type().is_file() {
            anyhow::bail!("print_pdf.output.unsafe: provider output is not a regular file");
        }
        if metadata.len() > self.policy.max_output_bytes {
            anyhow::bail!("print_pdf.bounds: output exceeds max_output_bytes");
        }
        let inspection = inspect_print_pdf(&temporary, &self.pages)
            .context("print_pdf.output.invalid: independent PDF inspection failed")?;
        if self
            .cancellation
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::SeqCst))
        {
            return Err(provider_error(
                "print_pdf.provider.cancelled",
                "execution was cancelled before artifact import",
            ));
        }
        store
            .import_path(
                &temporary,
                ArtifactDescriptor::for_format(Format::Pdf, ArtifactStorageClass::Intermediate)
                    .with_sources(inputs.iter().map(|artifact| artifact.id().clone()))
                    .with_metadata("renderflow.transform", PRINT_PDF_CAPABILITY)
                    .with_metadata(
                        "renderflow.print_pdf.policy",
                        serde_json::to_value(&self.policy)?,
                    )
                    .with_metadata(
                        "renderflow.print_pdf.inspection",
                        serde_json::to_value(&inspection)?,
                    )
                    .with_metadata("renderflow.print_pdf.provider", PRINT_PDF_PROVIDER)
                    .with_metadata(
                        "renderflow.print_pdf.provider_version",
                        self.policy.provider_version.clone(),
                    ),
            )
            .context("print_pdf.output.import: failed to persist validated PDF")
    }
}
