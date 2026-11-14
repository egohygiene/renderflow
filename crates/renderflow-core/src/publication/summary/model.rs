use crate::ai::profile::ProfilePlan;
use crate::ai::runtime::AiIdentityEvidence;
use crate::ai::{AiCandidateState, AiExecutionEvidence};
use crate::artifact::Artifact;
use serde::{Deserialize, Serialize};

pub const RECIPE_SCHEMA: &str = "renderflow.publication-summary/v1";
pub const CANDIDATE_SCHEMA: &str = "renderflow.summary-candidate/v1";
pub const SKILL_ID: &str = "skill.publication.summary";
pub const SKILL_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    AuthoredScript,
    Transcript,
    TextCompanion,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummarySource {
    pub id: String,
    pub kind: TextKind,
    pub path: String,
    pub sha256: String,
    pub ranges: Vec<TextRange>,
    pub approved_for_ai: bool,
    pub approval_reference: String,
    #[serde(default)]
    pub privacy_approved_for_remote: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryRecipe {
    pub schema_version: String,
    pub id: String,
    pub sources: Vec<SummarySource>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEvidence {
    pub source_id: String,
    pub artifact_id: String,
    pub sha256: String,
    pub size_bytes: usize,
    pub selected_bytes: usize,
    pub unselected_bytes: usize,
    pub approved_for_ai: bool,
    pub privacy_approved_for_remote: bool,
    pub approval_reference: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSlice {
    pub evidence_id: String,
    pub source_id: String,
    pub artifact_id: String,
    pub source_sha256: String,
    pub start: usize,
    pub end: usize,
    pub text_sha256: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Statement {
    pub text: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryBody {
    pub synopsis: Vec<Statement>,
    pub uncertainty: Vec<String>,
    pub omissions: Vec<String>,
    pub review_required: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessingIdentity {
    pub profile_sha256: String,
    pub skill_id: String,
    pub skill_version: String,
    pub skill_sha256: String,
    pub input_sha256: String,
    pub output_schema_sha256: String,
    pub settings_sha256: String,
    pub hygiene_sha256: String,
    pub cache_identity: String,
    pub model: AiIdentityEvidence,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryCandidate {
    pub schema_version: String,
    pub candidate_id: String,
    pub recipe_sha256: String,
    pub body: SummaryBody,
    pub sources: Vec<SourceEvidence>,
    pub slices: Vec<SourceSlice>,
    pub processing: ProcessingIdentity,
    pub state: AiCandidateState,
    pub review_required: bool,
    pub grounding: String,
    pub coverage_note: String,
    pub reference_validator: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryStatus {
    Disabled,
    Unavailable,
    Refused,
    Failed,
    Invalid,
    Cancelled,
    AwaitingReview,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryPlan {
    pub schema_version: String,
    pub recipe_sha256: String,
    pub sources: Vec<SourceEvidence>,
    pub slice_ids: Vec<String>,
    pub selected_input_bytes: usize,
    pub profile: ProfilePlan,
    pub coverage_note: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationalReport {
    pub invocation_id: String,
    pub duration_ms: u64,
    pub attempt_ceiling: u32,
    pub observed_attempts: Option<u32>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub cost_microunits: Option<u64>,
    pub cost_status: String,
    pub telemetry_status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryOutcome {
    pub schema_version: String,
    pub status: SummaryStatus,
    pub diagnostic_code: String,
    pub plan: SummaryPlan,
    pub candidate: Option<SummaryCandidate>,
    pub artifact: Option<Artifact>,
    pub execution: Option<AiExecutionEvidence>,
    pub operation: OperationalReport,
}
