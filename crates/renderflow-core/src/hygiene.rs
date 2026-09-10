//! Auditable, non-destructive publication hygiene.
//!
//! Hygiene runs after derivative generation and before terminal materialization.
//! It creates a new artifact record even when payload bytes are unchanged, so
//! the policy decision and lineage remain explicit.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore};
use crate::spec::{
    ContentRedactionPolicy, HygienePolicy, MetadataHygienePolicy, PublicationAudience,
    RedactionDeterminism,
};

pub const HYGIENE_EVIDENCE_SCHEMA_V1: &str = "renderflow.hygiene/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HygieneStatus {
    Passed,
    ReviewRequired,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HygieneFindingKind {
    Metadata,
    Secret,
    ProtectedReference,
    Redaction,
    Rights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HygieneFinding {
    pub code: String,
    pub kind: HygieneFindingKind,
    pub class: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HygieneEvidence {
    pub schema_version: String,
    pub policy_id: String,
    pub provider_id: String,
    pub provider_version: String,
    pub source_artifact_id: String,
    pub output_artifact_id: String,
    pub status: HygieneStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redaction: Option<RedactionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_field_classes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<HygieneFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedactionEvidence {
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_version: Option<String>,
    pub determinism: RedactionDeterminism,
    pub policy_reviewed: bool,
}

#[derive(Debug, Clone)]
pub struct HygieneOutcome {
    pub artifact: Artifact,
    pub evidence: HygieneEvidence,
}

impl HygieneOutcome {
    pub fn releasable(&self) -> bool {
        self.evidence.status == HygieneStatus::Passed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionRequest {
    pub artifact_id: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionResult {
    pub bytes: Vec<u8>,
    pub changed_classes: Vec<String>,
    pub findings: Vec<HygieneFinding>,
}

/// Replaceable content-redaction boundary. Implementations must not mutate the
/// source artifact and must return safe findings that omit sensitive values.
pub trait ContentRedactionProvider: Send + Sync {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    fn determinism(&self) -> RedactionDeterminism;
    fn redact(&self, request: &RedactionRequest) -> Result<RedactionResult>;
}

#[derive(Default)]
pub struct HygieneEngine {
    redaction_providers: Vec<Box<dyn ContentRedactionProvider>>,
}

impl HygieneEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_redaction_provider(
        mut self,
        provider: impl ContentRedactionProvider + 'static,
    ) -> Self {
        self.redaction_providers.push(Box::new(provider));
        self
    }

    pub fn apply(
        &self,
        policy_id: &str,
        policy: &HygienePolicy,
        artifact: &Artifact,
        store: &ArtifactStore,
    ) -> Result<HygieneOutcome> {
        let original_bytes = store.read_bytes(artifact)?;
        let (mut bytes, mut changed_classes) = sanitize_embedded_metadata(
            &original_bytes,
            artifact.media_type().as_str(),
            &policy.metadata,
        );
        let (metadata, metadata_changes) =
            sanitize_record_metadata(artifact.metadata(), &policy.metadata);
        changed_classes.extend(metadata_changes);
        let encoded_metadata = serde_json::to_vec(&metadata)
            .context("failed to encode artifact metadata for hygiene scan")?;

        let mut findings = Vec::new();
        if policy.secrets.enabled {
            findings.extend(scan_secrets(
                &bytes,
                artifact.media_type().as_str(),
                &policy.secrets.markers,
                policy.secrets.block,
            ));
            findings.extend(scan_secrets(
                &encoded_metadata,
                "application/json",
                &policy.secrets.markers,
                policy.secrets.block,
            ));
        }
        findings.extend(scan_protected_references(
            &bytes,
            artifact.media_type().as_str(),
            &policy.protected_references.terms,
            policy.protected_references.case_sensitive,
            policy.protected_references.block,
        ));
        for finding in scan_protected_references(
            &encoded_metadata,
            "application/json",
            &policy.protected_references.terms,
            policy.protected_references.case_sensitive,
            policy.protected_references.block,
        ) {
            if !findings.contains(&finding) {
                findings.push(finding);
            }
        }

        let mut redaction_evidence = None;
        if let Some(redaction) = &policy.redaction {
            let provider = self
                .redaction_providers
                .iter()
                .find(|provider| provider.id() == redaction.provider);
            redaction_evidence = Some(RedactionEvidence {
                provider_id: redaction.provider.clone(),
                provider_version: provider.map(|provider| provider.version().to_string()),
                determinism: redaction.determinism,
                policy_reviewed: redaction.reviewed,
            });
            let result = self.apply_redaction(redaction, artifact, bytes)?;
            bytes = result.bytes;
            changed_classes.extend(result.changed_classes);
            findings.extend(result.findings);
            if redaction.determinism == RedactionDeterminism::Probabilistic {
                findings.push(HygieneFinding {
                    code: "hygiene.redaction.review_required".to_string(),
                    kind: HygieneFindingKind::Redaction,
                    class: "probabilistic_redaction".to_string(),
                    message: "Probabilistic redaction requires explicit review of this candidate artifact".to_string(),
                    blocking: false,
                });
            }
        }

        findings.extend(rights_findings(policy));
        changed_classes.sort();
        changed_classes.dedup();

        let status = if findings.iter().any(|finding| finding.blocking) {
            HygieneStatus::Blocked
        } else if findings.iter().any(|finding| {
            finding.code == "hygiene.redaction.review_required"
                || finding.code == "hygiene.rights.review_required"
        }) {
            HygieneStatus::ReviewRequired
        } else {
            HygieneStatus::Passed
        };

        let mut descriptor = ArtifactDescriptor::new(
            artifact.format().clone(),
            artifact.media_type().clone(),
            ArtifactStorageClass::Terminal,
        )
        .with_source(artifact.id().clone());
        for (key, value) in metadata {
            descriptor = descriptor.with_metadata(key, value);
        }
        descriptor = descriptor
            .with_metadata("renderflow.hygiene.policy", policy_id)
            .with_metadata("renderflow.hygiene.status", serde_json::to_value(status)?);
        let output = store.put_bytes(&bytes, descriptor)?;
        let evidence = HygieneEvidence {
            schema_version: HYGIENE_EVIDENCE_SCHEMA_V1.to_string(),
            policy_id: policy_id.to_string(),
            provider_id: "renderflow.core-hygiene".to_string(),
            provider_version: env!("CARGO_PKG_VERSION").to_string(),
            source_artifact_id: artifact.id().to_string(),
            output_artifact_id: output.id().to_string(),
            status,
            redaction: redaction_evidence,
            changed_field_classes: changed_classes,
            findings,
        };
        Ok(HygieneOutcome {
            artifact: output,
            evidence,
        })
    }

    fn apply_redaction(
        &self,
        policy: &ContentRedactionPolicy,
        artifact: &Artifact,
        bytes: Vec<u8>,
    ) -> Result<RedactionResult> {
        let Some(provider) = self
            .redaction_providers
            .iter()
            .find(|provider| provider.id() == policy.provider)
        else {
            return Ok(RedactionResult {
                bytes,
                changed_classes: Vec::new(),
                findings: vec![HygieneFinding {
                    code: "hygiene.redaction.provider_unavailable".to_string(),
                    kind: HygieneFindingKind::Redaction,
                    class: "provider".to_string(),
                    message: format!(
                        "Configured redaction provider '{}' is unavailable; candidate was preserved",
                        policy.provider
                    ),
                    blocking: true,
                }],
            });
        };
        if provider.determinism() != policy.determinism {
            return Ok(RedactionResult {
                bytes,
                changed_classes: Vec::new(),
                findings: vec![HygieneFinding {
                    code: "hygiene.redaction.determinism_mismatch".to_string(),
                    kind: HygieneFindingKind::Redaction,
                    class: "provider".to_string(),
                    message:
                        "Configured redaction determinism does not match the selected provider"
                            .to_string(),
                    blocking: true,
                }],
            });
        }
        provider.redact(&RedactionRequest {
            artifact_id: artifact.id().to_string(),
            media_type: artifact.media_type().to_string(),
            bytes,
            classes: policy.classes.clone(),
        })
    }
}

fn sanitize_record_metadata(
    metadata: &BTreeMap<String, Value>,
    policy: &MetadataHygienePolicy,
) -> (BTreeMap<String, Value>, Vec<String>) {
    let mut retained = BTreeMap::new();
    let mut changed = BTreeSet::new();
    for (key, value) in metadata {
        let allowed = matches_any(key, &policy.allow);
        let denied = matches_any(key, &policy.deny);
        if denied || (policy.allowlist_only && !allowed) {
            changed.insert(metadata_class(key));
        } else {
            retained.insert(key.clone(), value.clone());
        }
    }
    (retained, changed.into_iter().collect())
}

fn matches_any(value: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        pattern
            .strip_suffix(".*")
            .map_or(value == pattern, |prefix| {
                value == prefix || value.starts_with(&format!("{prefix}."))
            })
    })
}

fn metadata_class(key: &str) -> String {
    key.split(['.', ':'])
        .next()
        .unwrap_or("metadata")
        .to_string()
}

fn sanitize_embedded_metadata(
    bytes: &[u8],
    media_type: &str,
    policy: &MetadataHygienePolicy,
) -> (Vec<u8>, Vec<String>) {
    if media_type == "image/jpeg" {
        return sanitize_jpeg(bytes, policy);
    }
    if media_type == "image/png" {
        return sanitize_png(bytes, policy);
    }
    (bytes.to_vec(), Vec::new())
}

fn should_remove_class(class: &str, policy: &MetadataHygienePolicy) -> bool {
    matches_any(class, &policy.deny)
        || (policy.allowlist_only && !matches_any(class, &policy.allow))
}

fn sanitize_jpeg(bytes: &[u8], policy: &MetadataHygienePolicy) -> (Vec<u8>, Vec<String>) {
    if bytes.len() < 2 || bytes[..2] != [0xff, 0xd8] {
        return (bytes.to_vec(), Vec::new());
    }
    let mut output = bytes[..2].to_vec();
    let mut cursor = 2;
    let mut changed = BTreeSet::new();
    while cursor + 1 < bytes.len() {
        if bytes[cursor] != 0xff {
            output.extend_from_slice(&bytes[cursor..]);
            break;
        }
        let marker = bytes[cursor + 1];
        if marker == 0xda || marker == 0xd9 {
            output.extend_from_slice(&bytes[cursor..]);
            break;
        }
        if cursor + 4 > bytes.len() {
            return (bytes.to_vec(), Vec::new());
        }
        let length = u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]]) as usize;
        if length < 2 || cursor + 2 + length > bytes.len() {
            return (bytes.to_vec(), Vec::new());
        }
        let segment_end = cursor + 2 + length;
        let payload = &bytes[cursor + 4..segment_end];
        let class = match marker {
            0xe1 if payload.starts_with(b"Exif\0\0") => Some("exif"),
            0xe1 if payload.starts_with(b"http://ns.adobe.com/xap/1.0/") => Some("xmp"),
            0xed => Some("iptc"),
            0xfe => Some("comment"),
            _ => None,
        };
        if class.is_some_and(|class| should_remove_class(class, policy)) {
            changed.insert(class.unwrap().to_string());
        } else {
            output.extend_from_slice(&bytes[cursor..segment_end]);
        }
        cursor = segment_end;
    }
    (output, changed.into_iter().collect())
}

