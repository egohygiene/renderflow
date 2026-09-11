//! Bounded HandBrakeCLI adapter for whole-file video delivery transforms.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::evidence::{
    sha256_serialized, unix_time_ms, DigestEvidence, FlowArtifactV1, FlowProducerV1,
    FLOW_ARTIFACT_SCHEMA_V1,
};
use crate::process::{
    ProcessCancellationToken, ProcessExecutor, ProcessExpectedOutput, ProcessNetworkPolicy,
    ProcessRequest, ProcessTermination,
};
use crate::sdk::{
    CancellationToken, ProgressEvent, ProgressReporter, ProgressStage, PROGRESS_EVENT_V1,
};
use crate::toolchain::ToolRegistry;

pub const HANDBRAKE_TRANSFORM_SCHEMA_V1: &str = "renderflow.handbrake-transform/v1";
pub const HANDBRAKE_CAPABILITY_CONTRACT_SCHEMA_V1: &str = "renderflow.handbrake-capability/v1";
pub const HANDBRAKE_TRANSFORM_CAPABILITY_ID_V1: &str = "video.transcode.whole_file";
pub const HANDBRAKE_PROVIDER_ID: &str = "adapter.media.handbrake";
pub const HANDBRAKE_TOOL_ID: &str = "tool.handbrake";
pub const ANIFLOW_SEGMENT_CAPABILITY_ID_V1: &str = "media.video.segment/v1";
pub const ANIFLOW_RECONSTRUCT_CAPABILITY_ID_V1: &str = "media.video.reconstruct/v1";

const MINIMUM_CAPTURE_BYTES: usize = 4 * 1024;
const MAXIMUM_CAPTURE_BYTES: usize = 4 * 1024 * 1024;
const MAXIMUM_TIMEOUT_SECONDS: u64 = 24 * 60 * 60;
const MAXIMUM_OUTPUT_BYTES_LIMIT: u64 = 1024 * 1024 * 1024 * 1024;

/// Allowlisted HandBrake presets with stable Renderflow identifiers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HandBrakePreset {
    Fast720p30,
    #[default]
    Fast1080p30,
    Creator1080p60,
    ProductionStandard,
}

impl HandBrakePreset {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fast720p30 => "fast_720p30",
            Self::Fast1080p30 => "fast_1080p30",
            Self::Creator1080p60 => "creator_1080p60",
            Self::ProductionStandard => "production_standard",
        }
    }

    pub const fn handbrake_name(self) -> &'static str {
        match self {
            Self::Fast720p30 => "Fast 720p30",
            Self::Fast1080p30 => "Fast 1080p30",
            Self::Creator1080p60 => "Creator 1080p60",
            Self::ProductionStandard => "Production Standard",
        }
    }

    pub const fn purpose(self) -> &'static str {
        match self {
            Self::Fast720p30 => "compact broadly compatible delivery",
            Self::Fast1080p30 => "default broadly compatible delivery",
            Self::Creator1080p60 => "high-quality creator-platform upload master",
            Self::ProductionStandard => "high-bitrate editing and mezzanine handoff",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandBrakePresetContract {
    pub id: String,
    pub handbrake_name: String,
    pub purpose: String,
    pub container: String,
    pub whole_file_only: bool,
}

impl From<HandBrakePreset> for HandBrakePresetContract {
    fn from(preset: HandBrakePreset) -> Self {
        Self {
            id: preset.id().to_string(),
            handbrake_name: preset.handbrake_name().to_string(),
            purpose: preset.purpose().to_string(),
            container: "mp4".to_string(),
            whole_file_only: true,
        }
    }
}

/// Resource policy applied to one HandBrake process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandBrakeLimits {
    pub timeout_seconds: u64,
    pub capture_limit_bytes: usize,
    pub progress_interval_ms: u64,
    pub maximum_output_bytes: u64,
}

impl Default for HandBrakeLimits {
    fn default() -> Self {
        Self {
            timeout_seconds: 2 * 60 * 60,
            capture_limit_bytes: 256 * 1024,
            progress_interval_ms: 1_000,
            maximum_output_bytes: 20 * 1024 * 1024 * 1024,
        }
    }
}

