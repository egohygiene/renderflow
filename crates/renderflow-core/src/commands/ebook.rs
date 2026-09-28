use std::path::Path;

use anyhow::Result;

use crate::ebook::{
    inspect_ebook, inspect_ebook_with_manifest, EbookCapabilityContract, EbookDiagnostic,
    EbookDiagnosticSeverity, FixedLayoutStatus,
};

pub fn run_inspect(
    input: &str,
    format: &str,
    epubcheck: bool,
    fixed_layout: bool,
    run_manifest: Option<&str>,
) -> Result<()> {
    if (fixed_layout || run_manifest.is_some())
        && std::fs::metadata(input)?.len() > 512 * 1024 * 1024 + 1024 * 1024
    {
        anyhow::bail!(
            "ebook.fixed_layout.bounds: EPUB file exceeds the exact route inspection bound"
        );
    }
    let mut inspection = match run_manifest {
        Some(manifest) => {
            inspect_ebook_with_manifest(Path::new(input), epubcheck, Path::new(manifest))?
        }
        None => inspect_ebook(Path::new(input), epubcheck)?,
    };
    if fixed_layout
        && !matches!(
            inspection.fixed_layout.as_ref().map(|item| item.status),
            Some(FixedLayoutStatus::Validated)
        )
    {
        if !matches!(
            inspection.fixed_layout.as_ref().map(|item| item.status),
            Some(FixedLayoutStatus::Invalid)
        ) {
            inspection.diagnostics.push(EbookDiagnostic {
                severity: EbookDiagnosticSeverity::Error,
                code: "ebook.fixed_layout.unsupported".to_string(),
                message: "EPUB does not satisfy the exact ordered PNG/JPEG fixed-layout route"
                    .to_string(),
            });
        }
        inspection.valid = false;
    }
    emit(&inspection, format)?;
    if !inspection.valid
        || (epubcheck && inspection.epubcheck.as_ref().and_then(|item| item.passed) != Some(true))
    {
        anyhow::bail!("e-book validation failed; see emitted evidence");
    }
    Ok(())
}

pub fn run_capabilities(format: &str) -> Result<()> {
    emit(&EbookCapabilityContract::builtin(), format)
}

fn emit<T: serde::Serialize>(value: &T, format: &str) -> Result<()> {
    match format.to_ascii_lowercase().as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(value)?),
        "yaml" | "yml" => print!("{}", serde_yaml_ng::to_string(value)?),
        "text" => {
            let value = serde_yaml_ng::to_string(value)?;
            print!("{value}");
        }
        other => {
            anyhow::bail!("unknown e-book output format '{other}'; supported: text, json, yaml")
        }
    }
    Ok(())
}