fn sanitize_png(bytes: &[u8], policy: &MetadataHygienePolicy) -> (Vec<u8>, Vec<String>) {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 8 || &bytes[..8] != SIGNATURE {
        return (bytes.to_vec(), Vec::new());
    }
    let mut output = bytes[..8].to_vec();
    let mut cursor = 8;
    let mut changed = BTreeSet::new();
    while cursor + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let end = cursor.saturating_add(12).saturating_add(length);
        if end > bytes.len() {
            return (bytes.to_vec(), Vec::new());
        }
        let chunk = &bytes[cursor + 4..cursor + 8];
        let class = match chunk {
            b"eXIf" => Some("exif"),
            b"tEXt" | b"zTXt" | b"iTXt" => Some("png.text"),
            _ => None,
        };
        if class.is_some_and(|class| should_remove_class(class, policy)) {
            changed.insert(class.unwrap().to_string());
        } else {
            output.extend_from_slice(&bytes[cursor..end]);
        }
        cursor = end;
        if chunk == b"IEND" {
            break;
        }
    }
    if cursor < bytes.len() {
        output.extend_from_slice(&bytes[cursor..]);
    }
    (output, changed.into_iter().collect())
}

fn scan_secrets(
    bytes: &[u8],
    media_type: &str,
    configured_markers: &[String],
    blocking: bool,
) -> Vec<HygieneFinding> {
    let Some(text) = scannable_text(bytes, media_type) else {
        return Vec::new();
    };
    let lower = text.to_ascii_lowercase();
    let mut classes = BTreeSet::new();
    if lower.contains("-----begin private key-----")
        || lower.contains("-----begin rsa private key-----")
    {
        classes.insert("private_key".to_string());
    }
    if has_prefixed_token(&lower, "github_pat_", 12) || has_prefixed_token(&lower, "ghp_", 20) {
        classes.insert("github_token".to_string());
    }
    if has_prefixed_token(text, "AKIA", 16) {
        classes.insert("aws_access_key".to_string());
    }
    if has_prefixed_token(&lower, "sk-", 20) {
        classes.insert("openai_key".to_string());
    }
    if has_credential_assignment(&lower, "authorization: bearer ", 8) {
        classes.insert("authorization".to_string());
    }
    if ["password=", "api_key=", "api-key="]
        .iter()
        .any(|marker| has_credential_assignment(&lower, marker, 6))
    {
        classes.insert("credential_assignment".to_string());
    }
    for marker in configured_markers {
        if !marker.is_empty() && text.contains(marker) {
            classes.insert("configured_secret".to_string());
        }
    }
    classes
        .into_iter()
        .map(|class| HygieneFinding {
            code: "hygiene.secret.detected".to_string(),
            kind: HygieneFindingKind::Secret,
            class,
            message: "Potential credential detected; the value was omitted from evidence"
                .to_string(),
            blocking,
        })
        .collect()
}

