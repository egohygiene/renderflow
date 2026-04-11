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
    let (config_file, _dir) = common::graph_config_file();

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
    let (config_file, _dir) = common::graph_config_file();

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

// ── inspect subcommand ────────────────────────────────────────────────────────

#[test]
fn test_inspect_help_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--help"])
        .output()
        .expect("failed to execute renderflow");

    assert!(output.status.success(), "inspect --help should exit with code 0");
}

#[test]
fn test_inspect_help_documents_config_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--config"),
        "inspect --help should document --config, got: {stdout}"
    );
}

#[test]
fn test_inspect_help_documents_output_format_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--output-format"),
        "inspect --help should document --output-format, got: {stdout}"
    );
}

#[test]
fn test_inspect_help_documents_export_option() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--export"),
        "inspect --help should document --export, got: {stdout}"
    );
}

#[test]
fn test_inspect_help_contains_examples() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--help"])
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Examples"),
        "inspect --help should contain an examples section, got: {stdout}"
    );
}

#[test]
fn test_inspect_missing_config_exits_with_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--config", "/nonexistent/renderflow.yaml"])
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "inspect with a missing config should exit with non-zero status"
    );
}

#[test]
fn test_inspect_without_transforms_exits_with_error() {
    let (f, _dir) = common::valid_config_file();
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--config"])
        .arg(f.path())
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "inspect without a 'transforms' key in config should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("transforms"),
        "error should mention 'transforms', got: {stderr}"
    );
}

#[test]
fn test_inspect_tree_output_contains_dag_header() {
    let (config_file, _dir) = common::graph_config_file();
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--config"])
        .arg(config_file.path())
        .output()
        .expect("failed to execute renderflow");

    assert!(
        output.status.success(),
        "inspect should succeed with a valid config, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("DAG Execution Plan"),
        "inspect tree output should contain 'DAG Execution Plan', got: {stdout}"
    );
}

#[test]
fn test_inspect_dot_output_contains_digraph() {
    let (config_file, _dir) = common::graph_config_file();
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--config"])
        .arg(config_file.path())
        .args(["--output-format", "dot"])
        .output()
        .expect("failed to execute renderflow");

    assert!(
        output.status.success(),
        "inspect --output-format dot should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("digraph renderflow"),
        "DOT output should contain 'digraph renderflow', got: {stdout}"
    );
}

#[test]
fn test_inspect_target_and_all_are_mutually_exclusive() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--target", "pdf", "--all"])
        .output()
        .expect("failed to execute renderflow");

    assert!(
        !output.status.success(),
        "--target and --all together should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflicts"),
        "--target and --all should conflict, got: {stderr}"
    );
}

#[test]
fn test_inspect_export_writes_file() {
    let (config_file, dir) = common::graph_config_file();
    let export_path = dir.path().join("dag.dot");

    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .args(["inspect", "--config"])
        .arg(config_file.path())
        .args(["--output-format", "dot", "--export"])
        .arg(&export_path)
        .output()
        .expect("failed to execute renderflow");

    assert!(
        output.status.success(),
        "inspect --export should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        export_path.exists(),
        "exported file should exist at {}",
        export_path.display()
    );
    let content = std::fs::read_to_string(&export_path).expect("failed to read export file");
    assert!(
        content.contains("digraph renderflow"),
        "exported file should contain DOT graph, got: {content}"
    );
}

#[test]
fn test_help_output_lists_inspect_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("--help")
        .output()
        .expect("failed to execute renderflow");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("inspect"),
        "--help should list the inspect subcommand, got: {stdout}"
    );
}
