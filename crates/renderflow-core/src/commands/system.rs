use anyhow::{bail, Result};
use std::{env, io::Write, path::PathBuf};

use crate::cli::DoctorReportFormat;
use crate::creative_doctor::{CreativeDoctorReport, CREATIVE_DISCOVERY_ONLY_TOOL_IDS};
use crate::toolchain::{ToolRegistry, ToolSupportTier};

pub fn run_version() {
    println!("renderflow {}", env!("CARGO_PKG_VERSION"));
}

pub fn run_env() {
    let exe = env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("<unknown>"))
        .display()
        .to_string();
    let path = env::var("PATH").unwrap_or_else(|_| "<unset>".to_string());

    println!("renderflow {}", env!("CARGO_PKG_VERSION"));
    println!("os={}", env::consts::OS);
    println!("arch={}", env::consts::ARCH);
    println!("executable={exe}");
    println!("path={path}");
}

pub fn run_doctor(strict: bool) -> Result<()> {
    let registry = ToolRegistry::builtins();
    let inventory = registry.assess_ids_current(
        registry.all()
            .filter(|tool| !CREATIVE_DISCOVERY_ONLY_TOOL_IDS.contains(&tool.id.as_str()))
            .map(|tool| tool.id.as_str()),
    );

    println!("Renderflow Doctor");
    println!("-----------------");
    println!("renderflow: {}", env!("CARGO_PKG_VERSION"));
    println!("platform: {} {}", env::consts::OS, env::consts::ARCH);
    println!("tool registry: {}", registry.schema());
    println!();

    let mut required_failures = 0usize;
    for tool in &inventory.tools {
        if tool.support_tier == ToolSupportTier::Required && !tool.is_available() {
            required_failures += 1;
        }
        let detail = tool
            .version_line
            .as_deref()
            .or(tool.diagnostic.as_deref())
            .unwrap_or("no additional detail");
        println!(
            "[{}|{}] {}: {}",
            tool.status.as_str(),
            tool.support_tier.as_str(),
            tool.id,
            detail
        );
    }

    if strict && required_failures > 0 {
        bail!("doctor found {required_failures} required toolchain issue(s)");
    }

    if required_failures == 0 {
        println!(
            "
Doctor completed: required toolchain providers look healthy."
        );
    } else {
        println!(
            "
Doctor completed with required-provider warnings."
        );
    }
    Ok(())
}

/// Inspect the explicitly selected optional profile. This never installs tools,
/// observes model/source bytes, or changes planner/provider configuration.
pub fn run_creative_doctor(strict: bool, format: DoctorReportFormat) -> Result<()> {
    let report = crate::creative_doctor::assess_current()?;
    let stdout = std::io::stdout();
    write_creative_doctor_report(&report, strict, format, &mut stdout.lock())
}

