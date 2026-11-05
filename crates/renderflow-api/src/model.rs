use std::path::PathBuf;
use serde::{Deserialize, Serialize};

pub const API_SCHEMA: &str = "renderflow.api/v1";
pub const SERVICE_SCHEMA: &str = "renderflow.api-service/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    pub schema_version: String,
    pub workspace_parent: PathBuf,
    pub pipelines: Vec<PipelineConfig>,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineConfig {
    pub id: String,
    pub root: PathBuf,
    pub config: String,
    /// Explicit package file allowlist; directories and symlinks are refused.
    pub files: Vec<String>,
    #[serde(default)]
    pub profile: PipelineProfile,
    /// Explicit operator opt-in for editorial Pandoc/Tectonic discovery/run.
    #[serde(default)]
    pub allow_local_processes: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineProfile {
    #[default]
    Composition,
    ManualLyrics,
    TimedText,
    FountainBody,
    Editorial,
}
impl PipelineProfile {
    pub const ALL: [Self;5] = [Self::Composition,Self::ManualLyrics,Self::TimedText,Self::FountainBody,Self::Editorial];
    pub fn name(self) -> &'static str {match self {
        Self::Composition=>renderflow::composition::COMPOSITION_PROFILE,
        Self::ManualLyrics=>"renderflow.api-profile/manual-lyrics/v1",
        Self::TimedText=>"renderflow.api-profile/timed-text/v1",
        Self::FountainBody=>"renderflow.api-profile/fountain-body/v1",
        Self::Editorial=>"renderflow.api-profile/editorial/v1",
    }}
    pub fn local_processes(self) -> bool {self == Self::Editorial}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub workers: usize,
    pub queued_jobs: usize,
    pub retained_jobs: usize,
    pub package_files: usize,
    pub package_bytes: u64,
    pub all_package_bytes: u64,
    pub result_bytes: usize,
    pub progress_events: usize,
    pub progress_bytes: usize,
    pub download_bytes: u64,
    pub job_storage_bytes: u64,
    pub workspace_storage_bytes: u64,
    pub job_timeout_seconds: u64,
    pub retention_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { workers: 2, queued_jobs: 4, retained_jobs: 32, package_files: 256,
            package_bytes: 32 * 1024 * 1024, all_package_bytes: 64 * 1024 * 1024,
            result_bytes: 2 * 1024 * 1024, progress_events: 64, progress_bytes: 64 * 1024,
            download_bytes: 8 * 1024 * 1024, job_storage_bytes: 256 * 1024 * 1024,workspace_storage_bytes:512*1024*1024,
            job_timeout_seconds: 120, retention_seconds: 900 }
    }
}

impl Limits {
    pub fn validate(&self) -> ApiResult<()> {
        if !(1..=8).contains(&self.workers) || !(1..=64).contains(&self.queued_jobs)
            || self.retained_jobs < self.workers + self.queued_jobs || self.retained_jobs > 256
            || !(1..=512).contains(&self.package_files) || self.package_bytes == 0
            || self.package_bytes > 64 * 1024 * 1024 || self.all_package_bytes < self.package_bytes
            || self.all_package_bytes > 128 * 1024 * 1024
            || !(1024..=4 * 1024 * 1024).contains(&self.result_bytes)
            || !(1..=256).contains(&self.progress_events) || !(1024..=256 * 1024).contains(&self.progress_bytes)
            || self.download_bytes == 0 || self.download_bytes > 32 * 1024 * 1024
            || self.job_storage_bytes < self.package_bytes || self.job_storage_bytes > 512 * 1024 * 1024
            || self.workspace_storage_bytes < self.job_storage_bytes || self.workspace_storage_bytes > 2*1024*1024*1024
            || !(1..=600).contains(&self.job_timeout_seconds) || !(1..=86400).contains(&self.retention_seconds) {
            return Err(ApiError::new("invalid_limits", "Service limits exceed the supported local profile", 400));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation { Inspect, Plan, Execute }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitRequest {
    pub pipeline_id: String,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState { Queued, Running, Cancelling, Planned, Complete, Partial, Failed, Cancelled }

impl JobState {
    pub fn terminal(self) -> bool {
        matches!(self,Self::Planned|Self::Complete|Self::Partial|Self::Failed|Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelReason { Client, Deadline, Storage, Shutdown }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", content = "value", rename_all = "snake_case")]
pub enum JobOutput {
    Inspect(renderflow::ArtifactProfile),
    Plan(renderflow::ProviderPlan),
    Execute(renderflow::ExecutionResult),
}

#[derive(Debug, Clone, Serialize)]
pub struct JobView {
    pub schema_version: &'static str,
    pub job_id: String,
    pub pipeline_id: String,
    pub profile: &'static str,
    pub local_processes: bool,
    pub package_sha256: String,
    pub effective_config_sha256: Option<String>,
    pub operation: Operation,
    pub state: JobState,
    pub cancel_requested: Option<CancelReason>,
    pub submitted_at_unix_ms: u64,
    pub started_at_unix_ms: Option<u64>,
    pub completed_at_unix_ms: Option<u64>,
    pub last_event_id: u64,
    pub dropped_events: u64,
    pub result: Option<JobOutput>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineView {
    pub id: String, pub package_sha256: String, pub file_count: usize,
    pub size_bytes: u64, pub profile: &'static str,
    pub local_processes: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordedEvent { pub id: u64, pub event: renderflow::ProgressEvent }

#[derive(Debug, Clone, Serialize)]
pub struct EventsView {
    pub schema_version: &'static str, pub job_id: String, pub state: JobState,
    pub events: Vec<RecordedEvent>, pub last_event_id: u64,
    pub dropped_events: u64, pub cursor_gap: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactView {
    pub id: String, pub target_id: String, pub artifact_id: String,
    pub format: String, pub media_type: String, pub size_bytes: u64, pub sha256: String,
    pub validation: renderflow::evidence::ValidationState,
    pub fidelity: renderflow::evidence::FidelityDeclaration,
    pub evidence: &'static str,
    pub download_path: String,
}

#[derive(Debug)]
pub struct Download {
    pub bytes: Vec<u8>, pub sha256: String, pub media_type: String, pub filename: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip)]
    pub status: u16,
}
impl ApiError {
    pub fn new(code: &str, message: &str, status: u16) -> Self {
        Self { code:code.into(),message:message.into(),status }
    }
    pub(crate) fn io() -> Self { Self::new("storage_error","A bounded service storage operation failed",500) }
    pub(crate) fn internal() -> Self { Self::new("internal_error","The service could not complete the request",500) }
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f,"{}: {}",self.code,self.message) }
}
impl std::error::Error for ApiError {}
pub type ApiResult<T> = Result<T,ApiError>;

pub fn stable_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && value.bytes().all(|byte|byte.is_ascii_alphanumeric() || matches!(byte,b'-'|b'_'|b'.'))
}
