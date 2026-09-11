//! Artifact-native execution of schema-bound AI skills.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::catalog::{
    AiAvailability, AiExecutionPreferenceV1, AiLocality, AiModelCatalog, AiResolutionReport,
};
use super::provider::AiProvider;
use super::request::{AiRequest, OutputFormat};
use super::retry::{execute_with_retry, RetryConfig};
use super::skill::{
    validate_json_instance, AiCandidateState, AiHygieneAction, AiProtectedReferenceRule,
    AiSkillRegistry, AiSkillSpec,
};
use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore};
use crate::graph::Format;

pub const AI_EXECUTION_EVIDENCE_SCHEMA_V1: &str = "renderflow.ai-execution/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiInputArtifactEvidence {
    pub artifact_id: String,
    pub digest: String,
    pub media_type: String,
    pub approved_for_ai: bool,
}

#[derive(Debug, Clone)]
pub struct AiSkillExecutionRequest {
    pub skill_id: String,
    pub skill_version: Option<String>,
    pub input: Value,
    pub input_artifacts: Vec<AiInputArtifactEvidence>,
    pub preference: AiExecutionPreferenceV1,
    pub allow_remote: bool,
    pub allow_unverified: bool,
    pub source_approved: bool,
    pub privacy_approved_for_remote: bool,
    pub additional_protected_references: Vec<AiProtectedReferenceRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiHygieneStatus {
    Passed,
    Rewritten,
    ReviewRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiHygieneFindingEvidence {
    pub code: String,
    pub class: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiHygieneEvidence {
    pub policy_id: String,
    pub status: AiHygieneStatus,
    pub findings: Vec<AiHygieneFindingEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiDigestEvidence {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiIdentityEvidence {
    pub provider_id: String,
    pub adapter: String,
    pub runtime_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_digest: Option<String>,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights_digest: Option<String>,
    pub locality: AiLocality,
    pub endpoint_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiUsageEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microunits: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiExecutionEvidence {
    pub schema_version: String,
    pub execution_id: String,
    pub occurred_at_unix_ms: u128,
    pub identity: AiIdentityEvidence,
    pub skill_id: String,
    pub skill_version: String,
    pub skill_digest: AiDigestEvidence,
    pub input_digest: AiDigestEvidence,
    pub input_artifacts: Vec<AiInputArtifactEvidence>,
    pub instruction_digest: AiDigestEvidence,
    pub prompt_digest: AiDigestEvidence,
    pub input_schema_digest: AiDigestEvidence,
    pub output_schema_digest: AiDigestEvidence,
    pub settings_digest: AiDigestEvidence,
    pub hygiene_policy_digest: AiDigestEvidence,
    pub cache_identity: AiDigestEvidence,
    pub cache_decision: String,
    pub determinism: String,
    pub usage: AiUsageEvidence,
    pub output_artifact_id: String,
    pub output_digest: AiDigestEvidence,
    pub output_state: AiCandidateState,
    pub approval_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
    pub validators: Vec<String>,
    pub validation_status: String,
    pub pre_prompt_hygiene: AiHygieneEvidence,
    pub post_output_hygiene: AiHygieneEvidence,
    pub raw_inputs_retained: bool,
    pub raw_prompts_retained: bool,
}

#[derive(Debug, Clone)]
pub struct AiSkillExecutionOutcome {
    pub artifact: Artifact,
    pub evidence: AiExecutionEvidence,
    pub resolution: AiResolutionReport,
}

pub struct AiSkillRuntime<'a> {
    catalog: &'a AiModelCatalog,
    skills: &'a AiSkillRegistry,
    providers: Vec<&'a dyn AiProvider>,
}

impl<'a> AiSkillRuntime<'a> {
    pub fn new(
        catalog: &'a AiModelCatalog,
        skills: &'a AiSkillRegistry,
        providers: Vec<&'a dyn AiProvider>,
    ) -> Self {
        Self {
            catalog,
            skills,
            providers,
        }
    }

    pub fn resolve(&self, request: &AiSkillExecutionRequest) -> Result<AiResolutionReport> {
        let skill = self.skill(request)?;
        Ok(self.catalog.resolve(&skill.resolution_request(
            request.preference,
            request.allow_remote,
            request.allow_unverified,
        )))
    }

    pub fn execute(
        &self,
        request: &AiSkillExecutionRequest,
        store: &ArtifactStore,
    ) -> Result<AiSkillExecutionOutcome> {
        let skill = self.skill(request)?;
        skill.validate()?;
        validate_json_instance(&skill.input_schema, &request.input)
            .context("AI skill input failed its declared schema")?;
        let input_bytes = serde_json::to_vec(&request.input)?;
        if input_bytes.len() > skill.budgets.max_input_bytes {
            anyhow::bail!("AI skill input exceeds its declared byte budget");
        }
        if !request.source_approved
            || request
                .input_artifacts
                .iter()
                .any(|artifact| !artifact.approved_for_ai)
        {
            anyhow::bail!("AI skill source material is not approved for model exposure");
        }

        let resolution = self.resolve(request)?;
        let selected = resolution
            .selected
            .as_ref()
            .context("no provider/model satisfies the AI skill and active policy")?;
        if selected.availability != AiAvailability::Available {
            anyhow::bail!(
                "selected model '{}:{}' is not execution-ready; inspect structured resolution evidence",
                selected.provider_id,
                selected.model_id
            );
        }
        if selected.locality == AiLocality::Remote {
            if !request.allow_remote || !skill.budgets.network || !skill.budgets.remote_execution {
                anyhow::bail!(
                    "remote execution is not explicitly allowed by both request and skill"
                );
            }
            if contains_pii(&input_bytes) && !request.privacy_approved_for_remote {
                anyhow::bail!(
                    "remote execution of detected PII requires explicit privacy approval"
                );
            }
            if !skill.hygiene.allow_private_remote_input
                && skill.variables.iter().any(|variable| variable.sensitive)
            {
                anyhow::bail!("this skill forbids sensitive variables from remote execution");
            }
        }

        let mut protected_references = skill.hygiene.protected_references.clone();
        for rule in &request.additional_protected_references {
            if rule.term.trim().is_empty() || !rule.term.is_ascii() {
                anyhow::bail!(
                    "additional protected-reference terms must be non-empty ASCII strings"
                );
            }
            if skill.hygiene.protected_reference_action == AiHygieneAction::Rewrite
                && rule
                    .descriptive_replacement
                    .as_deref()
                    .is_none_or(|replacement| replacement.trim().is_empty())
            {
                anyhow::bail!(
                    "additional protected-reference rewrites require descriptive replacements"
                );
            }
        }
        protected_references.extend(request.additional_protected_references.clone());
        let (rendered_prompt, pre_prompt_hygiene) =
            render_and_sanitize_prompt(skill, &request.input, &protected_references)?;
        ensure_not_blocked(&pre_prompt_hygiene, "pre-prompt")?;

        let provider = self
            .providers
            .iter()
            .find(|provider| provider.name() == selected.adapter)
            .with_context(|| {
                format!(
                    "resolved adapter '{}' is not configured in this runtime",
                    selected.adapter
                )
            })?;
        let max_attempts = skill.budgets.max_retries.saturating_add(1);
        let per_attempt_timeout_ms = skill.budgets.max_duration_ms / u64::from(max_attempts);
        let mut ai_request = AiRequest::new(&selected.model_id, &rendered_prompt)
            .with_output_format(OutputFormat::Json)
            .with_params(skill.generation.provider_neutral_parameters())
            .with_timeout_ms(per_attempt_timeout_ms)
            .with_prompt_version(format!("{}@{}", skill.id, skill.version));
        if skill.requires_json_schema {
            ai_request = ai_request.with_output_schema(skill.output_schema.clone());
        }
        let started = Instant::now();
        let retry_config = RetryConfig {
            max_attempts,
            initial_delay_ms: 0,
            max_delay_ms: 0,
            ..RetryConfig::default()
        };
        let response =
            execute_with_retry(&retry_config, &skill.id, || provider.execute(&ai_request))
                .with_context(|| {
                    format!(
                        "AI skill '{}@{}' failed through '{}:{}'",
                        skill.id, skill.version, selected.provider_id, selected.model_id
                    )
                })?;
        let observed_duration_ms = started.elapsed().as_millis() as u64;
        if observed_duration_ms > skill.budgets.max_duration_ms {
            anyhow::bail!("AI skill execution exceeded its declared time budget");
        }
        if response.provider != selected.adapter || response.model != selected.model_id {
            anyhow::bail!("AI provider returned identity that does not match the resolved model");
        }
        if response
            .output_tokens
            .is_some_and(|tokens| tokens > skill.budgets.max_tokens)
        {
            anyhow::bail!("AI skill output exceeded its declared token budget");
        }
        if response.content.len() > skill.budgets.max_output_bytes {
            anyhow::bail!("AI skill output exceeds its declared byte budget");
        }
        let mut output: Value = serde_json::from_str(&response.content)
            .context("AI skill output is not valid structured JSON")?;
        let post_output_hygiene = sanitize_json_output(&mut output, skill, &protected_references)?;
        ensure_not_blocked(&post_output_hygiene, "post-output")?;
        validate_json_instance(&skill.output_schema, &output)
            .context("AI skill output failed its declared schema")?;

        let output_bytes = serde_json::to_vec_pretty(&output)?;
        let artifact = store.put_bytes(
            &output_bytes,
            ArtifactDescriptor::for_format(Format::Json, ArtifactStorageClass::Intermediate)
                .with_metadata("renderflow.ai.skill_id", skill.id.clone())
                .with_metadata("renderflow.ai.skill_version", skill.version.clone())
                .with_metadata("renderflow.ai.provider_id", selected.provider_id.clone())
                .with_metadata("renderflow.ai.model_id", selected.model_id.clone())
                .with_metadata("renderflow.ai.output_state", "candidate")
                .with_metadata("renderflow.ai.review_required", true),
        )?;

        let skill_digest = digest_json(skill)?;
        let input_digest = digest_bytes(&input_bytes);
        let instruction_digest = digest_bytes(
            format!(
                "{}\n{}",
                skill.templates.system, skill.templates.instruction
            )
            .as_bytes(),
        );
        let prompt_digest = digest_bytes(rendered_prompt.as_bytes());
        let input_schema_digest = digest_json(&skill.input_schema)?;
        let output_schema_digest = digest_json(&skill.output_schema)?;
        let settings_digest = digest_json(&skill.generation)?;
        let hygiene_policy_digest = digest_json(&skill.hygiene)?;
        let cache_identity = digest_json(&serde_json::json!({
            "catalog_revision": self.catalog.revision,
            "provider": selected.provider_id,
            "runtime": selected.runtime_id,
            "runtime_revision": selected.runtime_revision,
            "runtime_digest": selected.runtime_digest,
            "model": selected.model_id,
            "model_revision": selected.model_revision,
            "weights_digest": selected.weights_digest,
            "skill": skill.id,
            "skill_version": skill.version,
            "skill_digest": skill_digest.value,
            "input_digest": input_digest.value,
            "input_schema_digest": input_schema_digest.value,
            "output_schema_digest": output_schema_digest.value,
            "settings_digest": settings_digest.value,
            "hygiene_policy_digest": hygiene_policy_digest.value,
        }))?;
        let execution_id = format!("ai:sha256:{}", cache_identity.value);
        let evidence = AiExecutionEvidence {
            schema_version: AI_EXECUTION_EVIDENCE_SCHEMA_V1.to_string(),
            execution_id,
            occurred_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            identity: AiIdentityEvidence {
                provider_id: selected.provider_id.clone(),
                adapter: selected.adapter.clone(),
                runtime_id: selected.runtime_id.clone(),
                runtime_revision: selected.runtime_revision.clone(),
                runtime_digest: selected.runtime_digest.clone(),
                model_id: selected.model_id.clone(),
                model_revision: selected.model_revision.clone(),
                weights_digest: selected.weights_digest.clone(),
                locality: selected.locality,
                endpoint_identity: selected.runtime_id.clone(),
            },
            skill_id: skill.id.clone(),
            skill_version: skill.version.clone(),
            skill_digest,
            input_digest,
            input_artifacts: request.input_artifacts.clone(),
            instruction_digest,
            prompt_digest,
            input_schema_digest,
            output_schema_digest,
            settings_digest,
            hygiene_policy_digest,
            cache_identity,
            cache_decision: "not_requested".to_string(),
            determinism: format!("{:?}", selected.determinism).to_lowercase(),
            usage: AiUsageEvidence {
                input_tokens: response.input_tokens,
                output_tokens: response.output_tokens,
                duration_ms: Some(response.duration_ms.unwrap_or(observed_duration_ms)),
                cost_microunits: None,
            },
            output_artifact_id: artifact.id().to_string(),
            output_digest: AiDigestEvidence {
                algorithm: artifact.digest().algorithm().to_string(),
                value: artifact.digest().value().to_string(),
            },
            output_state: AiCandidateState::Candidate,
            approval_required: true,
            approval_reference: None,
            validators: skill.approval.validators.clone(),
            validation_status: "valid_review_required".to_string(),
            pre_prompt_hygiene,
            post_output_hygiene,
            raw_inputs_retained: false,
            raw_prompts_retained: false,
        };
        Ok(AiSkillExecutionOutcome {
            artifact,
            evidence,
            resolution,
        })
    }

    fn skill(&self, request: &AiSkillExecutionRequest) -> Result<&AiSkillSpec> {
        self.skills
            .get(&request.skill_id, request.skill_version.as_deref())
            .with_context(|| {
                format!(
                    "AI skill '{}'{} is not registered",
                    request.skill_id,
                    request
                        .skill_version
                        .as_deref()
                        .map(|version| format!("@{version}"))
                        .unwrap_or_default()
                )
            })
    }
}

fn render_and_sanitize_prompt(
    skill: &AiSkillSpec,
    input: &Value,
    protected_references: &[AiProtectedReferenceRule],
) -> Result<(String, AiHygieneEvidence)> {
    let input = input
        .as_object()
        .context("AI skill input must be an object")?;
    let mut prompt = format!(
        "SYSTEM\n{}\n\nINSTRUCTION\n{}\n\nINPUT\n{}",
        skill.templates.system, skill.templates.instruction, skill.templates.prompt
    );
    for variable in &skill.variables {
        let value = input.get(&variable.name);
        if variable.required && value.is_none() {
            anyhow::bail!("AI skill variable '{}' is required", variable.name);
        }
        let rendered = match value {
            Some(Value::String(value)) => value.clone(),
            Some(value) => serde_json::to_string(value)?,
            None => String::new(),
        };
        if rendered.len() > variable.max_bytes {
            anyhow::bail!(
                "AI skill variable '{}' exceeds its byte limit",
                variable.name
            );
        }
        prompt = prompt.replace(&format!("{{{{{}}}}}", variable.name), &rendered);
    }
    if prompt.contains("{{") || prompt.contains("}}") {
        anyhow::bail!("AI skill prompt contains unresolved template variables");
    }
    if prompt.len() > skill.max_rendered_prompt_bytes {
        anyhow::bail!("rendered AI skill prompt exceeds its byte budget");
    }
    sanitize_text(&mut prompt, skill, protected_references, "prompt")
        .map(|evidence| (prompt, evidence))
}

fn sanitize_json_output(
    output: &mut Value,
    skill: &AiSkillSpec,
    protected_references: &[AiProtectedReferenceRule],
) -> Result<AiHygieneEvidence> {
    let mut findings = Vec::new();
    visit_strings(output, &mut |text| {
        let evidence = sanitize_text(text, skill, protected_references, "output")?;
        findings.extend(evidence.findings);
        Ok(())
    })?;
    Ok(hygiene_evidence(skill, findings))
}

fn visit_strings(
    value: &mut Value,
    visitor: &mut impl FnMut(&mut String) -> Result<()>,
) -> Result<()> {
    match value {
        Value::String(text) => visitor(text),
        Value::Array(values) => {
            for value in values {
                visit_strings(value, visitor)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                visit_strings(value, visitor)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn sanitize_text(
    text: &mut String,
    skill: &AiSkillSpec,
    protected_references: &[AiProtectedReferenceRule],
    stage: &str,
) -> Result<AiHygieneEvidence> {
    let mut findings = Vec::new();
    if skill.hygiene.scan_secrets && contains_secret(text.as_bytes()) {
        findings.push(AiHygieneFindingEvidence {
            code: format!("ai.hygiene.{stage}.secret"),
            class: "credential".to_string(),
            action: "block".to_string(),
        });
    }
    if contains_pii(text.as_bytes()) {
        findings.push(AiHygieneFindingEvidence {
            code: format!("ai.hygiene.{stage}.pii"),
            class: "possible_contact_identifier".to_string(),
            action: action_name(skill.hygiene.pii_action).to_string(),
        });
    }
    for rule in protected_references {
        if contains_case_insensitive(text, &rule.term) {
            let action = skill.hygiene.protected_reference_action;
            findings.push(AiHygieneFindingEvidence {
                code: format!("ai.hygiene.{stage}.protected_reference"),
                class: "configured_protected_reference".to_string(),
                action: action_name(action).to_string(),
            });
            if action == AiHygieneAction::Rewrite {
                let replacement = rule
                    .descriptive_replacement
                    .as_deref()
                    .context("protected-reference rewrite is missing a descriptive replacement")?;
                *text = replace_case_insensitive(text, &rule.term, replacement);
            }
        }
    }
    Ok(hygiene_evidence(skill, findings))
}

fn hygiene_evidence(
    skill: &AiSkillSpec,
    findings: Vec<AiHygieneFindingEvidence>,
) -> AiHygieneEvidence {
    let blocked = findings.iter().any(|finding| finding.action == "block");
    let rewritten = findings.iter().any(|finding| finding.action == "rewrite");
    let review = findings.iter().any(|finding| finding.action == "review")
        || (skill.hygiene.post_output_review && !findings.is_empty());
    AiHygieneEvidence {
        policy_id: skill.hygiene.policy_id.clone(),
        status: if blocked {
            AiHygieneStatus::Blocked
        } else if review {
            AiHygieneStatus::ReviewRequired
        } else if rewritten {
            AiHygieneStatus::Rewritten
        } else {
            AiHygieneStatus::Passed
        },
        findings,
    }
}

fn ensure_not_blocked(evidence: &AiHygieneEvidence, stage: &str) -> Result<()> {
    if evidence.status == AiHygieneStatus::Blocked {
        anyhow::bail!("AI {stage} hygiene blocked execution; raw findings were not retained");
    }
    Ok(())
}

fn action_name(action: AiHygieneAction) -> &'static str {
    match action {
        AiHygieneAction::Block => "block",
        AiHygieneAction::Rewrite => "rewrite",
        AiHygieneAction::Review => "review",
    }
}

fn contains_secret(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    text.contains("-----begin private key-----")
        || text.contains("github_pat_")
        || text.contains("authorization: bearer ")
        || text.contains("api_key=")
        || text.contains("password=")
        || text.split_whitespace().any(|word| {
            word.strip_prefix("sk-")
                .is_some_and(|tail| tail.len() >= 20)
        })
}

fn contains_pii(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes)
        .split_whitespace()
        .any(|word| {
            let trimmed = word.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && !matches!(character, '@' | '.' | '_' | '-')
            });
            let mut parts = trimmed.split('@');
            parts.next().is_some_and(|local| !local.is_empty())
                && parts
                    .next()
                    .is_some_and(|domain| domain.contains('.') && domain.len() >= 3)
                && parts.next().is_none()
        })
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    !needle.is_empty()
        && haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn replace_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = haystack.as_bytes()[cursor..]
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
    {
        let start = cursor + relative;
        output.push_str(&haystack[cursor..start]);
        output.push_str(replacement);
        cursor = start + needle.len();
    }
    output.push_str(&haystack[cursor..]);
    output
}

fn digest_json(value: &impl Serialize) -> Result<AiDigestEvidence> {
    Ok(digest_bytes(&serde_json::to_vec(value)?))
}

fn digest_bytes(bytes: &[u8]) -> AiDigestEvidence {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    AiDigestEvidence {
        algorithm: "sha256".to_string(),
        value: format!("{:x}", hasher.finalize()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::catalog::{AiAvailabilityProbe, AiProviderCatalogEntry, AiRuntimeDescriptor};
    use crate::ai::{AiCapabilities, AiModel, AiResponse};

    #[derive(Debug)]
    struct FixtureProvider;

    impl AiProvider for FixtureProvider {
        fn name(&self) -> &str {
            "fixture"
        }

        fn is_local(&self) -> bool {
            true
        }

        fn capabilities(&self) -> AiCapabilities {
            AiCapabilities::new()
        }

        fn models(&self) -> Vec<AiModel> {
            vec![AiModel::new("fixture-model", true)]
        }

        fn execute(&self, request: &AiRequest) -> Result<crate::ai::AiResponse> {
            assert!(request.output_schema.is_some());
            assert_eq!(request.params.seed, Some(42));
            assert_eq!(request.timeout_ms, Some(30_000));
            Ok(AiResponse::new(
                r#"{"title":"Synthetic booklet","summary":"Mindful geometric pauses.","keywords":["mindfulness"],"review_required":true}"#,
                "fixture-model",
                "fixture",
            ))
        }
    }

    fn fixture_catalog() -> AiModelCatalog {
        let mut catalog = AiModelCatalog::bundled().unwrap();
        let mut model = catalog.providers[0].models[0].clone();
        model.id = "fixture-model".to_string();
        model.availability = AiAvailability::Available;
        catalog.providers = vec![AiProviderCatalogEntry {
            id: "provider.fixture".to_string(),
            adapter: "fixture".to_string(),
            display_name: "Fixture".to_string(),
            locality: AiLocality::Local,
            requires_network_permission: false,
            runtime: AiRuntimeDescriptor {
                id: "runtime.fixture".to_string(),
                revision: Some("1".to_string()),
                digest: Some("sha256:fixture".to_string()),
                source: None,
                license: Some("MIT".to_string()),
                hardware_requirements: Vec::new(),
                availability_probe: AiAvailabilityProbe {
                    kind: "fixture".to_string(),
                    bounded_timeout_ms: 1,
                    endpoint: None,
                },
            },
            models: vec![model],
        }];
        catalog
    }

    #[test]
    fn runtime_creates_validated_candidate_artifact_and_redacted_evidence() {
        let catalog = fixture_catalog();
        let skills = AiSkillRegistry::bundled().unwrap();
        let provider = FixtureProvider;
        let runtime = AiSkillRuntime::new(&catalog, &skills, vec![&provider]);
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let outcome = runtime
            .execute(
                &AiSkillExecutionRequest {
                    skill_id: "skill.metadata.extract".to_string(),
                    skill_version: None,
                    input: serde_json::json!({"source_text": "Synthetic mindful geometry."}),
                    input_artifacts: Vec::new(),
                    preference: AiExecutionPreferenceV1::LocalOnly,
                    allow_remote: false,
                    allow_unverified: false,
                    source_approved: true,
                    privacy_approved_for_remote: false,
                    additional_protected_references: Vec::new(),
                },
                &store,
            )
            .unwrap();
        assert_eq!(outcome.evidence.output_state, AiCandidateState::Candidate);
        assert!(outcome.evidence.approval_required);
        assert!(!outcome.evidence.raw_prompts_retained);
        assert!(store.verify(&outcome.artifact).is_ok());
    }

    #[test]
    fn protected_reference_is_rewritten_without_leaking_term_into_evidence() {
        let skills = AiSkillRegistry::bundled().unwrap();
        let skill = skills.get("skill.prompt.from-sanitized-dna", None).unwrap();
        let (prompt, evidence) = render_and_sanitize_prompt(
            skill,
            &serde_json::json!({
                "dna": "Example Franchise mood",
                "intent": "Original divider"
            }),
            &skill.hygiene.protected_references,
        )
        .unwrap();
        assert!(!prompt.contains("Example Franchise"));
        assert!(prompt.contains("weathered post-industrial"));
        assert!(!serde_json::to_string(&evidence)
            .unwrap()
            .contains("Example Franchise"));
    }
}
