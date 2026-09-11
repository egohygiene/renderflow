//! Versioned, local-first font assets and semantic typography role resolution.
//!
//! Registries are inert declarations: loading or resolving one never performs
//! network access. Font acquisition is deliberately kept outside normal render
//! execution so canonical builds use reviewed, pinned local bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::evidence::DigestEvidence;

pub const FONT_REGISTRY_SCHEMA_V1: &str = "renderflow.font-registry/v1";
pub const FONT_RESOLUTION_SCHEMA_V1: &str = "renderflow.font-resolution/v1";
pub const FONT_REGISTRY_VARIABLE: &str = "renderflow-font-registry";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontFormat {
    Ttf,
    Otf,
    Woff,
    Woff2,
}

impl FontFormat {
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Ttf => "font/ttf",
            Self::Otf => "font/otf",
            Self::Woff => "font/woff",
            Self::Woff2 => "font/woff2",
        }
    }

    fn css_label(self) -> &'static str {
        match self {
            Self::Ttf => "truetype",
            Self::Otf => "opentype",
            Self::Woff => "woff",
            Self::Woff2 => "woff2",
        }
    }

    fn has_valid_signature(self, bytes: &[u8]) -> bool {
        match self {
            Self::Ttf => bytes.starts_with(&[0x00, 0x01, 0x00, 0x00]) || bytes.starts_with(b"true"),
            Self::Otf => bytes.starts_with(b"OTTO"),
            Self::Woff => bytes.starts_with(b"wOFF"),
            Self::Woff2 => bytes.starts_with(b"wOF2"),
        }
    }

    fn supports(self, target: FontTarget) -> bool {
        match target {
            FontTarget::Html | FontTarget::Epub => true,
            FontTarget::Latex | FontTarget::Pdf => matches!(self, Self::Ttf | Self::Otf),
            FontTarget::Docx => true,
        }
    }
}

impl fmt::Display for FontFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Ttf => "ttf",
            Self::Otf => "otf",
            Self::Woff => "woff",
            Self::Woff2 => "woff2",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontRole {
    Body,
    Heading,
    Display,
    Monospace,
    Caption,
    Math,
}

impl fmt::Display for FontRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Body => "body",
            Self::Heading => "heading",
            Self::Display => "display",
            Self::Monospace => "monospace",
            Self::Caption => "caption",
            Self::Math => "math",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontTarget {
    Html,
    Latex,
    Pdf,
    Epub,
    Docx,
}

impl fmt::Display for FontTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Html => "html",
            Self::Latex => "latex",
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Docx => "docx",
        })
    }
}

