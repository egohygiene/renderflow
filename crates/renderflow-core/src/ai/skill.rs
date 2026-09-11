//! Reviewed, versioned AI skill specifications and strict JSON contracts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::catalog::{AiModality, AiOperation, AiResolutionRequest};
use super::request::GenerationParameters;

pub const AI_SKILL_SCHEMA_V1: &str = "renderflow.ai-skill/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiCandidateState {
    Candidate,
    Approved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiHygieneAction {
    Block,
    Rewrite,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillVariable {
    pub name: String,
    pub required: bool,
    #[serde(default)]
    pub sensitive: bool,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillTemplates {
    pub system: String,
    pub instruction: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillGeneration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub provider_extensions: BTreeMap<String, Value>,
}

impl AiSkillGeneration {
    pub fn provider_neutral_parameters(&self) -> GenerationParameters {
        GenerationParameters {
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            seed: self.seed,
            top_p: self.top_p,
            stop: self.stop.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillBudgets {
    pub network: bool,
    pub remote_execution: bool,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    pub max_tokens: u32,
    pub max_duration_ms: u64,
    pub max_retries: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost_microunits: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiProtectedReferenceRule {
    pub term: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptive_replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillHygienePolicy {
    pub policy_id: String,
    pub scan_secrets: bool,
    pub pii_action: AiHygieneAction,
    pub protected_reference_action: AiHygieneAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_references: Vec<AiProtectedReferenceRule>,
    pub allow_private_remote_input: bool,
    pub retain_raw_prompts: bool,
    pub post_output_review: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillApprovalPolicy {
    pub initial_state: AiCandidateState,
    pub human_review_required: bool,
    pub validators: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillProvenancePolicy {
    pub cache_identity_fields: Vec<String>,
    pub evidence_fields: Vec<String>,
    pub redact_raw_inputs: bool,
    pub redact_raw_prompts: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillSpec {
    pub schema_version: String,
    pub id: String,
    pub version: String,
    pub purpose: String,
    pub artifact_families: Vec<String>,
    pub operations: Vec<AiOperation>,
    pub input_modalities: Vec<AiModality>,
    pub output_modalities: Vec<AiModality>,
    pub requires_json_schema: bool,
    pub input_schema: Value,
    pub output_schema: Value,
    pub templates: AiSkillTemplates,
    pub variables: Vec<AiSkillVariable>,
    pub max_rendered_prompt_bytes: usize,
    pub generation: AiSkillGeneration,
    pub budgets: AiSkillBudgets,
    pub hygiene: AiSkillHygienePolicy,
    pub approval: AiSkillApprovalPolicy,
    pub provenance: AiSkillProvenancePolicy,
    pub redistribution_notes: String,
    pub license_notes: String,
    pub fixture: Value,
}

impl AiSkillSpec {
    pub fn from_json(contents: &str) -> Result<Self> {
        let skill: Self = serde_json::from_str(contents).context("skill is not valid JSON")?;
        skill.validate()?;
        Ok(skill)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read AI skill '{}'", path.display()))?;
        Self::from_json(&contents).with_context(|| format!("invalid AI skill '{}'", path.display()))
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != AI_SKILL_SCHEMA_V1 {
            anyhow::bail!(
                "unsupported AI skill schema '{}'; expected '{}'",
                self.schema_version,
                AI_SKILL_SCHEMA_V1
            );
        }
        validate_skill_id(&self.id)?;
        if self.version.trim().is_empty() || self.purpose.trim().is_empty() {
            anyhow::bail!("AI skill version and purpose must not be empty");
        }
        if self.operations.is_empty()
            || self.input_modalities.is_empty()
            || self.output_modalities.is_empty()
        {
            anyhow::bail!("AI skill must declare operations and typed input/output modalities");
        }
        validate_json_schema_definition(&self.input_schema, "input_schema")?;
        validate_json_schema_definition(&self.output_schema, "output_schema")?;
        require_closed_object_schema(&self.input_schema, "input_schema")?;
        require_closed_object_schema(&self.output_schema, "output_schema")?;
        validate_json_instance(&self.input_schema, &self.fixture)
            .context("synthetic fixture does not satisfy input_schema")?;
        if self.max_rendered_prompt_bytes == 0
            || self.budgets.max_input_bytes == 0
            || self.budgets.max_output_bytes == 0
            || self.budgets.max_duration_ms == 0
        {
            anyhow::bail!("AI skill byte and time budgets must be greater than zero");
        }
        if self.budgets.remote_execution && !self.budgets.network {
            anyhow::bail!("AI skill cannot allow remote execution while network use is disabled");
        }
        if self.generation.max_tokens.unwrap_or(0) > self.budgets.max_tokens {
            anyhow::bail!("generation max_tokens exceeds the skill token budget");
        }
        if self.budgets.max_retries > 10 {
            anyhow::bail!("AI skill retry budget must not exceed 10 retries");
        }
        if self.budgets.max_duration_ms < u64::from(self.budgets.max_retries.saturating_add(1)) {
            anyhow::bail!("AI skill time budget must allocate at least 1 ms per attempt");
        }
        if self
            .generation
            .temperature
            .is_some_and(|temperature| !(0.0..=2.0).contains(&temperature))
        {
            anyhow::bail!("AI skill temperature must be between 0.0 and 2.0");
        }
        if self
            .generation
            .top_p
            .is_some_and(|top_p| !(0.0..=1.0).contains(&top_p))
        {
            anyhow::bail!("AI skill top_p must be between 0.0 and 1.0");
        }
        if self.approval.initial_state == AiCandidateState::Approved
            && self.approval.human_review_required
        {
            anyhow::bail!("a review-required AI skill must initially produce a candidate");
        }
        if self.hygiene.retain_raw_prompts || !self.provenance.redact_raw_prompts {
            anyhow::bail!("v1 AI skills must redact raw prompts from durable evidence");
        }

        let variable_names: BTreeSet<&str> = self
            .variables
            .iter()
            .map(|variable| variable.name.as_str())
            .collect();
        if variable_names.len() != self.variables.len() {
            anyhow::bail!("AI skill variable names must be unique");
        }
        for variable in &self.variables {
            validate_variable_name(&variable.name)?;
            if variable.max_bytes == 0 {
                anyhow::bail!(
                    "AI skill variable '{}' must have a positive byte limit",
                    variable.name
                );
            }
        }
        let referenced = referenced_variables(&format!(
            "{}\n{}\n{}",
            self.templates.system, self.templates.instruction, self.templates.prompt
        ))?;
        for referenced_name in &referenced {
            if !variable_names.contains(referenced_name.as_str()) {
                anyhow::bail!(
                    "AI skill template references undeclared variable '{{{{{referenced_name}}}}}'"
                );
            }
        }
        let input_properties = self.input_schema["properties"]
            .as_object()
            .context("input_schema.properties must be an object")?;
        let required_properties: BTreeSet<&str> = self.input_schema["required"]
            .as_array()
            .context("input_schema.required must be an array")?
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for variable in &self.variables {
            if !input_properties.contains_key(&variable.name) {
                anyhow::bail!(
                    "AI skill variable '{}' is missing from input_schema.properties",
                    variable.name
                );
            }
            if variable.required != required_properties.contains(variable.name.as_str()) {
                anyhow::bail!(
                    "AI skill variable '{}' required flag disagrees with input_schema.required",
                    variable.name
                );
            }
            if variable.required && !referenced.contains(&variable.name) {
                anyhow::bail!(
                    "required AI skill variable '{}' is not used by its templates",
                    variable.name
                );
            }
        }
        for rule in &self.hygiene.protected_references {
            if rule.term.trim().is_empty() {
                anyhow::bail!("protected-reference terms must not be empty");
            }
            if !rule.term.is_ascii() {
                anyhow::bail!("v1 protected-reference terms must be ASCII for stable matching");
            }
            if self.hygiene.protected_reference_action == AiHygieneAction::Rewrite
                && rule
                    .descriptive_replacement
                    .as_deref()
                    .is_none_or(|replacement| replacement.trim().is_empty())
            {
                anyhow::bail!(
                    "rewrite policy requires a descriptive replacement for every protected reference"
                );
            }
        }
        Ok(())
    }

    pub fn resolution_request(
        &self,
        preference: super::catalog::AiExecutionPreferenceV1,
        allow_remote: bool,
        allow_unverified: bool,
    ) -> AiResolutionRequest {
        AiResolutionRequest {
            operations: self.operations.clone(),
            input_modalities: self.input_modalities.clone(),
            output_modalities: self.output_modalities.clone(),
            requires_json_schema: self.requires_json_schema,
            preference,
            allow_remote,
            remote_policy_allows: self.budgets.remote_execution && self.budgets.network,
            allow_unverified,
        }
    }
}

fn require_closed_object_schema(schema: &Value, path: &str) -> Result<()> {
    if schema["type"] != "object" {
        anyhow::bail!("{path} must declare type 'object'");
    }
    if schema["additionalProperties"] != false {
        anyhow::bail!("{path} must set additionalProperties to false");
    }
    if !schema["properties"].is_object() {
        anyhow::bail!("{path}.properties must be an object");
    }
    if !schema["required"].is_array() {
        anyhow::bail!("{path}.required must be an array");
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct AiSkillRegistry {
    skills: Vec<AiSkillSpec>,
}

impl AiSkillRegistry {
    pub fn bundled() -> Result<Self> {
        let sources = [
            include_str!("../../data/ai/skills/metadata-extraction-v1.json"),
            include_str!("../../data/ai/skills/visual-dna-v1.json"),
            include_str!("../../data/ai/skills/prompt-from-dna-v1.json"),
            include_str!("../../data/ai/skills/accessibility-description-v1.json"),
        ];
        let mut registry = Self::default();
        for source in sources {
            registry.insert(AiSkillSpec::from_json(source)?)?;
        }
        Ok(registry)
    }

    pub fn insert(&mut self, skill: AiSkillSpec) -> Result<()> {
        skill.validate()?;
        if self
            .skills
            .iter()
            .any(|existing| existing.id == skill.id && existing.version == skill.version)
        {
            anyhow::bail!("duplicate AI skill '{}@{}'", skill.id, skill.version);
        }
        self.skills.push(skill);
        self.skills
            .sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
        Ok(())
    }

    pub fn get(&self, id: &str, version: Option<&str>) -> Option<&AiSkillSpec> {
        self.skills
            .iter()
            .rev()
            .find(|skill| skill.id == id && version.is_none_or(|value| skill.version == value))
    }

    pub fn iter(&self) -> impl Iterator<Item = &AiSkillSpec> {
        self.skills.iter()
    }

    pub fn validate_all(&self) -> Vec<AiSkillValidationResult> {
        self.skills
            .iter()
            .map(|skill| match skill.validate() {
                Ok(()) => AiSkillValidationResult {
                    id: skill.id.clone(),
                    version: skill.version.clone(),
                    valid: true,
                    error: None,
                },
                Err(error) => AiSkillValidationResult {
                    id: skill.id.clone(),
                    version: skill.version.clone(),
                    valid: false,
                    error: Some(error.to_string()),
                },
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSkillValidationResult {
    pub id: String,
    pub version: String,
    pub valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn validate_skill_id(value: &str) -> Result<()> {
    if !value.starts_with("skill.")
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        anyhow::bail!("AI skill id '{value}' must be a stable 'skill.*' identifier");
    }
    Ok(())
}

fn validate_variable_name(value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        anyhow::bail!("AI skill variable '{value}' must be alphanumeric or underscore");
    }
    Ok(())
}

fn referenced_variables(template: &str) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let mut remaining = template;
    while let Some(start) = remaining.find("{{") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("}}") else {
            anyhow::bail!("AI skill template contains an unclosed variable expression");
        };
        let name = after_start[..end].trim();
        validate_variable_name(name)?;
        names.insert(name.to_string());
        remaining = &after_start[end + 2..];
    }
    if remaining.contains("}}") {
        anyhow::bail!("AI skill template contains an unmatched closing variable expression");
    }
    Ok(names)
}

/// Validate the conservative JSON Schema subset supported by the v1 skill runtime.
///
/// Rejecting unknown validation keywords keeps contracts strict: a skill cannot
/// appear validated while relying on a keyword the runtime silently ignores.
pub fn validate_json_schema_definition(schema: &Value, path: &str) -> Result<()> {
    let object = schema
        .as_object()
        .with_context(|| format!("{path} must be a JSON Schema object"))?;
    let supported = [
        "$schema",
        "$id",
        "title",
        "description",
        "type",
        "const",
        "enum",
        "required",
        "properties",
        "additionalProperties",
        "items",
        "minLength",
        "maxLength",
        "minimum",
        "maximum",
        "minItems",
        "maxItems",
    ];
    for key in object.keys() {
        if !supported.contains(&key.as_str()) {
            anyhow::bail!("{path} uses unsupported JSON Schema keyword '{key}'");
        }
    }
    if let Some(schema_type) = object.get("type") {
        let schema_type = schema_type
            .as_str()
            .with_context(|| format!("{path}.type must be a string"))?;
        if ![
            "object", "array", "string", "number", "integer", "boolean", "null",
        ]
        .contains(&schema_type)
        {
            anyhow::bail!("{path}.type '{schema_type}' is not supported");
        }
    }
    if let Some(properties) = object.get("properties") {
        for (name, property) in properties
            .as_object()
            .with_context(|| format!("{path}.properties must be an object"))?
        {
            validate_json_schema_definition(property, &format!("{path}.properties.{name}"))?;
        }
    }
    if let Some(items) = object.get("items") {
        validate_json_schema_definition(items, &format!("{path}.items"))?;
    }
    if let Some(required) = object.get("required") {
        let required = required
            .as_array()
            .with_context(|| format!("{path}.required must be an array"))?;
        if required.iter().any(|item| !item.is_string()) {
            anyhow::bail!("{path}.required entries must be strings");
        }
    }
    if let Some(additional) = object.get("additionalProperties") {
        if !additional.is_boolean() {
            anyhow::bail!("{path}.additionalProperties must be a boolean");
        }
    }
    Ok(())
}

/// Validate a JSON value against the runtime's conservative schema subset.
pub fn validate_json_instance(schema: &Value, instance: &Value) -> Result<()> {
    validate_instance_at(schema, instance, "$")
}

fn validate_instance_at(schema: &Value, instance: &Value, path: &str) -> Result<()> {
    let object = schema
        .as_object()
        .context("validated schema must be an object")?;
    if let Some(expected) = object.get("const") {
        if instance != expected {
            anyhow::bail!("{path} does not match the schema const value");
        }
    }
    if let Some(allowed) = object.get("enum") {
        let allowed = allowed.as_array().context("schema enum must be an array")?;
        if !allowed.contains(instance) {
            anyhow::bail!("{path} is not one of the allowed enum values");
        }
    }
    if let Some(expected_type) = object.get("type").and_then(Value::as_str) {
        let valid_type = match expected_type {
            "object" => instance.is_object(),
            "array" => instance.is_array(),
            "string" => instance.is_string(),
            "number" => instance.is_number(),
            "integer" => instance.as_i64().is_some() || instance.as_u64().is_some(),
            "boolean" => instance.is_boolean(),
            "null" => instance.is_null(),
            _ => false,
        };
        if !valid_type {
            anyhow::bail!("{path} must be of type '{expected_type}'");
        }
    }
    if let Some(value) = instance.as_object() {
        let properties = object.get("properties").and_then(Value::as_object);
        if let Some(required) = object.get("required").and_then(Value::as_array) {
            for name in required.iter().filter_map(Value::as_str) {
                if !value.contains_key(name) {
                    anyhow::bail!("{path}.{name} is required");
                }
            }
        }
        if object.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
            for name in value.keys() {
                if !properties.is_some_and(|properties| properties.contains_key(name)) {
                    anyhow::bail!("{path}.{name} is not allowed by the schema");
                }
            }
        }
        if let Some(properties) = properties {
            for (name, child_schema) in properties {
                if let Some(child) = value.get(name) {
                    validate_instance_at(child_schema, child, &format!("{path}.{name}"))?;
                }
            }
        }
    }
    if let Some(value) = instance.as_array() {
        if let Some(minimum) = object.get("minItems").and_then(Value::as_u64) {
            if value.len() < minimum as usize {
                anyhow::bail!("{path} contains fewer than {minimum} items");
            }
        }
        if let Some(maximum) = object.get("maxItems").and_then(Value::as_u64) {
            if value.len() > maximum as usize {
                anyhow::bail!("{path} contains more than {maximum} items");
            }
        }
        if let Some(item_schema) = object.get("items") {
            for (index, child) in value.iter().enumerate() {
                validate_instance_at(item_schema, child, &format!("{path}[{index}]"))?;
            }
        }
    }
    if let Some(value) = instance.as_str() {
        if let Some(minimum) = object.get("minLength").and_then(Value::as_u64) {
            if value.chars().count() < minimum as usize {
                anyhow::bail!("{path} is shorter than {minimum} characters");
            }
        }
        if let Some(maximum) = object.get("maxLength").and_then(Value::as_u64) {
            if value.chars().count() > maximum as usize {
                anyhow::bail!("{path} is longer than {maximum} characters");
            }
        }
    }
    if let Some(value) = instance.as_f64() {
        if let Some(minimum) = object.get("minimum").and_then(Value::as_f64) {
            if value < minimum {
                anyhow::bail!("{path} is less than {minimum}");
            }
        }
        if let Some(maximum) = object.get("maximum").and_then(Value::as_f64) {
            if value > maximum {
                anyhow::bail!("{path} is greater than {maximum}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn bundled_skills_are_valid_and_candidate_first() {
        let registry = AiSkillRegistry::bundled().unwrap();
        assert_eq!(registry.iter().count(), 4);
        assert!(registry.iter().all(|skill| {
            skill.approval.initial_state == AiCandidateState::Candidate
                && skill.approval.human_review_required
        }));
    }

    #[test]
    fn visual_skill_fixtures_pin_the_bundled_synthetic_image() {
        let digest = format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!(
                "../../data/ai/fixtures/synthetic-layout.svg"
            ))
        );
        let registry = AiSkillRegistry::bundled().unwrap();
        for id in [
            "skill.visual-dna.describe",
            "skill.accessibility.describe-candidate",
        ] {
            assert_eq!(
                registry.get(id, None).unwrap().fixture["image_digest"],
                digest
            );
        }
    }

    #[test]
    fn strict_contract_rejects_unknown_fields() {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["title"],
            "properties": {"title": {"type": "string"}},
            "additionalProperties": false
        });
        let error = validate_json_instance(
            &schema,
            &serde_json::json!({"title": "Safe", "surprise": true}),
        )
        .unwrap_err();
        assert!(error.to_string().contains("surprise"));
    }

    #[test]
    fn unsupported_schema_keywords_are_rejected() {
        let error = validate_json_schema_definition(
            &serde_json::json!({"type": "string", "pattern": "unsafe"}),
            "schema",
        )
        .unwrap_err();
        assert!(error.to_string().contains("unsupported"));
    }
}
