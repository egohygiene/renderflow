//! Canonical JSON inspection and explicitly configured native conversion.
//!
//! Inspection never launches a tool or creates an output. Conversion invokes
//! the bounded provider and publishes a new receipt-last evidence bundle.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::cli::ScreenplayConvertArguments;
use crate::process::ProcessCancellationToken;
use crate::screenplay::{self, ScreenplayInspection, ScreenplayPreservation, MAX_INPUT_BYTES};

pub fn run_convert(arguments: ScreenplayConvertArguments) -> Result<()> {
    use crate::screenplay::provider::{ScripttoolConfig, ScripttoolRequest};

    let output = Path::new(&arguments.output_dir);
    require_new_bundle(output)?;
    let request = ScripttoolRequest {
        input: arguments.input.into(),
        source_format: native_format(&arguments.input_format)?,
        target_format: native_format(&arguments.output_format)?,
        config: ScripttoolConfig {
            executable: arguments.scripttool_path.into(),
            executable_sha256: arguments.scripttool_sha256,
            expected_version: arguments.expected_version,
            timeout_seconds: arguments.timeout_seconds,
            allow_unqualified: arguments.allow_unqualified,
        },
        expected_source_sha256: arguments.source_sha256,
    };
    let cancellation = ProcessCancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    let result = screenplay::provider::convert(&request, cancellation.clone())?;
    let mut evidence = serde_json::to_vec_pretty(&result.evidence)?;
    evidence.push(b'\n');
    let (files, receipt) = conversion_bundle(result.output, evidence)?;
    publish_conversion_bundle(output, &files, &cancellation)?;
    std::io::stdout().write_all(&receipt)?;
    Ok(())
}

fn native_format(value: &str) -> Result<screenplay::native::NativeScreenplayFormat> {
    use screenplay::native::NativeScreenplayFormat;
    match value {
        "fountain" => Ok(NativeScreenplayFormat::Fountain),
        "fdx" => Ok(NativeScreenplayFormat::Fdx),
        "osf" => Ok(NativeScreenplayFormat::Osf),
        "fadein" => Ok(NativeScreenplayFormat::FadeIn),
        _ => bail!("unsupported native screenplay format: {value}"),
    }
}

fn require_new_bundle(output: &Path) -> Result<()> {
    match std::fs::symlink_metadata(output) {
        Ok(_) => bail!("screenplay output directory must not already exist"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("cannot inspect screenplay output directory"),
    }
    let parent = output.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if !parent.is_dir() {
        bail!("screenplay output parent directory must already exist");
    }
    Ok(())
}

fn conversion_bundle(output: Vec<u8>, evidence: Vec<u8>) -> Result<(Vec<(String, Vec<u8>)>, Vec<u8>)> {
    let mut files = vec![
        ("screenplay.fountain".to_owned(), output),
        ("provider-evidence.json".to_owned(), evidence),
    ];
    let members = files.iter().map(|(name, bytes)| serde_json::json!({
        "path": name,
        "sha256": format!("{:x}", Sha256::digest(bytes)),
        "size_bytes": bytes.len(),
    })).collect::<Vec<_>>();
    let mut receipt = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": "renderflow.screenplay-bundle/v1",
        "status": "completed",
        "runtime_qualified": false,
        "files": members,
        "limits": "A successful run validates only the adopted body profile and observed tool invocation; native application acceptance and complete screenplay fidelity remain unqualified.",
    }))?;
    receipt.push(b'\n');
    files.push(("bundle.json".to_owned(), receipt.clone()));
    Ok((files, receipt))
}

/// Publish a new directory and write its completion receipt last. On I/O or
/// cancellation failure a partial directory is retained without a receipt.
fn publish_conversion_bundle(output: &Path, files: &[(String, Vec<u8>)], cancellation: &ProcessCancellationToken) -> Result<()> {
    if cancellation.is_cancelled() { bail!("screenplay publication cancelled"); }
    std::fs::create_dir(output).context("cannot create new screenplay output directory")?;
    for (name, bytes) in files {
        if cancellation.is_cancelled() { bail!("screenplay publication cancelled"); }
        let mut temporary = tempfile::NamedTempFile::new_in(output)?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        if cancellation.is_cancelled() { bail!("screenplay publication cancelled"); }
        temporary.persist_noclobber(output.join(name)).map_err(|error| error.error)?;
    }
    Ok(())
}

