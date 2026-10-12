//! Explicit screenplay PDF publication and independent Fountain analysis.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::ScreenplayPdfArguments;
use crate::process::ProcessCancellationToken;
use crate::screenplay::afterwriting::{self, AfterwritingConfig, AfterwritingRequest};
use crate::screenplay::{analysis, MAX_INPUT_BYTES};

use super::screenplay::{publish_conversion_bundle, require_new_bundle};

const MAX_CONFIG_BYTES: usize = 64 * 1024;
const BUNDLE_SCHEMA: &str = "renderflow.screenplay-pdf-bundle/v1";
const BUNDLE_LIMITS: &str = "PDF structure and the restricted source profile are independently inspected. Visual fidelity, font substitution, pagination stability, and byte reproducibility remain unqualified.";

pub fn run_analyze(input: &str) -> Result<()> {
    let bytes = read_regular(Path::new(input), MAX_INPUT_BYTES)?;
    let report = analysis::analyze_fountain(&bytes)?;
    std::io::stdout().write_all(&json_bytes(&report)?)?;
    Ok(())
}

pub fn run_package_fingerprint(package_root: &str) -> Result<()> {
    let cancellation = ProcessCancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    let fingerprint = afterwriting::package_fingerprint(Path::new(package_root), cancellation)?;
    std::io::stdout().write_all(&json_bytes(&fingerprint)?)?;
    Ok(())
}

pub fn run_pdf(arguments: ScreenplayPdfArguments) -> Result<()> {
    let output = Path::new(&arguments.output_dir);
    require_new_bundle(output)?;
    let config = read_config(Path::new(&arguments.config))?;
    let cancellation = ProcessCancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    let run = afterwriting::render(&AfterwritingRequest {
        input: arguments.input.into(), config,
        expected_source_sha256: arguments.source_sha256,
    }, cancellation.clone())?;
    let (files, receipt) = pdf_bundle(run.output, json_bytes(&run.analysis)?, json_bytes(&run.evidence)?)?;
    publish_conversion_bundle(output, &files, &cancellation)?;
    std::io::stdout().write_all(&receipt)?;
    Ok(())
}

fn read_config(path: &Path) -> Result<AfterwritingConfig> {
    let config: AfterwritingConfig = serde_json::from_slice(&read_regular(path, MAX_CONFIG_BYTES)?)
        .context("invalid afterwriting configuration JSON")?;
    config.validate()?;
    Ok(config)
}

fn read_regular(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path).context("cannot inspect screenplay file")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("screenplay input/config must be a regular file, not a symlink or special file");
    }
    if metadata.len() > maximum as u64 { bail!("screenplay file exceeds its {maximum}-byte limit"); }
    let file = File::open(path).context("cannot open screenplay file")?;
    if !file.metadata()?.is_file() { bail!("screenplay file must be regular"); }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum { bail!("screenplay file exceeds its {maximum}-byte limit"); }
    Ok(bytes)
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn pdf_bundle(pdf: Vec<u8>, analysis: Vec<u8>, evidence: Vec<u8>) -> Result<(Vec<(String, Vec<u8>)>, Vec<u8>)> {
    let mut files = vec![
        ("screenplay.pdf".to_owned(), pdf),
        ("analysis.json".to_owned(), analysis),
        ("provider-evidence.json".to_owned(), evidence),
    ];
    let members = files.iter().map(|(path, bytes)| serde_json::json!({
        "path": path, "sha256": format!("{:x}", Sha256::digest(bytes)), "size_bytes": bytes.len(),
    })).collect::<Vec<_>>();
    let receipt = json_bytes(&serde_json::json!({
        "schema_version": BUNDLE_SCHEMA,
        "status": "completed",
        "runtime_qualified": false,
        "files": members,
        "limits": BUNDLE_LIMITS,
    }))?;
    files.push(("bundle.json".to_owned(), receipt.clone()));
    Ok((files, receipt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_bundle_hashes_exact_members_and_publishes_receipt_last() {
        let (files, receipt) = pdf_bundle(b"pdf".to_vec(), b"analysis\n".to_vec(), b"evidence\n".to_vec()).unwrap();
        assert_eq!(files.iter().map(|(path, _)| path.as_str()).collect::<Vec<_>>(),
            vec!["screenplay.pdf", "analysis.json", "provider-evidence.json", "bundle.json"]);
        assert_eq!(files[3].1, receipt);
        let report: serde_json::Value = serde_json::from_slice(&receipt).unwrap();
        assert_eq!(report["schema_version"], BUNDLE_SCHEMA);
        assert_eq!(report["runtime_qualified"], false);
        for (index, (path, bytes)) in files[..3].iter().enumerate() {
            assert_eq!(report["files"][index]["path"], *path);
            assert_eq!(report["files"][index]["sha256"], format!("{:x}", Sha256::digest(bytes)));
            assert_eq!(report["files"][index]["size_bytes"], bytes.len());
        }
    }

    #[test]
    fn input_and_config_readers_refuse_oversize_and_directories() {
        let root = tempfile::tempdir().unwrap();
        assert!(read_regular(root.path(), 4).is_err());
        let file = root.path().join("input");
        std::fs::write(&file, b"12345").unwrap();
        assert!(read_regular(&file, 4).is_err());
        assert_eq!(read_regular(&file, 5).unwrap(), b"12345");
        assert_eq!(std::fs::read(file).unwrap(), b"12345");
    }

    #[test]
    fn invalid_config_is_refused_without_provider_execution() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("config.json");
        std::fs::write(&file, b"{\"unexpected\":true}").unwrap();
        assert!(read_config(&file).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn input_and_config_symlinks_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("source");
        let link = root.path().join("link");
        std::fs::write(&file, b"original").unwrap();
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(read_regular(&link, 64).is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"original");
    }
}
