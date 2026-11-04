//! CLI adaptation over shared native bundle evidence services.

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use crate::cli::BundleVerifyArguments;
use crate::evidence::verification::{PinnedRunReceipt,VerificationLimits};
use crate::process::ProcessCancellationToken;

pub fn verify(arguments:BundleVerifyArguments)->Result<()> {
    let limits=VerificationLimits {max_file_bytes:arguments.maximum_file_bytes,
        max_total_bytes:arguments.maximum_total_bytes,max_publications:arguments.maximum_publications,
        timeout_seconds:arguments.timeout_seconds,..VerificationLimits::default()};
    let cancellation=ProcessCancellationToken::new();
    let signal=cancellation.clone();
    ctrlc::set_handler(move||signal.cancel())?;
    let receipt=PinnedRunReceipt::read(Path::new(&arguments.manifest),&arguments.manifest_sha256,&limits,cancellation.clone())?;
    let result=receipt.verify(Path::new(&arguments.bundle_root),&limits,cancellation)?;
    let mut bytes=serde_json::to_vec_pretty(&result)?;
    bytes.push(b'\n');
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}
