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
fn test_help_output_lists_watch_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("--help")
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("watch"),
        "--help should list the watch subcommand, got: {stdout}"
    );
}

#[test]
fn test_watch_help_flag_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["watch", "--help"])
        .output()
        .expect("failed to execute renderflow");

    assert!(output.status.success(), "watch --help should exit with code 0");
}

#[test]
fn test_watch_help_documents_config_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["watch", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--config"),
        "watch --help should document --config, got: {stdout}"
    );
}

#[test]
fn test_watch_help_documents_debounce_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["watch", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--debounce"),
        "watch --help should document --debounce, got: {stdout}"
    );
}

#[test]
fn test_watch_help_contains_examples() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["watch", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Examples"),
        "watch --help should include an examples section, got: {stdout}"
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
fn test_dry_run_output_labeled() {
    let (f, _dir) = common::valid_config_file();
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--dry-run")
        .arg("--config")
        .arg(f.path())
        .output()
        .expect("failed to execute renderflow");

    assert!(output.status.success(), "dry-run should exit with code 0");
    // Tracing log messages go to stdout; verify [DRY RUN] prefix is present
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[DRY RUN]"),
        "dry-run output should contain [DRY RUN] label, got stdout: {stdout}"
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

// ── --target flag ────────────────────────────────────────────────────────────

#[test]
fn test_build_help_documents_target_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--target"),
        "build --help should document --target, got: {stdout}"
    );
}

#[test]
fn test_build_help_documents_all_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--all"),
        "build --help should document --all, got: {stdout}"
    );
}

#[test]
fn test_target_and_all_are_mutually_exclusive() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--target", "pdf", "--all"])
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "--target and --all together should fail with non-zero exit"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflicts"),
        "--target and --all should conflict, got: {stderr}"
    );
}

#[test]
fn test_target_with_missing_config_exits_with_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--config", "/nonexistent/renderflow.yaml", "--target", "pdf"])
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "--target with a missing config should exit with non-zero status"
    );
}

#[test]
fn test_all_with_missing_config_exits_with_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["build", "--config", "/nonexistent/renderflow.yaml", "--all"])
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "--all with a missing config should exit with non-zero status"
    );
}

#[test]
fn test_target_without_transforms_exits_with_error() {
    // A valid config with no 'transforms' key should cause graph-based execution to fail
    // with a descriptive error when --target is used.
    let (f, _dir) = common::valid_config_file();
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(f.path())
        .arg("--target")
        .arg("pdf")
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "--target without a 'transforms' key in config should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("transforms"),
        "error should mention 'transforms', got: {stderr}"
    );
}

#[test]
fn test_all_without_transforms_exits_with_error() {
    // A valid config with no 'transforms' key should cause graph-based execution to fail
    // with a descriptive error when --all is used.
    let (f, _dir) = common::valid_config_file();
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(f.path())
        .arg("--all")
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "--all without a 'transforms' key in config should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("transforms"),
        "error should mention 'transforms', got: {stderr}"
    );
}

#[test]
fn test_target_dry_run_exits_successfully() {
    use std::io::Write as _;

    // Build a minimal config with a transforms file so graph mode can run.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let input_path = dir.path().join("doc.md");
    std::fs::write(&input_path, "# Hello\n").expect("failed to write input");

    // A transforms YAML with a single markdown→html edge (program: cat).
    let transforms_path = dir.path().join("transforms.yaml");
    let transforms_yaml = "transforms:\n  - name: md-to-html\n    program: cat\n    from: markdown\n    to: html\n    cost: 1.0\n    quality: 0.9\n";
    std::fs::write(&transforms_path, transforms_yaml).expect("failed to write transforms");

    let output_dir = dir.path().join("dist");
    let config_content = format!(
        "input: \"{}\"\noutput_dir: \"{}\"\ntransforms: \"{}\"\n",
        input_path.display(),
        output_dir.display(),
        transforms_path.display(),
    );
    let mut config_file = tempfile::NamedTempFile::new().expect("failed to create temp config");
    config_file
        .write_all(config_content.as_bytes())
        .expect("failed to write config");

    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(config_file.path())
        .arg("--target")
        .arg("html")
        .arg("--dry-run")
        .output()
        .expect("failed to execute renderflow");

    assert!(
        output.status.success(),
        "--target html --dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[DRY RUN]"),
        "--target dry-run should print [DRY RUN] lines, got: {stdout}"
    );
}

#[test]
fn test_all_dry_run_exits_successfully() {
    use std::io::Write as _;

    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let input_path = dir.path().join("doc.md");
    std::fs::write(&input_path, "# Hello\n").expect("failed to write input");

    let transforms_path = dir.path().join("transforms.yaml");
    let transforms_yaml = "transforms:\n  - name: md-to-html\n    program: cat\n    from: markdown\n    to: html\n    cost: 1.0\n    quality: 0.9\n";
    std::fs::write(&transforms_path, transforms_yaml).expect("failed to write transforms");

    let output_dir = dir.path().join("dist");
    let config_content = format!(
        "input: \"{}\"\noutput_dir: \"{}\"\ntransforms: \"{}\"\n",
        input_path.display(),
        output_dir.display(),
        transforms_path.display(),
    );
    let mut config_file = tempfile::NamedTempFile::new().expect("failed to create temp config");
    config_file
        .write_all(config_content.as_bytes())
        .expect("failed to write config");

    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(config_file.path())
        .arg("--all")
        .arg("--dry-run")
        .output()
        .expect("failed to execute renderflow");

    assert!(
        output.status.success(),
        "--all --dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[DRY RUN]"),
        "--all dry-run should print [DRY RUN] lines, got: {stdout}"
    );
}

