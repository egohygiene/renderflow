//! Review-required screenplay recovery with a new, receipt-last evidence bundle.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::cli::ScreenplayRecoverArguments;
use crate::process::ProcessCancellationToken;
use crate::screenplay::{self, native::NativeScreenplayFormat};
use crate::screenplay::recovery::{export, model, provider};
use crate::screenplay::recovery::model::{RecoveryCandidate, RecoveryInputFormat};
use crate::screenplay::recovery::provider::{ExtractedDocument, ExtractionRequest,
    ExtractionSourceFormat, RecoveryExtractionConfig};

use super::screenplay::{publish_conversion_bundle, require_new_bundle};

const BUNDLE_SCHEMA: &str = "renderflow.screenplay-recovery-bundle/v1";
const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_MEMBER_BYTES: usize = 32 * 1024 * 1024;
const MAX_BUNDLE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Error)]
#[error("{code}: {message}")]
struct RecoveryBundleError {
    code: &'static str,
    message: String,
}

fn failure(code: &'static str, message: impl Into<String>) -> RecoveryBundleError {
    RecoveryBundleError { code, message: message.into() }
}

fn extraction_error(error: provider::ExtractionError) -> anyhow::Error {
    let detail = format!("{}: {error}", error.code());
    anyhow::Error::new(error).context(detail)
}

pub fn run_recover(arguments: ScreenplayRecoverArguments) -> Result<()> {
    // Validate all caller-selected declarations before touching any provider.
    let source_format = source_format(&arguments.source_format)?;
    let targets = target_formats(&arguments.targets)?;
    let output = Path::new(&arguments.output);
    let input = Path::new(&arguments.input);
    require_new_bundle(output)?;
    require_disjoint_source(input, output)?;
    let config = read_config(Path::new(&arguments.config))?;
    let cancellation = ProcessCancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    let extracted = provider::extract(&ExtractionRequest {
        input: input.to_owned(), source_format,
        expected_source_sha256: arguments.source_sha256,
        config: config.clone(),
    }, cancellation.clone()).map_err(extraction_error)?;
    let recovered_format = match source_format {
        ExtractionSourceFormat::Pdf => RecoveryInputFormat::Pdf,
        ExtractionSourceFormat::Text => RecoveryInputFormat::Text,
    };
    let candidate = model::recover_pages(&extracted.pages, &extracted.source_sha256, recovered_format)?;
    model::validate_candidate(&candidate)?;
    let (files, receipt) = recovery_bundle(&extracted, &candidate, &config, &targets, &cancellation)?;
    recheck_source(input, &extracted, config.limits.max_input_bytes, &cancellation)?;
    require_disjoint_source(input, output)?;
    publish_conversion_bundle(output, &files, &cancellation)?;
    eprintln!("Recovered screenplay candidate requires human review; extraction, structure, and native derivatives remain unqualified.");
    std::io::stdout().write_all(&receipt)?;
    Ok(())
}

fn source_format(value: &str) -> Result<ExtractionSourceFormat> {
    match value {
        "pdf" => Ok(ExtractionSourceFormat::Pdf),
        "text" => Ok(ExtractionSourceFormat::Text),
        _ => Err(failure("screenplay_recovery.invalid_request", "source format must be pdf or text").into()),
    }
}

fn target_formats(values: &[String]) -> Result<Vec<NativeScreenplayFormat>> {
    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();
    for value in values {
        let target = match value.as_str() {
            "fountain" => NativeScreenplayFormat::Fountain,
            "fdx" => NativeScreenplayFormat::Fdx,
            "fadein" => NativeScreenplayFormat::FadeIn,
            "osf" => NativeScreenplayFormat::Osf,
            _ => return Err(failure("screenplay_recovery.invalid_request", "native target must be fountain, fdx, fadein, or osf").into()),
        };
        if seen.insert(target.as_str()) { targets.push(target); }
    }
    Ok(targets)
}