fn has_prefixed_token(text: &str, prefix: &str, minimum_tail: usize) -> bool {
    text.match_indices(prefix).any(|(offset, _)| {
        text[offset + prefix.len()..]
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
            .count()
            >= minimum_tail
    })
}

fn has_credential_assignment(text: &str, marker: &str, minimum_value: usize) -> bool {
    text.match_indices(marker).any(|(offset, _)| {
        text[offset + marker.len()..]
            .trim_start_matches(['\'', '"'])
            .chars()
            .take_while(|character| {
                !character.is_ascii_whitespace() && !matches!(character, '\'' | '"' | ',' | ';')
            })
            .count()
            >= minimum_value
    })
}

fn scan_protected_references(
    bytes: &[u8],
    media_type: &str,
    terms: &[String],
    case_sensitive: bool,
    blocking: bool,
) -> Vec<HygieneFinding> {
    let Some(text) = scannable_text(bytes, media_type) else {
        return Vec::new();
    };
    let haystack = if case_sensitive {
        text.to_string()
    } else {
        text.to_lowercase()
    };
    terms
        .iter()
        .filter(|term| {
            let needle = if case_sensitive {
                (*term).clone()
            } else {
                term.to_lowercase()
            };
            !needle.is_empty() && haystack.contains(&needle)
        })
        .map(|term| HygieneFinding {
            code: "hygiene.protected_reference.detected".to_string(),
            kind: HygieneFindingKind::ProtectedReference,
            class: "configured_term".to_string(),
            message: format!(
                "Configured protected reference '{}' requires removal or explicit review",
                term
            ),
            blocking,
        })
        .collect()
}

