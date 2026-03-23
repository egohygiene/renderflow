mod common;

use std::process::Command;

#[test]
fn test_help_flag_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("--help")
        .output()
        .expect("failed to execute renderflow");

    assert!(output.status.success(), "--help should exit with code 0");
}

#[test]
fn test_help_output_contains_description() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("--help")
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("renderflow"),
        "--help should mention renderflow, got: {stdout}"
    );
    assert!(
        stdout.contains("rendering") || stdout.contains("document"),
        "--help should describe the tool purpose, got: {stdout}"
    );
}

#[test]
fn test_help_output_lists_build_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("--help")
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("build"),
        "--help should list the build subcommand, got: {stdout}"
    );
}

#[test]
fn test_build_help_flag_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--help"])
        .output()
        .expect("failed to execute renderflow");

    assert!(output.status.success(), "build --help should exit with code 0");
}

#[test]
fn test_build_help_documents_config_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--config"),
        "build --help should document --config, got: {stdout}"
    );
}

#[test]
fn test_build_help_documents_dry_run_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--dry-run"),
        "build --help should document --dry-run, got: {stdout}"
    );
}

#[test]
fn test_build_help_contains_examples() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Examples"),
        "build --help should include an examples section, got: {stdout}"
    );
}

#[test]
#[ignore = "requires pandoc to be installed"]
fn test_build_command_runs() {
    let (f, _dir) = common::valid_config_file();
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(f.path())
        .output()
        .expect("failed to execute renderflow");

    assert!(output.status.success(), "renderflow build exited with non-zero status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Running build pipeline"));
    assert!(stdout.contains("Loaded config successfully"));
}

#[test]
fn test_build_command_missing_config() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg("/nonexistent/renderflow.yaml")
        .output()
        .expect("failed to execute renderflow");

    assert!(!output.status.success(), "expected non-zero exit when config is missing");
}

#[test]
#[ignore = "requires pandoc to be installed"]
fn test_implicit_build_command_runs() {
    let (f, _dir) = common::valid_config_file();
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg(f.path())
        .output()
        .expect("failed to execute renderflow");

    assert!(output.status.success(), "renderflow <input> exited with non-zero status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Running build pipeline"));
    assert!(stdout.contains("Loaded config successfully"));
}

#[test]
fn test_implicit_build_missing_config() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("/nonexistent/renderflow.yaml")
        .output()
        .expect("failed to execute renderflow");

    assert!(!output.status.success(), "expected non-zero exit when config is missing");
}

#[test]
fn test_verbose_flag_accepted() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--verbose")
        .arg("--config")
        .arg("/nonexistent/renderflow.yaml")
        .output()
        .expect("failed to execute renderflow");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument '--verbose'"),
        "--verbose flag should be recognised, got: {stderr}"
    );
}

#[test]
fn test_debug_flag_accepted() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--debug")
        .arg("--config")
        .arg("/nonexistent/renderflow.yaml")
        .output()
        .expect("failed to execute renderflow");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument '--debug'"),
        "--debug flag should be recognised, got: {stderr}"
    );
}

#[test]
fn test_no_input_provided_exits_with_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .output()
        .expect("failed to execute renderflow");

    assert!(!output.status.success(), "expected non-zero exit when no input is provided");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No input provided"), "expected helpful error message, got: {stderr}");
}
