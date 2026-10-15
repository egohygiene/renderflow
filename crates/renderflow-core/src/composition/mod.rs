//! Bounded, localization-ready page composition. The canonical package remains
//! distinct from an exported SVG or a flattened image. This scoped locale/output
//! planner does not change the format-keyed general conversion DAG.

pub mod load;
pub mod model;
pub mod pdf;
mod pdf_plan;
pub mod raster;
mod raster_svg;
mod raster_tree;
pub mod render;
pub mod typography;
mod typography_provider;
mod execution;

use std::collections::BTreeSet;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use execution::{execute, CompositionBranchOutcome, CompositionBranchRun, CompositionRun, CompositionRunState};
pub use load::LoadedComposition;
pub use model::{CompositionDiagnostic, DiagnosticSeverity};
pub use raster::{RasterPlan, RasterSettings};
pub use pdf::{BindingSide, PdfPlan, PdfSettings};
use crate::font::FontTarget;

/// Historical receipt identifier; new plans use [`COMPOSITION_PLAN_SCHEMA_V4`].
pub const COMPOSITION_PLAN_SCHEMA_V1: &str = "renderflow.composition-plan/v1";
/// Historical receipt identifier; new runs use [`COMPOSITION_RUN_SCHEMA_V4`].
pub const COMPOSITION_RUN_SCHEMA_V1: &str = "renderflow.composition-run/v1";
pub const COMPOSITION_PLAN_SCHEMA_V2: &str = "renderflow.composition-plan/v2";
pub const COMPOSITION_RUN_SCHEMA_V2: &str = "renderflow.composition-run/v2";
pub const COMPOSITION_PLAN_SCHEMA_V3: &str = "renderflow.composition-plan/v3";
pub const COMPOSITION_RUN_SCHEMA_V3: &str = "renderflow.composition-run/v3";
pub const COMPOSITION_PLAN_SCHEMA_V4: &str = "renderflow.composition-plan/v4";
pub const COMPOSITION_RUN_SCHEMA_V4: &str = "renderflow.composition-run/v4";
pub const COMPOSITION_PROFILE: &str = "renderflow.layered-composition/v4";
const MAX_PLAN_TEXT_CODE_POINTS: usize = 16_384;
const MAX_PLAN_TYPOGRAPHY_BYTES: usize = 16 * 1024 * 1024;
const MAX_PLAN_OUTLINE_BYTES: usize = 16 * 1024 * 1024;

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
        matches!(self, Self::Svg | Self::Html | Self::TranscriptJson | Self::TranscriptText | Self::Png | Self::Pdf)
    }

    pub fn visual(self) -> bool { matches!(self, Self::Svg | Self::Html | Self::Png | Self::Pdf) }

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
    /// Applied only to PNG branches. Omitted serialized requests retain the
    /// fixed default settings rather than consulting the host environment.
    #[serde(default)]
    pub raster: RasterSettings,
    /// Applied only to PDF branches. Omitted requests retain the fixed defaults.
    #[serde(default)]
    pub pdf: PdfSettings,
}

