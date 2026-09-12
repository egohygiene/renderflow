//! Artifact validation, fidelity policy, and capability conformance evidence.

use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::artifact::{Artifact, ArtifactStore};
use crate::evidence::{
    redact_sensitive_text, ValidationDiagnostic, ValidationState, ValidatorEvidence,
};
use crate::graph::capability::{ArtifactCapability, FormatCapabilityRegistry, FormatDescriptor};
use crate::graph::Format;

pub const CONFORMANCE_MATRIX_SCHEMA_V1: &str = "renderflow.conformance/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorSupportTier {
    Production,
    Experimental,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorDescriptor {
    pub id: String,
    pub version: String,
    pub provider: String,
    pub formats: Vec<String>,
    pub support_tier: ValidatorSupportTier,
}

impl ValidatorDescriptor {
    pub fn supports(&self, format: Format) -> bool {
        self.formats.is_empty()
            || self
                .formats
                .iter()
                .any(|candidate| candidate == &format.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationCheck {
    pub state: ValidationState,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl ValidationCheck {
    pub fn valid() -> Self {
        Self {
            state: ValidationState::Valid,
            diagnostics: Vec::new(),
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            state: ValidationState::ValidWithWarnings,
            diagnostics: vec![ValidationDiagnostic {
                code: code.into(),
                message: message.into(),
            }],
        }
    }

    pub fn invalid(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            state: ValidationState::Invalid,
            diagnostics: vec![ValidationDiagnostic {
                code: code.into(),
                message: message.into(),
            }],
        }
    }

    pub fn unavailable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            state: ValidationState::Unavailable,
            diagnostics: vec![ValidationDiagnostic {
                code: code.into(),
                message: message.into(),
            }],
        }
    }
}

pub trait ArtifactValidator: Send + Sync {
    fn descriptor(&self) -> ValidatorDescriptor;
    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactValidationOutcome {
    pub state: ValidationState,
    pub validators: Vec<ValidatorEvidence>,
}

#[derive(Default)]
pub struct ValidationRegistry {
    validators: HashMap<String, Arc<dyn ArtifactValidator>>,
}

impl ValidationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builtins() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(NonEmptyValidator));
        registry.register(Arc::new(Utf8Validator));
        registry.register(Arc::new(JsonValidator));
        registry.register(Arc::new(YamlValidator));
        registry.register(Arc::new(MarkupValidator));
        registry.register(Arc::new(DelimitedTextValidator));
        registry.register(Arc::new(PdfValidator));
        registry.register(Arc::new(PngValidator));
        registry.register(Arc::new(JpegValidator));
        registry.register(Arc::new(ZipValidator));
        registry.register(Arc::new(EpubValidator));
        registry.register(Arc::new(RiffWaveValidator));
        registry.register(Arc::new(DeclaredSignatureValidator));
        registry
    }

    pub fn register(&mut self, validator: Arc<dyn ArtifactValidator>) -> &mut Self {
        let id = validator.descriptor().id;
        self.validators.insert(id, validator);
        self
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn ArtifactValidator>> {
        self.validators.get(id).cloned()
    }

    pub fn descriptors(&self) -> Vec<ValidatorDescriptor> {
        let mut descriptors = self
            .validators
            .values()
            .map(|validator| validator.descriptor())
            .collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));
        descriptors
    }

    pub fn validator_ids_for(&self, format: Format) -> Vec<String> {
        let mut ids = self
            .validators
            .values()
            .filter_map(|validator| {
                let descriptor = validator.descriptor();
                descriptor.supports(format).then_some(descriptor.id)
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub fn validate_in_store(
        &self,
        artifact: &Artifact,
        format: Format,
        requested: &[String],
        store: &ArtifactStore,
    ) -> ArtifactValidationOutcome {
        let selected = if requested.is_empty() {
            self.default_validator_ids(format)
        } else {
            requested.to_vec()
        };
        if selected.is_empty() {
            return unavailable_outcome(format);
        }
        let mut evidence = Vec::new();
        for id in selected {
            let Some(validator) = self.get(&id) else {
                evidence.push(unavailable_validator(
                    id.clone(),
                    format!("Requested validator '{id}' is not registered"),
                ));
                continue;
            };
            let descriptor = validator.descriptor();
            if !descriptor.supports(format) {
                evidence.push(ValidatorEvidence {
                    validator_id: descriptor.id,
                    validator_version: descriptor.version,
                    provider: descriptor.provider,
                    state: ValidationState::Unavailable,
                    diagnostics: vec![ValidationDiagnostic {
                        code: "validation.format_unsupported".to_string(),
                        message: format!("Validator '{id}' does not support '{format}'"),
                    }],
                });
                continue;
            }
            let check = validator.validate(artifact, store);
            evidence.push(match check {
                Ok(check) => ValidatorEvidence {
                    validator_id: descriptor.id,
                    validator_version: descriptor.version,
                    provider: descriptor.provider,
                    state: check.state,
                    diagnostics: check
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| ValidationDiagnostic {
                            code: diagnostic.code,
                            message: redact_sensitive_text(&diagnostic.message),
                        })
                        .collect(),
                },
                Err(error) => ValidatorEvidence {
                    validator_id: descriptor.id,
                    validator_version: descriptor.version,
                    provider: descriptor.provider,
                    state: ValidationState::Unavailable,
                    diagnostics: vec![ValidationDiagnostic {
                        code: "validation.validator_error".to_string(),
                        message: redact_sensitive_text(&error.to_string()),
                    }],
                },
            });
        }
        ArtifactValidationOutcome {
            state: aggregate_validation_state(&evidence),
            validators: evidence,
        }
    }

    fn default_validator_ids(&self, format: Format) -> Vec<String> {
        let mut ids = vec!["validator.core.non_empty".to_string()];
        let specific = match format {
            Format::Json => Some("validator.core.json"),
            Format::Yaml => Some("validator.core.yaml"),
            Format::Html | Format::Xml | Format::Svg => Some("validator.core.markup"),
            Format::Csv | Format::Tsv => Some("validator.core.delimited_text"),
            Format::Pdf => Some("validator.core.pdf"),
            Format::Png => Some("validator.core.png"),
            Format::Jpeg => Some("validator.core.jpeg"),
            Format::Epub | Format::Kepub => Some("validator.core.epub"),
            Format::Zip | Format::Docx | Format::Cbz => Some("validator.core.zip"),
            Format::Wav | Format::Bwf => Some("validator.core.riff_wave"),
            Format::Markdown
            | Format::Rst
            | Format::Latex
            | Format::Fountain
            | Format::Toml
            | Format::Srt
            | Format::WebVtt => Some("validator.core.utf8"),
            _ => Some("validator.core.declared_signature"),
        };
        if let Some(specific) = specific {
            ids.push(specific.to_string());
        }
        ids
    }
}

fn aggregate_validation_state(evidence: &[ValidatorEvidence]) -> ValidationState {
    if evidence
        .iter()
        .any(|validator| validator.state == ValidationState::Invalid)
    {
        ValidationState::Invalid
    } else if evidence
        .iter()
        .any(|validator| validator.state == ValidationState::Unavailable)
    {
        ValidationState::Unavailable
    } else if evidence
        .iter()
        .any(|validator| validator.state == ValidationState::ValidWithWarnings)
    {
        ValidationState::ValidWithWarnings
    } else {
        ValidationState::Valid
    }
}

fn unavailable_outcome(format: Format) -> ArtifactValidationOutcome {
    ArtifactValidationOutcome {
        state: ValidationState::Unavailable,
        validators: vec![unavailable_validator(
            "none".to_string(),
            format!("No validator is registered for format '{format}'"),
        )],
    }
}

fn unavailable_validator(id: String, message: String) -> ValidatorEvidence {
    ValidatorEvidence {
        validator_id: id,
        validator_version: "unknown".to_string(),
        provider: "unavailable".to_string(),
        state: ValidationState::Unavailable,
        diagnostics: vec![ValidationDiagnostic {
            code: "validation.validator_unavailable".to_string(),
            message,
        }],
    }
}

fn descriptor(
    id: &str,
    formats: &[Format],
    support_tier: ValidatorSupportTier,
) -> ValidatorDescriptor {
    ValidatorDescriptor {
        id: id.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        provider: "renderflow.native".to_string(),
        formats: formats.iter().map(ToString::to_string).collect(),
        support_tier,
    }
}

struct NonEmptyValidator;

impl ArtifactValidator for NonEmptyValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.non_empty",
            &[],
            ValidatorSupportTier::Production,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        store.verify(artifact)?;
        if artifact.size_bytes() == 0 {
            Ok(ValidationCheck::invalid(
                "validation.empty_artifact",
                "Artifact payload is empty",
            ))
        } else {
            Ok(ValidationCheck::valid())
        }
    }
}

