use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::publication::lulu::{
    evaluate_request, LuluConformanceReport, LuluEligibility, LuluRulePack,
};

pub fn run_lulu_rules(format: &str, output: Option<&str>) -> Result<()> {
    emit(&LuluRulePack::builtin()?, format, output)
}

pub fn run_lulu_preflight(
    request: &str,
    format: &str,
    output: Option<&str>,
    epubcheck: bool,
) -> Result<()> {
    let report = evaluate_request(Path::new(request), epubcheck)?;
    emit_report(&report, format, output)?;
    if report.channels.iter().any(|channel| {
        matches!(
            channel.eligibility,
            LuluEligibility::Ineligible | LuluEligibility::Unknown
        )
    }) {
        anyhow::bail!("one or more requested Lulu channels are not upload-ready; see report")
    }
    Ok(())
}

fn emit_report(report: &LuluConformanceReport, format: &str, output: Option<&str>) -> Result<()> {
    if format.eq_ignore_ascii_case("text") {
        let mut text = format!(
            "Lulu candidate preflight\nRule pack: {} (observed {})\nUpload performed: no\n\nChannels:\n",
            report.rule_pack, report.observed_on
        );
        for channel in &report.channels {
            text.push_str(&format!(
                "  {:?}: {:?}\n",
                channel.channel, channel.eligibility
            ));
            for code in &channel.blocking_findings {
                text.push_str(&format!("    BLOCK: {code}\n"));
            }
            for code in &channel.warning_findings {
                text.push_str(&format!("    WARN: {code}\n"));
            }
        }
        text.push_str("\nFindings:\n");
        for finding in &report.findings {
            text.push_str(&format!(
                "  [{:?}] {}: {}\n",
                finding.severity, finding.code, finding.message
            ));
        }
        write(&text, output)
    } else {
        emit(report, format, output)
    }
}

fn emit<T: Serialize>(value: &T, format: &str, output: Option<&str>) -> Result<()> {
    let content = match format.to_ascii_lowercase().as_str() {
        "json" => format!("{}\n", serde_json::to_string_pretty(value)?),
        "yaml" | "yml" | "text" => serde_yaml_ng::to_string(value)?,
        other => anyhow::bail!(
            "unknown publication output format '{other}'; supported: text, json, yaml"
        ),
    };
    write(&content, output)
}

fn write(content: &str, output: Option<&str>) -> Result<()> {
    if let Some(path) = output {
        fs::write(path, content)
            .with_context(|| format!("failed to write publication report '{path}'"))?;
    } else {
        print!("{content}");
    }
    Ok(())
}
