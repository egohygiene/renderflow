//! CLI adaptation; the publication SDK owns input capture, planning and packaging.

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use crate::cli::BriefingArguments;
use crate::process::ProcessCancellationToken;
use crate::publication::briefing::{self, BriefingRequest};

pub fn plan(input: &str) -> Result<()> {
    emit(&serde_json::to_value(briefing::plan(Path::new(input), cancellation()?)?)?)
}

pub fn build(arguments: BriefingArguments) -> Result<()> {
    emit(&briefing::build(&BriefingRequest {input:arguments.input.into(),
        output_dir:arguments.output_dir.into(),allow_draft:arguments.allow_draft},cancellation()?)?)
}

fn cancellation() -> Result<ProcessCancellationToken> {
    let token = ProcessCancellationToken::new();
    let signal = token.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    Ok(token)
}

fn emit(value: &serde_json::Value) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}