impl std::str::FromStr for FontTarget {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "html" => Ok(Self::Html),
            "latex" | "tex" => Ok(Self::Latex),
            "pdf" => Ok(Self::Pdf),
            "epub" | "kepub" => Ok(Self::Epub),
            "docx" => Ok(Self::Docx),
            _ => anyhow::bail!(
                "unknown font target '{value}'; expected html, latex, pdf, epub, or docx"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontEmbeddingPermission {
    Allowed,
    PrintOnly,
    Prohibited,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontRedistributionStatus {
    Allowed,
    Restricted,
    Prohibited,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontLicense {
    pub spdx_id: String,
    pub license_file: String,
    pub redistribution: FontRedistributionStatus,
    pub embedding: FontEmbeddingPermission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontProvenance {
    pub source_url: String,
    pub source_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_url: Option<String>,
}

fn default_style() -> String {
    "normal".to_string()
}

fn default_weight() -> u16 {
    400
}

fn default_stretch() -> String {
    "normal".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontAsset {
    pub id: String,
    pub family: String,
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default = "default_weight")]
    pub weight: u16,
    #[serde(default = "default_stretch")]
    pub stretch: String,
    pub path: String,
    pub format: FontFormat,
    pub digest: DigestEvidence,
    pub provenance: FontProvenance,
    pub license: FontLicense,
    #[serde(default)]
    pub unicode_ranges: Vec<String>,
    #[serde(default)]
    pub intended_roles: BTreeSet<FontRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontRoleBinding {
    pub primary: String,
    #[serde(default)]
    pub fallbacks: Vec<String>,
    #[serde(default = "default_style")]
    pub style: String,
    #[serde(default = "default_weight")]
    pub weight: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontRegistry {
    pub schema: String,
    pub registry_id: String,
    pub version: String,
    pub assets: Vec<FontAsset>,
    pub roles: BTreeMap<FontRole, FontRoleBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontDiagnostic {
    pub severity: FontDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<FontRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
}

impl FontDiagnostic {
    fn error(code: &str, message: impl Into<String>, asset_id: Option<&str>) -> Self {
        Self {
            severity: FontDiagnosticSeverity::Error,
            code: code.to_string(),
            message: message.into(),
            role: None,
            asset_id: asset_id.map(str::to_string),
        }
    }

    fn rejected(role: FontRole, asset: &FontAsset, reason: impl Into<String>) -> Self {
        Self {
            severity: FontDiagnosticSeverity::Warning,
            code: "font.candidate.rejected".to_string(),
            message: reason.into(),
            role: Some(role),
            asset_id: Some(asset.id.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontValidationReport {
    pub schema: String,
    pub registry_id: String,
    pub registry_digest: DigestEvidence,
    pub valid: bool,
    pub diagnostics: Vec<FontDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFont {
    pub role: FontRole,
    pub asset_id: String,
    pub family: String,
    pub style: String,
    pub weight: u16,
    pub format: FontFormat,
    pub path: PathBuf,
    pub digest: DigestEvidence,
    pub fallback_index: usize,
    pub license: FontLicense,
    pub provenance: FontProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontResolutionReport {
    pub schema: String,
    pub registry_id: String,
    pub registry_version: String,
    pub registry_digest: DigestEvidence,
    pub target: FontTarget,
    pub resolved: BTreeMap<FontRole, ResolvedFont>,
    pub diagnostics: Vec<FontDiagnostic>,
}

impl FontResolutionReport {
    /// Path-independent evidence safe to attach to cache and artifact records.
    pub fn evidence(&self) -> Value {
        let resolved = self
            .resolved
            .iter()
            .map(|(role, font)| {
                (
                    role.to_string(),
                    json!({
                        "asset_id": font.asset_id,
                        "family": font.family,
                        "style": font.style,
                        "weight": font.weight,
                        "format": font.format,
                        "digest": font.digest,
                        "fallback_index": font.fallback_index,
                        "license": font.license,
                        "provenance": font.provenance,
                    }),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let diagnostics = self
            .diagnostics
            .iter()
            .map(|item| {
                json!({
                    "severity": item.severity,
                    "code": item.code,
                    "role": item.role,
                    "asset_id": item.asset_id,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "schema": self.schema,
            "registry_id": self.registry_id,
            "registry_version": self.registry_version,
            "registry_digest": self.registry_digest,
            "target": self.target,
            "resolved": resolved,
            "diagnostics": diagnostics,
        })
    }

    pub fn fingerprint(&self) -> Result<String> {
        let bytes = serde_json::to_vec(&self.evidence())?;
        Ok(sha256_hex(&bytes))
    }

    pub fn css(&self) -> String {
        let mut css =
            String::from("/* Generated by Renderflow from pinned local font assets. */\n");
        let mut emitted_assets = BTreeSet::new();
        for font in self.resolved.values() {
            if !emitted_assets.insert(font.asset_id.as_str()) {
                continue;
            }
            let source = file_url(&font.path);
            css.push_str(&format!(
                "@font-face {{ font-family: \"{}\"; src: url(\"{}\") format(\"{}\"); font-style: {}; font-weight: {}; font-display: swap; }}\n",
                css_escape(&font.family),
                css_escape(&source),
                font.format.css_label(),
                css_identifier(&font.style),
                font.weight
            ));
        }
        if let Some(font) = self.resolved.get(&FontRole::Body) {
            css.push_str(&format!(
                ":root {{ --renderflow-font-body: \"{}\"; }}\nbody {{ font-family: var(--renderflow-font-body); font-style: {}; font-weight: {}; }}\n",
                css_escape(&font.family),
                css_identifier(&font.style),
                font.weight
            ));
        }
        for (role, selector, variable) in [
            (FontRole::Heading, "h1, h2, h3, h4, h5, h6", "heading"),
            (FontRole::Display, ".display, header", "display"),
            (FontRole::Monospace, "code, pre, kbd, samp", "monospace"),
            (FontRole::Caption, "figcaption, caption", "caption"),
            (FontRole::Math, ".math", "math"),
        ] {
            if let Some(font) = self.resolved.get(&role) {
                css.push_str(&format!(
                    ":root {{ --renderflow-font-{variable}: \"{}\"; }}\n{selector} {{ font-family: var(--renderflow-font-{variable}); font-style: {}; font-weight: {}; }}\n",
                    css_escape(&font.family),
                    css_identifier(&font.style),
                    font.weight
                ));
            }
        }
        css
    }

    pub fn latex_variables(&self) -> BTreeMap<String, String> {
        let mut variables = BTreeMap::new();
        for (role, family_key, file_key) in [
            (FontRole::Body, "mainfont", "renderflow-main-font-file"),
            (FontRole::Heading, "sansfont", "renderflow-sans-font-file"),
            (FontRole::Monospace, "monofont", "renderflow-mono-font-file"),
        ] {
            if let Some(font) = self.resolved.get(&role) {
                variables.insert(family_key.to_string(), font.family.clone());
                variables.insert(
                    file_key.to_string(),
                    font.path.to_string_lossy().into_owned(),
                );
            }
        }
        variables
    }

    pub fn embeddable_paths(&self) -> Vec<PathBuf> {
        self.resolved
            .values()
            .map(|font| font.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct LoadedFontRegistry {
    pub registry: FontRegistry,
    pub source_path: PathBuf,
    pub registry_digest: DigestEvidence,
    root: PathBuf,
}

impl LoadedFontRegistry {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read font registry '{}'", path.display()))?;
        let registry = match path.extension().and_then(|value| value.to_str()) {
            Some("json") => serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid JSON font registry '{}'", path.display()))?,
            _ => serde_yaml_ng::from_slice(&bytes)
                .with_context(|| format!("invalid YAML font registry '{}'", path.display()))?,
        };
        let source_path = path.canonicalize().with_context(|| {
            format!("failed to canonicalize font registry '{}'", path.display())
        })?;
        let root = source_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Ok(Self {
            registry,
            source_path,
            registry_digest: digest(&bytes),
            root,
        })
    }

    pub fn validate(&self) -> FontValidationReport {
        let mut diagnostics = Vec::new();
        if self.registry.schema != FONT_REGISTRY_SCHEMA_V1 {
            diagnostics.push(FontDiagnostic::error(
                "font.schema.unsupported",
                format!(
                    "expected schema '{FONT_REGISTRY_SCHEMA_V1}', got '{}'",
                    self.registry.schema
                ),
                None,
            ));
        }
        if !is_stable_id(&self.registry.registry_id) {
            diagnostics.push(FontDiagnostic::error(
                "font.registry_id.invalid",
                "registry_id must use ASCII letters, digits, '.', '_', or '-'",
                None,
            ));
        }
        if self.registry.assets.is_empty() {
            diagnostics.push(FontDiagnostic::error(
                "font.assets.empty",
                "at least one local font asset is required",
                None,
            ));
        }
        if self.registry.roles.is_empty() {
            diagnostics.push(FontDiagnostic::error(
                "font.roles.empty",
                "at least one semantic typography role is required",
                None,
            ));
        }

        let mut ids = BTreeSet::new();
        for asset in &self.registry.assets {
            if !is_stable_id(&asset.id) {
                diagnostics.push(FontDiagnostic::error(
                    "font.asset_id.invalid",
                    "font asset id must be stable",
                    Some(&asset.id),
                ));
            }
            if !ids.insert(asset.id.as_str()) {
                diagnostics.push(FontDiagnostic::error(
                    "font.asset_id.duplicate",
                    "font asset id is declared more than once",
                    Some(&asset.id),
                ));
            }
            if asset.family.trim().is_empty() {
                diagnostics.push(FontDiagnostic::error(
                    "font.family.empty",
                    "font family must not be empty",
                    Some(&asset.id),
                ));
            }
            if !(1..=1000).contains(&asset.weight) {
                diagnostics.push(FontDiagnostic::error(
                    "font.weight.invalid",
                    "font weight must be between 1 and 1000",
                    Some(&asset.id),
                ));
            }
            if asset.digest.algorithm != "sha256" || asset.digest.value.len() != 64 {
                diagnostics.push(FontDiagnostic::error(
                    "font.digest.invalid",
                    "font digest must be a 64-character sha256 value",
                    Some(&asset.id),
                ));
            }
            for unicode_range in &asset.unicode_ranges {
                if !is_unicode_range(unicode_range) {
                    diagnostics.push(FontDiagnostic::error(
                        "font.unicode_range.invalid",
                        format!("invalid Unicode range declaration '{unicode_range}'"),
                        Some(&asset.id),
                    ));
                }
            }
            let path = self.asset_path(asset);
            match fs::read(&path) {
                Ok(bytes) => {
                    let actual = sha256_hex(&bytes);
                    if actual != asset.digest.value.to_ascii_lowercase() {
                        diagnostics.push(FontDiagnostic::error(
                            "font.digest.mismatch",
                            format!(
                                "font bytes at '{}' do not match the pinned digest",
                                path.display()
                            ),
                            Some(&asset.id),
                        ));
                    }
                    if !asset.format.has_valid_signature(&bytes) {
                        diagnostics.push(FontDiagnostic::error(
                            "font.format.signature_mismatch",
                            format!("font bytes do not have a valid {} signature", asset.format),
                            Some(&asset.id),
                        ));
                    }
                }
                Err(_) => diagnostics.push(FontDiagnostic::error(
                    "font.asset.missing",
                    format!("local font asset '{}' does not exist", path.display()),
                    Some(&asset.id),
                )),
            }
            let license_path = self.root.join(&asset.license.license_file);
            if !license_path.is_file() {
                diagnostics.push(FontDiagnostic::error(
                    "font.license.missing",
                    format!(
                        "license artifact '{}' does not exist",
                        license_path.display()
                    ),
                    Some(&asset.id),
                ));
            }
        }

        for (role, binding) in &self.registry.roles {
            for id in std::iter::once(&binding.primary).chain(binding.fallbacks.iter()) {
                if !ids.contains(id.as_str()) {
                    diagnostics.push(FontDiagnostic {
                        severity: FontDiagnosticSeverity::Error,
                        code: "font.role.asset_unknown".to_string(),
                        message: format!("role '{role}' references unknown font asset '{id}'"),
                        role: Some(*role),
                        asset_id: Some(id.clone()),
                    });
                }
            }
        }

        FontValidationReport {
            schema: FONT_REGISTRY_SCHEMA_V1.to_string(),
            registry_id: self.registry.registry_id.clone(),
            registry_digest: self.registry_digest.clone(),
            valid: !diagnostics
                .iter()
                .any(|item| item.severity == FontDiagnosticSeverity::Error),
            diagnostics,
        }
    }

    pub fn resolve(&self, target: FontTarget) -> Result<FontResolutionReport> {
        let validation = self.validate();
        let structural_errors = validation.diagnostics.iter().filter(|item| {
            item.severity == FontDiagnosticSeverity::Error
                && !matches!(
                    item.code.as_str(),
                    "font.asset.missing"
                        | "font.digest.mismatch"
                        | "font.format.signature_mismatch"
                )
        });
        let structural_errors = structural_errors
            .map(|item| item.message.clone())
            .collect::<Vec<_>>();
        if !structural_errors.is_empty() {
            anyhow::bail!("font registry is invalid: {}", structural_errors.join("; "));
        }

        let assets = self
            .registry
            .assets
            .iter()
            .map(|asset| (asset.id.as_str(), asset))
            .collect::<BTreeMap<_, _>>();
        let mut resolved = BTreeMap::new();
        let mut diagnostics = Vec::new();

        for (role, binding) in &self.registry.roles {
            let candidates = std::iter::once(&binding.primary).chain(binding.fallbacks.iter());
            let mut selected = None;
            for (index, id) in candidates.enumerate() {
                let asset = assets
                    .get(id.as_str())
                    .expect("validated role references an existing asset");
                let path = self.asset_path(asset);
                if !path.is_file() {
                    diagnostics.push(FontDiagnostic::rejected(
                        *role,
                        asset,
                        format!("local asset '{}' is missing", path.display()),
                    ));
                    continue;
                }
                let bytes = fs::read(&path)?;
                if sha256_hex(&bytes) != asset.digest.value.to_ascii_lowercase() {
                    diagnostics.push(FontDiagnostic::rejected(
                        *role,
                        asset,
                        "local bytes do not match the pinned sha256 digest",
                    ));
                    continue;
                }
                if !asset.format.has_valid_signature(&bytes) {
                    diagnostics.push(FontDiagnostic::rejected(
                        *role,
                        asset,
                        format!("local bytes are not a valid {} container", asset.format),
                    ));
                    continue;
                }
                if !asset.format.supports(target) {
                    diagnostics.push(FontDiagnostic::rejected(
                        *role,
                        asset,
                        format!(
                            "{} fonts are unsupported by the {target} adapter",
                            asset.format
                        ),
                    ));
                    continue;
                }
                if asset.style != binding.style || asset.weight != binding.weight {
                    diagnostics.push(FontDiagnostic::rejected(
                        *role,
                        asset,
                        format!(
                            "style/weight {} {} does not satisfy requested {} {}",
                            asset.style, asset.weight, binding.style, binding.weight
                        ),
                    ));
                    continue;
                }
                if !embedding_permitted(asset, target) {
                    diagnostics.push(FontDiagnostic::rejected(
                        *role,
                        asset,
                        format!("license metadata does not permit embedding for {target}"),
                    ));
                    continue;
                }
                selected = Some(ResolvedFont {
                    role: *role,
                    asset_id: asset.id.clone(),
                    family: asset.family.clone(),
                    style: asset.style.clone(),
                    weight: asset.weight,
                    format: asset.format,
                    path: path.canonicalize().unwrap_or(path),
                    digest: asset.digest.clone(),
                    fallback_index: index,
                    license: asset.license.clone(),
                    provenance: asset.provenance.clone(),
                });
                if index > 0 {
                    diagnostics.push(FontDiagnostic {
                        severity: FontDiagnosticSeverity::Warning,
                        code: "font.fallback.selected".to_string(),
                        message: format!(
                            "role '{role}' selected deterministic fallback '{}' at index {index}",
                            asset.id
                        ),
                        role: Some(*role),
                        asset_id: Some(asset.id.clone()),
                    });
                }
                break;
            }
            let Some(font) = selected else {
                anyhow::bail!(
                    "no usable local font remains for role '{role}' and target '{target}'"
                );
            };
            resolved.insert(*role, font);
        }

        Ok(FontResolutionReport {
            schema: FONT_RESOLUTION_SCHEMA_V1.to_string(),
            registry_id: self.registry.registry_id.clone(),
            registry_version: self.registry.version.clone(),
            registry_digest: self.registry_digest.clone(),
            target,
            resolved,
            diagnostics,
        })
    }

    fn asset_path(&self, asset: &FontAsset) -> PathBuf {
        let path = Path::new(&asset.path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }
}

pub fn resolve_from_variables(
    variables: &std::collections::HashMap<String, String>,
    target: FontTarget,
) -> Result<Option<FontResolutionReport>> {
    let Some(path) = variables.get(FONT_REGISTRY_VARIABLE) else {
        return Ok(None);
    };
    Ok(Some(LoadedFontRegistry::load(path)?.resolve(target)?))
}

fn embedding_permitted(asset: &FontAsset, target: FontTarget) -> bool {
    let embedding = match asset.license.embedding {
        FontEmbeddingPermission::Allowed => true,
        FontEmbeddingPermission::PrintOnly => matches!(target, FontTarget::Latex | FontTarget::Pdf),
        FontEmbeddingPermission::Prohibited | FontEmbeddingPermission::Unknown => false,
    };
    let redistribution = match target {
        FontTarget::Html | FontTarget::Epub => {
            asset.license.redistribution == FontRedistributionStatus::Allowed
        }
        FontTarget::Latex | FontTarget::Pdf | FontTarget::Docx => {
            asset.license.redistribution != FontRedistributionStatus::Prohibited
        }
    };
    embedding && redistribution
}

fn digest(bytes: &[u8]) -> DigestEvidence {
    DigestEvidence {
        algorithm: "sha256".to_string(),
        value: sha256_hex(bytes),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_unicode_range(value: &str) -> bool {
    let Some(value) = value.strip_prefix("U+") else {
        return false;
    };
    let mut bounds = value.split('-');
    let valid_bound = |bound: &str| {
        (1..=6).contains(&bound.len())
            && bound
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'?')
    };
    let Some(start) = bounds.next() else {
        return false;
    };
    valid_bound(start) && bounds.next().is_none_or(valid_bound) && bounds.next().is_none()
}

fn file_url(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy().replace('\\', "/"))
}

fn css_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn css_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_fixture(dir: &TempDir, name: &str, bytes: &[u8]) -> (String, String) {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        (name.to_string(), sha256_hex(bytes))
    }

    fn fixture_registry(dir: &TempDir) -> LoadedFontRegistry {
        let (missing_path, missing_digest) = (
            "Missing-Regular.otf".to_string(),
            sha256_hex(b"OTTOmissing"),
        );
        let (body_path, body_digest) = write_fixture(dir, "Fixture-Regular.otf", b"OTTOfixture");
        let (mono_path, mono_digest) = write_fixture(dir, "Fixture-Mono.ttf", &[0, 1, 0, 0, 1]);
        fs::write(
            dir.path().join("OFL.txt"),
            "SIL Open Font License 1.1 fixture",
        )
        .unwrap();
        let asset =
            |id: &str, family: &str, path: String, format, digest: String, role| FontAsset {
                id: id.to_string(),
                family: family.to_string(),
                style: "normal".to_string(),
                weight: 400,
                stretch: "normal".to_string(),
                path,
                format,
                digest: DigestEvidence {
                    algorithm: "sha256".to_string(),
                    value: digest,
                },
                provenance: FontProvenance {
                    source_url: "https://example.invalid/font".to_string(),
                    source_version: "fixture-v1".to_string(),
                    project_url: None,
                },
                license: FontLicense {
                    spdx_id: "OFL-1.1".to_string(),
                    license_file: "OFL.txt".to_string(),
                    redistribution: FontRedistributionStatus::Allowed,
                    embedding: FontEmbeddingPermission::Allowed,
                },
                unicode_ranges: vec!["U+0000-00FF".to_string()],
                intended_roles: BTreeSet::from([role]),
            };
        let registry = FontRegistry {
            schema: FONT_REGISTRY_SCHEMA_V1.to_string(),
            registry_id: "fixture.fonts".to_string(),
            version: "1.0.0".to_string(),
            assets: vec![
                asset(
                    "font.missing",
                    "Missing",
                    missing_path,
                    FontFormat::Otf,
                    missing_digest,
                    FontRole::Body,
                ),
                asset(
                    "font.body",
                    "Fixture Serif",
                    body_path,
                    FontFormat::Otf,
                    body_digest,
                    FontRole::Body,
                ),
                asset(
                    "font.mono",
                    "Fixture Mono",
                    mono_path,
                    FontFormat::Ttf,
                    mono_digest,
                    FontRole::Monospace,
                ),
            ],
            roles: BTreeMap::from([
                (
                    FontRole::Body,
                    FontRoleBinding {
                        primary: "font.missing".to_string(),
                        fallbacks: vec!["font.body".to_string()],
                        style: "normal".to_string(),
                        weight: 400,
                    },
                ),
                (
                    FontRole::Monospace,
                    FontRoleBinding {
                        primary: "font.mono".to_string(),
                        fallbacks: Vec::new(),
                        style: "normal".to_string(),
                        weight: 400,
                    },
                ),
            ]),
        };
        let registry_path = dir.path().join("registry.yaml");
        fs::write(&registry_path, serde_yaml_ng::to_string(&registry).unwrap()).unwrap();
        LoadedFontRegistry::load(registry_path).unwrap()
    }

    #[test]
    fn resolves_semantic_roles_with_observable_fallback() {
        let dir = TempDir::new().unwrap();
        let report = fixture_registry(&dir).resolve(FontTarget::Pdf).unwrap();
        assert_eq!(report.resolved[&FontRole::Body].asset_id, "font.body");
        assert_eq!(report.resolved[&FontRole::Body].fallback_index, 1);
        assert!(report
            .diagnostics
            .iter()
            .any(|item| item.code == "font.fallback.selected"));
    }

    #[test]
    fn emits_html_font_face_and_semantic_roles() {
        let dir = TempDir::new().unwrap();
        let report = fixture_registry(&dir).resolve(FontTarget::Html).unwrap();
        let css = report.css();
        assert!(css.contains("@font-face"));
        assert!(css.contains("--renderflow-font-body"));
        assert!(css.contains("file://"));
        assert!(!css.contains("http://"));
        assert!(!css.contains("https://"));
    }

    #[test]
    fn emits_latex_variables_with_local_files() {
        let dir = TempDir::new().unwrap();
        let report = fixture_registry(&dir).resolve(FontTarget::Pdf).unwrap();
        let variables = report.latex_variables();
        assert_eq!(variables["mainfont"], "Fixture Serif");
        assert!(variables["renderflow-main-font-file"].ends_with("Fixture-Regular.otf"));
        assert!(variables["renderflow-mono-font-file"].ends_with("Fixture-Mono.ttf"));
    }

    #[test]
    fn rejects_license_blocked_web_embedding() {
        let dir = TempDir::new().unwrap();
        let mut loaded = fixture_registry(&dir);
        for asset in &mut loaded.registry.assets {
            asset.license.redistribution = FontRedistributionStatus::Prohibited;
        }
        let error = loaded.resolve(FontTarget::Html).unwrap_err().to_string();
        assert!(error.contains("no usable local font"));
    }

    #[test]
    fn registry_digest_and_asset_digest_are_deterministic() {
        let first_dir = TempDir::new().unwrap();
        let second_dir = TempDir::new().unwrap();
        let first_registry = fixture_registry(&first_dir);
        let second_registry = fixture_registry(&second_dir);
        let first = first_registry
            .resolve(FontTarget::Pdf)
            .unwrap()
            .fingerprint()
            .unwrap();
        let second = second_registry
            .resolve(FontTarget::Pdf)
            .unwrap()
            .fingerprint()
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first_registry.registry_digest.algorithm, "sha256");
        assert_eq!(
            first_registry.registry_digest,
            second_registry.registry_digest
        );
    }

    #[test]
    fn committed_redistribution_safe_fixture_validates_and_resolves_for_html() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/fonts/registry-v1.yaml");
        let loaded = LoadedFontRegistry::load(path).unwrap();
        let validation = loaded.validate();
        assert!(validation.valid, "{:?}", validation.diagnostics);
        let report = loaded.resolve(FontTarget::Html).unwrap();
        assert_eq!(report.registry_id, "fixture.redistribution-safe");
        assert_eq!(report.resolved.len(), 4);
        assert_eq!(report.css().matches("@font-face").count(), 1);
    }

    #[test]
    fn unicode_range_declarations_are_bounded() {
        assert!(is_unicode_range("U+0000-00FF"));
        assert!(is_unicode_range("U+4??"));
        assert!(!is_unicode_range("0000-00FF"));
        assert!(!is_unicode_range("U+0000000"));
        assert!(!is_unicode_range("U+0000-00FF-extra"));
    }

    #[test]
    fn bundled_json_schema_is_well_formed() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../schemas/renderflow-font-registry-v1.schema.json"
        ))
        .unwrap();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(
            schema["properties"]["schema"]["const"],
            FONT_REGISTRY_SCHEMA_V1
        );
    }
}