/// Typed request for a single whole-file transform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandBrakeTransformRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    #[serde(default)]
    pub preset: HandBrakePreset,
    #[serde(default)]
    pub limits: HandBrakeLimits,
}

impl HandBrakeTransformRequest {
    pub fn new(
        input: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
        preset: HandBrakePreset,
    ) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
            preset,
            limits: HandBrakeLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: HandBrakeLimits) -> Self {
        self.limits = limits;
        self
    }
}

/// Side-effect-free normalized execution plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandBrakeTransformPlan {
    pub schema_version: String,
    pub capability_id: String,
    pub provider_id: String,
    pub tool_id: String,
    pub temporal_scope: String,
    pub input: PathBuf,
    pub input_digest: DigestEvidence,
    pub input_size_bytes: u64,
    pub output: PathBuf,
    pub preset: HandBrakePresetContract,
    pub arguments: Vec<String>,
    pub limits: HandBrakeLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandBrakeValidation {
    pub non_empty: bool,
    pub mp4_file_type_box: bool,
    pub within_output_limit: bool,
}

/// Durable provenance for a successful transform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandBrakeTransformReport {
    pub schema_version: String,
    pub capability_id: String,
    pub provider_id: String,
    pub renderflow_version: String,
    pub handbrake_version: String,
    pub temporal_scope: String,
    pub preset: HandBrakePresetContract,
    pub input: PathBuf,
    pub input_digest: DigestEvidence,
    pub input_size_bytes: u64,
    pub output: PathBuf,
    pub provenance: PathBuf,
    pub output_digest: DigestEvidence,
    pub output_size_bytes: u64,
    pub argv_digest: DigestEvidence,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: u64,
    pub duration_ms: u64,
    pub stdout_total_bytes: u64,
    pub stderr_total_bytes: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub validation: HandBrakeValidation,
    pub flow_artifact: FlowArtifactV1,
    pub aniflow_segment_capability: String,
    pub aniflow_reconstruct_capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandBrakeCapabilityContract {
    pub schema_version: String,
    pub capability_id: String,
    pub provider_id: String,
    pub tool_id: String,
    pub temporal_scope: String,
    pub owned_transforms: Vec<String>,
    pub excluded_responsibilities: Vec<String>,
    pub presets: Vec<HandBrakePresetContract>,
    pub progress_schema: String,
    pub flow_artifact_schema: String,
    pub aniflow_segment_capability: String,
    pub aniflow_reconstruct_capability: String,
}

impl HandBrakeCapabilityContract {
    pub fn builtin() -> Self {
        Self {
            schema_version: HANDBRAKE_CAPABILITY_CONTRACT_SCHEMA_V1.to_string(),
            capability_id: HANDBRAKE_TRANSFORM_CAPABILITY_ID_V1.to_string(),
            provider_id: HANDBRAKE_PROVIDER_ID.to_string(),
            tool_id: HANDBRAKE_TOOL_ID.to_string(),
            temporal_scope: "whole_file".to_string(),
            owned_transforms: vec![
                "container_and_codec_delivery_transcode".to_string(),
                "resolution_and_frame_rate_limit_from_typed_preset".to_string(),
                "web_fast_start".to_string(),
                "metadata_and_chapter_preservation".to_string(),
            ],
            excluded_responsibilities: vec![
                "temporal_decomposition".to_string(),
                "segment_boundary_selection".to_string(),
                "segment_manifest_ordering".to_string(),
                "segment_reconstruction".to_string(),
            ],
            presets: [
                HandBrakePreset::Fast720p30,
                HandBrakePreset::Fast1080p30,
                HandBrakePreset::Creator1080p60,
                HandBrakePreset::ProductionStandard,
            ]
            .into_iter()
            .map(HandBrakePresetContract::from)
            .collect(),
            progress_schema: PROGRESS_EVENT_V1.to_string(),
            flow_artifact_schema: FLOW_ARTIFACT_SCHEMA_V1.to_string(),
            aniflow_segment_capability: ANIFLOW_SEGMENT_CAPABILITY_ID_V1.to_string(),
            aniflow_reconstruct_capability: ANIFLOW_RECONSTRUCT_CAPABILITY_ID_V1.to_string(),
        }
    }
}