pub fn run_inspect(input: &str, format: &str) -> Result<()> {
    // Reject an unsupported output request before touching the input, including
    // callers that bypass Clap and call this command through the library.
    require_report_format(format)?;
    let bytes = read_canonical_input(Path::new(input))?;
    let inspection = screenplay::inspect_json(&bytes)?;
    let output = render_report(&inspection, format)?;
    std::io::stdout().write_all(output.as_bytes())?;
    Ok(())
}

fn require_report_format(format: &str) -> Result<()> {
    if !matches!(format, "json" | "text") {
        bail!("unsupported screenplay report format: {format}; supported formats: json, text");
    }
    Ok(())
}

fn require_canonical_extension(input: &Path) -> Result<()> {
    let extension = input.extension().and_then(|value| value.to_str())
        .unwrap_or_default().to_ascii_lowercase();
    match extension.as_str() {
        "json" => Ok(()),
        "fdx" | "fadein" | "fade_in" | "osf" | "fountain" => {
            bail!(
                "native screenplay filename extension .{extension} is recognized; \
                screenplay inspect accepts only renderflow.screenplay/v1 JSON. \
                Use screenplay convert with explicit provider pins for supported native body imports"
            )
        }
        _ => bail!(
            "screenplay inspect requires a canonical .screenplay.json or .json file; \
            the document must declare renderflow.screenplay/v1"
        ),
    }
}

fn read_canonical_input(input: &Path) -> Result<Vec<u8>> {
    require_canonical_extension(input)?;
    let metadata = std::fs::symlink_metadata(input).context("cannot inspect screenplay input")?;
    if !metadata.file_type().is_file() {
        bail!("screenplay input must be a regular file, not a symlink or special file");
    }
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        bail!("screenplay input exceeds the {MAX_INPUT_BYTES}-byte limit");
    }
    let file = File::open(input).context("cannot open screenplay input")?;
    if !file.metadata()?.is_file() {
        bail!("screenplay input must be a regular file");
    }
    read_bounded(file)
}

