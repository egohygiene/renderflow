//! Provider-neutral publication contracts and deterministic release metadata.

pub mod lulu;
pub mod magazine_guidance;

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
                    "publication.minimum_image_dpi.invalid",
                    "minimum image DPI must be greater than zero",
                ));
            }
        }
        if let Some(hygiene) = hygiene {
            if hygiene.rights.required && !self.rights.reviewed {
                diagnostics.push(diagnostic(
                    "$.publication.rights.reviewed",
                    "publication.rights.review_required",
                    "active hygiene policy requires an explicit rights review",
                ));
            }
            if hygiene.rights.required && self.rights.approval_reference.is_none() {
                diagnostics.push(diagnostic(
                    "$.publication.rights.approval_reference",
                    "publication.rights.approval_reference_required",
                    "active hygiene policy requires an explicit rights approval reference",
                ));
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationBundleEvidence {
    pub schema_version: String,
    pub publication_digest: DigestEvidence,
    pub toolchain_digest: DigestEvidence,
    pub artifacts: Vec<ArtifactEvidence>,
    pub validation: ValidationState,
}

pub fn publication_contract_digest(contract: &PublicationContract) -> Result<DigestEvidence> {
    let bytes = serde_json::to_vec(contract)?;
    let digest = Sha256::digest(bytes);
    Ok(DigestEvidence {
        algorithm: "sha256".to_string(),
        value: format!("{digest:x}"),
    })
}

pub fn write_release_metadata(
    output_dir: &Path,
    contract: &PublicationContract,
    toolchain: &ToolchainSnapshot,
    artifacts: &[ArtifactEvidence],
    validation: ValidationState,
) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create release directory '{}'", output_dir.display()))?;
    let evidence = PublicationBundleEvidence {
        schema_version: PUBLICATION_BUNDLE_V1.to_string(),
        publication_digest: publication_contract_digest(contract)?,
        toolchain_digest: DigestEvidence {
            algorithm: "sha256".to_string(),
            value: toolchain.digest.clone(),
        },
        artifacts: artifacts.to_vec(),
        validation,
    };
    let path = output_dir.join("publication-bundle.json");
    fs::write(&path, serde_json::to_vec_pretty(&evidence)?)
        .with_context(|| format!("failed to write publication metadata '{}'", path.display()))?;
    Ok(path)
}
