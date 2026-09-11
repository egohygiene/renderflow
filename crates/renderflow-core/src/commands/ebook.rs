use std::path::Path;

use anyhow::Result;

use crate::ebook::{inspect_ebook, EbookCapabilityContract};

pub fn run_inspect(input: &str, format: &str, epubcheck: bool) -> Result<()> {
    let inspection = inspect_ebook(Path::new(input), epubcheck)?;
    emit(&inspection, format)?;
    if !inspection.valid
        || inspection.epubcheck.as_ref().and_then(|item| item.passed) == Some(false)
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
