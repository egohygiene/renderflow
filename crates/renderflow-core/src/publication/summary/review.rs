//! Exact-candidate operator review and local export. These records declare a
//! reviewer reference; they do not authenticate a human or grant publication.
use super::*;
use serde::{Deserialize, Serialize};

pub const DECISION_SCHEMA: &str = "renderflow.summary-review-decision/v1";
pub const REVIEW_SCHEMA: &str = "renderflow.summary-review/v1";
pub const REVIEW_SCOPE: &str =
    "synopsis_text_only; no media, rights, provider or publication approval";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAction {
    Accept,
    Revise,
    Reject,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecision {
    pub schema_version: String,
    pub candidate_id: String,
    pub candidate_sha256: String,
    pub action: ReviewAction,
    pub reviewer_reference: String,
    #[serde(default)]
    pub semantic_review_completed: bool,
    #[serde(default)]
    pub revised_body: Option<SummaryBody>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRecord {
    pub schema_version: String,
    pub review_id: String,
    pub candidate_artifact: Artifact,
    pub decision: ReviewDecision,
    pub accepted_body: Option<SummaryBody>,
    pub accepted_body_sha256: Option<String>,
    pub output_state: Option<AiCandidateState>,
    pub authority_kind: String,
    pub scope: String,
    pub publication_approved: bool,
}
pub struct ReviewedArtifact {
    pub record: ReviewRecord,
    pub artifact: Artifact,
}

pub fn parse_candidate(bytes: &[u8]) -> Result<SummaryCandidate> {
    let candidate = crate::evidence::json::object(bytes, 131072)?;
    validate_candidate(&candidate)?;
    Ok(candidate)
}

fn selected(
    candidate: &SummaryCandidate,
    bytes: &[u8],
    decision: &ReviewDecision,
) -> Result<Option<SummaryBody>> {
    ensure!(
        decision.schema_version == DECISION_SCHEMA
            && decision.candidate_id == candidate.candidate_id
            && decision.candidate_sha256 == local::sha256(bytes),
        "summary.review.exact_subject"
    );
    local::text(&decision.reviewer_reference, 1024)?;
    let body = match decision.action {
        ReviewAction::Reject => {
            ensure!(
                decision.revised_body.is_none(),
                "summary.review.rejected_revision"
            );
            None
        }
        ReviewAction::Accept => {
            ensure!(
                decision.semantic_review_completed && decision.revised_body.is_none(),
                "summary.review.semantic_review_required"
            );
            Some(candidate.body.clone())
        }
        ReviewAction::Revise => {
            ensure!(
                decision.semantic_review_completed,
                "summary.review.semantic_review_required"
            );
            let body = decision
                .revised_body
                .clone()
                .context("summary.review.revision_required")?;
            validate_body(&body, &candidate.slices)?;
            Some(body)
        }
    };
    Ok(body)
}
fn id(record: &ReviewRecord) -> Result<String> {
    Ok(format!(
        "review-{}",
        local::sha256(&serde_json::to_vec(&(
            &record.candidate_artifact,
            &record.decision,
            &record.accepted_body,
            &record.accepted_body_sha256,
            &record.output_state,
            &record.authority_kind,
            &record.scope,
            record.publication_approved
        ))?)
    ))
}

/// Read an exact bounded CAS subject through the shared confined observer path.
/// A serialized Artifact locator is never authority to read outside this store.
pub fn observed_artifact(
    store: &ArtifactStore,
    artifact: &Artifact,
    maximum: usize,
    token: ProcessCancellationToken,
) -> Result<Vec<u8>> {
    ensure!(
        artifact.size_bytes() <= maximum as u64
            && artifact.digest().algorithm() == crate::artifact::DigestAlgorithm::Sha256,
        "summary.artifact.bounds_or_digest"
    );
    ensure!(
        artifact.format().as_str() == "json"
            && artifact.media_type().as_str() == "application/json"
            && artifact.storage_class() == ArtifactStorageClass::Intermediate
            && artifact.id()
                == &crate::artifact::ArtifactId::from_record(
                    artifact.digest(),
                    artifact.format(),
                    artifact.sources()
                ),
        "summary.artifact.record_identity"
    );
    let relative = artifact
        .payload()
        .relative_path()
        .to_str()
        .context("summary.artifact.path")?;
    let mut capture = local::Capture::new(store.root(), token)?;
    let bytes = capture.read(relative, maximum)?;
    ensure!(
        bytes.len() as u64 == artifact.size_bytes()
            && local::sha256(&bytes) == artifact.digest().value(),
        "summary.artifact.identity"
    );
    Ok(bytes)
}

pub fn apply(
    candidate_bytes: &[u8],
    decision: &ReviewDecision,
    store: &ArtifactStore,
    token: ProcessCancellationToken,
) -> Result<ReviewedArtifact> {
    cancelled(&token)?;
    let candidate = parse_candidate(candidate_bytes)?;
    let body = selected(&candidate, candidate_bytes, decision)?;
    let subject = store.put_bytes(
        candidate_bytes,
        ArtifactDescriptor::for_format(Format::Json, ArtifactStorageClass::Intermediate)
            .with_metadata("renderflow.ai.output_state", "candidate")
            .with_metadata(
                "renderflow.summary.candidate_id",
                candidate.candidate_id.clone(),
            ),
    )?;
    observed_artifact(store, &subject, 131072, token.clone())?;
    let mut record = ReviewRecord {
        schema_version: REVIEW_SCHEMA.into(),
        review_id: String::new(),
        candidate_artifact: subject,
        decision: decision.clone(),
        accepted_body_sha256: body
            .as_ref()
            .map(|body| serde_json::to_vec(body).map(|bytes| local::sha256(&bytes)))
            .transpose()?,
        output_state: body.as_ref().map(|_| AiCandidateState::Approved),
        accepted_body: body,
        authority_kind: "operator_supplied_review_reference; not authenticated".into(),
        scope: REVIEW_SCOPE.into(),
        publication_approved: false,
    };
    record.review_id = id(&record)?;
    let bytes = serde_json::to_vec_pretty(&record)?;
    ensure!(bytes.len() <= 65536, "summary.review.byte_limit");
    cancelled(&token)?;
    let artifact = store.put_bytes(
        &bytes,
        ArtifactDescriptor::for_format(Format::Json, ArtifactStorageClass::Intermediate)
            .with_source(record.candidate_artifact.id().clone())
            .with_metadata("renderflow.summary.review_id", record.review_id.clone())
            .with_metadata(
                "renderflow.ai.output_state",
                if record.output_state.is_some() {
                    "approved"
                } else {
                    "candidate"
                },
            )
            .with_metadata(
                "renderflow.summary.review_action",
                serde_json::to_value(record.decision.action)?,
            ),
    )?;
    observed_artifact(store, &artifact, 65536, token)?;
    Ok(ReviewedArtifact { record, artifact })
}

pub fn validate_record(
    record: &ReviewRecord,
    store: &ArtifactStore,
    token: ProcessCancellationToken,
) -> Result<SummaryCandidate> {
    ensure!(
        record.schema_version == REVIEW_SCHEMA
            && record.scope == REVIEW_SCOPE
            && !record.publication_approved
            && record.authority_kind == "operator_supplied_review_reference; not authenticated"
            && record.review_id == id(record)?,
        "summary.review.contract_or_identity"
    );
    let bytes = observed_artifact(store, &record.candidate_artifact, 131072, token)?;
    let candidate = parse_candidate(&bytes)?;
    let expected = selected(&candidate, &bytes, &record.decision)?;
    ensure!(
        record.accepted_body == expected
            && record.output_state == expected.as_ref().map(|_| AiCandidateState::Approved)
            && record.accepted_body_sha256
                == expected
                    .as_ref()
                    .map(|body| serde_json::to_vec(body).map(|bytes| local::sha256(&bytes)))
                    .transpose()?,
        "summary.review.accepted_content_changed"
    );
    Ok(candidate)
}

pub fn publish_outcome(
    outcome: &SummaryOutcome,
    store: &ArtifactStore,
    output: &Path,
    token: ProcessCancellationToken,
) -> Result<Value> {
    ensure!(
        if outcome.status == SummaryStatus::AwaitingReview {
            outcome.candidate.is_some() && outcome.artifact.is_some()
        } else {
            outcome.candidate.is_none() && outcome.artifact.is_none()
        },
        "summary.bundle.invalid_state"
    );
    let mut files = BTreeMap::new();
    files.insert("outcome.json".into(), serde_json::to_vec_pretty(outcome)?);
    if let Some(artifact) = &outcome.artifact {
        ensure!(
            outcome.status == SummaryStatus::AwaitingReview,
            "summary.bundle.invalid_state"
        );
        let bytes = observed_artifact(store, artifact, 131072, token.clone())?;
        let candidate = parse_candidate(&bytes)?;
        ensure!(
            outcome
                .candidate
                .as_ref()
                .is_some_and(|value| value.candidate_id == candidate.candidate_id),
            "summary.bundle.subject"
        );
        ensure!(
            serde_json::to_value(
                outcome
                    .candidate
                    .as_ref()
                    .context("summary.bundle.candidate_missing")?
            )? == serde_json::to_value(&candidate)?,
            "summary.bundle.candidate_mismatch"
        );
        files.insert("candidate.json".into(), bytes);
    }
    local::publish(
        output,
        &files,
        "summary-receipt.json",
        json!({"schema_version":"renderflow.summary-bundle/v1","status":outcome.status,
        "candidate_id":outcome.candidate.as_ref().map(|candidate|&candidate.candidate_id),"publication_approved":false,"runtime_qualified":false}),
        token,
    )
}
pub fn publish_review(
    review: &ReviewedArtifact,
    store: &ArtifactStore,
    output: &Path,
    token: ProcessCancellationToken,
) -> Result<Value> {
    validate_record(&review.record, store, token.clone())?;
    let bytes = observed_artifact(store, &review.artifact, 65536, token.clone())?;
    ensure!(
        local::sha256(&bytes) == local::sha256(&serde_json::to_vec_pretty(&review.record)?),
        "summary.review.bundle_mismatch"
    );
    let mut files = BTreeMap::new();
    files.insert("review.json".into(), bytes);
    local::publish(
        output,
        &files,
        "review-receipt.json",
        json!({"schema_version":"renderflow.summary-review-bundle/v1","review_id":review.record.review_id,
        "action":review.record.decision.action,"export_allowed":review.record.output_state.is_some(),"publication_approved":false}),
        token,
    )
}

/// Explicit local materialization only. Existing directories are never replaced;
/// a final receipt follows observed files. No canonical metadata or network post.
pub fn export(
    review_bytes: &[u8],
    expected_review_sha256: &str,
    store: &ArtifactStore,
    output: &Path,
    token: ProcessCancellationToken,
) -> Result<Value> {
    cancelled(&token)?;
    ensure!(
        local::sha256(review_bytes) == expected_review_sha256,
        "summary.export.review_subject_changed"
    );
    let record: ReviewRecord = crate::evidence::json::object(review_bytes, 65536)?;
    let candidate = validate_record(&record, store, token.clone())?;
    ensure!(
        record.output_state == Some(AiCandidateState::Approved)
            && record.decision.action != ReviewAction::Reject,
        "summary.export.not_accepted"
    );
    let body = record
        .accepted_body
        .as_ref()
        .context("summary.export.content_missing")?;
    let text = body
        .synopsis
        .iter()
        .map(|statement| statement.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n";
    let references=candidate.slices.iter().map(|slice|json!({"evidence_id":slice.evidence_id,"source_id":slice.source_id,"artifact_id":slice.artifact_id,
        "source_sha256":slice.source_sha256,"start":slice.start,"end":slice.end,"text_sha256":slice.text_sha256})).collect::<Vec<_>>();
    let summary = json!({"schema_version":"renderflow.accepted-summary/v1","candidate_id":candidate.candidate_id,"candidate_sha256":record.decision.candidate_sha256,
        "review_id":record.review_id,"body":body,"references":references,"processing":candidate.processing,
        "coverage_note":candidate.coverage_note,"source_coverage":candidate.sources,
        "reviewer_reference":record.decision.reviewer_reference,"semantic_review_declared":true,"scope":REVIEW_SCOPE,"publication_approved":false});
    let mut files = BTreeMap::new();
    files.insert("summary.txt".into(), text.into_bytes());
    files.insert("summary.json".into(), serde_json::to_vec_pretty(&summary)?);
    files.insert("review.json".into(), review_bytes.to_vec());
    local::publish(
        output,
        &files,
        "export-receipt.json",
        json!({"schema_version":"renderflow.summary-export/v1","review_id":record.review_id,
        "review_sha256":expected_review_sha256,"candidate_id":record.decision.candidate_id,"scope":REVIEW_SCOPE,"publication_approved":false,
        "plain_text_losses":["citation links, uncertainty, omissions and source coverage remain in companion summary.json"]}),
        token,
    )
}

#[cfg(test)]
mod tests;