fn read_config(path: &Path) -> Result<RecoveryExtractionConfig> {
    let metadata = std::fs::symlink_metadata(path).context("cannot inspect screenplay recovery config")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err(failure("screenplay_recovery.invalid_request", "config must be a regular, non-symlink JSON file of at most 64 KiB").into());
    }
    let file = File::open(path).context("cannot open screenplay recovery config")?;
    if !file.metadata()?.is_file() {
        return Err(failure("screenplay_recovery.invalid_request", "config must be a regular file").into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(failure("screenplay_recovery.invalid_request", "config exceeds 64 KiB").into());
    }
    let config: RecoveryExtractionConfig = serde_json::from_slice(&bytes)
        .map_err(|error| failure("screenplay_recovery.invalid_request", format!("invalid screenplay recovery configuration JSON: {error}")))?;
    config.validate().map_err(extraction_error)?;
    Ok(config)
}

fn require_disjoint_source(input: &Path, output: &Path) -> Result<()> {
    let source = input.canonicalize().context("cannot resolve screenplay recovery source")?;
    let parent = output.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = output.file_name().ok_or_else(|| failure("screenplay_recovery.invalid_request", "output must name a new bundle directory"))?;
    let destination = parent.canonicalize().context("cannot resolve screenplay recovery destination parent")?.join(name);
    if destination == source || destination.starts_with(&source) || source.starts_with(&destination) {
        return Err(failure("screenplay_recovery.invalid_request", "source and output bundle must not overlap").into());
    }
    Ok(())
}

fn recovery_bundle(
    extracted: &ExtractedDocument,
    candidate: &RecoveryCandidate,
    config: &RecoveryExtractionConfig,
    targets: &[NativeScreenplayFormat],
    cancellation: &ProcessCancellationToken,
) -> Result<(Vec<(String, Vec<u8>)>, Vec<u8>)> {
    provider::validate_extraction_evidence(&extracted.pages, &extracted.evidence, config, &extracted.source_sha256)
        .map_err(extraction_error)?;
    model::validate_candidate(candidate)?;
    let expected_format = match extracted.evidence.source_format {
        ExtractionSourceFormat::Pdf => RecoveryInputFormat::Pdf,
        ExtractionSourceFormat::Text => RecoveryInputFormat::Text,
    };
    if candidate.source_sha256 != extracted.source_sha256 || candidate.source_format != expected_format
        || !candidate.lossy || !candidate.review_required || candidate.pages.len() != extracted.pages.len()
        || candidate.pages.iter().zip(&extracted.pages).any(|(recovered, extracted)| {
            recovered.page_number != extracted.page_number || recovered.method != extracted.method || recovered.text != extracted.text
        }) {
        return Err(failure("screenplay_recovery.invalid_output", "candidate lost its source or review-required recovery identity").into());
    }
    let mut files = Vec::new();
    append_member(&mut files, "screenplay.screenplay.json".to_owned(), screenplay::render_json(&candidate.screenplay)?)?;
    append_member(&mut files, "recovery.json".to_owned(), json_bytes(candidate)?)?;
    append_member(&mut files, "extracted-pages.json".to_owned(), json_bytes(&extracted.pages)?)?;
    append_member(&mut files, "provider-evidence.json".to_owned(), json_bytes(&extracted.evidence)?)?;
    append_member(&mut files, "recovery-config.json".to_owned(), json_bytes(config)?)?;
    let mut derivatives = Vec::new();
    for target in targets {
        require_not_cancelled(cancellation)?;
        let rendered = export::export(candidate, *target)?;
        export::validate_export(candidate, *target, &rendered.bytes, &rendered.evidence)?;
        let path = format!("screenplay.{}", target.extension());
        let evidence_path = format!("{}-evidence.json", target.as_str());
        derivatives.push(serde_json::json!({
            "path": path, "format": target.as_str(), "evidence": evidence_path,
            "source_sha256": candidate.source_sha256, "recovery_candidate": "recovery.json",
            "lossy": true, "review_required": true, "human_reviewed": false,
        }));
        append_member(&mut files, path, rendered.bytes)?;
        append_member(&mut files, evidence_path, json_bytes(&rendered.evidence)?)?;
    }
    require_not_cancelled(cancellation)?;
    let members = files.iter().map(|(path, bytes)| serde_json::json!({
        "path": path, "sha256": digest(bytes), "size_bytes": bytes.len(),
    })).collect::<Vec<_>>();
    let receipt = json_bytes(&serde_json::json!({
        "schema_version": BUNDLE_SCHEMA,
        "status": "completed",
        "approval_status": "review_required",
        "source_sha256": candidate.source_sha256,
        "source_format": candidate.source_format,
        "source_untouched": true,
        "lossy": true,
        "review_required": true,
        "human_reviewed": false,
        "runtime_qualified": false,
        "canonical": "screenplay.screenplay.json",
        "recovery_candidate": "recovery.json",
        "extracted_pages": "extracted-pages.json",
        "provider_evidence": "provider-evidence.json",
        "configuration": "recovery-config.json",
        "derivatives": derivatives,
        "files": members,
        "limits": "Completed means the bounded recovery bundle was published. It does not approve recovered text or structure, establish extraction accuracy, or qualify native application acceptance. Keep the original source and every evidence member for review.",
    }))?;
    append_member(&mut files, "bundle.json".to_owned(), receipt.clone())?;
    Ok((files, receipt))
}