pub fn plan_handbrake(request: &HandBrakeTransformRequest) -> Result<HandBrakeTransformPlan> {
    validate_request(request)?;
    let input = request
        .input
        .canonicalize()
        .with_context(|| format!("failed to resolve input '{}'", request.input.display()))?;
    let output = absolute_path(&request.output)?;
    let input_size_bytes = fs::metadata(&input)?.len();
    let input_digest = sha256_file(&input)?;
    let arguments = build_arguments(&input, &output, request.preset)?;
    Ok(HandBrakeTransformPlan {
        schema_version: HANDBRAKE_TRANSFORM_SCHEMA_V1.to_string(),
        capability_id: HANDBRAKE_TRANSFORM_CAPABILITY_ID_V1.to_string(),
        provider_id: HANDBRAKE_PROVIDER_ID.to_string(),
        tool_id: HANDBRAKE_TOOL_ID.to_string(),
        temporal_scope: "whole_file".to_string(),
        input,
        input_digest,
        input_size_bytes,
        output,
        preset: request.preset.into(),
        arguments,
        limits: request.limits.clone(),
    })
}

pub fn execute_handbrake(request: &HandBrakeTransformRequest) -> Result<HandBrakeTransformReport> {
    execute_handbrake_with_progress(request, &CancellationToken::new(), None)
}

