//! Provider-neutral publication contracts and deterministic release metadata.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::evidence::{ArtifactEvidence, DigestEvidence, ValidationState};
use crate::spec::HygienePolicy;
use crate::toolchain::ToolchainSnapshot;

pub const PUBLICATION_CONTRACT_V1: &str = "renderflow.publication/v1";
pub const PUBLICATION_BUNDLE_V1: &str = "renderflow.publication-bundle/v1";

fn default_publication_schema() -> String {
    PUBLICATION_CONTRACT_V1.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationStatus {
    #[default]
    Draft,
    Reviewed,
    Approved,
    Released,
}

impl PublicationStatus {
    pub fn is_release_intent(self) -> bool {
        matches!(self, Self::Approved | Self::Released)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationContributor {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub identifier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationAsset {
    pub role: String,
    pub path: String,
    #[serde(default)]
    pub alt_text: Option<String>,
    #[serde(default)]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageGeometry {
    pub width: f64,
    pub height: f64,
    #[serde(default = "default_geometry_unit")]
    pub unit: String,
    #[serde(default)]
    pub margin: Option<f64>,
    #[serde(default)]
    pub bleed: Option<f64>,
    #[serde(default)]
    pub safe_area: Option<f64>,
}

fn default_geometry_unit() -> String {
    "mm".to_string()
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationRights {
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub rights_holder: Option<String>,
    #[serde(default)]
    pub approval_reference: Option<String>,
    #[serde(default)]
    pub reviewed: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessibilityMetadata {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub access_modes: Vec<String>,
    #[serde(default)]
    pub hazards: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationRoleConstraints {
    pub format: String,
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub geometry: Option<PageGeometry>,
    #[serde(default)]
    pub color_policy: Option<String>,
    #[serde(default)]
    pub minimum_image_dpi: Option<u32>,
    #[serde(default)]
    pub require_embedded_fonts: bool,
    #[serde(default)]
    pub validators: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationContract {
    #[serde(default = "default_publication_schema")]
    pub schema: String,
    pub publication: String,
    #[serde(default)]
    pub series: Option<String>,
    pub issue_id: String,
    #[serde(default)]
    pub issue_number: Option<String>,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub contributors: Vec<PublicationContributor>,
    pub publication_date: String,
    #[serde(default)]
    pub status: PublicationStatus,
    pub language: String,
    #[serde(default)]
    pub artwork: Vec<PublicationAsset>,
    pub geometry: PageGeometry,
    #[serde(default)]
    pub color_policy: Option<String>,
    #[serde(default)]
    pub font_policy: Option<String>,
    #[serde(default)]
    pub asset_policy: Option<String>,
    #[serde(default)]
    pub rights: PublicationRights,
    #[serde(default)]
    pub accessibility: AccessibilityMetadata,
    #[serde(default)]
    pub canonical_url: Option<String>,
    #[serde(default)]
    pub identifiers: BTreeMap<String, String>,
    /// Explicit, independently inspectable constraints keyed by target role.
    #[serde(default)]
    pub output_roles: BTreeMap<String, PublicationRoleConstraints>,
    #[serde(default)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationContractDiagnostic {
    pub path: String,
    pub code: String,
    pub message: String,
}

impl PublicationContract {
    pub fn diagnostics(
        &self,
        hygiene: Option<&HygienePolicy>,
    ) -> Vec<PublicationContractDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.schema != PUBLICATION_CONTRACT_V1 {
            diagnostics.push(diagnostic(
                "$.publication.schema",
                "publication.schema.unsupported",
                "supported publication schema is renderflow.publication/v1",
            ));
        }
        for (path, value) in [
            ("$.publication.publication", self.publication.as_str()),
            ("$.publication.issue_id", self.issue_id.as_str()),
            ("$.publication.title", self.title.as_str()),
            (
                "$.publication.publication_date",
                self.publication_date.as_str(),
            ),
            ("$.publication.language", self.language.as_str()),
        ] {
            if value.trim().is_empty() {
                diagnostics.push(diagnostic(
                    path,
                    "publication.required.empty",
                    "required publication value must not be empty",
                ));
            }
        }
        if self.geometry.width <= 0.0 || self.geometry.height <= 0.0 {
            diagnostics.push(diagnostic(
                "$.publication.geometry",
                "publication.geometry.invalid",
                "page width and height must be greater than zero",
            ));
        }
        for (role, constraints) in &self.output_roles {
            if role.trim().is_empty() || constraints.format.trim().is_empty() {
                diagnostics.push(diagnostic(
                    "$.publication.output_roles",
                    "publication.output_role.invalid",
                    "publication output roles and their formats must not be empty",
                ));
            }
            if constraints.minimum_image_dpi == Some(0) {
                diagnostics.push(diagnostic(
                    &format!("$.publication.output_roles.{role}.minimum_image_dpi"),
                    "publication.output_role.dpi_invalid",
                    "minimum image DPI must be greater than zero",
                ));
            }
        }
        for (index, contributor) in self.contributors.iter().enumerate() {
            if contributor.name.trim().is_empty() || contributor.role.trim().is_empty() {
                diagnostics.push(diagnostic(
                    &format!("$.publication.contributors[{index}]"),
                    "publication.contributor.invalid",
                    "contributor names and roles must not be empty",
                ));
            }
        }
        if self.status.is_release_intent() {
            for (path, value) in [
                (
                    "$.publication.rights.license",
                    self.rights.license.as_deref(),
                ),
                (
                    "$.publication.rights.rights_holder",
                    self.rights.rights_holder.as_deref(),
                ),
                (
                    "$.publication.rights.approval_reference",
                    self.rights.approval_reference.as_deref(),
                ),
            ] {
                if value.is_none_or(|value| value.trim().is_empty()) {
                    diagnostics.push(diagnostic(
                        path,
                        "publication.release.rights_missing",
                        "approved and released publications require complete rights metadata",
                    ));
                }
            }
            if !self.rights.reviewed {
                diagnostics.push(diagnostic(
                    "$.publication.rights.reviewed",
                    "publication.release.rights_unreviewed",
                    "approved and released publications require an explicit rights review",
                ));
            }
            let visual_access = self
                .accessibility
                .access_modes
                .iter()
                .any(|mode| mode.eq_ignore_ascii_case("visual"));
            for (index, artwork) in self.artwork.iter().enumerate() {
                if artwork
                    .approval_reference
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                {
                    diagnostics.push(diagnostic(
                        &format!("$.publication.artwork[{index}].approval_reference"),
                        "publication.release.artwork_unapproved",
                        "approved and released artwork requires an approval reference",
                    ));
                }
                if visual_access
                    && artwork
                        .alt_text
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty())
                {
                    diagnostics.push(diagnostic(
                        &format!("$.publication.artwork[{index}].alt_text"),
                        "publication.release.alt_text_missing",
                        "visual artwork requires alt text for this accessibility contract",
                    ));
                }
            }
            match hygiene {
                Some(policy) if policy.rights.required && policy.rights.reviewed => {
                    for (field, publication_value, hygiene_value) in [
                        (
                            "license",
                            self.rights.license.as_deref(),
                            policy.rights.license.as_deref(),
                        ),
                        (
                            "rights_holder",
                            self.rights.rights_holder.as_deref(),
                            policy.rights.rights_holder.as_deref(),
                        ),
                        (
                            "approval_reference",
                            self.rights.approval_reference.as_deref(),
                            policy.rights.approval_reference.as_deref(),
                        ),
                    ] {
                        if publication_value != hygiene_value {
                            diagnostics.push(diagnostic(
                                &format!("$.publication.rights.{field}"),
                                "publication.release.rights_mismatch",
                                "publication and hygiene rights metadata must match for release",
                            ));
                        }
                    }
                }
                _ => diagnostics.push(diagnostic(
                    "$.execution.hygiene_policy",
                    "publication.release.hygiene_required",
                    "approved and released publications require a hygiene policy with reviewed rights gates",
                )),
            }
        }
        diagnostics
    }
}

fn diagnostic(path: &str, code: &str, message: &str) -> PublicationContractDiagnostic {
    PublicationContractDiagnostic {
        path: path.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
struct PublicationBundleManifest<'a> {
    schema: &'static str,
    publication: &'a PublicationContract,
    artifacts: &'a [ArtifactEvidence],
}

#[derive(Debug, Clone, Serialize)]
struct PublicationProvenance<'a> {
    schema: &'static str,
    source_spec_digest: &'a DigestEvidence,
    execution_plan_digest: &'a DigestEvidence,
    toolchain: Option<&'a ToolchainSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
struct PublicationPreflight<'a> {
    schema: &'static str,
    roles: Vec<PublicationRolePreflight<'a>>,
}

#[derive(Debug, Clone, Serialize)]
struct PublicationRolePreflight<'a> {
    role: &'a str,
    constraints: &'a PublicationRoleConstraints,
    artifact_id: Option<&'a str>,
    validation: ValidationState,
    diagnostics: Vec<&'static str>,
}

pub fn write_release_metadata(
    output_root: &Path,
    contract: &PublicationContract,
    artifacts: &[ArtifactEvidence],
    output_paths: &[String],
    source_spec_digest: &DigestEvidence,
    execution_plan_digest: &DigestEvidence,
    toolchain: Option<&ToolchainSnapshot>,
) -> Result<Vec<String>> {
    let metadata_root = output_root.join("metadata");
    fs::create_dir_all(&metadata_root).with_context(|| {
        format!(
            "failed to create publication metadata directory '{}'",
            metadata_root.display()
        )
    })?;
    let publication_path = metadata_root.join("publication.json");
    let manifest_path = metadata_root.join("manifest.json");
    let provenance_path = metadata_root.join("provenance.json");
    let preflight_path = metadata_root.join("preflight.json");
    write_json(&publication_path, contract)?;
    write_json(
        &manifest_path,
        &PublicationBundleManifest {
            schema: PUBLICATION_BUNDLE_V1,
            publication: contract,
            artifacts,
        },
    )?;
    write_json(
        &provenance_path,
        &PublicationProvenance {
            schema: PUBLICATION_BUNDLE_V1,
            source_spec_digest,
            execution_plan_digest,
            toolchain,
        },
    )?;
    let roles = contract
        .output_roles
        .iter()
        .map(|(role, constraints)| {
            let artifact = artifacts.iter().find(|artifact| artifact.role == *role);
            let mut validation = artifact
                .map(|artifact| artifact.validation)
                .unwrap_or(ValidationState::Unavailable);
            let mut diagnostics = Vec::new();
            if artifact.is_none() {
                diagnostics.push("role was not materialized on this host");
            }
            if constraints.geometry.is_some() {
                diagnostics.push("deep PDF geometry inspection requires a configured validator");
            }
            if constraints.minimum_image_dpi.is_some() {
                diagnostics.push("embedded image resolution requires a configured validator");
            }
            if constraints.require_embedded_fonts {
                diagnostics.push("font embedding requires a configured validator");
            }
            if !diagnostics.is_empty() && validation == ValidationState::Valid {
                validation = ValidationState::ValidWithWarnings;
            }
            PublicationRolePreflight {
                role,
                constraints,
                artifact_id: artifact.map(|artifact| artifact.artifact_id.as_str()),
                validation,
                diagnostics,
            }
        })
        .collect();
    write_json(
        &preflight_path,
        &PublicationPreflight {
            schema: PUBLICATION_BUNDLE_V1,
            roles,
        },
    )?;

    let mut checksum_inputs = output_paths.iter().map(PathBuf::from).collect::<Vec<_>>();
    checksum_inputs.extend([
        publication_path.clone(),
        manifest_path.clone(),
        provenance_path.clone(),
        preflight_path.clone(),
    ]);
    checksum_inputs.sort();
    checksum_inputs.dedup();
    let checksum_path = metadata_root.join("checksums.sha256");
    let mut checksum_text = String::new();
    for path in checksum_inputs.iter().filter(|path| path.is_file()) {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read publication artifact '{}'", path.display()))?;
        let digest = format!("{:x}", Sha256::digest(bytes));
        let locator = path.strip_prefix(output_root).unwrap_or(path);
        checksum_text.push_str(&format!("{digest}  {}\n", locator.display()));
    }
    fs::write(&checksum_path, checksum_text).with_context(|| {
        format!(
            "failed to write publication checksums '{}'",
            checksum_path.display()
        )
    })?;

    Ok([
        publication_path,
        manifest_path,
        provenance_path,
        preflight_path,
        checksum_path,
    ]
    .into_iter()
    .map(|path| path.display().to_string())
    .collect())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)
        .with_context(|| format!("failed to write publication metadata '{}'", path.display()))
}
