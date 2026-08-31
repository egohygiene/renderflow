use anyhow::{bail, Result};
use std::{env, path::PathBuf};

use crate::process::{ProcessExecutor, ToolProbeStatus};

struct ToolCheck {
    name: &'static str,
    required: bool,
}

// `pandoc` is required for core document rendering, while `tectonic` (PDF)
// and `ffmpeg` (media conversions) are optional unless those outputs are used.
const TOOL_CHECKS: [ToolCheck; 3] = [
    ToolCheck {
        name: "pandoc",
        required: true,
    },
    ToolCheck {
        name: "tectonic",
        required: false,
    },
    ToolCheck {
        name: "ffmpeg",
        required: false,
    },
];

fn probe_tool_version(name: &str) -> Result<String, String> {
    let probe = ProcessExecutor::new().probe_version(name);
    match probe.status {
        ToolProbeStatus::Available => Ok(probe
            .version_line
            .unwrap_or_else(|| "available".to_string())),
        ToolProbeStatus::Missing => Err(format!("missing ({name} not found in PATH)")),
        ToolProbeStatus::TimedOut => Err(format!("installed but version probe timed out ({name} --version)")),
        ToolProbeStatus::Failed => Err(probe
            .diagnostic
            .unwrap_or_else(|| format!("installed but failed to execute ({name} --version)"))),
    }
}

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
    println!("Renderflow Doctor");
    println!("-----------------");
    println!("renderflow: {}", env!("CARGO_PKG_VERSION"));
    println!("platform: {} {}", env::consts::OS, env::consts::ARCH);

    let mut missing = 0usize;

    for check in TOOL_CHECKS {
        match probe_tool_version(check.name) {
            Ok(version) => println!("[ok] {}: {version}", check.name),
            Err(reason) => {
                if check.required {
                    missing += 1;
                    println!("[missing|required] {}: {reason}", check.name);
                } else {
                    println!("[missing|optional] {}: {reason}", check.name);
                }
            }
        }
    }

    if strict && missing > 0 {
        bail!("doctor found {missing} required dependency issue(s)");
    }

    if missing == 0 {
        println!("Doctor completed: required dependencies look healthy.");
    } else {
        println!("Doctor completed with warnings. Install missing required tools and retry.");
    }

    Ok(())
}
