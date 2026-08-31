use anyhow::Result;
use tracing::{error, info};

use crate::process::{
    is_explicit_shell_invocation, ProcessExecutor, ProcessRequest, DEFAULT_CAPTURE_LIMIT_BYTES,
    DEFAULT_PROCESS_TIMEOUT,
};

/// Run one external command through Renderflow's canonical bounded process executor.
///
/// `program` is executed directly with the provided argv. If the caller
/// explicitly names a shell program and supplies a shell-evaluation flag such as
/// `-c`, the request is classified as an explicit shell invocation so the wider
/// trust boundary remains visible in process diagnostics.
pub fn run_command(program: &str, args: &[&str]) -> Result<()> {
    let owned_args: Vec<String> = args.iter().map(|value| (*value).to_string()).collect();
    let request = if is_explicit_shell_invocation(program, &owned_args) {
        ProcessRequest::shell(program)
    } else {
        ProcessRequest::direct(program)
    }
    .args(owned_args)
    .timeout(DEFAULT_PROCESS_TIMEOUT)
    .capture_limit(DEFAULT_CAPTURE_LIMIT_BYTES);

    let result = ProcessExecutor::new().execute(request)?;

    if !result.stdout().redacted_text().is_empty() {
        info!(
            stdout = %result.stdout().redacted_text().trim_end(),
            truncated = result.stdout().truncated(),
            "Command stdout"
        );
    }

    if !result.stderr().redacted_text().is_empty() {
        if result.is_success() {
            info!(
                stderr = %result.stderr().redacted_text().trim_end(),
                truncated = result.stderr().truncated(),
                "Command stderr"
            );
        } else {
            error!(
                stderr = %result.stderr().redacted_text().trim_end(),
                truncated = result.stderr().truncated(),
                "Command stderr"
            );
        }
    }

    result.ensure_success()?;
    info!(program = program, duration_ms = result.duration_ms(), "Command completed successfully");
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

    #[cfg(unix)]
    #[test]
    fn test_failure() {
        let result = run_command("false", &[]);
        assert!(result.is_err(), "false should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exited with code"),
            "error message should mention exit code"
        );
    }

    #[test]
    fn test_nonexistent_program() {
        let result = run_command("__nonexistent_program__", &[]);
        assert!(
            result.is_err(),
            "nonexistent program should return an error"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("was not found") && err.contains("PATH"),
            "error should mention the program was not found and how to fix it: {}",
            err
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_failure_error_includes_stderr() {
        let result = run_command("sh", &["-c", "printf '%s' 'some error output' >&2; exit 1"]);
        assert!(result.is_err(), "command should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("some error output"),
            "error message should include stderr output: {}",
            err
        );
    }
}
