mod common;

use std::process::Command;

#[test]
fn test_build_command_runs() {
    let f = common::valid_config_file();
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


