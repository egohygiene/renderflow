//! CLI adaptation over shared native bundle evidence services.

use std::io::Write;
use std::path::Path;

use crate::cli::{
    BundleExportArguments, BundleIoArguments, BundlePortableVerifyArguments, BundleVerifyArguments,
};
use crate::evidence::verification::{PinnedRunReceipt, VerificationLimits};
use crate::process::ProcessCancellationToken;
use anyhow::Result;

pub fn verify(arguments: BundleVerifyArguments) -> Result<()> {
    let limits = limits(&arguments.limits);
    let cancellation = cancellation()?;
    let receipt = PinnedRunReceipt::read(
        Path::new(&arguments.manifest),
        &arguments.manifest_sha256,
        &limits,
        cancellation.clone(),
    )?;
    let result = receipt.verify(Path::new(&arguments.bundle_root), &limits, cancellation)?;
    emit(&result)
}

pub fn export(arguments: BundleExportArguments) -> Result<()> {
    let limits = limits(&arguments.input.limits);
    let cancellation = cancellation()?;
    let receipt = PinnedRunReceipt::read(
        Path::new(&arguments.input.manifest),
        &arguments.input.manifest_sha256,
        &limits,
        cancellation.clone(),
    )?;
    let options = crate::evidence::portable::ExportOptions {
        include_sources: arguments.include_sources,
        max_records: arguments.maximum_records,
    };
    let result = crate::evidence::portable::export(
        &receipt,
        Path::new(&arguments.input.bundle_root),
        arguments.artifact_store_root.as_deref().map(Path::new),
        Path::new(&arguments.output_dir),
        &options,
        &limits,
        cancellation,
    )?;
    emit(&result)
}

pub fn verify_portable(arguments: BundlePortableVerifyArguments) -> Result<()> {
    let limits = limits(&arguments.limits);
    let result = crate::evidence::portable::verify(
        Path::new(&arguments.bundle_root),
        &arguments.index_sha256,
        arguments.maximum_records,
        &limits,
        cancellation()?,
    )?;
    emit(&result.report()?)
}

fn limits(arguments: &BundleIoArguments) -> VerificationLimits {
    VerificationLimits {
        max_file_bytes: arguments.maximum_file_bytes,
        max_total_bytes: arguments.maximum_total_bytes,
        max_publications: arguments.maximum_publications,
        timeout_seconds: arguments.timeout_seconds,
        ..VerificationLimits::default()
    }
}

fn cancellation() -> Result<ProcessCancellationToken> {
    let cancellation = ProcessCancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    Ok(cancellation)
}

fn emit(value: &serde_json::Value) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}
