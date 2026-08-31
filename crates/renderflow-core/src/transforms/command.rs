use std::io::Write;

use anyhow::{Context, Result};

use super::Transform;
use crate::process::{
    is_explicit_shell_invocation, ProcessExpectedOutput, ProcessExecutor, ProcessInput,
    ProcessOutputMode, ProcessRequest, DEFAULT_CAPTURE_LIMIT_BYTES, DEFAULT_PROCESS_TIMEOUT,
};

/// A [`Transform`] that executes an external command to process document content.
///
/// All subprocess policy is delegated to Renderflow's canonical process
/// executor. The legacy transform still returns UTF-8 text; binary-native
/// command transforms belong on the artifact API introduced separately.
pub struct CommandTransform {
    name: String,
    program: String,
    args: Vec<String>,
}

impl CommandTransform {
    pub fn new(name: impl Into<String>, program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            program: program.into(),
            args,
        }
    }
}

impl Transform for CommandTransform {
    fn name(&self) -> &str {
        &self.name
    }

    fn apply(&self, input: String) -> Result<String> {
        let has_input_placeholder = self.args.iter().any(|arg| arg.contains("{input}"));
        let has_output_placeholder = self.args.iter().any(|arg| arg.contains("{output}"));

        let input_file = if has_input_placeholder {
            let mut file =
                tempfile::NamedTempFile::new().context("Failed to create input temp file")?;
            file.write_all(input.as_bytes())
                .context("Failed to write to input temp file")?;
            Some(file)
        } else {
            None
        };

        let output_file = if has_output_placeholder {
            Some(tempfile::NamedTempFile::new().context("Failed to create output temp file")?)
        } else {
            None
        };

        let processed_args: Vec<String> = self
            .args
            .iter()
            .map(|arg| {
                let mut processed = arg.clone();
                if let Some(ref file) = input_file {
                    processed = processed.replace("{input}", &file.path().to_string_lossy());
                }
                if let Some(ref file) = output_file {
                    processed = processed.replace("{output}", &file.path().to_string_lossy());
                }
                processed
            })
            .collect();

        let request = if is_explicit_shell_invocation(&self.program, &processed_args) {
            ProcessRequest::shell(&self.program)
        } else {
            ProcessRequest::direct(&self.program)
        }
        .args(processed_args)
        .stdin(if has_input_placeholder {
            ProcessInput::Null
        } else {
            ProcessInput::Bytes(input.into_bytes())
        })
        .stdout(if has_output_placeholder {
            ProcessOutputMode::Null
        } else {
            ProcessOutputMode::capture(DEFAULT_CAPTURE_LIMIT_BYTES)
        })
        .stderr(ProcessOutputMode::capture(DEFAULT_CAPTURE_LIMIT_BYTES))
        .timeout(DEFAULT_PROCESS_TIMEOUT);

        let request = if let Some(ref file) = output_file {
            request.expect_output(ProcessExpectedOutput::file(file.path()).require_change())
        } else {
            request
        };

        let result = ProcessExecutor::new()
            .execute_checked(request)
            .with_context(|| format!("Command transform '{}' failed", self.name))?;

        if let Some(ref file) = output_file {
            std::fs::read_to_string(file.path()).with_context(|| {
                format!(
                    "Failed to read UTF-8 transform output file '{}'",
                    file.path().display()
                )
            })
        } else {
            String::from_utf8(result.stdout().bytes().to_vec())
                .context("Command stdout is not valid UTF-8")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_transform_name() {
        let transform = CommandTransform::new("my-transform", "cat", vec![]);
        assert_eq!(transform.name(), "my-transform");
    }

    #[cfg(unix)]
    #[test]
    fn test_pipe_based_passthrough_via_cat() {
        let transform = CommandTransform::new("cat-pass", "cat", vec![]);
        let result = transform.apply("hello world".to_string()).unwrap();
        assert_eq!(result, "hello world");
    }

    #[cfg(unix)]
    #[test]
    fn test_pipe_based_multiline_input() {
        let transform = CommandTransform::new("cat-multi", "cat", vec![]);
        let input = "line one\nline two\nline three".to_string();
        let result = transform.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[cfg(unix)]
    #[test]
    fn test_pipe_based_empty_input() {
        let transform = CommandTransform::new("cat-empty", "cat", vec![]);
        let result = transform.apply(String::new()).unwrap();
        assert_eq!(result, "");
    }

    #[cfg(unix)]
    #[test]
    fn test_file_based_input_placeholder() {
        let transform = CommandTransform::new("cat-file", "cat", vec!["{input}".to_string()]);
        let result = transform.apply("file content".to_string()).unwrap();
        assert_eq!(result, "file content");
    }

    #[cfg(unix)]
    #[test]
    fn test_file_based_output_placeholder() {
        let transform = CommandTransform::new(
            "echo-to-file",
            "sh",
            vec!["-c".to_string(), "printf '%s' hello > {output}".to_string()],
        );
        let result = transform.apply(String::new()).unwrap();
        assert_eq!(result, "hello");
    }

    #[cfg(unix)]
    #[test]
    fn test_both_placeholders_copy_input_to_output() {
        let transform = CommandTransform::new(
            "cp-transform",
            "cp",
            vec!["{input}".to_string(), "{output}".to_string()],
        );
        let result = transform.apply("copied content".to_string()).unwrap();
        assert_eq!(result, "copied content");
    }

    #[test]
    fn test_nonexistent_program_returns_error() {
        let transform =
            CommandTransform::new("bad-program", "__nonexistent_program_renderflow__", vec![]);
        let error = transform.apply("input".to_string()).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("was not found") || message.contains("Command transform"),
            "unexpected error: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_nonzero_exit_code_returns_error() {
        let transform = CommandTransform::new("false-cmd", "false", vec![]);
        let error = transform.apply("input".to_string()).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("exited with code") || message.contains("Command transform"),
            "unexpected error: {message}"
        );
    }
}
