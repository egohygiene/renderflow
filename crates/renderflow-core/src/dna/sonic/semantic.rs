use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

use super::facts::SonicFact;
use super::recipe::{SemanticMode, SonicRecipe};
use crate::ai::{
    AiExecutionEvidence, AiInputArtifactEvidence, AiResolutionReport, AiSkillExecutionRequest,
    AiSkillRuntime, AiSkillSpec,
};
use crate::artifact::{Artifact, ArtifactStore};
use crate::dna::ArtifactDna;
use crate::process::ProcessCancellationToken;
use crate::publication::local;

pub const SKILL_ID: &str = "skill.sonic.describe-evidence";
pub const SKILL_VERSION: &str = "1.0.0";
pub const SKILL_SOURCE: &str = include_str!("../../../data/ai/skills/sonic-description-v1.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceText {
    pub text: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptionSection {
    pub topic: String,
    pub text: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptionCandidate {
    pub summary: EvidenceText,
    pub sections: Vec<DescriptionSection>,
    pub uncertainty: Vec<String>,
    pub review_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticReport {
    pub mode: SemanticMode,
    pub status: String,
    pub diagnostic_code: String,
    pub resolution: Option<AiResolutionReport>,
    pub execution: Option<AiExecutionEvidence>,
}

pub(super) fn request(
    recipe: &SonicRecipe,
    facts: &[SonicFact],
    dna_sha256: Option<&str>,
    artifact: Option<&Artifact>,
) -> Result<AiSkillExecutionRequest> {
    Ok(AiSkillExecutionRequest {
        skill_id: SKILL_ID.into(),
        skill_version: Some(SKILL_VERSION.into()),
        // A compatibility-only query has no fabricated DNA or executable input.
        input: if let Some(digest) = dna_sha256 {
            json!({"evidence":serde_json::to_string(facts)?,"source_sha256":recipe.audio.sha256,"dna_sha256":digest})
        } else {
            Value::Null
        },
        input_artifacts: artifact
            .map(|artifact| {
                vec![AiInputArtifactEvidence {
                    artifact_id: artifact.id().to_string(),
                    digest: artifact.digest().value().into(),
                    media_type: artifact.media_type().to_string(),
                    approved_for_ai: recipe.semantic.source_approved_for_ai,
                }]
            })
            .unwrap_or_default(),
        preference: recipe.semantic.preference,
        allow_remote: recipe.policy.allow_remote && recipe.policy.allow_network,
        allow_unverified: false,
        source_approved: recipe.semantic.source_approved_for_ai,
        privacy_approved_for_remote: false,
        additional_protected_references: Vec::new(),
    })
}

pub(super) fn availability(
    recipe: &SonicRecipe,
    request: &AiSkillExecutionRequest,
    runtime: Option<&AiSkillRuntime<'_>>,
) -> SemanticReport {
    let mut report = SemanticReport {
        mode: recipe.semantic.mode,
        status: "disabled".into(),
        diagnostic_code: "sonic.semantic.disabled".into(),
        resolution: None,
        execution: None,
    };
    if recipe.semantic.mode == SemanticMode::Disabled {
        return report;
    }
    if !recipe.policy.allow_ai || !recipe.semantic.source_approved_for_ai {
        report.status = "blocked".into();
        report.diagnostic_code = "sonic.semantic.explicit_ai_and_source_permission_required".into();
        return report;
    }
    let Some(runtime) = runtime else {
        report.status = "not_configured".into();
        report.diagnostic_code = "sonic.semantic.runtime_not_supplied".into();
        return report;
    };
    match runtime.resolve(request) {
        Ok(resolution) => {
            if resolution
                .selected
                .as_ref()
                .is_some_and(|selected| selected.execution_ready)
            {
                report.status = "ready".into();
                report.diagnostic_code = "sonic.semantic.catalog_declared_ready".into();
            } else {
                report.status = "unavailable".into();
                report.diagnostic_code = "sonic.semantic.no_compatible_available_model".into();
            }
            report.resolution = Some(resolution);
        }
        Err(_) => {
            report.status = "unavailable".into();
            report.diagnostic_code = "sonic.semantic.skill_resolution_failed".into();
        }
    }
    report
}

pub(super) fn execute(
    recipe: &SonicRecipe,
    dna: &ArtifactDna,
    artifact: &Artifact,
    facts: &[SonicFact],
    runtime: Option<&AiSkillRuntime<'_>>,
    store: &ArtifactStore,
    token: &ProcessCancellationToken,
) -> Result<(SemanticReport, Option<DescriptionCandidate>)> {
    ensure!(!token.is_cancelled(), "sonic.cancelled");
    let request = request(
        recipe,
        facts,
        Some(artifact.digest().value()),
        Some(artifact),
    )?;
    let mut report = availability(recipe, &request, runtime);
    if report.status != "ready" {
        return Ok((report, None));
    }
    let skill = AiSkillSpec::from_json(SKILL_SOURCE)?;
    if serde_json::to_vec(&request.input)?.len() > skill.budgets.max_input_bytes {
        report.status = "blocked".into();
        report.diagnostic_code = "sonic.semantic.input_budget".into();
        return Ok((report, None));
    }
    let outcome = match runtime
        .context("sonic.semantic.runtime")?
        .execute(&request, store)
    {
        Ok(outcome) => outcome,
        Err(_) => {
            report.status = "failed".into();
            report.diagnostic_code = "sonic.semantic.runtime_execution_failed".into();
            return Ok((report, None));
        }
    };
    ensure!(!token.is_cancelled(), "sonic.cancelled");
    let checked = (|| -> Result<DescriptionCandidate> {
        store.verify(&outcome.artifact)?;
        ensure!(
            outcome.evidence.skill_digest.value == local::sha256(&serde_json::to_vec(&skill)?)
                && outcome.evidence.skill_id == SKILL_ID
                && outcome.evidence.skill_version == SKILL_VERSION
                && outcome.evidence.output_digest.value == outcome.artifact.digest().value()
                && outcome.evidence.output_artifact_id == outcome.artifact.id().as_str()
                && outcome.evidence.input_digest.value
                    == local::sha256(&serde_json::to_vec(&request.input)?),
            "sonic.semantic.skill_or_output_identity"
        );
        let bytes = store.read_bytes(&outcome.artifact)?;
        ensure!(bytes.len() <= 16 * 1024, "sonic.semantic.output_budget");
        let mut value: Value = local::parse(&bytes)?;
        let mut findings = Vec::new();
        ensure!(
            crate::dna::sanitize_value(&mut value, &recipe.policy, &mut findings),
            "sonic.semantic.domain_hygiene"
        );
        let candidate: DescriptionCandidate = serde_json::from_value(value)?;
        validate(&candidate, facts)?;
        ensure!(dna.approval.human_review_required, "sonic.semantic.review");
        Ok(candidate)
    })();
    report.execution = Some(outcome.evidence);
    match checked {
        Ok(candidate) => {
            report.status = "produced".into();
            report.diagnostic_code = "sonic.semantic.review_required_candidate".into();
            Ok((report, Some(candidate)))
        }
        Err(_) => {
            report.status = "invalid".into();
            report.diagnostic_code = "sonic.semantic.citation_hygiene_or_contract_failure".into();
            Ok((report, None))
        }
    }
}

pub fn validate(candidate: &DescriptionCandidate, facts: &[SonicFact]) -> Result<()> {
    ensure!(
        candidate.review_required
            && !candidate.sections.is_empty()
            && candidate.sections.len() <= 32
            && candidate.uncertainty.len() <= 32,
        "sonic.description.bounds_review"
    );
    let known = facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect::<BTreeSet<_>>();
    validate_text(
        &candidate.summary.text,
        &candidate.summary.evidence_ids,
        &known,
        2048,
    )?;
    for section in &candidate.sections {
        ensure!(
            matches!(
                section.topic.as_str(),
                "technical"
                    | "rhythm"
                    | "harmony"
                    | "timbre"
                    | "vocal"
                    | "arrangement"
                    | "mix"
                    | "lyrics"
                    | "other"
            ),
            "sonic.description.topic"
        );
        validate_text(&section.text, &section.evidence_ids, &known, 4096)?;
    }
    for note in &candidate.uncertainty {
        local::text(note, 2048)?;
    }
    Ok(())
}

fn validate_text(text: &str, ids: &[String], known: &BTreeSet<&str>, maximum: usize) -> Result<()> {
    local::text(text, maximum)?;
    ensure!(
        !ids.is_empty() && ids.len() <= 16,
        "sonic.description.citations.count"
    );
    let mut unique = BTreeSet::new();
    for id in ids {
        ensure!(
            unique.insert(id) && known.contains(id.as_str()),
            "sonic.description.citation.unknown_or_repeated"
        );
    }
    Ok(())
}