pub fn execute_handbrake_with_progress(
    request: &HandBrakeTransformRequest,
    cancellation: &CancellationToken,
    reporter: Option<&dyn ProgressReporter>,
) -> Result<HandBrakeTransformReport> {
    let plan = plan_handbrake(request)?;
    if cancellation.is_cancelled() {
        emit_progress(
            reporter,
            ProgressStage::Cancelled,
            "HandBrake transform cancelled",
            "cancelled",
        );
        bail!("HandBrake transform was cancelled before launch");
    }
    if plan.output.exists() {
        bail!("output already exists: {}", plan.output.display());
    }
    let provenance_path = provenance_path(&plan.output)?;
    if provenance_path.exists() {
        bail!(
            "provenance output already exists: {}",
            provenance_path.display()
        );
    }
    let parent = plan
        .output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create output directory '{}'", parent.display()))?;
    let temporary = temporary_output(parent)?;
    let actual_arguments = build_arguments(&plan.input, &temporary, request.preset)?;
    let argv_digest = sha256_serialized(&actual_arguments)?;
    let process_cancellation = ProcessCancellationToken::from_shared(cancellation.flag());
    let inventory = ToolRegistry::builtins().assess_ids_current([HANDBRAKE_TOOL_ID]);
    let availability = inventory
        .get(HANDBRAKE_TOOL_ID)
        .context("HandBrake tool registry entry is missing")?;
    if !availability.is_available() {
        remove_file_if_present(&temporary)?;
        bail!("HandBrakeCLI is unavailable: {}", availability.summary());
    }
    let executable = availability
        .selected_executable
        .clone()
        .context("available HandBrake provider did not select an executable")?;
    let handbrake_version = availability
        .version_line
        .clone()
        .unwrap_or_else(|| "version unavailable".to_string());
    let process_request = ProcessRequest::direct(executable)
        .args(actual_arguments)
        .timeout(Duration::from_secs(request.limits.timeout_seconds))
        .capture_limit(request.limits.capture_limit_bytes)
        .cancellation(process_cancellation)
        .network_policy(ProcessNetworkPolicy::Deny)
        .sandbox_profile("renderflow.handbrake.whole-file/v1")
        .expect_output(ProcessExpectedOutput::file(&temporary).require_non_empty());
    let executor = ProcessExecutor::new();
    let started_at_unix_ms = unix_time_ms();
    let started = Instant::now();
    emit_progress(
        reporter,
        ProgressStage::Executing,
        "HandBrake transform started",
        "running",
    );
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let _ = sender.send(executor.execute(process_request));
    });
    let process_result = loop {
        match receiver.recv_timeout(Duration::from_millis(request.limits.progress_interval_ms)) {
            Ok(result) => break Some(result),
            Err(mpsc::RecvTimeoutError::Timeout) => emit_progress(
                reporter,
                ProgressStage::Executing,
                format!(
                    "HandBrake transform running for {} ms",
                    started.elapsed().as_millis()
                ),
                "running",
            ),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break None;
            }
        }
    };
    if worker.join().is_err() {
        remove_file_if_present(&temporary)?;
        bail!("HandBrake process worker panicked");
    }
    let result = match process_result {
        Some(Ok(result)) => result,
        Some(Err(error)) => {
            remove_file_if_present(&temporary)?;
            return Err(error.into());
        }
        None => {
            remove_file_if_present(&temporary)?;
            bail!("HandBrake process worker disconnected");
        }
    };
    if result.termination() == ProcessTermination::Cancelled {
        remove_file_if_present(&temporary)?;
        emit_progress(
            reporter,
            ProgressStage::Cancelled,
            "HandBrake transform cancelled",
            "cancelled",
        );
        bail!("HandBrake transform was cancelled");
    }
    if let Err(error) = result.ensure_success() {
        remove_file_if_present(&temporary)?;
        bail!("{error}");
    }

    let output_size_bytes = fs::metadata(&temporary)?.len();
    let validation = HandBrakeValidation {
        non_empty: output_size_bytes > 0,
        mp4_file_type_box: has_mp4_file_type_box(&temporary)?,
        within_output_limit: output_size_bytes <= request.limits.maximum_output_bytes,
    };
    if !validation.non_empty || !validation.mp4_file_type_box || !validation.within_output_limit {
        remove_file_if_present(&temporary)?;
        bail!("HandBrake output failed validation: {validation:?}");
    }
    if plan.output.exists() {
        remove_file_if_present(&temporary)?;
        bail!("output appeared during execution; refusing to replace it");
    }
    let output_digest = sha256_file(&temporary)?;
    fs::rename(&temporary, &plan.output).with_context(|| {
        format!(
            "failed to atomically publish HandBrake output '{}'",
            plan.output.display()
        )
    })?;
    let completed_at_unix_ms = unix_time_ms();
    let flow_artifact = FlowArtifactV1 {
        schema_version: FLOW_ARTIFACT_SCHEMA_V1.to_string(),
        artifact_id: format!("artifact:sha256-{}", output_digest.value),
        role: "video_delivery".to_string(),
        media_type: "video/mp4".to_string(),
        digest: output_digest.clone(),
        size_bytes: output_size_bytes,
        producer: FlowProducerV1 {
            owner: "renderflow".to_string(),
            capability_id: HANDBRAKE_TRANSFORM_CAPABILITY_ID_V1.to_string(),
            provider_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        sources: vec![format!("artifact:sha256-{}", plan.input_digest.value)],
    };
    let report = HandBrakeTransformReport {
        schema_version: HANDBRAKE_TRANSFORM_SCHEMA_V1.to_string(),
        capability_id: HANDBRAKE_TRANSFORM_CAPABILITY_ID_V1.to_string(),
        provider_id: HANDBRAKE_PROVIDER_ID.to_string(),
        renderflow_version: env!("CARGO_PKG_VERSION").to_string(),
        handbrake_version,
        temporal_scope: "whole_file".to_string(),
        preset: plan.preset,
        input: plan.input,
        input_digest: plan.input_digest,
        input_size_bytes: plan.input_size_bytes,
        output: plan.output,
        provenance: provenance_path.clone(),
        output_digest,
        output_size_bytes,
        argv_digest,
        started_at_unix_ms,
        completed_at_unix_ms,
        duration_ms: result.duration_ms(),
        stdout_total_bytes: result.stdout().total_bytes(),
        stderr_total_bytes: result.stderr().total_bytes(),
        stdout_truncated: result.stdout().truncated(),
        stderr_truncated: result.stderr().truncated(),
        validation,
        flow_artifact,
        aniflow_segment_capability: ANIFLOW_SEGMENT_CAPABILITY_ID_V1.to_string(),
        aniflow_reconstruct_capability: ANIFLOW_RECONSTRUCT_CAPABILITY_ID_V1.to_string(),
    };
    if let Err(error) = write_json_atomic(&provenance_path, &report) {
        remove_file_if_present(&report.output)?;
        return Err(error);
    }
    emit_progress(
        reporter,
        ProgressStage::Completed,
        "HandBrake transform completed",
        "complete",
    );
    Ok(report)
}