fn append_member(files: &mut Vec<(String, Vec<u8>)>, path: String, bytes: Vec<u8>) -> Result<()> {
    let total = files.iter().try_fold(bytes.len(), |total, (_, bytes)| total.checked_add(bytes.len()))
        .ok_or_else(|| failure("screenplay_recovery.limit_exceeded", "bundle byte count overflow"))?;
    if bytes.len() > MAX_MEMBER_BYTES || total > MAX_BUNDLE_BYTES {
        return Err(failure("screenplay_recovery.limit_exceeded", "recovery bundle exceeds 32 MiB per member or 128 MiB total").into());
    }
    if files.iter().any(|(name, _)| name == &path) {
        return Err(failure("screenplay_recovery.invalid_output", "recovery bundle contains a duplicate member").into());
    }
    files.push((path, bytes));
    Ok(())
}

struct BoundedJson { bytes: Vec<u8> }

impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_MEMBER_BYTES.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other("recovery JSON member exceeds 32 MiB"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut writer = BoundedJson { bytes: Vec::new() };
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    Ok(writer.bytes)
}

fn require_not_cancelled(cancellation: &ProcessCancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(failure("screenplay_recovery.cancelled", "recovery publication was cancelled").into());
    }
    Ok(())
}

fn recheck_source(input: &Path, extracted: &ExtractedDocument, maximum: u64, cancellation: &ProcessCancellationToken) -> Result<()> {
    let started = Instant::now();
    require_not_cancelled(cancellation)?;
    let before = std::fs::symlink_metadata(input).context("cannot recheck screenplay recovery source")?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() > maximum {
        return Err(failure("screenplay_recovery.source_changed", "source is no longer a bounded regular file").into());
    }
    let mut file = File::open(input).context("cannot reopen screenplay recovery source")?;
    if !file.metadata()?.is_file() {
        return Err(failure("screenplay_recovery.source_changed", "source is no longer a regular file").into());
    }
    let mut digest_state = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        require_not_cancelled(cancellation)?;
        if started.elapsed() > Duration::from_secs(30) {
            return Err(failure("screenplay_recovery.timed_out", "source publication recheck exceeded 30 seconds").into());
        }
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        size += count as u64;
        if size > maximum {
            return Err(failure("screenplay_recovery.source_changed", "source grew past its declared bound").into());
        }
        digest_state.update(&buffer[..count]);
    }
    let after = std::fs::symlink_metadata(input)?;
    if !after.is_file() || after.file_type().is_symlink() || after.len() != size
        || before.len() != size || size != extracted.evidence.source_bytes
        || format!("{:x}", digest_state.finalize()) != extracted.source_sha256 {
        return Err(failure("screenplay_recovery.source_changed", "source bytes changed before bundle publication").into());
    }
    require_not_cancelled(cancellation)?;
    Ok(())
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

