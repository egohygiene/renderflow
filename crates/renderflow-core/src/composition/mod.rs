//! Bounded, localization-ready page composition. The canonical package remains
//! distinct from an exported SVG or a flattened image. This scoped locale/output
//! planner does not change the format-keyed general conversion DAG.

pub mod load;
pub mod model;
pub mod render;
mod execution;

use std::collections::BTreeSet;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use execution::{execute, CompositionBranchOutcome, CompositionBranchRun, CompositionRun, CompositionRunState};
pub use load::LoadedComposition;
pub use model::{CompositionDiagnostic, DiagnosticSeverity};
use crate::font::FontTarget;

pub const COMPOSITION_PLAN_SCHEMA_V1: &str = "renderflow.composition-plan/v1";
pub const COMPOSITION_RUN_SCHEMA_V1: &str = "renderflow.composition-run/v1";
pub const COMPOSITION_PROFILE: &str = "renderflow.layered-svg/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocaleSelector {
    Exact { locale: String },
    Subset { locales: Vec<String> },
    AllDeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionOutput { Svg, Html, TranscriptJson, TranscriptText, Png, Pdf, Ora, Psd, Idml }

impl CompositionOutput {
    pub fn name(self) -> &'static str {
        match self {
            Self::Svg => "svg", Self::Html => "html", Self::TranscriptJson => "transcript_json",
            Self::TranscriptText => "transcript_text", Self::Png => "png", Self::Pdf => "pdf",
            Self::Ora => "ora", Self::Psd => "psd", Self::Idml => "idml",
        }
    }

    pub fn supported(self) -> bool {
        matches!(self, Self::Svg | Self::Html | Self::TranscriptJson | Self::TranscriptText)
    }

    pub fn visual(self) -> bool { matches!(self, Self::Svg | Self::Html) }

    pub fn extension(self) -> &'static str {
        match self {
            Self::TranscriptJson => "transcript.json", Self::TranscriptText => "transcript.txt",
            _ => self.name(),
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::Svg => "image/svg+xml", Self::Html => "text/html",
            Self::TranscriptJson => "application/json", Self::TranscriptText => "text/plain",
            Self::Png => "image/png", Self::Pdf => "application/pdf", Self::Ora => "image/openraster",
            Self::Psd => "image/vnd.adobe.photoshop", Self::Idml => "application/vnd.adobe.indesign-idml-package",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionRequest {
    pub locales: LocaleSelector,
    pub targets: Vec<CompositionOutput>,
}

impl Default for CompositionRequest {
    fn default() -> Self {
        Self { locales: LocaleSelector::AllDeclared,
            targets: vec![CompositionOutput::Svg, CompositionOutput::Html, CompositionOutput::TranscriptJson] }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionBranchState { Planned, Blocked, Unavailable }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionBranchPlan {
    pub target_id: String,
    pub locale: String,
    pub output: CompositionOutput,
    pub state: CompositionBranchState,
    pub content_revision: Option<String>,
    pub content_sha256: Option<String>,
    pub cache_key: String,
    pub diagnostics: Vec<CompositionDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionPlan {
    pub schema: String,
    pub profile: String,
    pub renderflow_version: String,
    pub composition_id: String,
    pub composition_sha256: String,
    pub source_locale: String,
    pub default_locale: String,
    pub request: CompositionRequest,
    /// Digested snapshot inputs; sharing does not imply an observed cache hit.
    pub inputs: Vec<load::CompositionInputEvidence>,
    pub font_resolution: Option<serde_json::Value>,
    pub branches: Vec<CompositionBranchPlan>,
    pub diagnostics: Vec<CompositionDiagnostic>,
    pub runtime_qualified: bool,
    pub review_required: bool,
    pub deterministic_serialization: bool,
    pub visual_determinism_qualified: bool,
}

/// Plan from immutable captured bytes. No output directory, cache, subprocess,
/// network access or creative approval is produced by planning.
pub fn plan(snapshot: &LoadedComposition, request: &CompositionRequest) -> Result<CompositionPlan> {
    if request.targets.is_empty() { bail!("composition.targets.empty: at least one output is required"); }
    let locales = match &request.locales {
        LocaleSelector::Exact { locale } => BTreeSet::from([locale.clone()]),
        LocaleSelector::Subset { locales } => locales.iter().cloned().collect(),
        LocaleSelector::AllDeclared => snapshot.manifest.locales.keys().cloned().collect(),
    };
    if locales.is_empty() || locales.len() > 64 {
        bail!("composition.locales.bounds: request must select 1 to 64 locales");
    }
    let outputs = request.targets.iter().copied().collect::<BTreeSet<_>>();
    let font_resolution = snapshot.fonts.resolve(FontTarget::Html);
    let font_evidence = font_resolution.as_ref().ok().map(|report| report.evidence());
    let mut branches = Vec::new();
    for locale in locales {
        for output in &outputs {
            let mut diagnostics = snapshot.diagnostics.iter().filter(|item|
                item.locale.as_deref().is_none_or(|value| value == locale)).cloned().collect::<Vec<_>>();
            let content = snapshot.contents.get(&locale);
            if !snapshot.manifest.locales.contains_key(&locale) {
                diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
                    "composition.locale.undeclared", "Requested locale is not declared; no locale fallback is inferred",
                    Some(&locale), None));
            }
            if content.is_none() {
                diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
                    "composition.content.unavailable", "Requested content snapshot is unavailable",
                    Some(&locale), None));
            }
            if output.visual() && content.is_some() {
                diagnostics.extend(render::diagnostics(snapshot, &locale));
                match &font_resolution {
                    Err(error) => diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
                        "composition.font.unavailable", error.to_string(), Some(&locale), None)),
                    Ok(report) => for item in &report.diagnostics {
                        diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Warning,
                            &item.code, &item.message, Some(&locale), None));
                    },
                }
            }
            if !output.supported() {
                diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
                    "composition.output.unavailable", format!("No adopted {} composition adapter is registered", output.name()),
                    Some(&locale), None));
            }
            let state = if !output.supported() { CompositionBranchState::Unavailable }
                else if diagnostics.iter().any(|item| item.severity == DiagnosticSeverity::Error) { CompositionBranchState::Blocked }
                else { CompositionBranchState::Planned };
            let target_id = format!("target-{}", hash_json(&serde_json::json!({
                "schema": "renderflow.composition-target/v1", "composition_id": snapshot.manifest.composition_id,
                "locale": locale, "output": output,
            }))?);
            let cache_key = hash_json(&serde_json::json!({
                "schema": "renderflow.composition-cache-key/v1", "profile": COMPOSITION_PROFILE,
                "version": env!("CARGO_PKG_VERSION"), "target_id": target_id,
                "composition_sha256": snapshot.manifest_sha256, "inputs": snapshot.inputs,
                "fonts": font_evidence,
            }))?;
            branches.push(CompositionBranchPlan {
                target_id, locale: locale.clone(), output: *output, state,
                content_revision: content.map(|value| value.document.revision.clone()),
                content_sha256: content.map(|value| value.sha256.clone()), cache_key, diagnostics,
            });
        }
    }
    Ok(CompositionPlan {
        schema: COMPOSITION_PLAN_SCHEMA_V1.into(), profile: COMPOSITION_PROFILE.into(),
        renderflow_version: env!("CARGO_PKG_VERSION").into(),
        composition_id: snapshot.manifest.composition_id.clone(), composition_sha256: snapshot.manifest_sha256.clone(),
        source_locale: snapshot.manifest.source_locale.clone(), default_locale: snapshot.manifest.default_locale.clone(),
        request: request.clone(), inputs: snapshot.inputs.clone(), font_resolution: font_evidence,
        branches, diagnostics: snapshot.diagnostics.clone(), runtime_qualified: false, review_required: true,
        deterministic_serialization: true, visual_determinism_qualified: false,
    })
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
pub(crate) fn hash_json(value: &impl Serialize) -> Result<String> { Ok(hash_bytes(&serde_json::to_vec(value)?)) }

#[cfg(test)]
mod tests;