const UTF8_FORMATS: &[Format] = &[
    Format::Markdown,
    Format::Html,
    Format::Rst,
    Format::Latex,
    Format::Fountain,
    Format::Json,
    Format::Yaml,
    Format::Toml,
    Format::Csv,
    Format::Tsv,
    Format::Xml,
    Format::Svg,
    Format::Srt,
    Format::WebVtt,
];

struct Utf8Validator;

impl ArtifactValidator for Utf8Validator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.utf8",
            UTF8_FORMATS,
            ValidatorSupportTier::Production,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        match String::from_utf8(store.read_bytes(artifact)?) {
            Ok(_) => Ok(ValidationCheck::valid()),
            Err(_) => Ok(ValidationCheck::invalid(
                "validation.invalid_utf8",
                "Text artifact is not valid UTF-8",
            )),
        }
    }
}

struct JsonValidator;

impl ArtifactValidator for JsonValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.json",
            &[Format::Json],
            ValidatorSupportTier::Production,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let bytes = store.read_bytes(artifact)?;
        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(_) => Ok(ValidationCheck::valid()),
            Err(error) => Ok(ValidationCheck::invalid(
                "validation.invalid_json",
                format!("JSON parse failed: {error}"),
            )),
        }
    }
}

struct YamlValidator;