fn validate_request(request: &HandBrakeTransformRequest) -> Result<()> {
    if !request.input.is_file() {
        bail!("input video does not exist: {}", request.input.display());
    }
    let input_extension = extension(&request.input)?;
    if !matches!(
        input_extension.as_str(),
        "avi" | "m4v" | "mkv" | "mov" | "mp4" | "webm"
    ) {
        bail!("unsupported HandBrake input extension '{input_extension}'");
    }
    if extension(&request.output)? != "mp4" {
        bail!("bounded HandBrake presets require an .mp4 output");
    }
    if request.limits.timeout_seconds == 0
        || request.limits.timeout_seconds > MAXIMUM_TIMEOUT_SECONDS
    {
        bail!("timeout_seconds must be between 1 and {MAXIMUM_TIMEOUT_SECONDS}");
    }
    if !(MINIMUM_CAPTURE_BYTES..=MAXIMUM_CAPTURE_BYTES)
        .contains(&request.limits.capture_limit_bytes)
    {
        bail!(
            "capture_limit_bytes must be between {MINIMUM_CAPTURE_BYTES} and {MAXIMUM_CAPTURE_BYTES}"
        );
    }
    if !(100..=60_000).contains(&request.limits.progress_interval_ms) {
        bail!("progress_interval_ms must be between 100 and 60000");
    }
    if request.limits.maximum_output_bytes == 0
        || request.limits.maximum_output_bytes > MAXIMUM_OUTPUT_BYTES_LIMIT
    {
        bail!("maximum_output_bytes must be between 1 and {MAXIMUM_OUTPUT_BYTES_LIMIT}");
    }
    let input = request.input.canonicalize()?;
    let output = absolute_path(&request.output)?;
    if output.exists() && output.canonicalize()? == input {
        bail!("input and output must be different files");
    }
    Ok(())
}

fn build_arguments(input: &Path, output: &Path, preset: HandBrakePreset) -> Result<Vec<String>> {
    let input = input
        .to_str()
        .context("HandBrake input path must be valid UTF-8")?;
    let output = output
        .to_str()
        .context("HandBrake output path must be valid UTF-8")?;
    Ok(vec![
        "--json".to_string(),
        "--input".to_string(),
        input.to_string(),
        "--output".to_string(),
        output.to_string(),
        "--preset".to_string(),
        preset.handbrake_name().to_string(),
        "--format".to_string(),
        "av_mp4".to_string(),
        "--optimize".to_string(),
        "--markers".to_string(),
        "--keep-metadata".to_string(),
    ])
}

fn emit_progress(
    reporter: Option<&dyn ProgressReporter>,
    stage: ProgressStage,
    message: impl Into<String>,
    state: &str,
) {
    if let Some(reporter) = reporter {
        reporter.on_event(&ProgressEvent {
            schema_version: PROGRESS_EVENT_V1.to_string(),
            stage,
            message: message.into(),
            run_id: None,
            step_id: Some("handbrake-whole-file-transcode".to_string()),
            artifact_ids: Vec::new(),
            state: Some(state.to_string()),
            diagnostics: Vec::new(),
        });
    }
}

fn sha256_file(path: &Path) -> Result<DigestEvidence> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open '{}' for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(DigestEvidence {
        algorithm: "sha256".to_string(),
        value: format!("{:x}", hasher.finalize()),
    })
}

fn has_mp4_file_type_box(path: &Path) -> Result<bool> {
    let mut file = File::open(path)?;
    let mut prefix = [0_u8; 64];
    let read = file.read(&mut prefix)?;
    Ok(prefix[..read].windows(4).any(|window| window == b"ftyp"))
}

fn extension(path: &Path) -> Result<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .context("video path requires a UTF-8 file extension")
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn temporary_output(parent: &Path) -> Result<PathBuf> {
    let temporary = tempfile::Builder::new()
        .prefix(".renderflow-handbrake-")
        .suffix(".mp4")
        .tempfile_in(parent)?;
    let path = temporary.path().to_path_buf();
    drop(temporary);
    Ok(path)
}

fn provenance_path(output: &Path) -> Result<PathBuf> {
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .context("output filename must be valid UTF-8")?;
    Ok(output.with_file_name(format!("{name}.renderflow.json")))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, value)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to publish provenance '{}'", path.display()))?;
    Ok(())
}

fn remove_file_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
