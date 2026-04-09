use std::io::Write;
use std::process::Stdio;

use anyhow::{Context, Result};

use super::Transform;

/// A [`Transform`] that executes an external command to process document content.
///
/// `CommandTransform` supports two modes of operation depending on which
/// placeholders appear in `args`:
///
/// * **File-based** – if `{input}` appears in any argument it is replaced with
///   the path of a temporary file that contains the input string.  If `{output}`
///   appears it is replaced with the path of a (initially empty) temporary
///   output file; the file's content is read after the command exits and
///   returned as the transform result.
///
/// * **Pipe-based** – when neither `{input}` nor `{output}` is present the
///   input string is written to the command's `stdin` and the transform result
///   is read from `stdout`.
///
/// The two modes may be mixed: `{input}` with no `{output}` reads the result
/// from `stdout`; `{output}` with no `{input}` still pipes input via `stdin`.
///
/// The command must exit with status 0; a non-zero exit code causes
/// [`Transform::apply`] to return an error containing the program name, exit
/// status, and any output written to `stderr`.
pub struct CommandTransform {
    /// Human-readable name used in log messages and error context.
    name: String,
    /// External binary to invoke (looked up on `PATH`).
    program: String,
    /// Arguments passed to the program; may contain `{input}` and `{output}`
    /// placeholder strings.
    args: Vec<String>,
}

impl CommandTransform {
    /// Create a new `CommandTransform`.
    ///
    /// # Parameters
    ///
    /// * `name`    – human-readable identifier for log messages and errors.
    /// * `program` – external binary to invoke (resolved via `PATH`).
    /// * `args`    – command-line arguments; may contain `{input}` and
    ///   `{output}` placeholders that are replaced with temporary file paths.
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
        let has_input_placeholder = self.args.iter().any(|a| a.contains("{input}"));
        let has_output_placeholder = self.args.iter().any(|a| a.contains("{output}"));

        // Write input to a temp file when the {input} placeholder is used.
        let input_file = if has_input_placeholder {
            let mut f = tempfile::NamedTempFile::new().context("Failed to create input temp file")?;
            f.write_all(input.as_bytes()).context("Failed to write to input temp file")?;
            Some(f)
        } else {
            None
        };

        // Create an (empty) temp file when the {output} placeholder is used.
        let output_file = if has_output_placeholder {
            Some(tempfile::NamedTempFile::new().context("Failed to create output temp file")?)
        } else {
            None
        };

        // Replace placeholders in each argument.
        let processed_args: Vec<String> = self
            .args
            .iter()
            .map(|arg| {
                let mut a = arg.clone();
                if let Some(ref f) = input_file {
                    a = a.replace("{input}", &f.path().to_string_lossy());
                }
                if let Some(ref f) = output_file {
                    a = a.replace("{output}", &f.path().to_string_lossy());
                }
                a
            })
            .collect();

        // When reading from a file the command doesn't need stdin.
        let stdin_mode = if has_input_placeholder { Stdio::null() } else { Stdio::piped() };
        // When writing to a file the command's stdout is irrelevant.
        let stdout_mode = if has_output_placeholder { Stdio::null() } else { Stdio::piped() };

        let mut child = std::process::Command::new(&self.program)
            .args(&processed_args)
            .stdin(stdin_mode)
            .stdout(stdout_mode)
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to start program '{}'", self.program))?;

        // Pipe input via stdin when the {input} placeholder is not used.
        if !has_input_placeholder {
            if let Some(mut stdin_handle) = child.stdin.take() {
                stdin_handle
                    .write_all(input.as_bytes())
                    .context("Failed to write to command stdin")?;
                // Drop `stdin_handle` here to close stdin before `wait_with_output`.
            }
        }

        let cmd_output = child
            .wait_with_output()
            .with_context(|| format!("Failed to wait for program '{}'", self.program))?;

        if !cmd_output.status.success() {
            let stderr = String::from_utf8_lossy(&cmd_output.stderr);
            anyhow::bail!(
                "Command '{}' exited with status {}: {}",
                self.program,
                cmd_output.status,
                stderr.trim()
            );
        }

        // Read result from the output file when the {output} placeholder was used,
        // otherwise parse stdout as UTF-8.
        if let Some(ref f) = output_file {
            std::fs::read_to_string(f.path())
                .with_context(|| format!("Failed to read output file '{}'", f.path().display()))
        } else {
            String::from_utf8(cmd_output.stdout).context("Command stdout is not valid UTF-8")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── pipe-based (stdin / stdout) ───────────────────────────────────────────

    #[test]
    fn test_command_transform_name() {
        let t = CommandTransform::new("my-transform", "cat", vec![]);
        assert_eq!(t.name(), "my-transform");
    }

    #[test]
    fn test_pipe_based_passthrough_via_cat() {
        // `cat` with no arguments echoes stdin to stdout.
        let t = CommandTransform::new("cat-pass", "cat", vec![]);
        let result = t.apply("hello world".to_string()).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_pipe_based_multiline_input() {
        let t = CommandTransform::new("cat-multi", "cat", vec![]);
        let input = "line one\nline two\nline three".to_string();
        let result = t.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_pipe_based_empty_input() {
        let t = CommandTransform::new("cat-empty", "cat", vec![]);
        let result = t.apply(String::new()).unwrap();
        assert_eq!(result, "");
    }

    // ── file-based ({input} placeholder) ─────────────────────────────────────

    #[test]
    fn test_file_based_input_placeholder() {
        // `cat {input}` reads the input from a temp file.
        let t = CommandTransform::new("cat-file", "cat", vec!["{input}".to_string()]);
        let result = t.apply("file content".to_string()).unwrap();
        assert_eq!(result, "file content");
    }

    // ── file-based ({output} placeholder) ────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn test_file_based_output_placeholder() {
        // `sh -c "echo hello > {output}"` writes to the output temp file.
        let t = CommandTransform::new(
            "echo-to-file",
            "sh",
            vec!["-c".to_string(), "printf '%s' hello > {output}".to_string()],
        );
        let result = t.apply(String::new()).unwrap();
        assert_eq!(result, "hello");
    }

    // ── both placeholders ─────────────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn test_both_placeholders_copy_input_to_output() {
        // `cp {input} {output}` copies the input temp file to the output temp file.
        let t = CommandTransform::new(
            "cp-transform",
            "cp",
            vec!["{input}".to_string(), "{output}".to_string()],
        );
        let result = t.apply("copied content".to_string()).unwrap();
        assert_eq!(result, "copied content");
    }

    // ── error handling ────────────────────────────────────────────────────────

    #[test]
    fn test_nonexistent_program_returns_error() {
        let t = CommandTransform::new(
            "bad-program",
            "__nonexistent_program_renderflow__",
            vec![],
        );
        let err = t.apply("input".to_string()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Failed to start program"),
            "expected 'Failed to start program' in: {msg}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_nonzero_exit_code_returns_error() {
        // `false` always exits with a non-zero status.
        let t = CommandTransform::new("false-cmd", "false", vec![]);
        let err = t.apply("input".to_string()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("exited with status"),
            "expected 'exited with status' in: {msg}"
        );
    }
}