impl ArtifactValidator for YamlValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.yaml",
            &[Format::Yaml],
            ValidatorSupportTier::Production,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let text = store.read_text(artifact)?;
        match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text) {
            Ok(_) => Ok(ValidationCheck::valid()),
            Err(error) => Ok(ValidationCheck::invalid(
                "validation.invalid_yaml",
                format!("YAML parse failed: {error}"),
            )),
        }
    }
}

struct MarkupValidator;

impl ArtifactValidator for MarkupValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.markup",
            &[Format::Html, Format::Xml, Format::Svg],
            ValidatorSupportTier::Experimental,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let text = store.read_text(artifact)?;
        let trimmed = text.trim();
        if !trimmed.starts_with('<') || !trimmed.contains('>') {
            return Ok(ValidationCheck::invalid(
                "validation.invalid_markup",
                "Markup artifact has no recognizable element",
            ));
        }
        Ok(ValidationCheck::warning(
            "validation.structural_probe_only",
            "Markup passed the native structural probe; full schema validation was not requested",
        ))
    }
}

struct DelimitedTextValidator;

impl ArtifactValidator for DelimitedTextValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.delimited_text",
            &[Format::Csv, Format::Tsv],
            ValidatorSupportTier::Experimental,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let text = store.read_text(artifact)?;
        let delimiter = if artifact.format().as_str() == "tsv" {
            '\t'
        } else {
            ','
        };
        let mut widths = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.split(delimiter).count());
        let Some(expected) = widths.next() else {
            return Ok(ValidationCheck::invalid(
                "validation.empty_table",
                "Delimited artifact contains no rows",
            ));
        };
        if widths.any(|width| width != expected) {
            return Ok(ValidationCheck::invalid(
                "validation.inconsistent_columns",
                "Delimited artifact has inconsistent column counts",
            ));
        }
        Ok(ValidationCheck::warning(
            "validation.quoting_not_parsed",
            "Column counts passed; quoted-field semantics require an external validator",
        ))
    }
}

struct PdfValidator;

impl ArtifactValidator for PdfValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.pdf",
            &[Format::Pdf],
            ValidatorSupportTier::Experimental,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let bytes = store.read_bytes(artifact)?;
        if !bytes.starts_with(b"%PDF-") || !bytes.windows(5).rev().take(1024).any(|w| w == b"%%EOF")
        {
            return Ok(ValidationCheck::invalid(
                "validation.invalid_pdf_envelope",
                "PDF header or end-of-file marker is missing",
            ));
        }
        Ok(ValidationCheck::warning(
            "validation.pdf_envelope_only",
            "PDF envelope is intact; deep object validation requires a configured provider",
        ))
    }
}

struct PngValidator;

impl ArtifactValidator for PngValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.png",
            &[Format::Png],
            ValidatorSupportTier::Production,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let bytes = store.read_bytes(artifact)?;
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.windows(4).any(|w| w == b"IEND") {
            Ok(ValidationCheck::valid())
        } else {
            Ok(ValidationCheck::invalid(
                "validation.invalid_png",
                "PNG signature or IEND chunk is missing",
            ))
        }
    }
}

struct JpegValidator;

impl ArtifactValidator for JpegValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.jpeg",
            &[Format::Jpeg],
            ValidatorSupportTier::Production,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let bytes = store.read_bytes(artifact)?;
        if bytes.starts_with(&[0xff, 0xd8]) && bytes.ends_with(&[0xff, 0xd9]) {
            Ok(ValidationCheck::valid())
        } else {
            Ok(ValidationCheck::invalid(
                "validation.invalid_jpeg",
                "JPEG start or end marker is missing",
            ))
        }
    }
}

