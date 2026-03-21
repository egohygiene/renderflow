use std::process::Command;

#[test]
fn test_build_command_runs() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .output()
        .expect("failed to execute renderflow");

    assert!(output.status.success(), "renderflow build exited with non-zero status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Running build pipeline"));
}