fn write_creative_doctor_report(
    report: &CreativeDoctorReport,
    strict: bool,
    format: DoctorReportFormat,
    output: &mut impl Write,
) -> Result<()> {
    match format {
        DoctorReportFormat::Json => {
            serde_json::to_writer_pretty(&mut *output, report)?;
            writeln!(output)?;
        }
        DoctorReportFormat::Text => {
            writeln!(output, "Renderflow Doctor: {}", report.profile)?;
            writeln!(output, "platform: {} {}", report.operating_system, report.architecture)?;
            writeln!(output, "tool registry: {}", report.registry_schema)?;
            writeln!(output, "fingerprint: {}", report.fingerprint)?;
            writeln!(output, "fingerprint scope: {}", report.fingerprint_scope)?;
            writeln!(output, "\nTool discovery (does not enable conversion routes):")?;
            for tool in &report.tools {
                let availability = &tool.availability;
                writeln!(output, "[{}] {}", availability.status.as_str(), availability.id)?;
                if let Some(executable) = &availability.selected_executable {
                    writeln!(output, "  executable: {executable}")?;
                }
                if let Some(version) = &availability.version_line {
                    writeln!(output, "  observed version: {version}")?;
                }
                if let Some(diagnostic) = &availability.diagnostic {
                    writeln!(output, "  diagnostic: {diagnostic}")?;
                }
                if let Some(descriptor) = &tool.descriptor {
                    let capabilities = descriptor.capabilities.iter()
                        .map(ToString::to_string).collect::<Vec<_>>().join(", ");
                    writeln!(output, "  declared capabilities: {capabilities}")?;
                    if let Some(notes) = &descriptor.license_notes {
                        writeln!(output, "  executable licensing: {notes}")?;
                    }
                    if let Some(notes) = &descriptor.distribution_notes {
                        writeln!(output, "  distribution: {notes}")?;
                    }
                }
            }
            writeln!(output, "\nRoute readiness:")?;
            for route in &report.routes {
                writeln!(output, "[{}|providers:{}] {}", route.implementation.as_str(),
                    route.provider_status.as_str(), route.id)?;
                writeln!(output, "  {}", route.detail)?;
                if !route.providers.is_empty() {
                    writeln!(output, "  providers: {}", route.providers.join(", "))?;
                }
                if !route.blocked_providers.is_empty() {
                    writeln!(output, "  unavailable or unobserved providers: {}", route.blocked_providers.join(", "))?;
                }
                if !route.required_configuration.is_empty() {
                    writeln!(output, "  required configuration: {}", route.required_configuration.join(", "))?;
                }
            }
            writeln!(output, "\nAdditional requirements:")?;
            for requirement in &report.requirements {
                writeln!(output, "[{}] {}: {}", requirement.status, requirement.id, requirement.detail)?;
            }
            writeln!(output, "\nScope and limitations:")?;
            for limitation in &report.limitations {
                writeln!(output, "- {limitation}")?;
            }
            writeln!(output, "\nFailed supported discovery probes: {}", report.unavailable_tool_count())?;
            writeln!(output, "Unprobed profile tools: {}", report.unprobed_tool_count())?;
            writeln!(output, "Strict mode covers supported discovery probes only; unprobed tools do not fail strict mode. It does not qualify adapters, models, fonts, or complete workflows.")?;
        }
    }
    // Preserve the entire JSON/text report even when strict discovery fails.
    output.flush()?;
    if strict && !report.strict_success() {
        bail!("creative-publishing doctor found {} failed supported discovery probe(s); the full report includes {} unprobed tool(s)", report.unavailable_tool_count(), report.unprobed_tool_count());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{
        ProcessPlatform, ProcessTreeTermination, ToolProbeEvidence, ToolProbeStatus,
    };
    use crate::toolchain::{ToolProbe, ToolRuntimeContext};

    struct MissingProbe;

    impl ToolProbe for MissingProbe {
        fn probe(&self, executable: &str, _version_args: &[String]) -> ToolProbeEvidence {
            ToolProbeEvidence {
                executable: executable.to_owned(),
                status: ToolProbeStatus::Missing,
                version_line: None,
                duration_ms: 0,
                platform: ProcessPlatform {
                    os: "linux", arch: "x86_64",
                    tree_termination: ProcessTreeTermination::DirectChild,
                },
                diagnostic: Some("synthetic missing executable".to_owned()),
            }
        }
    }

    fn missing_report() -> CreativeDoctorReport {
        crate::creative_doctor::assess_with(
            &ToolRegistry::builtins(), &MissingProbe,
            &ToolRuntimeContext::for_platform("linux", "x86_64"),
        ).unwrap()
    }

    #[test]
    fn strict_failure_preserves_complete_machine_readable_report() {
        let report = missing_report();
        let mut output = Vec::new();
        let result = write_creative_doctor_report(
            &report, true, DoctorReportFormat::Json, &mut output,
        );
        assert!(result.is_err());
        let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value, serde_json::to_value(&report).unwrap());
        assert!(!value["routes"].as_array().unwrap().is_empty());
        assert!(!value["requirements"].as_array().unwrap().is_empty());
        assert_eq!(output.last(), Some(&b'\n'));
    }

    #[test]
    fn optional_profile_warnings_succeed_without_strict() {
        let mut output = Vec::new();
        write_creative_doctor_report(
            &missing_report(), false, DoctorReportFormat::Json, &mut output,
        ).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value["profile"], "creative-publishing");
    }

    #[test]
    fn text_report_preserves_tool_failures_and_route_limitations() {
        let report = missing_report();
        let mut output = Vec::new();
        assert!(write_creative_doctor_report(
            &report, true, DoctorReportFormat::Text, &mut output,
        ).is_err());
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("missing_executable"));
        assert!(text.contains("Route readiness:"));
        assert!(text.contains("not_implemented"));
        assert!(text.contains("Strict mode covers supported discovery probes only"));
        assert!(text.contains("Unprobed profile tools: 1"));
        for route in &report.routes {
            assert!(text.contains(&route.id));
            assert!(text.contains(&route.detail));
        }
        for requirement in &report.requirements {
            assert!(text.contains(&requirement.id));
            assert!(text.contains(&requirement.detail));
        }
    }
}
