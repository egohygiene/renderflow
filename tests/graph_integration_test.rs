mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

fn write_graph_config(
    dir: &tempfile::TempDir,
    input_name: &str,
    input_contents: &str,
    transforms_yaml: &str,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let input_path = dir.path().join(input_name);
    fs::write(&input_path, input_contents).expect("failed to write graph input");

    let transforms_path = dir.path().join("transforms.yaml");
    fs::write(&transforms_path, transforms_yaml).expect("failed to write transforms yaml");

    let output_dir = dir.path().join("dist");
    let config_path = dir.path().join("renderflow.yaml");
    let config = format!(
        "input: \"{}\"\noutput_dir: \"{}\"\ntransforms: \"{}\"\n",
        input_path.display(),
        output_dir.display(),
        transforms_path.display(),
    );
    fs::write(&config_path, config).expect("failed to write graph config");

    (config_path, output_dir, transforms_path)
}

fn run_graph_build(config_path: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_renderflow"))
        .arg("build")
        .arg("--config")
        .arg(config_path)
        .args(args)
        .output()
        .expect("failed to execute renderflow")
}

#[test]
fn test_graph_single_node_execution_builds_output() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let (config_path, output_dir, _) = write_graph_config(
        &dir,
        "doc.md",
        "hello",
        r#"
transforms:
  - name: md-to-html
    program: python3
    args:
      - -c
      - "from pathlib import Path; import sys; Path(sys.argv[2]).write_text(Path(sys.argv[1]).read_text() + '->html')"
      - "{input}"
      - "{output}"
    from: markdown
    to: html
    cost: 1.0
    quality: 1.0
"#,
    );

    let output = run_graph_build(&config_path, &["--target", "html"]);
    assert!(
        output.status.success(),
        "graph build should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_dir.join("doc.html")).expect("missing html output");
    assert_eq!(html, "hello->html");
}

#[test]
fn test_graph_multi_target_execution_reuses_shared_intermediate_and_ordering() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let (config_path, output_dir, _) = write_graph_config(
        &dir,
        "doc.md",
        "start",
        r#"
transforms:
  - name: md-to-html
    program: python3
    args:
      - -c
      - "from pathlib import Path; import sys; Path(sys.argv[2]).write_text(Path(sys.argv[1]).read_text() + '->html')"
      - "{input}"
      - "{output}"
    from: markdown
    to: html
    cost: 1.0
    quality: 1.0
  - name: html-to-pdf
    program: python3
    args:
      - -c
      - "from pathlib import Path; import sys; Path(sys.argv[2]).write_text(Path(sys.argv[1]).read_text() + '->pdf')"
      - "{input}"
      - "{output}"
    from: html
    to: pdf
    cost: 1.0
    quality: 1.0
  - name: html-to-docx
    program: python3
    args:
      - -c
      - "from pathlib import Path; import sys; Path(sys.argv[2]).write_text(Path(sys.argv[1]).read_text() + '->docx')"
      - "{input}"
      - "{output}"
    from: html
    to: docx
    cost: 1.0
    quality: 1.0
"#,
    );

    let output = run_graph_build(&config_path, &["--all"]);
    assert!(
        output.status.success(),
        "graph build should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        fs::read_to_string(output_dir.join("doc.html")).expect("missing html output"),
        "start->html"
    );
    assert_eq!(
        fs::read_to_string(output_dir.join("doc.pdf")).expect("missing pdf output"),
        "start->html->pdf"
    );
    assert_eq!(
        fs::read_to_string(output_dir.join("doc.docx")).expect("missing docx output"),
        "start->html->docx"
    );
}

#[test]
fn test_graph_incremental_cache_reuses_previous_output() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let counter_path = dir.path().join("counter.txt");
    let (config_path, output_dir, _) = write_graph_config(
        &dir,
        "doc.md",
        "cache-me",
        &format!(
            r#"
transforms:
  - name: md-to-html
    program: python3
    args:
      - -c
      - "from pathlib import Path; import sys; counter = Path(sys.argv[3]); count = int(counter.read_text()) + 1 if counter.exists() else 1; counter.write_text(str(count)); Path(sys.argv[2]).write_text(Path(sys.argv[1]).read_text() + '->html')"
      - "{{input}}"
      - "{{output}}"
      - "{}"
    from: markdown
    to: html
    cost: 1.0
    quality: 1.0
"#,
            counter_path.display()
        ),
    );

    let first = run_graph_build(&config_path, &["--target", "html"]);
    assert!(
        first.status.success(),
        "first graph build should succeed, stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let second = run_graph_build(&config_path, &["--target", "html"]);
    assert!(
        second.status.success(),
        "second graph build should succeed, stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let count = fs::read_to_string(&counter_path).expect("missing counter file");
    assert_eq!(count.trim(), "1", "cached graph node should not re-execute");
    assert_eq!(
        fs::read_to_string(output_dir.join("doc.html")).expect("missing html output"),
        "cache-me->html"
    );
}

#[test]
fn test_graph_error_propagation_surfaces_transform_failure() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let (config_path, _output_dir, _) = write_graph_config(
        &dir,
        "doc.md",
        "boom",
        r#"
transforms:
  - name: md-to-html
    program: python3
    args:
      - -c
      - "from pathlib import Path; import sys; Path(sys.argv[2]).write_text(Path(sys.argv[1]).read_text() + '->html')"
      - "{input}"
      - "{output}"
    from: markdown
    to: html
    cost: 1.0
    quality: 1.0
  - name: html-to-pdf
    program: python3
    args:
      - -c
      - "import sys; print('intentional graph failure', file=sys.stderr); sys.exit(1)"
      - "{input}"
      - "{output}"
    from: html
    to: pdf
    cost: 1.0
    quality: 1.0
"#,
    );

    let output = run_graph_build(&config_path, &["--target", "pdf"]);
    assert!(!output.status.success(), "graph build should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Graph execution failed"),
        "stderr should contain graph execution context, got: {stderr}"
    );
    assert!(
        stderr.contains("intentional graph failure"),
        "stderr should include underlying transform failure, got: {stderr}"
    );
}

#[test]
fn test_graph_common_fixture_supports_dry_run() {
    let (config_file, _dir) = common::graph_config_file();
    let output = run_graph_build(config_file.path(), &["--target", "html", "--dry-run"]);

    assert!(
        output.status.success(),
        "graph dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
