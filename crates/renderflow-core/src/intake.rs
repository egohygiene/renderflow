//! Universal, binary-safe artifact intake, inspection, and bounded extraction.
//!
//! Intake deliberately produces normal [`Artifact`] values in the shared
//! content-addressed store. Providers may add structural evidence and extract
//! child artifacts, but they never create a parallel payload or graph model.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::artifact::{
    Artifact, ArtifactCollection, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore,
    CanonicalFormat, MediaType,
};
use crate::graph::capability::{ArtifactCapability, FormatCapabilityRegistry};
use crate::graph::Format;

pub const INTAKE_SCHEMA_V1: &str = "renderflow.intake/v1";
const PROBE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceOrigin {
    SourceReported,
    ProviderObserved,
    Inferred,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceValue {
    pub value: Value,
    pub origin: EvidenceOrigin,
    pub provider_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntakeSignalKind {
    DeclaredFormat,
    Extension,
    Magic,
    MediaType,
    Structural,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeSignal {
    pub kind: IntakeSignalKind,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    pub confidence: u8,
    pub provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeConflict {
    pub formats: Vec<String>,
    pub signal_kinds: Vec<IntakeSignalKind>,
    pub resolution: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionConfidence {
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedArtifactProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    pub media_type: String,
    pub confidence: DetectionConfidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_format: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stream_formats: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub families: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, ProvenanceValue>,
    #[serde(default)]
    pub available_operations: Vec<String>,
    #[serde(default)]
    pub signals: Vec<IntakeSignal>,
    #[serde(default)]
    pub conflicts: Vec<IntakeConflict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntakeDiagnosticSeverity {
    Info,
    Warning,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeDiagnostic {
    pub severity: IntakeDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IntakeBudgets {
    pub max_depth: u32,
    pub max_artifacts: u64,
    pub max_extracted_bytes: u64,
    pub max_expansion_ratio: f64,
}

impl Default for IntakeBudgets {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_artifacts: 1_000,
            max_extracted_bytes: 512 * 1024 * 1024,
            max_expansion_ratio: 100.0,
        }
    }
}

impl IntakeBudgets {
    /// Project execution-policy budgets (#353) onto recursive intake limits.
    /// The expansion-ratio guard remains at its conservative intake default
    /// because the execution policy does not expose that archive-specific axis.
    pub fn from_resource_budgets(budgets: &crate::spec::ResourceBudgets) -> Self {
        let defaults = Self::default();
        let byte_limits = [budgets.max_output_bytes, budgets.max_storage_bytes]
            .into_iter()
            .flatten();
        Self {
            max_depth: budgets.max_depth.unwrap_or(defaults.max_depth),
            max_artifacts: budgets.max_artifacts.unwrap_or(defaults.max_artifacts),
            max_extracted_bytes: byte_limits.min().unwrap_or(defaults.max_extracted_bytes),
            max_expansion_ratio: defaults.max_expansion_ratio,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntakeRequest {
    pub path: PathBuf,
    pub declared_format: Option<String>,
    pub declared_media_type: Option<String>,
    pub extract: bool,
    pub recursive: bool,
    pub allow_encrypted: bool,
    pub budgets: IntakeBudgets,
}

impl IntakeRequest {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            declared_format: None,
            declared_media_type: None,
            extract: false,
            recursive: false,
            allow_encrypted: false,
            budgets: IntakeBudgets::default(),
        }
    }

    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.declared_format = Some(format.into());
        self
    }

    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.declared_media_type = Some(media_type.into());
        self
    }

    pub fn with_extraction(mut self, recursive: bool) -> Self {
        self.extract = true;
        self.recursive = recursive;
        self
    }

    pub fn with_budgets(mut self, budgets: IntakeBudgets) -> Self {
        self.budgets = budgets;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveredArtifact {
    pub artifact: Artifact,
    pub parent_artifact_id: String,
    pub logical_path: String,
    pub depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct IntakeBudgetUsage {
    pub artifacts: u64,
    pub extracted_bytes: u64,
    pub deepest_level: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeReport {
    pub schema_version: String,
    pub source: Artifact,
    pub profile: ResolvedArtifactProfile,
    #[serde(default)]
    pub discovered: Vec<DiscoveredArtifact>,
    #[serde(default)]
    pub diagnostics: Vec<IntakeDiagnostic>,
    pub budget_usage: IntakeBudgetUsage,
}

impl IntakeReport {
    /// Return source and extracted children as normal graph-consumable artifacts.
    pub fn artifact_collection(&self) -> ArtifactCollection {
        let mut artifacts = Vec::with_capacity(self.discovered.len() + 1);
        artifacts.push(self.source.clone());
        artifacts.extend(self.discovered.iter().map(|child| child.artifact.clone()));
        ArtifactCollection::new(artifacts)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderInspection {
    pub format: Option<Format>,
    pub container_format: Option<String>,
    pub stream_formats: Vec<String>,
    pub metadata: BTreeMap<String, ProvenanceValue>,
    pub operations: Vec<String>,
    pub confidence: u8,
}

pub struct InspectionContext<'a> {
    pub path: &'a Path,
    pub bytes: &'a [u8],
    pub declared_media_type: Option<&'a str>,
}

/// Provider extension point for deterministic structure inspection.
pub trait ArtifactIntakeProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn inspect(&self, context: &InspectionContext<'_>) -> Result<Option<ProviderInspection>>;

    /// Extract supported child artifacts into the shared store.
    ///
    /// Return `true` when the provider handled this profile. Providers that
    /// only inspect can retain the default no-op implementation.
    fn extract(
        &self,
        _store: &ArtifactStore,
        _request: &IntakeRequest,
        _report: &mut IntakeReport,
    ) -> Result<bool> {
        Ok(false)
    }
}

pub struct IntakeEngine {
    providers: Vec<Arc<dyn ArtifactIntakeProvider>>,
}

impl Default for IntakeEngine {
    fn default() -> Self {
        Self {
            providers: vec![Arc::new(NativeStructureProvider), Arc::new(ZipProvider)],
        }
    }
}

impl IntakeEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider(mut self, provider: Arc<dyn ArtifactIntakeProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Hash/import the source, inspect it, and optionally extract safe children.
    pub fn intake(&self, request: &IntakeRequest, store: &ArtifactStore) -> Result<IntakeReport> {
        if !request.path.is_file() {
            anyhow::bail!("intake source '{}' is not a file", request.path.display());
        }
        validate_budgets(&request.budgets)?;
        let probe = read_probe(&request.path)?;
        let (profile, provider_id) = self.inspect_profile(request, &probe)?;
        let canonical = CanonicalFormat::new(profile.format.as_deref().unwrap_or("unknown"))?;
        let media_type = MediaType::new(profile.media_type.clone())?;
        let metadata = serde_json::to_value(&profile.metadata)?;
        let source = store.import_path(
            &request.path,
            ArtifactDescriptor::new(canonical, media_type, ArtifactStorageClass::Source)
                .with_metadata("renderflow.intake.schema", INTAKE_SCHEMA_V1)
                .with_metadata("renderflow.intake.provider", provider_id)
                .with_metadata("renderflow.intake.metadata", metadata),
        )?;

        let mut report = IntakeReport {
            schema_version: INTAKE_SCHEMA_V1.to_string(),
            source,
            profile,
            discovered: Vec::new(),
            diagnostics: Vec::new(),
            budget_usage: IntakeBudgetUsage {
                artifacts: 1,
                extracted_bytes: 0,
                deepest_level: 0,
            },
        };
        if report.profile.format.is_none() {
            report.diagnostics.push(IntakeDiagnostic {
                severity: IntakeDiagnosticSeverity::Warning,
                code: "intake.format_unknown".to_string(),
                message: "No available evidence identifies a supported format; the artifact remains inspectable but conversion capability is unknown".to_string(),
                artifact_path: None,
            });
        }
        if !report.profile.conflicts.is_empty() {
            report.diagnostics.push(IntakeDiagnostic {
                severity: IntakeDiagnosticSeverity::Warning,
                code: "intake.detection_conflict".to_string(),
                message: "Detection signals disagree; the profile records the deterministic confidence-based resolution".to_string(),
                artifact_path: None,
            });
        }
        if request.extract {
            let mut handled = false;
            for provider in &self.providers {
                if provider.extract(store, request, &mut report)? {
                    handled = true;
                    break;
                }
            }
            if !handled {
                report.diagnostics.push(IntakeDiagnostic {
                    severity: IntakeDiagnosticSeverity::Info,
                    code: "intake.extraction_unavailable".to_string(),
                    message: "No extraction provider is available for the resolved format"
                        .to_string(),
                    artifact_path: None,
                });
            }
        }
        profile_available_operations(&mut report.profile, !report.discovered.is_empty());
        Ok(report)
    }

    fn inspect_profile(
        &self,
        request: &IntakeRequest,
        bytes: &[u8],
    ) -> Result<(ResolvedArtifactProfile, String)> {
        let registry = FormatCapabilityRegistry::global();
        let mut signals = Vec::new();
        if let Some(declared) = request.declared_format.as_deref() {
            let format = declared.parse::<Format>().ok();
            signals.push(IntakeSignal {
                kind: IntakeSignalKind::DeclaredFormat,
                value: declared.to_string(),
                format: format.map(|value| value.to_string()),
                confidence: 75,
                provider_id: "source.declaration".to_string(),
            });
        }
        if let Some(extension) = request.path.extension().and_then(|value| value.to_str()) {
            let format = extension.to_ascii_lowercase().parse::<Format>().ok();
            signals.push(IntakeSignal {
                kind: IntakeSignalKind::Extension,
                value: extension.to_ascii_lowercase(),
                format: format.map(|value| value.to_string()),
                confidence: 40,
                provider_id: "renderflow.registry".to_string(),
            });
        }
        let mut magic_matches: Vec<_> = registry
            .all()
            .filter(|descriptor| descriptor.matches_magic(bytes))
            .collect();
        magic_matches.sort_by(|left, right| {
            let left_length = left
                .magic_signatures
                .iter()
                .filter(|signature| signature.matches(bytes))
                .map(|signature| signature.bytes.len())
                .max()
                .unwrap_or(0);
            let right_length = right
                .magic_signatures
                .iter()
                .filter(|signature| signature.matches(bytes))
                .map(|signature| signature.bytes.len())
                .max()
                .unwrap_or(0);
            right_length
                .cmp(&left_length)
                .then_with(|| left.id.cmp(right.id))
        });
        let magic_match = if bytes.starts_with(b"PK") {
            magic_matches
                .iter()
                .find(|descriptor| descriptor.id == "zip")
                .copied()
        } else {
            magic_matches.first().copied()
        };
        if let Some(descriptor) = magic_match {
            signals.push(IntakeSignal {
                kind: IntakeSignalKind::Magic,
                value: descriptor.id.to_string(),
                format: Some(descriptor.id.to_string()),
                confidence: 90,
                provider_id: "renderflow.registry".to_string(),
            });
        }
        if let Some(media_type) = request.declared_media_type.as_deref() {
            let matching = registry
                .all()
                .find(|descriptor| descriptor.media_types.contains(&media_type))
                .map(|descriptor| descriptor.id.to_string());
            signals.push(IntakeSignal {
                kind: IntakeSignalKind::MediaType,
                value: media_type.to_string(),
                format: matching,
                confidence: 70,
                provider_id: "source.declaration".to_string(),
            });
        }

        let context = InspectionContext {
            path: &request.path,
            bytes,
            declared_media_type: request.declared_media_type.as_deref(),
        };
        let mut inspections = Vec::new();
        for provider in &self.providers {
            if let Some(inspection) = provider.inspect(&context)? {
                if let Some(format) = inspection.format {
                    signals.push(IntakeSignal {
                        kind: IntakeSignalKind::Structural,
                        value: format.to_string(),
                        format: Some(format.to_string()),
                        confidence: inspection.confidence,
                        provider_id: provider.id().to_string(),
                    });
                }
                inspections.push((provider.id().to_string(), inspection));
            }
        }

        signals.sort_by(|left, right| {
            right
                .confidence
                .cmp(&left.confidence)
                .then_with(|| left.provider_id.cmp(&right.provider_id))
                .then_with(|| left.value.cmp(&right.value))
        });
        signals.dedup_by(|left, right| {
            left.kind == right.kind
                && left.format == right.format
                && left.provider_id == right.provider_id
        });
        let selected = signals
            .iter()
            .filter_map(|signal| {
                signal
                    .format
                    .as_ref()
                    .map(|format| (format, signal.confidence))
            })
            .max_by_key(|(_, confidence)| *confidence)
            .map(|(format, confidence)| (format.clone(), confidence));
        let mut distinct: BTreeSet<String> = signals
            .iter()
            .filter_map(|signal| signal.format.clone())
            .collect();
        let resolved_format = selected.as_ref().map(|(format, _)| format.clone());
        let format = resolved_format
            .as_deref()
            .and_then(|value| value.parse::<Format>().ok());
        let descriptor = format.and_then(|format| registry.get(format));
        let mut metadata = BTreeMap::new();
        let mut operations = vec!["identify".to_string(), "inspect".to_string()];
        let mut container_format = None;
        let mut stream_formats = Vec::new();
        let mut provider_id = "renderflow.registry".to_string();
        for (id, inspection) in inspections {
            if inspection.confidence >= selected.as_ref().map(|(_, value)| *value).unwrap_or(0) {
                provider_id = id;
            }
            metadata.extend(inspection.metadata);
            operations.extend(inspection.operations);
            container_format = inspection.container_format.or(container_format);
            stream_formats.extend(inspection.stream_formats);
        }
        if let Some(format) = &request.declared_format {
            metadata.insert(
                "identity.declared_format".to_string(),
                ProvenanceValue {
                    value: json!(format),
                    origin: EvidenceOrigin::SourceReported,
                    provider_id: "source.declaration".to_string(),
                },
            );
        }
        if let Some(media_type) = &request.declared_media_type {
            metadata.insert(
                "identity.declared_media_type".to_string(),
                ProvenanceValue {
                    value: json!(media_type),
                    origin: EvidenceOrigin::SourceReported,
                    provider_id: "source.declaration".to_string(),
                },
            );
        }
        if let Some(extension) = request.path.extension().and_then(|value| value.to_str()) {
            metadata.insert(
                "identity.extension".to_string(),
                ProvenanceValue {
                    value: json!(extension.to_ascii_lowercase()),
                    origin: EvidenceOrigin::Inferred,
                    provider_id: "renderflow.registry".to_string(),
                },
            );
        }
        if container_format.as_ref() != resolved_format.as_ref() {
            if let Some(container) = &container_format {
                distinct.remove(container);
            }
        }
        let conflicts = if distinct.len() > 1 {
            vec![IntakeConflict {
                formats: distinct.into_iter().collect(),
                signal_kinds: signals.iter().map(|signal| signal.kind).collect(),
                resolution: resolved_format
                    .as_ref()
                    .map(|format| format!("highest-confidence evidence selected '{format}'"))
                    .unwrap_or_else(|| "no format selected".to_string()),
            }]
        } else {
            Vec::new()
        };
        operations.sort();
        operations.dedup();
        stream_formats.sort();
        stream_formats.dedup();
        let confidence = match selected.map(|(_, confidence)| confidence) {
            Some(85..) => DetectionConfidence::High,
            Some(60..=84) => DetectionConfidence::Medium,
            Some(_) => DetectionConfidence::Low,
            None => DetectionConfidence::Unknown,
        };
        let media_type = descriptor
            .and_then(|value| {
                request
                    .declared_media_type
                    .as_ref()
                    .filter(|declared| value.media_types.contains(&declared.as_str()))
                    .cloned()
                    .or_else(|| {
                        value
                            .media_types
                            .first()
                            .map(|media_type| media_type.to_string())
                    })
            })
            .or_else(|| request.declared_media_type.clone())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let families = descriptor
            .map(|value| value.families.iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        if descriptor.is_some_and(|value| value.has_capability(ArtifactCapability::Extract)) {
            operations.push("extract".to_string());
        }
        Ok((
            ResolvedArtifactProfile {
                format: resolved_format,
                media_type,
                confidence,
                container_format,
                stream_formats,
                families,
                metadata,
                available_operations: operations,
                signals,
                conflicts,
            },
            provider_id,
        ))
    }
}

fn profile_available_operations(profile: &mut ResolvedArtifactProfile, extracted: bool) {
    if profile.container_format.is_some()
        && !profile.available_operations.iter().any(|v| v == "extract")
    {
        profile.available_operations.push("extract".to_string());
    }
    if extracted {
        profile
            .available_operations
            .push("plan_children".to_string());
    }
    profile.available_operations.sort();
    profile.available_operations.dedup();
}

fn validate_budgets(budgets: &IntakeBudgets) -> Result<()> {
    if budgets.max_artifacts == 0
        || budgets.max_extracted_bytes == 0
        || !budgets.max_expansion_ratio.is_finite()
        || budgets.max_expansion_ratio < 1.0
    {
        anyhow::bail!("intake budgets must be positive and expansion ratio must be at least 1.0");
    }
    Ok(())
}

fn read_probe(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(PROBE_BYTES as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

struct NativeStructureProvider;

impl ArtifactIntakeProvider for NativeStructureProvider {
    fn id(&self) -> &'static str {
        "provider.renderflow.native-inspection"
    }

    fn inspect(&self, context: &InspectionContext<'_>) -> Result<Option<ProviderInspection>> {
        let bytes = context.bytes;
        let mut result = ProviderInspection::default();
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
            result.format = Some(Format::Png);
            result.confidence = 100;
            result.metadata.insert(
                "image.width".to_string(),
                observed(
                    json!(u32::from_be_bytes(bytes[16..20].try_into()?)),
                    self.id(),
                ),
            );
            result.metadata.insert(
                "image.height".to_string(),
                observed(
                    json!(u32::from_be_bytes(bytes[20..24].try_into()?)),
                    self.id(),
                ),
            );
            result.metadata.insert(
                "image.alpha".to_string(),
                observed(json!(matches!(bytes.get(25), Some(4 | 6))), self.id()),
            );
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            result.format = Some(Format::Gif);
            result.confidence = 100;
            if bytes.len() >= 10 {
                result.metadata.insert(
                    "image.width".to_string(),
                    observed(json!(u16::from_le_bytes([bytes[6], bytes[7]])), self.id()),
                );
                result.metadata.insert(
                    "image.height".to_string(),
                    observed(json!(u16::from_le_bytes([bytes[8], bytes[9]])), self.id()),
                );
            }
            result.metadata.insert(
                "image.frames_observed".to_string(),
                observed(
                    json!(bytes.iter().filter(|byte| **byte == 0x2c).count()),
                    self.id(),
                ),
            );
        } else if bytes.starts_with(b"%PDF-") {
            result.format = Some(Format::Pdf);
            result.confidence = 100;
            let page_count = bytes
                .windows(11)
                .filter(|window| *window == b"/Type /Page")
                .count();
            result.metadata.insert(
                "document.pages_observed".to_string(),
                observed(json!(page_count), self.id()),
            );
            result.metadata.insert(
                "document.text_markers_present".to_string(),
                observed(
                    json!(bytes.windows(3).any(|window| window == b" BT")),
                    self.id(),
                ),
            );
        } else if bytes.len() >= 36
            && bytes.starts_with(b"RIFF")
            && bytes.get(8..12) == Some(b"WAVE")
        {
            result.format = Some(Format::Wav);
            result.container_format = Some("riff".to_string());
            result.confidence = 100;
            let codec = u16::from_le_bytes([bytes[20], bytes[21]]);
            result.stream_formats.push(match codec {
                1 => "pcm".to_string(),
                other => format!("wave-codec-{other}"),
            });
            result.metadata.insert(
                "audio.channels".to_string(),
                observed(json!(u16::from_le_bytes([bytes[22], bytes[23]])), self.id()),
            );
            result.metadata.insert(
                "audio.sample_rate_hz".to_string(),
                observed(
                    json!(u32::from_le_bytes(bytes[24..28].try_into()?)),
                    self.id(),
                ),
            );
            result.metadata.insert(
                "audio.bits_per_sample".to_string(),
                observed(json!(u16::from_le_bytes([bytes[34], bytes[35]])), self.id()),
            );
        } else if bytes.len() >= 12 && bytes.get(4..8) == Some(b"ftyp") {
            result.format = Some(Format::Mp4);
            result.container_format = Some("mp4".to_string());
            result.confidence = 100;
            result.metadata.insert(
                "media.major_brand".to_string(),
                observed(
                    json!(String::from_utf8_lossy(&bytes[8..12]).to_string()),
                    self.id(),
                ),
            );
        } else if let Ok(text) = std::str::from_utf8(bytes) {
            result.confidence = 55;
            result.metadata.insert(
                "text.encoding".to_string(),
                observed(json!("utf-8"), self.id()),
            );
            result.metadata.insert(
                "text.lines_observed".to_string(),
                observed(json!(text.lines().count()), self.id()),
            );
            if let Ok(value) = serde_json::from_str::<Value>(text) {
                result.format = Some(Format::Json);
                result.confidence = 95;
                result.metadata.insert(
                    "data.shape".to_string(),
                    observed(json!(json_shape(&value)), self.id()),
                );
                if let Some(count) = bounded_record_count(&value) {
                    result.metadata.insert(
                        "data.records".to_string(),
                        observed(json!(count), self.id()),
                    );
                }
            }
        }
        if result.metadata.is_empty() && result.format.is_none() {
            Ok(None)
        } else {
            result.operations.push("inspect".to_string());
            Ok(Some(result))
        }
    }
}

struct ZipProvider;

impl ArtifactIntakeProvider for ZipProvider {
    fn id(&self) -> &'static str {
        "provider.renderflow.zip"
    }

    fn inspect(&self, context: &InspectionContext<'_>) -> Result<Option<ProviderInspection>> {
        if !context.bytes.starts_with(b"PK\x03\x04")
            && !context.bytes.starts_with(b"PK\x05\x06")
            && !context.bytes.starts_with(b"PK\x07\x08")
        {
            return Ok(None);
        }
        let mut result = ProviderInspection {
            format: Some(Format::Zip),
            container_format: Some("zip".to_string()),
            confidence: 98,
            ..ProviderInspection::default()
        };
        let file = File::open(context.path)?;
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(archive) => archive,
            Err(error) => {
                result.metadata.insert(
                    "archive.malformed".to_string(),
                    observed(json!(true), self.id()),
                );
                result.metadata.insert(
                    "archive.error".to_string(),
                    observed(json!(error.to_string()), self.id()),
                );
                result.operations.push("inspect".to_string());
                return Ok(Some(result));
            }
        };
        let mut compressed = 0_u64;
        let mut expanded = 0_u64;
        let mut encrypted = 0_u64;
        let mut unsafe_paths = 0_u64;
        let mut names = BTreeSet::new();
        for index in 0..archive.len() {
            let entry = archive.by_index_raw(index)?;
            compressed = compressed.saturating_add(entry.compressed_size());
            expanded = expanded.saturating_add(entry.size());
            encrypted += u64::from(entry.encrypted());
            unsafe_paths += u64::from(!safe_archive_path(Path::new(entry.name())));
            names.insert(entry.name().to_string());
        }
        if names.contains("mimetype") {
            if let Ok(mut entry) = archive.by_name("mimetype") {
                let mut value = String::new();
                let _ = entry.read_to_string(&mut value);
                if value.trim() == "application/epub+zip" {
                    result.format = Some(Format::Epub);
                    result.confidence = 100;
                }
            }
        } else if names.contains("[Content_Types].xml")
            && names.iter().any(|name| name.starts_with("word/"))
        {
            result.format = Some(Format::Docx);
            result.confidence = 100;
        } else {
            let files: Vec<&str> = names
                .iter()
                .map(String::as_str)
                .filter(|name| !name.ends_with('/'))
                .filter(|name| !name.starts_with("__MACOSX/"))
                .collect();
            if !files.is_empty() && files.iter().all(|name| is_image_entry_name(name)) {
                result.format = Some(Format::Cbz);
                result.confidence = 100;
            }
        }
        result.metadata.insert(
            "archive.entries".to_string(),
            observed(json!(archive.len()), self.id()),
        );
        result.metadata.insert(
            "archive.compressed_bytes".to_string(),
            observed(json!(compressed), self.id()),
        );
        result.metadata.insert(
            "archive.expanded_bytes_declared".to_string(),
            observed(json!(expanded), self.id()),
        );
        result.metadata.insert(
            "archive.encrypted_entries".to_string(),
            observed(json!(encrypted), self.id()),
        );
        result.metadata.insert(
            "archive.unsafe_paths".to_string(),
            observed(json!(unsafe_paths), self.id()),
        );
        result
            .operations
            .extend(["inspect".to_string(), "extract".to_string()]);
        Ok(Some(result))
    }

    fn extract(
        &self,
        store: &ArtifactStore,
        request: &IntakeRequest,
        report: &mut IntakeReport,
    ) -> Result<bool> {
        if report.profile.container_format.as_deref() != Some("zip") {
            return Ok(false);
        }
        extract_zip_tree(store, request, report)?;
        Ok(true)
    }
}

fn observed(value: Value, provider_id: &str) -> ProvenanceValue {
    ProvenanceValue {
        value,
        origin: EvidenceOrigin::ProviderObserved,
        provider_id: provider_id.to_string(),
    }
}

fn json_shape(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn bounded_record_count(value: &Value) -> Option<usize> {
    match value {
        Value::Array(values) if values.len() <= 100_000 => Some(values.len()),
        Value::Object(values) if values.len() <= 100_000 => Some(values.len()),
        _ => None,
    }
}

fn is_image_entry_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "bmp" | "tif" | "tiff"
            )
        })
}

fn extract_zip_tree(
    store: &ArtifactStore,
    request: &IntakeRequest,
    report: &mut IntakeReport,
) -> Result<()> {
    let source = report.source.clone();
    extract_zip_artifact(store, request, report, &source, 1, "")
}

fn extract_zip_artifact(
    store: &ArtifactStore,
    request: &IntakeRequest,
    report: &mut IntakeReport,
    parent: &Artifact,
    depth: u32,
    prefix: &str,
) -> Result<()> {
    if depth > request.budgets.max_depth {
        report.diagnostics.push(rejected(
            "intake.budget.depth",
            "Nested extraction depth budget reached",
            Some(prefix),
        ));
        return Ok(());
    }
    let file = store.open(parent)?;
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(error) => {
            report.diagnostics.push(rejected(
                "intake.archive.malformed",
                &format!("Archive could not be opened: {error}"),
                Some(prefix),
            ));
            return Ok(());
        }
    };
    for index in 0..archive.len() {
        if report.budget_usage.artifacts >= request.budgets.max_artifacts {
            report.diagnostics.push(rejected(
                "intake.budget.artifacts",
                "Artifact count budget reached",
                Some(prefix),
            ));
            break;
        }
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                report.diagnostics.push(rejected(
                    "intake.archive.entry_unreadable",
                    &error.to_string(),
                    Some(prefix),
                ));
                continue;
            }
        };
        let raw_name = entry.name().to_string();
        let logical_path = if prefix.is_empty() {
            raw_name.clone()
        } else {
            format!("{prefix}!/{raw_name}")
        };
        if entry.is_dir() {
            continue;
        }
        if !safe_archive_path(Path::new(&raw_name)) || entry.enclosed_name().is_none() {
            report.diagnostics.push(rejected(
                "intake.archive.path_traversal",
                "Archive entry has an absolute or traversal path",
                Some(&logical_path),
            ));
            continue;
        }
        if entry.is_symlink()
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            report.diagnostics.push(rejected(
                "intake.archive.unsafe_link",
                "Archive symbolic links are not extracted",
                Some(&logical_path),
            ));
            continue;
        }
        if entry.encrypted() && !request.allow_encrypted {
            report.diagnostics.push(rejected(
                "intake.archive.encrypted",
                "Encrypted archive entry requires explicit policy",
                Some(&logical_path),
            ));
            continue;
        }
        let compressed = entry.compressed_size();
        let expanded = entry.size();
        let ratio = expanded as f64 / compressed.max(1) as f64;
        if ratio > request.budgets.max_expansion_ratio {
            report.diagnostics.push(rejected(
                "intake.budget.expansion_ratio",
                &format!(
                    "Entry expansion ratio {ratio:.2} exceeds budget {:.2}",
                    request.budgets.max_expansion_ratio
                ),
                Some(&logical_path),
            ));
            continue;
        }
        if report.budget_usage.extracted_bytes.saturating_add(expanded)
            > request.budgets.max_extracted_bytes
        {
            report.diagnostics.push(rejected(
                "intake.budget.bytes",
                "Extracted byte budget reached",
                Some(&logical_path),
            ));
            break;
        }
        let probe = {
            let mut bytes = Vec::new();
            entry
                .by_ref()
                .take(PROBE_BYTES as u64)
                .read_to_end(&mut bytes)?;
            bytes
        };
        drop(entry);
        let mut entry = archive.by_index(index)?;
        let detected = resolve_child_format(&raw_name, &probe);
        let descriptor = ArtifactDescriptor::new(
            CanonicalFormat::new(
                detected
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unknown".to_string()),
            )?,
            detected
                .map(MediaType::for_format)
                .unwrap_or(MediaType::new("application/octet-stream")?),
            ArtifactStorageClass::Intermediate,
        )
        .with_source(parent.id().clone())
        .with_metadata("renderflow.intake.logical_path", logical_path.clone())
        .with_metadata("renderflow.intake.depth", depth);
        let artifact = store.put_reader(&mut entry, descriptor)?;
        report.budget_usage.artifacts += 1;
        report.budget_usage.extracted_bytes = report
            .budget_usage
            .extracted_bytes
            .saturating_add(artifact.size_bytes());
        report.budget_usage.deepest_level = report.budget_usage.deepest_level.max(depth);
        report.discovered.push(DiscoveredArtifact {
            artifact: artifact.clone(),
            parent_artifact_id: parent.id().to_string(),
            logical_path: logical_path.clone(),
            depth,
        });
        if request.recursive && is_zip_artifact(&artifact, &probe) {
            extract_zip_artifact(store, request, report, &artifact, depth + 1, &logical_path)?;
        }
    }
    Ok(())
}

fn resolve_child_format(name: &str, bytes: &[u8]) -> Option<Format> {
    let extension = crate::detect::detect_from_extension(name);
    if bytes.starts_with(b"PK") {
        return extension
            .filter(|format| {
                matches!(
                    format,
                    Format::Zip | Format::Docx | Format::Epub | Format::Cbz
                )
            })
            .or(Some(Format::Zip));
    }
    crate::detect::detect_from_bytes(bytes).or(extension)
}

fn is_zip_artifact(artifact: &Artifact, bytes: &[u8]) -> bool {
    matches!(artifact.format().as_str(), "zip" | "epub" | "docx" | "cbz")
        || bytes.starts_with(b"PK\x03\x04")
}

fn safe_archive_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn rejected(code: &str, message: &str, path: Option<&str>) -> IntakeDiagnostic {
    IntakeDiagnostic {
        severity: IntakeDiagnosticSeverity::Rejected,
        code: code.to_string(),
        message: message.to_string(),
        artifact_path: path
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn unknown_binary_is_a_valid_inspectable_artifact() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("mystery.bin");
        std::fs::write(&source, b"\xde\xad\xbe\xef").unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let report = IntakeEngine::new()
            .intake(&IntakeRequest::from_path(source), &store)
            .unwrap();
        assert_eq!(report.profile.format, None);
        assert_eq!(report.source.format().as_str(), "unknown");
        store.verify(&report.source).unwrap();
    }

    #[test]
    fn conflicting_extension_and_magic_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("not-a-jpeg.jpg");
        std::fs::write(
            &source,
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x02\0\0\0\x03\x08\x06",
        )
        .unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let report = IntakeEngine::new()
            .intake(&IntakeRequest::from_path(source), &store)
            .unwrap();
        assert_eq!(report.profile.format.as_deref(), Some("png"));
        assert!(!report.profile.conflicts.is_empty());
    }

    #[test]
    fn text_structured_data_and_media_metadata_are_normalized() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let json_path = directory.path().join("records.json");
        std::fs::write(&json_path, br#"[{"id":1},{"id":2}]"#).unwrap();
        let json_report = IntakeEngine::new()
            .intake(&IntakeRequest::from_path(json_path), &store)
            .unwrap();
        assert_eq!(json_report.profile.format.as_deref(), Some("json"));
        assert_eq!(json_report.profile.metadata["data.records"].value, json!(2));

        let wav_path = directory.path().join("audio.bin");
        let mut wav = vec![0_u8; 44];
        wav[0..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        wav[20..22].copy_from_slice(&1_u16.to_le_bytes());
        wav[22..24].copy_from_slice(&2_u16.to_le_bytes());
        wav[24..28].copy_from_slice(&48_000_u32.to_le_bytes());
        wav[34..36].copy_from_slice(&24_u16.to_le_bytes());
        std::fs::write(&wav_path, wav).unwrap();
        let wav_report = IntakeEngine::new()
            .intake(&IntakeRequest::from_path(wav_path), &store)
            .unwrap();
        assert_eq!(wav_report.profile.format.as_deref(), Some("wav"));
        assert_eq!(wav_report.profile.stream_formats, vec!["pcm"]);
    }

    #[test]
    fn malformed_zip_is_reported_without_a_panic_or_false_extraction() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("broken.zip");
        std::fs::write(&source, b"PK\x03\x04truncated").unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let report = IntakeEngine::new()
            .intake(
                &IntakeRequest::from_path(source).with_extraction(true),
                &store,
            )
            .unwrap();
        assert_eq!(report.profile.format.as_deref(), Some("zip"));
        assert_eq!(
            report.profile.metadata["archive.malformed"].value,
            json!(true)
        );
        assert!(report.discovered.is_empty());
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "intake.archive.malformed"));
    }

    #[test]
    fn zip_traversal_is_rejected_and_safe_child_is_parent_linked() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("archive.zip");
        let file = File::create(&source).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("../escape.txt", options).unwrap();
        writer.write_all(b"bad").unwrap();
        writer.start_file("safe/data.json", options).unwrap();
        writer.write_all(br#"[{"id":1}]"#).unwrap();
        writer
            .add_symlink("unsafe-link", "../outside", options)
            .unwrap();
        writer.finish().unwrap();
        let store = ArtifactStore::new(directory.path().join("store")).unwrap();
        let request = IntakeRequest::from_path(source).with_extraction(true);
        let report = IntakeEngine::new().intake(&request, &store).unwrap();
        assert_eq!(report.discovered.len(), 1);
        assert_eq!(
            report.discovered[0].artifact.sources(),
            &[report.source.id().clone()]
        );
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "intake.archive.path_traversal"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "intake.archive.unsafe_link"));
    }
}