#[cfg(test)]
mod tests {
    use super::*;

    fn text_extraction(input: &Path) -> (ExtractedDocument, RecoveryExtractionConfig) {
        let config = RecoveryExtractionConfig { allow_unqualified: true, ..Default::default() };
        let extracted = provider::extract(&ExtractionRequest {
            input: input.to_owned(), source_format: ExtractionSourceFormat::Text,
            expected_source_sha256: None, config: config.clone(),
        }, ProcessCancellationToken::new()).unwrap();
        (extracted, config)
    }

    #[test]
    fn native_bundle_retains_review_required_candidate_and_exact_evidence_members() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("source.txt");
        let source = b"INT. ROOM - DAY\n\nA lamp flickers.\n\nMAYA\nHello.\n";
        std::fs::write(&input, source).unwrap();
        let (extracted, config) = text_extraction(&input);
        let candidate = model::recover_pages(&extracted.pages, &extracted.source_sha256, RecoveryInputFormat::Text).unwrap();
        let cancellation = ProcessCancellationToken::new();
        let (files, receipt) = recovery_bundle(&extracted, &candidate, &config, &[NativeScreenplayFormat::Fdx], &cancellation).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&receipt).unwrap();
        assert_eq!(report["review_required"], true);
        assert_eq!(report["lossy"], true);
        assert_eq!(report["human_reviewed"], false);
        assert_eq!(report["runtime_qualified"], false);
        assert_eq!(report["derivatives"][0]["review_required"], true);
        assert_eq!(files.last().unwrap().0, "bundle.json");
        for (index, (path, bytes)) in files[..files.len() - 1].iter().enumerate() {
            assert_eq!(report["files"][index]["path"], *path);
            assert_eq!(report["files"][index]["sha256"], digest(bytes));
            assert_eq!(report["files"][index]["size_bytes"], bytes.len());
        }
        let saved = files.iter().find(|(name, _)| name == "recovery.json").unwrap();
        let retained: RecoveryCandidate = serde_json::from_slice(&saved.1).unwrap();
        assert_eq!(retained, candidate);
        let mut approved = candidate.clone();
        approved.review_required = false;
        assert!(recovery_bundle(&extracted, &approved, &config, &[], &cancellation).is_err());
        assert_eq!(std::fs::read(input).unwrap(), source);
    }

    #[test]
    fn changed_source_and_mismatched_extraction_evidence_refuse_publication() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("source.txt");
        std::fs::write(&input, b"INT. ROOM - DAY\n\nA lamp flickers.\n").unwrap();
        let (mut extracted, config) = text_extraction(&input);
        let candidate = model::recover_pages(&extracted.pages, &extracted.source_sha256, RecoveryInputFormat::Text).unwrap();
        let cancellation = ProcessCancellationToken::new();
        recheck_source(&input, &extracted, config.limits.max_input_bytes, &cancellation).unwrap();
        std::fs::write(&input, b"INT. ROOM - DAY\n\nA lamp changes.\n").unwrap();
        let error = recheck_source(&input, &extracted, config.limits.max_input_bytes, &cancellation).unwrap_err();
        assert_eq!(error.downcast_ref::<RecoveryBundleError>().unwrap().code, "screenplay_recovery.source_changed");
        extracted.evidence.review_required = false;
        assert!(recovery_bundle(&extracted, &candidate, &config, &[], &cancellation).is_err());
        assert!(require_disjoint_source(&input, &input).is_err());
    }
}
