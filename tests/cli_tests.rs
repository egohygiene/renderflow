mod common;

use std::process::Command;

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
fn test_no_input_provided_exits_with_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .output()
        .expect("failed to execute renderflow");

    assert!(!output.status.success(), "expected non-zero exit when no input is provided");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No input provided"), "expected helpful error message, got: {stderr}");
}
