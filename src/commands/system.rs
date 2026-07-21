use anyhow::{bail, Result};
use std::{env, path::PathBuf, process::Command};

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
    Command::new(name)
        .arg("--version")
        .output()
        .map_err(|_| format!("missing ({name} not found in PATH)"))
        .and_then(|output| {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let line = stdout.lines().next().or_else(|| stderr.lines().next());
                Ok(line.unwrap_or("available").trim().to_string())
            } else {
                Err(format!("installed but failed to execute ({name} --version)"))
            }
        })
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