fn scannable_text<'a>(bytes: &'a [u8], media_type: &str) -> Option<&'a str> {
    let textual = media_type.starts_with("text/")
        || matches!(
            media_type,
            "application/json"
                | "application/yaml"
                | "application/toml"
                | "application/xml"
                | "application/x-latex"
        );
    textual.then(|| std::str::from_utf8(bytes).ok()).flatten()
}

fn rights_findings(policy: &HygienePolicy) -> Vec<HygieneFinding> {
    if !policy.rights.required
        || matches!(
            policy.audience,
            PublicationAudience::Candidate | PublicationAudience::Private
        )
    {
        return Vec::new();
    }
    let complete = policy.rights.reviewed
        && policy
            .rights
            .license
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && policy
            .rights
            .approval_reference
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    if complete {
        Vec::new()
    } else {
        vec![HygieneFinding {
            code: "hygiene.rights.review_required".to_string(),
            kind: HygieneFindingKind::Rights,
            class: "publication_rights".to_string(),
            message: "Public/commercial release requires reviewed rights, license, and approval metadata; the local candidate was preserved".to_string(),
            blocking: true,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass};
    use crate::graph::Format;
    use crate::spec::{
        MetadataHygienePolicy, ProtectedReferencePolicy, RightsHygienePolicy, SecretHygienePolicy,
    };

    #[test]
    fn metadata_sanitization_is_allowlisted_and_non_destructive() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let source = store
            .put_bytes(
                b"publication candidate",
                ArtifactDescriptor::for_format(Format::Markdown, ArtifactStorageClass::Source)
                    .with_metadata("dc.title", "Allowed title")
                    .with_metadata("dc.creator", "Private author")
                    .with_metadata("geolocation.latitude", "42.0"),
            )
            .unwrap();
        let policy = HygienePolicy {
            metadata: MetadataHygienePolicy {
                allow: vec!["dc.title".to_string()],
                deny: vec!["geolocation.*".to_string()],
                allowlist_only: true,
            },
            ..HygienePolicy::default()
        };

        let outcome = HygieneEngine::new()
            .apply("public", &policy, &source, &store)
            .unwrap();

        assert_eq!(
            source.metadata().get("dc.creator").unwrap(),
            "Private author"
        );
        assert_eq!(store.read_bytes(&source).unwrap(), b"publication candidate");
        assert_eq!(
            outcome.artifact.metadata().get("dc.title").unwrap(),
            "Allowed title"
        );
        assert!(!outcome.artifact.metadata().contains_key("dc.creator"));
        assert!(!outcome
            .artifact
            .metadata()
            .contains_key("geolocation.latitude"));
        assert!(outcome
            .evidence
            .changed_field_classes
            .contains(&"geolocation".to_string()));
    }

    #[test]
    fn jpeg_exif_is_removed_into_a_derived_artifact() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe1, 0x00, 0x08, b'E', b'x', b'i', b'f', 0x00, 0x00, 0xff, 0xd9,
        ];
        let source = store
            .put_bytes(
                &jpeg,
                ArtifactDescriptor::for_format(Format::Jpeg, ArtifactStorageClass::Source),
            )
            .unwrap();
        let policy = HygienePolicy {
            metadata: MetadataHygienePolicy {
                deny: vec!["exif".to_string()],
                ..MetadataHygienePolicy::default()
            },
            ..HygienePolicy::default()
        };

        let outcome = HygieneEngine::new()
            .apply("images", &policy, &source, &store)
            .unwrap();

        assert_eq!(store.read_bytes(&source).unwrap(), jpeg);
        assert_eq!(
            store.read_bytes(&outcome.artifact).unwrap(),
            [0xff, 0xd8, 0xff, 0xd9]
        );
        assert_eq!(outcome.evidence.changed_field_classes, vec!["exif"]);
        assert_eq!(
            outcome.artifact.sources(),
            std::slice::from_ref(source.id())
        );
    }

    #[test]
    fn secrets_references_and_rights_block_without_leaking_secret_values() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let source = store
            .put_bytes(
                b"Example Franchise with FIXTURE-CREDENTIAL-MARKER",
                ArtifactDescriptor::for_format(Format::Markdown, ArtifactStorageClass::Source),
            )
            .unwrap();
        let policy = HygienePolicy {
            audience: PublicationAudience::Commercial,
            secrets: SecretHygienePolicy {
                markers: vec!["FIXTURE-CREDENTIAL-MARKER".to_string()],
                ..SecretHygienePolicy::default()
            },
            protected_references: ProtectedReferencePolicy {
                terms: vec!["Example Franchise".to_string()],
                ..ProtectedReferencePolicy::default()
            },
            rights: RightsHygienePolicy {
                required: true,
                ..RightsHygienePolicy::default()
            },
            ..HygienePolicy::default()
        };

        let outcome = HygieneEngine::new()
            .apply("commercial", &policy, &source, &store)
            .unwrap();

        assert_eq!(outcome.evidence.status, HygieneStatus::Blocked);
        let evidence = serde_json::to_string(&outcome.evidence).unwrap();
        assert!(!evidence.contains("FIXTURE-CREDENTIAL-MARKER"));
        assert!(outcome
            .evidence
            .findings
            .iter()
            .any(|finding| finding.kind == HygieneFindingKind::ProtectedReference));
        assert!(outcome
            .evidence
            .findings
            .iter()
            .any(|finding| finding.kind == HygieneFindingKind::Rights));
    }
}
