use anyhow::Result;
use std::process::Command;

/// Check whether a tool is available in the system PATH.
fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Verify that `pandoc` is installed and available in PATH.
///
/// Returns an error with a clear install instruction if it is not found.
pub fn check_pandoc() -> Result<()> {
    if !tool_available("pandoc") {
        anyhow::bail!(
            "Pandoc not found. Please install pandoc to continue.\n\
             See: https://pandoc.org/installing.html"
        );
    }
    Ok(())
}

/// Verify that `tectonic` is installed and available in PATH.
///
/// Returns an error with a clear install instruction if it is not found.
pub fn check_tectonic() -> Result<()> {
    if !tool_available("tectonic") {
        anyhow::bail!(
            "Tectonic not found. Please install tectonic to continue.\n\
             See: https://tectonic-typesetting.github.io/en-US/"
        );
    }
    Ok(())
}

/// Validate all required system dependencies before the pipeline runs.
///
/// * `pandoc` is always required.
/// * `tectonic` is required only when PDF output is requested.
pub fn validate_dependencies(pdf_requested: bool) -> Result<()> {
    check_pandoc()?;
    if pdf_requested {
        check_tectonic()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pandoc_available() -> bool {
        tool_available("pandoc")
    }

    fn tectonic_is_available() -> bool {
        tool_available("tectonic")
    }

    #[test]
    fn test_check_pandoc_result_matches_availability() {
        let result = check_pandoc();
        if pandoc_available() {
            assert!(result.is_ok(), "check_pandoc should succeed when pandoc is installed");
        } else {
            assert!(result.is_err(), "check_pandoc should fail when pandoc is not installed");
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("Pandoc not found"),
                "error should say 'Pandoc not found', got: {msg}"
            );
            assert!(
                msg.contains("pandoc"),
                "error should mention pandoc, got: {msg}"
            );
        }
    }

    #[test]
    fn test_check_pandoc_error_contains_install_hint() {
        let result = check_pandoc();
        if pandoc_available() {
            return; // nothing to assert
        }
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("install") || msg.contains("https://"),
            "error should contain an install hint, got: {msg}"
        );
    }

    #[test]
    fn test_check_tectonic_result_matches_availability() {
        let result = check_tectonic();
        if tectonic_is_available() {
            assert!(result.is_ok(), "check_tectonic should succeed when tectonic is installed");
        } else {
            assert!(result.is_err(), "check_tectonic should fail when tectonic is not installed");
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("Tectonic not found"),
                "error should say 'Tectonic not found', got: {msg}"
            );
        }
    }

    #[test]
    fn test_check_tectonic_error_contains_install_hint() {
        let result = check_tectonic();
        if tectonic_is_available() {
            return;
        }
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("install") || msg.contains("https://"),
            "error should contain an install hint, got: {msg}"
        );
    }

    #[test]
    fn test_validate_dependencies_without_pdf_only_checks_pandoc() {
        // When pdf_requested is false, tectonic is not checked.
        // We can only verify the outcome matches what pandoc availability predicts.
        let result = validate_dependencies(false);
        if pandoc_available() {
            assert!(result.is_ok(), "validation without PDF should succeed when pandoc is present");
        } else {
            assert!(result.is_err());
            let msg = result.unwrap_err().to_string();
            assert!(msg.contains("Pandoc not found"), "error should be about pandoc: {msg}");
        }
    }

    #[test]
    fn test_validate_dependencies_with_pdf_checks_both() {
        let result = validate_dependencies(true);
        if pandoc_available() && tectonic_is_available() {
            assert!(result.is_ok(), "validation with PDF should succeed when both tools are present");
        } else {
            assert!(result.is_err(), "validation with PDF should fail when a tool is missing");
        }
    }

    #[test]
    fn test_tool_available_with_known_tool() {
        // `cargo` is always available in a Rust build environment and supports `--version`.
        assert!(tool_available("cargo"), "cargo should always be available in a Rust build environment");
    }

    #[test]
    fn test_tool_available_with_nonexistent_tool() {
        assert!(
            !tool_available("__renderflow_nonexistent_tool__"),
            "a made-up tool should not be reported as available"
        );
    }
}