fn read_bounded(reader: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(MAX_INPUT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT_BYTES {
        bail!("screenplay input exceeds the {MAX_INPUT_BYTES}-byte limit");
    }
    Ok(bytes)
}

fn render_report(inspection: &ScreenplayInspection, format: &str) -> Result<String> {
    require_report_format(format)?;
    if format == "json" {
        return Ok(format!("{}\n", serde_json::to_string_pretty(inspection)?));
    }

    let mut output = format!(
        "Canonical screenplay structure accepted\n\
        Observed JSON: {} bytes, SHA-256 {}\n\
        Declared upstream source (not reobserved): {}\n\
        Title-page fields: {}\n\
        Elements: {}\n\
        Dual-dialogue groups: {}\n\
        Retained extension entries: {}\n\
        Recorded prior losses: {}\n",
        inspection.input_bytes,
        inspection.input_sha256,
        serde_json::to_string(&inspection.source)?,
        inspection.title_page_fields,
        inspection.elements,
        inspection.dual_dialogue_groups,
        inspection.extension_entries,
        inspection.prior_losses.len(),
    );
    for (kind, count) in &inspection.element_counts {
        output.push_str(&format!("  {kind}: {count}\n"));
    }
    for loss in &inspection.prior_losses {
        // JSON quoting keeps user-authored text escaped in terminal output.
        output.push_str(&format!("  Prior loss: {}\n", serde_json::to_string(loss)?));
    }
    output.push_str("Import assessments (native sources not read):\n");
    for assessment in &inspection.import_assessments {
        let preservation = preservation_label(assessment.preservation);
        let reader = if assessment.adapter_available { "available" } else { "unavailable" };
        output.push_str(&format!(
            "  {}: {preservation}; reader {reader}; review required: {}\n",
            assessment.source_format.as_str(), serde_json::to_string(&assessment.review_required)?,
        ));
    }
    output.push_str("Export assessments (no exports performed):\n");
    for assessment in &inspection.export_assessments {
        let preservation = preservation_label(assessment.preservation);
        let writer = if assessment.adapter_available { "available" } else { "unavailable" };
        output.push_str(&format!(
            "  {}: {preservation}; writer {writer}; review required: {}\n",
            assessment.target.as_str(), serde_json::to_string(&assessment.review_required)?,
        ));
    }
    output.push_str(
        "Native adapters: not selected by canonical inspection; no conversion was attempted.\n\
        Structure acceptance does not establish source fidelity, semantic accuracy, \
        or a lossless native-format round trip.\n",
    );
    Ok(output)
}

fn preservation_label(preservation: ScreenplayPreservation) -> &'static str {
    match preservation {
        ScreenplayPreservation::CanonicalModel => "canonical model retained",
        ScreenplayPreservation::Unassessed => "preservation unassessed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_bundle_receipt_binds_exact_bytes_and_is_last() {
        let (files, receipt) = conversion_bundle(b"Scene.\n".to_vec(), b"{}\n".to_vec()).unwrap();
        assert_eq!(files.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(),
            vec!["screenplay.fountain", "provider-evidence.json", "bundle.json"]);
        assert_eq!(files[2].1, receipt);
        let value: serde_json::Value = serde_json::from_slice(&receipt).unwrap();
        assert_eq!(value["runtime_qualified"], false);
        for (index, (name, bytes)) in files[..2].iter().enumerate() {
            assert_eq!(value["files"][index]["path"], *name);
            assert_eq!(value["files"][index]["size_bytes"], bytes.len());
            assert_eq!(value["files"][index]["sha256"], format!("{:x}", Sha256::digest(bytes)));
        }
    }

    #[test]
    fn conversion_publication_refuses_existing_paths_and_missing_parents() {
        let directory = tempfile::tempdir().unwrap();
        assert!(require_new_bundle(directory.path()).is_err());
        let existing = directory.path().join("source.fdx");
        std::fs::write(&existing, b"original").unwrap();
        assert!(require_new_bundle(&existing).is_err());
        assert!(require_new_bundle(&directory.path().join("missing").join("bundle")).is_err());
        assert_eq!(std::fs::read(existing).unwrap(), b"original");
    }

    #[test]
    fn conversion_publication_cancellation_creates_no_directory() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("bundle");
        let cancellation = ProcessCancellationToken::new();
        cancellation.cancel();
        let (files, _) = conversion_bundle(b"Scene.\n".to_vec(), b"{}\n".to_vec()).unwrap();
        assert!(publish_conversion_bundle(&output, &files, &cancellation).is_err());
        assert!(!output.exists());
    }

    #[test]
    fn partial_conversion_publication_has_no_completion_receipt() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("bundle");
        // A duplicate member deterministically exercises persist_noclobber
        // failure after a member was published but before the receipt.
        let files = vec![("screenplay.fountain".to_owned(), b"first".to_vec()),
            ("screenplay.fountain".to_owned(), b"second".to_vec()),
            ("bundle.json".to_owned(), b"{}".to_vec())];
        assert!(publish_conversion_bundle(&output, &files, &ProcessCancellationToken::new()).is_err());
        assert_eq!(std::fs::read(output.join("screenplay.fountain")).unwrap(), b"first");
        assert!(!output.join("bundle.json").exists());
    }

    #[test]
    fn conversion_publication_commits_bundle_members_without_replacing_source() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("bundle");
        let source = directory.path().join("scene.fdx");
        std::fs::write(&source, b"original").unwrap();
        let (files, receipt) = conversion_bundle(b"Scene.\n".to_vec(), b"{}\n".to_vec()).unwrap();
        publish_conversion_bundle(&output, &files, &ProcessCancellationToken::new()).unwrap();
        assert_eq!(std::fs::read(output.join("bundle.json")).unwrap(), receipt);
        assert_eq!(std::fs::read(source).unwrap(), b"original");
    }

    fn canonical_bytes() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": "renderflow.screenplay/v1",
            "source": {"sha256": "1".repeat(64), "format": "fountain"},
            "title_page": [],
            "elements": [{"id": "action-1", "kind": "action", "text": "Mira waits."}],
            "dual_dialogue": [],
            "losses": [{
                "direction": "import", "format": "fountain", "kind": "text_styling",
                "detail": "Declared by an earlier adapter.\nStill unreviewed.",
            }],
            "extensions": {"example:unhandled": {"revision": "α"}},
        })).unwrap()
    }

    #[test]
    fn json_report_separates_observed_bytes_from_declared_source_and_preservation() {
        let bytes = canonical_bytes();
        let inspection = screenplay::inspect_json(&bytes).unwrap();
        let report: serde_json::Value = serde_json::from_str(&render_report(&inspection, "json").unwrap()).unwrap();
        assert_eq!(report["schema_version"], "renderflow.screenplay-inspection/v1");
        assert_eq!(report["input_bytes"], bytes.len());
        assert_ne!(report["input_sha256"], report["source"]["sha256"]);
        assert_eq!(report["source_identity_verified"], false);
        assert_eq!(report["native_adapters_available"], false);
        assert_eq!(report["extension_entries"], 1);
        assert_eq!(report["prior_losses"][0]["kind"], "text_styling");
        let imports = report["import_assessments"].as_array().unwrap();
        let canonical = imports.iter()
            .find(|assessment| assessment["source_format"] == "screenplay_json").unwrap();
        assert_eq!(canonical["adapter_available"], true);
        assert_eq!(canonical["preservation"], "canonical_model");
        let native_import = imports.iter()
            .find(|assessment| assessment["source_format"] == "fountain").unwrap();
        assert_eq!(native_import["adapter_available"], false);
        assert_eq!(native_import["preservation"], "unassessed");
        assert!(!native_import["review_required"].as_array().unwrap().is_empty());
        let fountain = report["export_assessments"].as_array().unwrap().iter()
            .find(|assessment| assessment["target"] == "fountain").unwrap();
        assert_eq!(fountain["adapter_available"], false);
        assert_eq!(fountain["preservation"], "unassessed");
    }

    #[test]
    fn text_report_keeps_limitations_and_declared_loss_controls_visible() {
        let inspection = screenplay::inspect_json(&canonical_bytes()).unwrap();
        let report = render_report(&inspection, "text").unwrap();
        assert!(report.contains("Declared upstream source (not reobserved)"));
        assert!(report.contains("Retained extension entries: 1"));
        assert!(report.contains("Recorded prior losses: 1"));
        assert!(report.contains("Declared by an earlier adapter.\\nStill unreviewed."));
        assert!(report.contains("screenplay_json: canonical model retained; reader available"));
        assert!(report.contains("fountain: preservation unassessed; reader unavailable"));
        assert!(report.contains("screenplay_json: canonical model retained; writer available"));
        assert!(report.contains("fountain: preservation unassessed; writer unavailable"));
        assert!(report.contains("native sources not read"));
        assert!(report.contains("no exports performed"));
        assert!(report.contains("does not establish source fidelity"));
    }

    #[test]
    fn unsupported_report_format_is_refused_before_input_is_opened() {
        let error = run_inspect("missing.screenplay.json", "yaml").unwrap_err().to_string();
        assert!(error.contains("unsupported screenplay report format"));
    }

    #[test]
    fn recognizes_native_extensions_without_reading_or_parsing_them() {
        for extension in ["fdx", "FDX", "fadein", "fade_in", "osf", "fountain"] {
            let error = read_canonical_input(Path::new(&format!("missing.{extension}")))
                .unwrap_err().to_string();
            assert!(error.contains("screenplay inspect accepts only renderflow.screenplay/v1 JSON"));
        }
    }

    #[test]
    fn rejects_foreign_extensions_instead_of_reading_generic_content() {
        for input in ["script.xml", "script.txt", "script.pdf", "script"] {
            assert!(require_canonical_extension(Path::new(input)).is_err());
        }
        assert!(require_canonical_extension(Path::new("script.screenplay.json")).is_ok());
        assert!(require_canonical_extension(Path::new("script.JSON")).is_ok());
    }

    #[test]
    fn bounded_reader_accepts_exact_limit_and_refuses_one_more_byte() {
        let exact = std::io::repeat(b" "[0]).take(MAX_INPUT_BYTES as u64);
        assert_eq!(read_bounded(exact).unwrap().len(), MAX_INPUT_BYTES);
        let excessive = std::io::repeat(b" "[0]).take(MAX_INPUT_BYTES as u64 + 1);
        assert!(read_bounded(excessive).is_err());
    }

    #[test]
    fn requires_regular_file_and_leaves_input_unchanged() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("script.screenplay.json");
        std::fs::write(&input, b"original").unwrap();
        assert_eq!(read_canonical_input(&input).unwrap(), b"original");
        assert_eq!(std::fs::read(&input).unwrap(), b"original");
        let nested = directory.path().join("directory.json");
        std::fs::create_dir(&nested).unwrap();
        assert!(read_canonical_input(&nested).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_input() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("script.screenplay.json");
        let link = directory.path().join("link.screenplay.json");
        std::fs::write(&source, b"original").unwrap();
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(read_canonical_input(&link).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"original");
    }
}
