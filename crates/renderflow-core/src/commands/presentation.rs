//! CLI adaptation only; the public presentation SDK owns content and execution.

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::cli::PresentationArguments;
use crate::presentation::{self, PresentationRequest, PresentationTarget};
use crate::process::ProcessCancellationToken;

pub fn fingerprint(installation_root: &str) -> Result<()> {
    let fingerprint = crate::screenplay::afterwriting::package_fingerprint(
        Path::new(installation_root),
        ProcessCancellationToken::new(),
    )?;
    emit(&serde_json::to_value(fingerprint)?)
}

pub fn inspect(input: &str) -> Result<()> {
    let bytes = presentation::read_file(Path::new(input), presentation::MAX_INPUT_BYTES)?;
    let model = presentation::parse(&bytes)?;
    emit(
        &json!({"schema_version":"renderflow.presentation-inspection/v1",
        "input_sha256":presentation::sha256(&bytes),"presentation":model,
        "review_is_declared":true,"provider_executed":false,"runtime_qualified":false}),
    )
}

pub fn run(arguments: PresentationArguments, execute: bool) -> Result<()> {
    let provider = arguments
        .provider
        .map(|path| {
            presentation::slidev::parse_provider(&presentation::read_file(
                Path::new(&path),
                64 * 1024,
            )?)
        })
        .transpose()?;
    let request = PresentationRequest {
        input: arguments.input.into(),
        output_dir: arguments.output_dir.into(),
        provider,
        targets: arguments
            .target
            .iter()
            .map(|name| match name.as_str() {
                "source" => Ok(PresentationTarget::Source),
                "web" => Ok(PresentationTarget::Web),
                "pdf" => Ok(PresentationTarget::Pdf),
                _ => anyhow::bail!("presentation.target: unsupported target {name}"),
            })
            .collect::<Result<Vec<_>>>()?,
        include_speaker_notes: arguments.include_speaker_notes,
        allow_unqualified: arguments.allow_unqualified,
    };
    if execute {
        let cancellation = ProcessCancellationToken::new();
        let signal = cancellation.clone();
        ctrlc::set_handler(move || signal.cancel())?;
        emit(&presentation::build(&request, cancellation)?)
    } else {
        emit(&serde_json::to_value(presentation::plan(&request)?)?)
    }
}

fn emit(value: &serde_json::Value) -> Result<()> {
    let mut output = serde_json::to_vec_pretty(value)?;
    output.push(b'\n');
    std::io::stdout().write_all(&output)?;
    Ok(())
}
