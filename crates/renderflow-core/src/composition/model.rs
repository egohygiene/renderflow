//! Candidate, intentionally bounded layered page composition contract.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::font::FontRole;

pub const PAGE_COMPOSITION_SCHEMA_V1: &str = "renderflow.page-composition/v1";
pub const PAGE_CONTENT_SCHEMA_V1: &str = "renderflow.page-content/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageComposition {
    pub schema: String,
    pub composition_id: String,
    pub revision: String,
    pub page: PageGeometry,
    pub source_locale: String,
    pub default_locale: String,
    /// Each declared locale has its own pinned content document; no implicit fallback.
    pub locales: BTreeMap<String, FileReference>,
    pub font_registry: FileReference,
    pub assets: Vec<CompositionAsset>,
    /// Paint order, from back to front.
    pub layers: Vec<Layer>,
    /// Accessible order of text layer IDs, independent of paint order.
    pub reading_order: Vec<String>,
    pub provenance: CompositionProvenance,
    #[serde(default)]
    pub locale_overrides: BTreeMap<String, Vec<LocaleOverride>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileReference {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageGeometry {
    pub width_mm: f64,
    pub height_mm: f64,
    /// All geometry is relative to the full canvas. These are layout intentions,
    /// not a declaration of print profile or PDF conformance.
    pub trim: NormalizedRect,
    pub bleed: NormalizedRect,
    pub safe: NormalizedRect,
    pub gutter_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetFormat { Png, Jpeg, Svg }

impl AssetFormat {
    pub fn media_type(self) -> &'static str {
        match self { Self::Png => "image/png", Self::Jpeg => "image/jpeg", Self::Svg => "image/svg+xml" }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionAsset {
    pub id: String,
    pub file: FileReference,
    pub format: AssetFormat,
    pub alt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub id: String,
    pub bounds: NormalizedRect,
    #[serde(default)]
    pub clip: Option<NormalizedRect>,
    #[serde(default = "one")]
    pub opacity: f64,
    #[serde(default)]
    pub blend: BlendMode,
    pub content: LayerContent,
}

fn one() -> f64 { 1.0 }

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode { #[default] Normal }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerContent {
    Raster { asset_id: String },
    Vector { asset_id: String },
    Shape { shape: ShapeKind, fill: String, #[serde(default)] stroke: Option<String>, #[serde(default)] stroke_width_pt: f64 },
    Text { content_id: String, typography: Typography },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind { Rect, Ellipse }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Typography {
    pub role: FontRole,
    pub font_size_pt: f64,
    pub min_font_size_pt: f64,
    /// Multiples of the current font size.
    pub line_height: f64,
    pub padding_pt: f64,
    pub alignment: TextAlignment,
    pub color: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlignment { Start, Center, End }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextDirection { Ltr, Rtl }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritingMode { HorizontalTb, VerticalRl, VerticalLr }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageContent {
    pub schema: String,
    pub composition_id: String,
    pub locale: String,
    pub revision: String,
    /// Revision of the source-language content this document was reviewed against.
    pub source_revision: String,
    pub reviewed: bool,
    pub direction: TextDirection,
    pub writing_mode: WritingMode,
    pub content: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocaleOverride {
    pub layer_id: String,
    pub reason: String,
    pub author: String,
    #[serde(default)]
    pub bounds: Option<NormalizedRect>,
    #[serde(default)]
    pub font_size_pt: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind { TextlessOriginal, FlattenedOriginal, Reconstructed }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionProvenance {
    pub kind: ProvenanceKind,
    pub originals: Vec<FileReference>,
    /// Human declarations only. Renderflow never sets these flags automatically.
    pub separation_reviewed: bool,
    pub requires_human_review: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity { Info, Warning, Error }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_id: Option<String>,
}

impl CompositionDiagnostic {
    pub fn new(severity: DiagnosticSeverity, code: &str, message: impl Into<String>, locale: Option<&str>, layer_id: Option<&str>) -> Self {
        Self { severity, code: code.into(), message: message.into(), locale: locale.map(str::to_owned), layer_id: layer_id.map(str::to_owned) }
    }
}
