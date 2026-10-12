//! Read-only inspection of the canonical screenplay JSON model.
//!
//! Native screenplay import and export remain separate adapters. This command
//! never launches a tool, reads a declared upstream source, or creates an output.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::screenplay::{self, ScreenplayInspection, ScreenplayPreservation, MAX_INPUT_BYTES};

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
                "native screenplay filename extension .{extension} is recognized, but its parser is not implemented; \
                native adapters remain pending in #424. screenplay inspect accepts only \
                renderflow.screenplay/v1 JSON; it does not treat native files as generic text"
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
        "Native adapters: pending; no conversion was attempted.\n\
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
            assert!(error.contains("native adapters remain pending in #424"));
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