impl Default for CompositionRequest {
    fn default() -> Self {
        Self { locales: LocaleSelector::AllDeclared,
            targets: vec![CompositionOutput::Svg, CompositionOutput::Html, CompositionOutput::TranscriptJson],
            raster: RasterSettings::default(), pdf: PdfSettings::default() }
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
    /// Measured visual layout from the exact captured font/content bytes. SVG
    /// paths stay private in memory; their digests are retained in this report.
    /// Transcripts and unavailable output kinds do not require visual shaping.
    pub typography: Option<typography::TypographyReport>,
    /// Effective bounded PNG dimensions and provider policy; null for other
    /// targets or when the requested raster settings cannot be planned.
    pub raster: Option<RasterPlan>,
    /// Effective PDF page boxes, flattened image settings, and explicit color
    /// policy; null for other targets or when PDF planning is blocked.
    pub pdf: Option<PdfPlan>,
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
    /// Registry-wide declared defaults, retained for context. A visual branch's
    /// typography report records its actual whole-layer candidate selection.
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
    let needs_typography = outputs.iter().any(|output| output.visual());
    if needs_typography { check_typography_text_budget(snapshot, &locales)?; }
    let raster = if outputs.contains(&CompositionOutput::Png) {
        Some(raster::plan(snapshot, &request.raster))
    } else { None };
    let pdf = if outputs.contains(&CompositionOutput::Pdf) {
        Some(pdf::plan(snapshot, &request.pdf))
    } else { None };
    let font_resolution = snapshot.fonts.resolve(FontTarget::Html);
    let font_evidence = font_resolution.as_ref().ok().map(|report| report.evidence());
    let mut branches = Vec::new();
    let mut typography_bytes = 0usize;
    let mut outline_bytes = 0usize;
    for locale in locales {
        let typography = if needs_typography && snapshot.contents.contains_key(&locale) {
            Some(typography::measure(snapshot, &locale))
        } else { None };
        let typography_sha256 = if let Some(report) = &typography {
            let bytes = serde_json::to_vec(report)?;
            typography_bytes = typography_bytes.saturating_add(bytes.len());
            // The immutable private paths are shared by all visual branch clones.
            // Count each measured locale once, before retaining its branches.
            for layer in &report.layers {
                if let Some(paths) = report.outlines(&layer.layer_id) {
                    for outline in paths { outline_bytes = outline_bytes.saturating_add(outline.path.len()); }
                }
            }
            if typography_bytes > MAX_PLAN_TYPOGRAPHY_BYTES || outline_bytes > MAX_PLAN_OUTLINE_BYTES {
                bail!("composition.typography.plan_budget: selected locales exceed the 16 MiB aggregate typography report or 16 MiB outline path budget; select fewer locales per request");
            }
            Some(hash_bytes(&bytes))
        } else { None };
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
                diagnostics.extend(render::layout_diagnostics(snapshot, &locale));
                if let Some(report) = &typography { diagnostics.extend(report.diagnostics.clone()); }
            }
            if *output == CompositionOutput::Png { diagnostics.extend(render::raster_diagnostics(&locale)); }
            let branch_raster = if *output == CompositionOutput::Png {
                match &raster {
                    Some(Ok(plan)) => Some(plan.clone()),
                    Some(Err(error)) => {
                        diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
                            "composition.raster.plan", format!("{error:#}"), Some(&locale), None));
                        None
                    },
                    None => None,
                }
            } else { None };
            let branch_pdf = if *output == CompositionOutput::Pdf {
                diagnostics.extend(render::pdf_diagnostics(&locale));
                match &pdf {
                    Some(Ok(plan)) => {
                        // The PDF plan has checked explicit binding clearance.
                        // Other visual targets retain the unresolved warning.
                        diagnostics.retain(|item| item.code != "composition.page.gutter_unresolved");
                        Some(plan.clone())
                    },
                    Some(Err(error)) => {
                        diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
                            "composition.pdf.plan", format!("{error:#}"), Some(&locale), None));
                        None
                    },
                    None => None,
                }
            } else { None };
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
                "schema": "renderflow.composition-cache-key/v4", "profile": COMPOSITION_PROFILE,
                "version": env!("CARGO_PKG_VERSION"), "target_id": target_id,
                "composition_sha256": snapshot.manifest_sha256, "inputs": snapshot.inputs,
                "fonts": font_evidence,
                "renderer": { "id": render::COMPOSITION_RENDERER_ID, "version": render::COMPOSITION_RENDERER_VERSION },
                "typography_sha256": if output.visual() { typography_sha256.as_deref() } else { None },
                "raster": branch_raster,
                "pdf": branch_pdf,
            }))?;
            branches.push(CompositionBranchPlan {
                target_id, locale: locale.clone(), output: *output, state,
                content_revision: content.map(|value| value.document.revision.clone()),
                content_sha256: content.map(|value| value.sha256.clone()), cache_key,
                typography: if output.visual() { typography.clone() } else { None },
                raster: branch_raster, pdf: branch_pdf, diagnostics,
            });
        }
    }
    Ok(CompositionPlan {
        schema: COMPOSITION_PLAN_SCHEMA_V4.into(), profile: COMPOSITION_PROFILE.into(),
        renderflow_version: env!("CARGO_PKG_VERSION").into(),
        composition_id: snapshot.manifest.composition_id.clone(), composition_sha256: snapshot.manifest_sha256.clone(),
        source_locale: snapshot.manifest.source_locale.clone(), default_locale: snapshot.manifest.default_locale.clone(),
        request: request.clone(), inputs: snapshot.inputs.clone(), font_resolution: font_evidence,
        branches, diagnostics: snapshot.diagnostics.clone(), runtime_qualified: false, review_required: true,
        deterministic_serialization: true, visual_determinism_qualified: false,
    })
}

/// Bound selected text before invoking any shaping provider. Count authored
/// scalars, including whitespace and line breaks, rather than assuming every
/// character will produce a visible glyph. Repeated output formats do not charge
/// the same locale twice; transcript-only requests do not enter this path.
fn check_typography_text_budget(snapshot: &LoadedComposition, locales: &BTreeSet<String>) -> Result<()> {
    let mut total = 0usize;
    for locale in locales {
        let Some(content) = snapshot.contents.get(locale) else { continue };
        for layer in &snapshot.manifest.layers {
            let model::LayerContent::Text { content_id, .. } = &layer.content else { continue };
            let Some(text) = content.document.content.get(content_id) else { continue };
            total = total.saturating_add(text.chars().count());
            if total > MAX_PLAN_TEXT_CODE_POINTS {
                bail!("composition.typography.plan_budget: selected locales exceed 16384 referenced text code points; select fewer locales per request");
            }
        }
    }
    Ok(())
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
pub(crate) fn hash_json(value: &impl Serialize) -> Result<String> { Ok(hash_bytes(&serde_json::to_vec(value)?)) }

#[cfg(test)]
mod tests;
