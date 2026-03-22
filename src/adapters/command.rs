use anyhow::{bail, Result};
use std::process::Command;
use tracing::{error, info};

pub fn run_command(program: &str, args: &[&str]) -> Result<()> {
    info!(program = program, args = ?args, "Running command");

    let output = Command::new(program).args(args).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stdout.is_empty() {
        info!(stdout = %stdout.trim_end(), "Command stdout");
    }

    if !stderr.is_empty() {
        if output.status.success() {
            info!(stderr = %stderr.trim_end(), "Command stderr");
        } else {
            error!(stderr = %stderr.trim_end(), "Command stderr");
        }
    }

    if !output.status.success() {
        match output.status.code() {
            Some(code) => {
                error!(program = program, exit_code = code, "Command failed");
                bail!("Command `{}` failed with exit code {}", program, code);
            }
            None => {
                error!(program = program, "Command terminated by signal");
                bail!("Command `{}` was terminated by a signal", program);
            }
        }
    }

    info!(program = program, "Command completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success() {
        let result = run_command("echo", &["hello"]);
        assert!(result.is_ok(), "echo should succeed");
    }

    #[test]
    fn test_with_multiple_args() {
        let result = run_command("echo", &["hello", "world"]);
        assert!(result.is_ok(), "echo with multiple args should succeed");
    }

    #[test]
    fn test_failure() {
        let result = run_command("false", &[]);
        assert!(result.is_err(), "false should fail");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed with exit code"), "error message should mention exit code");
    }

    #[test]
    fn test_nonexistent_program() {
        let result = run_command("__nonexistent_program__", &[]);
        assert!(result.is_err(), "nonexistent program should return an error");
    }
}