struct ZipValidator;

impl ArtifactValidator for ZipValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.zip",
            &[Format::Zip, Format::Docx, Format::Cbz],
            ValidatorSupportTier::Experimental,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let bytes = store.read_bytes(artifact)?;
        let starts = bytes.starts_with(b"PK\x03\x04")
            || bytes.starts_with(b"PK\x05\x06")
            || bytes.starts_with(b"PK\x07\x08");
        let has_eocd = bytes.windows(4).any(|window| window == b"PK\x05\x06");
        if !starts || !has_eocd {
            return Ok(ValidationCheck::invalid(
                "validation.invalid_zip_envelope",
                "ZIP header or end-of-central-directory record is missing",
            ));
        }
        Ok(ValidationCheck::warning(
            "validation.zip_envelope_only",
            "ZIP envelope is intact; member-specific validation requires a format provider",
        ))
    }
}

struct EpubValidator;

impl ArtifactValidator for EpubValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.epub",
            &[Format::Epub, Format::Kepub],
            ValidatorSupportTier::Experimental,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        store.verify(artifact)?;
        let path = store.payload_path(artifact)?;
        let inspection = crate::ebook::inspect_ebook(&path, false)?;
        let diagnostics = inspection
            .diagnostics
            .into_iter()
            .map(|diagnostic| ValidationDiagnostic {
                code: diagnostic.code,
                message: diagnostic.message,
            })
            .collect::<Vec<_>>();
        if !inspection.valid {
            return Ok(ValidationCheck {
                state: ValidationState::Invalid,
                diagnostics,
            });
        }
        Ok(ValidationCheck {
            state: if diagnostics.is_empty() {
                ValidationState::Valid
            } else {
                ValidationState::ValidWithWarnings
            },
            diagnostics,
        })
    }
}

struct RiffWaveValidator;

impl ArtifactValidator for RiffWaveValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.riff_wave",
            &[Format::Wav, Format::Bwf],
            ValidatorSupportTier::Production,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let mut file = store.open(artifact)?;
        let mut header = [0_u8; 12];
        if file.read_exact(&mut header).is_err()
            || &header[..4] != b"RIFF"
            || &header[8..] != b"WAVE"
        {
            return Ok(ValidationCheck::invalid(
                "validation.invalid_wave",
                "RIFF/WAVE header is missing or truncated",
            ));
        }
        Ok(ValidationCheck::valid())
    }
}

struct DeclaredSignatureValidator;

impl ArtifactValidator for DeclaredSignatureValidator {
    fn descriptor(&self) -> ValidatorDescriptor {
        descriptor(
            "validator.core.declared_signature",
            &[],
            ValidatorSupportTier::Experimental,
        )
    }

