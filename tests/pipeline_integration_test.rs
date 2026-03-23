mod common;

use std::fs;
use std::process::Command;

/// End-to-end integration test: verifies the full pipeline
///
/// CLI → config → transform → pipeline → strategy → output
///
/// The test creates a real markdown input file that contains an emoji
/// (to exercise the EmojiTransform), builds a YAML config pointing at it,
/// invokes the compiled `renderflow` binary, and then asserts that:
///   1. The process exits successfully.
///   2. The expected output file exists on disk.
///   3. The output file is non-empty.
#[test]
#[ignore = "requires pandoc to be installed"]
fn test_full_pipeline_end_to_end() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    // Write a non-trivial markdown input that exercises the emoji transform.
    let input_path = dir.path().join("document.md");
    fs::write(
        &input_path,
        "# Renderflow Integration Test 😀\n\nThis document is rendered end-to-end.\n",
    )
    .expect("failed to write input file");

    let output_dir = dir.path().join("dist");

    let config_content = format!(
        "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
        input_path.display(),
        output_dir.display(),
    );

    let config_path = dir.path().join("renderflow.yaml");
    fs::write(&config_path, &config_content).expect("failed to write config file");

    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "renderflow exited with non-zero status\nstdout: {stdout}\nstderr: {stderr}"
    );

    assert!(
        stdout.contains("Loaded config successfully"),
        "expected config load message in stdout, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Running build pipeline"),
        "expected pipeline start message in stdout, got:\n{stdout}"
    );

    // Verify the output file was actually written to disk.
    let output_file = output_dir.join("document.html");
    assert!(
        output_file.exists(),
        "expected output file to exist at {}, but it was not found",
        output_file.display()
    );

    let output_content = fs::read_to_string(&output_file)
        .expect("failed to read output file");
    assert!(
        !output_content.is_empty(),
        "expected output file to be non-empty"
    );
}

/// Verifies that the CLI fails gracefully when the config references a
/// non-existent input file, so the pipeline is never started.
#[test]
fn test_full_pipeline_missing_input_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let output_dir = dir.path().join("dist");

    let config_content = format!(
        "outputs:\n  - type: html\ninput: \"/nonexistent/document.md\"\noutput_dir: \"{}\"\n",
        output_dir.display(),
    );

    let config_path = dir.path().join("renderflow.yaml");
    fs::write(&config_path, &config_content).expect("failed to write config file");

    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "expected non-zero exit when input file is missing"
    );
}

/// Verifies that the CLI fails gracefully when the config has empty outputs
/// and an empty input field, so a clear error is returned.
#[test]
fn test_full_pipeline_invalid_config() {
    let (config_file, _dir) = common::valid_config_file();

    // Overwrite with invalid YAML to trigger a config parse error.
    fs::write(config_file.path(), "outputs: []\ninput: \"\"\n")
        .expect("failed to overwrite config file");

    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(config_file.path())
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "expected non-zero exit for invalid config"
    );
}
