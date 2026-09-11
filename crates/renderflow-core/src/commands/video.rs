use anyhow::Result;

use crate::sdk::{CancellationToken, ProgressEvent, ProgressReporter};
use crate::video::{
    execute_handbrake_with_progress, plan_handbrake, HandBrakeCapabilityContract, HandBrakeLimits,
    HandBrakePreset, HandBrakeTransformRequest,
};

pub fn run_capabilities(format: &str) -> Result<()> {
    emit(&HandBrakeCapabilityContract::builtin(), format)
}

pub fn run_plan(
    input: &str,
    output: &str,
    preset: HandBrakePreset,
    limits: HandBrakeLimits,
    format: &str,
) -> Result<()> {
    let request = HandBrakeTransformRequest::new(input, output, preset).with_limits(limits);
    emit(&plan_handbrake(&request)?, format)
}

pub fn run_transcode(
    input: &str,
    output: &str,
    preset: HandBrakePreset,
    limits: HandBrakeLimits,
    format: &str,
) -> Result<()> {
    let cancellation = CancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    let request = HandBrakeTransformRequest::new(input, output, preset).with_limits(limits);
    let reporter = CliVideoProgress;
    let report = execute_handbrake_with_progress(&request, &cancellation, Some(&reporter))?;
    emit(&report, format)
}

struct CliVideoProgress;

impl ProgressReporter for CliVideoProgress {
    fn on_event(&self, event: &ProgressEvent) {
        eprintln!("{}", event.message);
    }
}

fn emit<T: serde::Serialize>(value: &T, format: &str) -> Result<()> {
    match format.to_ascii_lowercase().as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(value)?),
        "yaml" | "yml" => print!("{}", serde_yaml_ng::to_string(value)?),
        "text" => print!("{}", serde_yaml_ng::to_string(value)?),
        other => {
            anyhow::bail!("unknown video output format '{other}'; supported: text, json, yaml")
        }
    }
    Ok(())
}