    fn validate(&self, artifact: &Artifact, store: &ArtifactStore) -> Result<ValidationCheck> {
        let format = artifact
            .format()
            .as_str()
            .parse::<Format>()
            .context("artifact carries an unknown canonical format")?;
        let registry = FormatCapabilityRegistry::global();
        let Some(format_descriptor) = registry.get(format) else {
            return Ok(ValidationCheck::invalid(
                "validation.unknown_format",
                "Artifact format is absent from the capability registry",
            ));
        };
        if format_descriptor.magic_signatures.is_empty() {
            return Ok(ValidationCheck::unavailable(
                "validation.no_signature",
                "No native structural signature is registered for this format",
            ));
        }
        let bytes = store.read_bytes(artifact)?;
        if format_descriptor
            .magic_signatures
            .iter()
            .any(|signature| signature.matches(&bytes))
        {
            Ok(ValidationCheck::warning(
                "validation.signature_only",
                "Declared signature matched; deep structural validation is unavailable",
            ))
        } else {
            Ok(ValidationCheck::invalid(
                "validation.signature_mismatch",
                "Artifact does not match any declared format signature",
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceSupportStatus {
    Implemented,
    Experimental,
    Unavailable,
    Planned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceRow {
    pub format: String,
    pub name: String,
    pub families: Vec<String>,
    pub declared_capabilities: Vec<String>,
    pub executor_implemented: bool,
    pub required_provider_ids: Vec<String>,
    pub validator_ids: Vec<String>,
    pub fixture_ids: Vec<String>,
    pub supported_platforms: Vec<String>,
    pub deterministic_validation: bool,
    pub loss_profile: String,
    pub support_status: ConformanceSupportStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityConformanceMatrix {
    pub schema_version: String,
    pub engine_version: String,
    pub formats: Vec<ConformanceRow>,
}

impl CapabilityConformanceMatrix {
    pub fn builtins() -> Self {
        let formats = FormatCapabilityRegistry::global();
        let validators = ValidationRegistry::builtins();
        let mut rows = formats
            .all()
            .map(|format| conformance_row(format, &validators))
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.format.cmp(&right.format));
        Self {
            schema_version: CONFORMANCE_MATRIX_SCHEMA_V1.to_string(),
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            formats: rows,
        }
    }
}

fn conformance_row(
    descriptor: &FormatDescriptor,
    validators: &ValidationRegistry,
) -> ConformanceRow {
    let format = descriptor.id.parse::<Format>().ok();
    let validator_ids = format
        .map(|format| validators.default_validator_ids(format))
        .unwrap_or_default();
    let executor_implemented = descriptor.has_capability(ArtifactCapability::Convert)
        || descriptor.has_capability(ArtifactCapability::Generate);
    let has_specific_validator = validator_ids
        .iter()
        .any(|id| id != "validator.core.non_empty" && id != "validator.core.declared_signature");
    let has_structural_probe = has_specific_validator || !descriptor.magic_signatures.is_empty();
    let has_production_validator = validator_ids.iter().any(|id| {
        matches!(
            id.as_str(),
            "validator.core.utf8"
                | "validator.core.json"
                | "validator.core.yaml"
                | "validator.core.png"
                | "validator.core.jpeg"
                | "validator.core.riff_wave"
        )
    });
    let support_status = if executor_implemented && has_production_validator {
        ConformanceSupportStatus::Implemented
    } else if executor_implemented && has_structural_probe {
        ConformanceSupportStatus::Experimental
    } else if descriptor.has_capability(ArtifactCapability::Inspect)
        || descriptor.has_capability(ArtifactCapability::Detect)
    {
        ConformanceSupportStatus::Unavailable
    } else {
        ConformanceSupportStatus::Planned
    };
    ConformanceRow {
        format: descriptor.id.to_string(),
        name: descriptor.name.to_string(),
        families: descriptor
            .families
            .iter()
            .map(ToString::to_string)
            .collect(),
        declared_capabilities: descriptor
            .capabilities
            .iter()
            .map(ToString::to_string)
            .collect(),
        executor_implemented,
        required_provider_ids: descriptor
            .external_requirements
            .iter()
            .map(|tool| tool.stable_id().to_string())
            .collect(),
        validator_ids,
        fixture_ids: golden_fixture_ids(descriptor.id),
        supported_platforms: vec![
            "linux".to_string(),
            "macos".to_string(),
            "windows".to_string(),
        ],
        deterministic_validation: true,
        loss_profile: descriptor.loss_profile.to_string(),
        support_status,
    }
}

fn golden_fixture_ids(format: &str) -> Vec<String> {
    let ids: &[&str] = match format {
        "markdown" => &["fixture.document.markdown"],
        "pdf" => &["fixture.document.pdf"],
        "png" => &["fixture.image.png", "fixture.mismatch.jpeg-png"],
        "svg" => &["fixture.image.svg"],
        "wav" => &["fixture.audio.wav"],
        "mp4" => &["fixture.video.mp4"],
        "zip" => &["fixture.archive.zip", "fixture.corrupt.zip"],
        "json" => &["fixture.data.json"],
        "srt" => &["fixture.subtitle.srt"],
        _ => &[],
    };
    ids.iter().map(|id| (*id).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{ArtifactDescriptor, ArtifactStorageClass};

    #[test]
    fn truncated_png_is_invalid_even_when_non_empty() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let artifact = store
            .put_bytes(
                b"\x89PNG\r\n\x1a\ntruncated",
                ArtifactDescriptor::for_format(Format::Png, ArtifactStorageClass::Terminal),
            )
            .unwrap();
        let outcome =
            ValidationRegistry::builtins().validate_in_store(&artifact, Format::Png, &[], &store);
        assert_eq!(outcome.state, ValidationState::Invalid);
    }

    #[test]
    fn conformance_matrix_is_deterministic_and_non_empty() {
        let first = CapabilityConformanceMatrix::builtins();
        let second = CapabilityConformanceMatrix::builtins();
        assert_eq!(first, second);
        assert!(!first.formats.is_empty());
        assert_eq!(
            first
                .formats
                .iter()
                .find(|row| row.format == "wav")
                .unwrap()
                .fixture_ids,
            vec!["fixture.audio.wav"]
        );
    }
}
