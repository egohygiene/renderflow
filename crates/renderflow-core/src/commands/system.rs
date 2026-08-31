use anyhow::{bail, Result};
use std::{env, path::PathBuf};

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
    let inventory = registry.assess_all_current();

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
